import {
  Terminal,
  type IDecoration,
  type IDisposable,
  type IMarker,
} from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { WebglAddon } from "@xterm/addon-webgl";
import { SearchAddon } from "@xterm/addon-search";
import { WebLinksAddon } from "@xterm/addon-web-links";
import { Unicode11Addon } from "@xterm/addon-unicode11";
import { getTerminalTheme } from "./theme";
import type { ThemeVariant } from "./themes/index";
import {
  terminalGetForegroundProcess,
  terminalWrite,
  terminalResize,
  terminalSubscribe,
} from "./tauriCommands";
import type { TerminalOutputData } from "../types";

const textEncoder = new TextEncoder();
const BACKSPACE_INPUTS = new Set(["\b", "\u007f"]);
const DIRECT_SHELL_PROCESSES = new Set([
  "bash",
  "dash",
  "fish",
  "nu",
  "pwsh",
  "sh",
  "xonsh",
  "zsh",
]);
const PREDICTIVE_ECHO_POLL_MS = 150;
const TRACE_TERMINAL_LATENCY =
  import.meta.env.DEV &&
  typeof window !== "undefined" &&
  window.localStorage.getItem("lantern:trace-terminal-latency") === "1";

interface InputTrace {
  seq: number;
  kind: "text" | "binary";
  bytes: number;
  sentAt: number;
}

interface PredictiveEchoState {
  foregroundProcessName: string | null;
  pendingText: string;
  anchorX: number | null;
  marker: IMarker | null;
  decoration: IDecoration | null;
  decorationRenderDisposable: IDisposable | null;
}

interface ManagedTerminal {
  tabId: string;
  terminal: Terminal;
  fitAddon: FitAddon;
  searchAddon: SearchAddon;
  container: HTMLElement;
  resizeObserver: ResizeObserver;
  cleanupHandlers: Array<() => void>;
  nextInputSeq: number;
  pendingInputTrace: InputTrace[];
  predictiveEcho: PredictiveEchoState;
  subscribed: boolean;
}

function normalizeForegroundProcessName(name: string | null | undefined): string | null {
  if (!name) return null;

  const normalized = name
    .split("/")
    .pop()
    ?.toLowerCase()
    .replace(/^-+/, "");

  return normalized || null;
}

function isPredictiveEchoCharacter(data: string): boolean {
  if (data.length !== 1) return false;

  const code = data.charCodeAt(0);
  return code >= 0x20 && code <= 0x7e;
}

function shouldPredictForForegroundProcess(name: string | null): boolean {
  return name !== null && DIRECT_SHELL_PROCESSES.has(name);
}

function applyPredictiveEchoStyles(
  element: HTMLElement,
  terminal: Terminal,
  pendingText: string
): void {
  const theme = terminal.options.theme ?? {};
  const foreground = theme.foreground ?? "currentColor";
  const background = theme.background ?? "transparent";
  const caret = theme.cursor ?? foreground;

  element.textContent = pendingText;
  element.setAttribute("aria-hidden", "true");
  element.style.pointerEvents = "none";
  element.style.whiteSpace = "pre";
  element.style.display = "flex";
  element.style.alignItems = "center";
  element.style.height = "100%";
  element.style.boxSizing = "border-box";
  element.style.overflow = "hidden";
  element.style.color = foreground;
  element.style.backgroundColor = background;
  element.style.borderRight = `1px solid ${caret}`;
}

class TerminalManager {
  private terminals = new Map<string, ManagedTerminal>();
  private activeTabId: string | null = null;
  private foregroundProcessPollTimer: number | null = null;
  private foregroundProcessPollInFlight = false;

  private addCleanup(managed: ManagedTerminal, cleanup: () => void): void {
    managed.cleanupHandlers.push(cleanup);
  }

  private disposeManagedResources(managed: ManagedTerminal): void {
    while (managed.cleanupHandlers.length > 0) {
      const cleanup = managed.cleanupHandlers.pop();
      try {
        cleanup?.();
      } catch {
        // Ignore cleanup failures during terminal teardown
      }
    }
  }

  private createInputTrace(
    managed: ManagedTerminal,
    tabId: string,
    kind: InputTrace["kind"],
    bytes: number
  ): number | undefined {
    if (!TRACE_TERMINAL_LATENCY) return undefined;

    const seq = managed.nextInputSeq++;
    managed.pendingInputTrace.push({
      seq,
      kind,
      bytes,
      sentAt: performance.now(),
    });

    console.debug(
      `[lantern][terminal:${tabId}] send seq=${seq} kind=${kind} bytes=${bytes}`
    );

    return seq;
  }

  private completeInputTrace(
    managed: ManagedTerminal,
    tabId: string,
    echoedBytes: number
  ): void {
    if (!TRACE_TERMINAL_LATENCY || managed.pendingInputTrace.length === 0) return;

    const trace = managed.pendingInputTrace.shift();
    if (!trace) return;

    const elapsedMs = performance.now() - trace.sentAt;
    console.debug(
      `[lantern][terminal:${tabId}] echo seq=${trace.seq} kind=${trace.kind} send_bytes=${trace.bytes} recv_bytes=${echoedBytes} latency_ms=${elapsedMs.toFixed(1)}`
    );
  }

  private refreshCursorRow(terminal: Terminal): void {
    try {
      terminal.refresh(terminal.buffer.active.cursorY, terminal.buffer.active.cursorY);
    } catch {
      // Ignore refresh errors during teardown or hidden terminal states
    }
  }

  private clearPredictiveEcho(managed: ManagedTerminal): void {
    const { predictiveEcho } = managed;
    const hadPredictiveEcho =
      predictiveEcho.pendingText.length > 0 ||
      predictiveEcho.decoration !== null ||
      predictiveEcho.marker !== null;

    predictiveEcho.decorationRenderDisposable?.dispose();
    predictiveEcho.decorationRenderDisposable = null;

    predictiveEcho.decoration?.dispose();
    predictiveEcho.decoration = null;

    predictiveEcho.marker?.dispose();
    predictiveEcho.marker = null;

    predictiveEcho.pendingText = "";
    predictiveEcho.anchorX = null;

    if (hadPredictiveEcho) {
      this.refreshCursorRow(managed.terminal);
    }
  }

  private isPredictiveEchoEligible(managed: ManagedTerminal): boolean {
    if (managed.tabId !== this.activeTabId) return false;
    if (!shouldPredictForForegroundProcess(managed.predictiveEcho.foregroundProcessName)) {
      return false;
    }

    const { terminal } = managed;
    const buffer = terminal.buffer.active;

    if (buffer.type !== "normal") return false;
    if (terminal.hasSelection()) return false;
    if (buffer.baseY !== buffer.viewportY) return false;
    if (buffer.cursorY !== terminal.rows - 1) return false;

    return true;
  }

  private renderPredictiveEcho(managed: ManagedTerminal): boolean {
    const { predictiveEcho, terminal } = managed;
    if (!predictiveEcho.marker || predictiveEcho.anchorX === null) return false;
    if (predictiveEcho.pendingText.length === 0) return false;

    predictiveEcho.decorationRenderDisposable?.dispose();
    predictiveEcho.decorationRenderDisposable = null;

    predictiveEcho.decoration?.dispose();
    predictiveEcho.decoration = null;

    const decoration = terminal.registerDecoration({
      marker: predictiveEcho.marker,
      x: predictiveEcho.anchorX,
      width: predictiveEcho.pendingText.length,
    });

    if (!decoration) {
      return false;
    }

    const renderElement = (element: HTMLElement) => {
      applyPredictiveEchoStyles(element, terminal, predictiveEcho.pendingText);
    };

    if (decoration.element) {
      renderElement(decoration.element);
    }

    predictiveEcho.decorationRenderDisposable = decoration.onRender((element) => {
      renderElement(element);
    });
    predictiveEcho.decoration = decoration;

    this.refreshCursorRow(terminal);
    return true;
  }

  private extendPredictiveEcho(managed: ManagedTerminal, input: string): boolean {
    if (!this.isPredictiveEchoEligible(managed)) return false;

    const { predictiveEcho, terminal } = managed;
    const anchorX = predictiveEcho.anchorX ?? terminal.buffer.active.cursorX;
    const nextText = `${predictiveEcho.pendingText}${input}`;

    if (anchorX + nextText.length > terminal.cols) {
      return false;
    }

    if (predictiveEcho.anchorX === null) {
      predictiveEcho.anchorX = anchorX;
      predictiveEcho.marker = terminal.registerMarker(0) ?? null;
      if (!predictiveEcho.marker) {
        predictiveEcho.anchorX = null;
        return false;
      }
    }

    predictiveEcho.pendingText = nextText;
    if (!this.renderPredictiveEcho(managed)) {
      this.clearPredictiveEcho(managed);
      return false;
    }

    return true;
  }

  private shrinkPredictiveEcho(managed: ManagedTerminal): boolean {
    if (!this.isPredictiveEchoEligible(managed)) return false;
    if (managed.predictiveEcho.pendingText.length === 0) return false;

    managed.predictiveEcho.pendingText = managed.predictiveEcho.pendingText.slice(0, -1);
    if (managed.predictiveEcho.pendingText.length === 0) {
      this.clearPredictiveEcho(managed);
      return true;
    }

    if (!this.renderPredictiveEcho(managed)) {
      this.clearPredictiveEcho(managed);
      return false;
    }

    return true;
  }

  private handlePredictiveInput(managed: ManagedTerminal, data: string): void {
    if (isPredictiveEchoCharacter(data)) {
      if (this.extendPredictiveEcho(managed, data)) return;
    } else if (BACKSPACE_INPUTS.has(data)) {
      if (this.shrinkPredictiveEcho(managed)) return;
    }

    if (managed.predictiveEcho.pendingText.length > 0) {
      this.clearPredictiveEcho(managed);
    }
  }

  private updateForegroundProcessName(
    managed: ManagedTerminal,
    processName: string | null
  ): void {
    if (managed.predictiveEcho.foregroundProcessName === processName) return;

    managed.predictiveEcho.foregroundProcessName = processName;
    if (!shouldPredictForForegroundProcess(processName)) {
      this.clearPredictiveEcho(managed);
    }
  }

  private stopForegroundProcessPolling(): void {
    if (this.foregroundProcessPollTimer !== null) {
      window.clearInterval(this.foregroundProcessPollTimer);
      this.foregroundProcessPollTimer = null;
    }
    this.foregroundProcessPollInFlight = false;
  }

  private async pollForegroundProcess(): Promise<void> {
    const tabId = this.activeTabId;
    if (!tabId || this.foregroundProcessPollInFlight) return;

    const managed = this.terminals.get(tabId);
    if (!managed) return;

    this.foregroundProcessPollInFlight = true;

    try {
      const processInfo = await terminalGetForegroundProcess(tabId);
      if (this.activeTabId !== tabId) return;

      this.updateForegroundProcessName(
        managed,
        normalizeForegroundProcessName(processInfo?.name)
      );
    } catch {
      if (this.activeTabId !== tabId) return;
      this.updateForegroundProcessName(managed, null);
    } finally {
      this.foregroundProcessPollInFlight = false;
    }
  }

  private startForegroundProcessPolling(): void {
    this.stopForegroundProcessPolling();

    if (!this.activeTabId) return;
    if (!this.terminals.has(this.activeTabId)) return;

    void this.pollForegroundProcess();
    this.foregroundProcessPollTimer = window.setInterval(() => {
      void this.pollForegroundProcess();
    }, PREDICTIVE_ECHO_POLL_MS);
  }

  setActiveTab(tabId: string | null): void {
    if (this.activeTabId && this.activeTabId !== tabId) {
      const previous = this.terminals.get(this.activeTabId);
      if (previous) {
        this.clearPredictiveEcho(previous);
      }
    }

    if (this.activeTabId === tabId && this.foregroundProcessPollTimer !== null) {
      return;
    }

    this.activeTabId = tabId;
    this.startForegroundProcessPolling();
  }

  async create(
    tabId: string,
    container: HTMLElement,
    config: { fontFamily: string; fontSize: number; scrollback: number },
    onExit?: (code: number | null) => void
  ): Promise<void> {
    if (this.terminals.has(tabId)) return;

    const terminal = new Terminal({
      fontFamily: config.fontFamily,
      fontSize: config.fontSize,
      scrollback: config.scrollback,
      theme: getTerminalTheme(),
      cursorBlink: true,
      cursorStyle: "bar",
      allowProposedApi: true,
    });

    const fitAddon = new FitAddon();
    const searchAddon = new SearchAddon();

    terminal.loadAddon(fitAddon);
    terminal.loadAddon(searchAddon);
    terminal.loadAddon(
      new WebLinksAddon((_event, uri) => {
        window.open(uri, "_blank");
      })
    );

    const unicode11 = new Unicode11Addon();
    terminal.loadAddon(unicode11);
    terminal.unicode.activeVersion = "11";

    terminal.open(container);

    // Try WebGL, fall back to canvas
    try {
      const webgl = new WebglAddon();
      webgl.onContextLoss(() => webgl.dispose());
      terminal.loadAddon(webgl);
    } catch {
      // Canvas renderer as fallback
    }

    fitAddon.fit();

    const managed: ManagedTerminal = {
      tabId,
      terminal,
      fitAddon,
      searchAddon,
      container,
      resizeObserver: null as unknown as ResizeObserver,
      cleanupHandlers: [],
      nextInputSeq: 1,
      pendingInputTrace: [],
      predictiveEcho: {
        foregroundProcessName: null,
        pendingText: "",
        anchorX: null,
        marker: null,
        decoration: null,
        decorationRenderDisposable: null,
      },
      subscribed: false,
    };

    // Wire user input → PTY
    const dataDisposable = terminal.onData((data) => {
      this.handlePredictiveInput(managed, data);
      const bytes = textEncoder.encode(data);
      const seq = this.createInputTrace(managed, tabId, "text", bytes.length);
      terminalWrite(tabId, bytes, seq).catch(console.error);
    });
    this.addCleanup(managed, () => dataDisposable.dispose());

    const binaryDisposable = terminal.onBinary((data) => {
      this.clearPredictiveEcho(managed);
      const bytes = Uint8Array.from(data, (c) => c.charCodeAt(0));
      const seq = this.createInputTrace(managed, tabId, "binary", bytes.length);
      terminalWrite(tabId, bytes, seq).catch(console.error);
    });
    this.addCleanup(managed, () => binaryDisposable.dispose());

    // Wire resize
    const resizeDisposable = terminal.onResize(({ cols, rows }) => {
      this.clearPredictiveEcho(managed);
      terminalResize(tabId, cols, rows).catch(console.error);
    });
    this.addCleanup(managed, () => resizeDisposable.dispose());

    const bufferChangeDisposable = terminal.buffer.onBufferChange(() => {
      this.clearPredictiveEcho(managed);
    });
    this.addCleanup(managed, () => bufferChangeDisposable.dispose());

    const handleBlur = () => {
      this.clearPredictiveEcho(managed);
    };
    terminal.textarea?.addEventListener("blur", handleBlur);
    this.addCleanup(managed, () => {
      terminal.textarea?.removeEventListener("blur", handleBlur);
    });

    // ResizeObserver for container changes
    const resizeObserver = new ResizeObserver(() => {
      requestAnimationFrame(() => {
        try {
          fitAddon.fit();
        } catch {
          // Terminal may be disposed
        }
      });
    });
    resizeObserver.observe(container);
    managed.resizeObserver = resizeObserver;

    this.terminals.set(tabId, managed);
    if (this.activeTabId === tabId) {
      this.startForegroundProcessPolling();
    }

    // Subscribe to PTY output
    try {
      await terminalSubscribe(tabId, (output: TerminalOutputData) => {
        if (output.kind === "Data") {
          this.clearPredictiveEcho(managed);
          this.completeInputTrace(managed, tabId, output.data.length);
          terminal.write(output.data);
        } else if (output.kind === "Exited") {
          this.clearPredictiveEcho(managed);
          this.updateForegroundProcessName(managed, null);
          managed.pendingInputTrace = [];
          onExit?.(output.code);
        }
      });
      managed.subscribed = true;
    } catch (e) {
      console.error("Failed to subscribe to PTY:", e);
    }
  }

  destroy(tabId: string): void {
    const managed = this.terminals.get(tabId);
    if (!managed) return;

    if (this.activeTabId === tabId) {
      this.activeTabId = null;
      this.stopForegroundProcessPolling();
    }

    this.clearPredictiveEcho(managed);
    this.disposeManagedResources(managed);
    managed.resizeObserver.disconnect();
    managed.terminal.dispose();
    this.terminals.delete(tabId);
  }

  fit(tabId: string): void {
    const managed = this.terminals.get(tabId);
    if (!managed) return;
    try {
      managed.fitAddon.fit();
    } catch {
      // Ignore if terminal is not visible
    }
  }

  focus(tabId: string): void {
    const managed = this.terminals.get(tabId);
    if (!managed) return;
    managed.terminal.focus();
  }

  fitAndFocus(tabId: string): void {
    requestAnimationFrame(() => {
      this.fit(tabId);
      this.focus(tabId);
    });
  }

  search(tabId: string, query: string): boolean {
    const managed = this.terminals.get(tabId);
    if (!managed) return false;
    return managed.searchAddon.findNext(query);
  }

  getTerminal(tabId: string): Terminal | undefined {
    return this.terminals.get(tabId)?.terminal;
  }

  getDimensions(tabId: string): { cols: number; rows: number } | undefined {
    const managed = this.terminals.get(tabId);
    if (!managed) return undefined;
    return {
      cols: managed.terminal.cols,
      rows: managed.terminal.rows,
    };
  }

  has(tabId: string): boolean {
    return this.terminals.has(tabId);
  }

  updateAllFontSize(size: number): void {
    for (const [, managed] of this.terminals) {
      managed.terminal.options.fontSize = size;
      try { managed.fitAddon.fit(); } catch {}
    }
  }

  updateAllFontFamily(family: string): void {
    for (const [, managed] of this.terminals) {
      managed.terminal.options.fontFamily = family;
      try { managed.fitAddon.fit(); } catch {}
    }
  }

  updateAllThemes(variant: ThemeVariant): void {
    const theme = getTerminalTheme(variant.terminal);
    const bg = variant.colors.bgPrimary;
    const fg = variant.colors.textPrimary;
    const cursor = variant.colors.accent;
    const cursorAccent = variant.colors.bgPrimary;
    for (const [, managed] of this.terminals) {
      managed.terminal.options.theme = {
        ...theme,
        background: bg,
        foreground: fg,
        cursor,
        cursorAccent,
      };
    }
  }

  destroyAll(): void {
    this.activeTabId = null;
    this.stopForegroundProcessPolling();
    for (const tabId of this.terminals.keys()) {
      this.destroy(tabId);
    }
  }
}

export const terminalManager = new TerminalManager();
