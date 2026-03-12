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
import type {
  ProcessInfo,
  TerminalLatencyMode,
  TerminalOutputData,
} from "../types";

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
const RECOGNIZED_AGENT_PROCESSES = new Set([
  "aider",
  "claude",
  "codex",
  "opencode",
]);
const FALLBACK_FOREGROUND_PROCESS_POLL_MS = 1000;
const INPUT_BATCH_WINDOW_MS = 6;
const PREDICTIVE_ECHO_DELAY_MS = 45;
const SHELL_INTEGRATION_PREFIX = "\u001b]633;Lantern;";
const SHELL_INTEGRATION_PROMPT = "Prompt";
const SHELL_INTEGRATION_TERMINATOR = "\u0007";
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
  foregroundProcessInfo: ProcessInfo | null;
  foregroundProcessPolled: boolean;
  integrationSeen: boolean;
  promptReady: boolean;
  pendingText: string;
  visible: boolean;
  anchorX: number | null;
  marker: IMarker | null;
  decoration: IDecoration | null;
  decorationRenderDisposable: IDisposable | null;
  revealTimer: number | null;
  renderFrame: number | null;
  outputBuffer: string;
}

interface PendingInput {
  kind: InputTrace["kind"];
  bytes: Uint8Array;
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
  latencyMode: TerminalLatencyMode;
  pendingInput: PendingInput[];
  inputFlushTimer: number | null;
  pendingOutput: string[];
  outputWriteInFlight: boolean;
  receivedOutput: boolean;
  predictiveEcho: PredictiveEchoState;
  subscribed: boolean;
}

type PredictiveEchoMode = "agent" | "shell" | null;
type ActiveProcessListener = (processInfo: ProcessInfo | null) => void;

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

function areProcessInfosEqual(
  left: ProcessInfo | null,
  right: ProcessInfo | null
): boolean {
  if (left === right) return true;
  if (!left || !right) return false;

  return (
    left.name === right.name &&
    left.is_agent === right.is_agent &&
    left.agent_label === right.agent_label
  );
}

function isDirectShellProcess(processInfo: ProcessInfo | null): boolean {
  const normalizedName = normalizeForegroundProcessName(processInfo?.name);
  return normalizedName !== null && DIRECT_SHELL_PROCESSES.has(normalizedName);
}

function isRecognizedAgentProcess(processInfo: ProcessInfo | null): boolean {
  if (!processInfo) return false;
  if (processInfo.is_agent) return true;

  const normalizedName = normalizeForegroundProcessName(processInfo.name);
  return (
    normalizedName !== null &&
    RECOGNIZED_AGENT_PROCESSES.has(normalizedName)
  );
}

function shouldFlushInputImmediately(
  data: string,
  latencyMode: TerminalLatencyMode
): boolean {
  if (latencyMode === "compatible") return true;
  if (data.length !== 1) return true;
  return !isPredictiveEchoCharacter(data);
}

function hasPromptSubmitInput(data: string): boolean {
  return data.includes("\r") || data.includes("\n");
}

function findTrailingPrefixStart(value: string, prefix: string): number {
  const minIndex = Math.max(0, value.length - prefix.length + 1);
  for (let index = minIndex; index < value.length; index += 1) {
    if (prefix.startsWith(value.slice(index))) {
      return index;
    }
  }

  return -1;
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
  private activeProcessInfo: ProcessInfo | null = null;
  private activeProcessListeners = new Set<ActiveProcessListener>();
  private foregroundProcessPollTimer: number | null = null;
  private foregroundProcessPollInFlight = false;

  private setActiveProcessInfo(processInfo: ProcessInfo | null): void {
    if (areProcessInfosEqual(this.activeProcessInfo, processInfo)) return;

    this.activeProcessInfo = processInfo;
    for (const listener of this.activeProcessListeners) {
      listener(processInfo);
    }
  }

  private syncActiveProcessInfo(): void {
    if (!this.activeTabId) {
      this.setActiveProcessInfo(null);
      return;
    }

    const managed = this.terminals.get(this.activeTabId);
    this.setActiveProcessInfo(managed?.predictiveEcho.foregroundProcessInfo ?? null);
  }

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

  private stopPredictiveEchoTimers(managed: ManagedTerminal): void {
    const { predictiveEcho } = managed;

    if (predictiveEcho.revealTimer !== null) {
      window.clearTimeout(predictiveEcho.revealTimer);
      predictiveEcho.revealTimer = null;
    }

    if (predictiveEcho.renderFrame !== null) {
      cancelAnimationFrame(predictiveEcho.renderFrame);
      predictiveEcho.renderFrame = null;
    }
  }

  private stopPendingInputFlush(managed: ManagedTerminal): void {
    if (managed.inputFlushTimer !== null) {
      window.clearTimeout(managed.inputFlushTimer);
      managed.inputFlushTimer = null;
    }
  }

  private clearPredictiveEcho(managed: ManagedTerminal): void {
    const { predictiveEcho } = managed;
    const hadPredictiveEcho =
      predictiveEcho.pendingText.length > 0 ||
      predictiveEcho.decoration !== null ||
      predictiveEcho.marker !== null;

    this.stopPredictiveEchoTimers(managed);

    predictiveEcho.decorationRenderDisposable?.dispose();
    predictiveEcho.decorationRenderDisposable = null;

    predictiveEcho.decoration?.dispose();
    predictiveEcho.decoration = null;

    predictiveEcho.marker?.dispose();
    predictiveEcho.marker = null;

    predictiveEcho.pendingText = "";
    predictiveEcho.visible = false;
    predictiveEcho.anchorX = null;

    if (hadPredictiveEcho) {
      this.refreshCursorRow(managed.terminal);
    }
  }

  private setPromptReady(managed: ManagedTerminal, promptReady: boolean): void {
    managed.predictiveEcho.promptReady = promptReady;
    if (!promptReady) {
      this.clearPredictiveEcho(managed);
    }
  }

  private sendInput(
    managed: ManagedTerminal,
    kind: InputTrace["kind"],
    bytes: Uint8Array
  ): void {
    const seq = this.createInputTrace(managed, managed.tabId, kind, bytes.length);
    terminalWrite(managed.tabId, bytes, seq).catch(console.error);
  }

  private flushPendingInput(managed: ManagedTerminal): void {
    this.stopPendingInputFlush(managed);
    if (managed.pendingInput.length === 0) return;

    let totalBytes = 0;
    let kind: InputTrace["kind"] = "text";
    for (const chunk of managed.pendingInput) {
      totalBytes += chunk.bytes.length;
      if (chunk.kind === "binary") {
        kind = "binary";
      }
    }

    const combined = new Uint8Array(totalBytes);
    let offset = 0;
    for (const chunk of managed.pendingInput) {
      combined.set(chunk.bytes, offset);
      offset += chunk.bytes.length;
    }

    managed.pendingInput = [];
    this.sendInput(managed, kind, combined);
  }

  private drainPendingOutput(managed: ManagedTerminal): void {
    if (managed.outputWriteInFlight) return;
    if (managed.pendingOutput.length === 0) return;

    const output = managed.pendingOutput.join("");
    managed.pendingOutput = [];
    managed.outputWriteInFlight = true;

    managed.terminal.write(output, () => {
      managed.outputWriteInFlight = false;
      if (!this.terminals.has(managed.tabId)) return;
      this.drainPendingOutput(managed);
    });
  }

  private queueOutput(managed: ManagedTerminal, output: string): void {
    if (output.length === 0) return;
    managed.pendingOutput.push(output);
    this.drainPendingOutput(managed);
  }

  private queueInput(
    managed: ManagedTerminal,
    kind: InputTrace["kind"],
    bytes: Uint8Array,
    immediate = false
  ): void {
    if (immediate) {
      this.flushPendingInput(managed);
      this.sendInput(managed, kind, bytes);
      return;
    }

    managed.pendingInput.push({ kind, bytes });
    if (managed.inputFlushTimer !== null) return;

    managed.inputFlushTimer = window.setTimeout(() => {
      managed.inputFlushTimer = null;
      this.flushPendingInput(managed);
    }, INPUT_BATCH_WINDOW_MS);
  }

  private getPredictiveEchoMode(managed: ManagedTerminal): PredictiveEchoMode {
    const { predictiveEcho } = managed;

    if (isRecognizedAgentProcess(predictiveEcho.foregroundProcessInfo)) {
      return "agent";
    }

    if (
      predictiveEcho.foregroundProcessPolled &&
      predictiveEcho.foregroundProcessInfo
    ) {
      if (!isDirectShellProcess(predictiveEcho.foregroundProcessInfo)) {
        return null;
      }
      if (predictiveEcho.integrationSeen && !predictiveEcho.promptReady) {
        return null;
      }
      return "shell";
    }

    if (predictiveEcho.integrationSeen) {
      return predictiveEcho.promptReady ? "shell" : null;
    }

    if (predictiveEcho.foregroundProcessPolled) return null;

    return "shell";
  }

  private isPredictiveEchoEligible(managed: ManagedTerminal): boolean {
    if (managed.tabId !== this.activeTabId) return false;

    const mode = this.getPredictiveEchoMode(managed);
    if (!mode) return false;

    const { terminal } = managed;
    const buffer = terminal.buffer.active;

    if (terminal.hasSelection()) return false;
    if (mode === "shell") {
      if (buffer.type !== "normal") return false;
      if (buffer.baseY !== buffer.viewportY) return false;
    }

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

  private queuePredictiveEchoRender(managed: ManagedTerminal): void {
    if (!managed.predictiveEcho.visible) return;
    if (managed.predictiveEcho.renderFrame !== null) return;

    managed.predictiveEcho.renderFrame = requestAnimationFrame(() => {
      managed.predictiveEcho.renderFrame = null;
      if (!managed.predictiveEcho.visible) return;
      if (!this.renderPredictiveEcho(managed)) {
        this.clearPredictiveEcho(managed);
      }
    });
  }

  private schedulePredictiveEchoReveal(managed: ManagedTerminal): void {
    const mode = this.getPredictiveEchoMode(managed);
    if (!mode) return;

    if (managed.latencyMode === "compatible" || mode === "agent") {
      managed.predictiveEcho.visible = true;
      this.queuePredictiveEchoRender(managed);
      return;
    }

    if (managed.predictiveEcho.visible) {
      this.queuePredictiveEchoRender(managed);
      return;
    }

    if (managed.predictiveEcho.revealTimer !== null) return;

    managed.predictiveEcho.revealTimer = window.setTimeout(() => {
      managed.predictiveEcho.revealTimer = null;
      if (!this.isPredictiveEchoEligible(managed)) return;
      if (managed.predictiveEcho.pendingText.length === 0) return;
      managed.predictiveEcho.visible = true;
      this.queuePredictiveEchoRender(managed);
    }, PREDICTIVE_ECHO_DELAY_MS);
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
    this.schedulePredictiveEchoReveal(managed);
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

    if (managed.predictiveEcho.visible) {
      this.queuePredictiveEchoRender(managed);
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
    processInfo: ProcessInfo | null
  ): void {
    managed.predictiveEcho.foregroundProcessPolled = true;
    managed.predictiveEcho.foregroundProcessInfo = processInfo;

    if (managed.tabId === this.activeTabId) {
      this.setActiveProcessInfo(processInfo);
    }

    if (!isDirectShellProcess(processInfo) && !isRecognizedAgentProcess(processInfo)) {
      this.clearPredictiveEcho(managed);
    }
  }

  private handleShellIntegrationMarker(
    managed: ManagedTerminal,
    marker: string
  ): void {
    if (marker !== SHELL_INTEGRATION_PROMPT) return;

    managed.predictiveEcho.integrationSeen = true;
    this.setPromptReady(managed, true);
  }

  private consumeShellIntegration(
    managed: ManagedTerminal,
    data: string
  ): string {
    const combined = managed.predictiveEcho.outputBuffer + data;
    let cursor = 0;
    let visible = "";

    while (cursor < combined.length) {
      const start = combined.indexOf(SHELL_INTEGRATION_PREFIX, cursor);
      if (start === -1) {
        const trailingPrefixStart = findTrailingPrefixStart(
          combined.slice(cursor),
          SHELL_INTEGRATION_PREFIX
        );
        if (trailingPrefixStart >= 0) {
          const absoluteStart = cursor + trailingPrefixStart;
          visible += combined.slice(cursor, absoluteStart);
          managed.predictiveEcho.outputBuffer = combined.slice(absoluteStart);
          return visible;
        }

        visible += combined.slice(cursor);
        managed.predictiveEcho.outputBuffer = "";
        return visible;
      }

      visible += combined.slice(cursor, start);

      const terminator = combined.indexOf(
        SHELL_INTEGRATION_TERMINATOR,
        start + SHELL_INTEGRATION_PREFIX.length
      );
      if (terminator === -1) {
        managed.predictiveEcho.outputBuffer = combined.slice(start);
        return visible;
      }

      const marker = combined.slice(
        start + SHELL_INTEGRATION_PREFIX.length,
        terminator
      );
      this.handleShellIntegrationMarker(managed, marker);
      cursor = terminator + SHELL_INTEGRATION_TERMINATOR.length;
    }

    managed.predictiveEcho.outputBuffer = "";
    return visible;
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

      this.updateForegroundProcessName(managed, processInfo ?? null);
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
    const managed = this.terminals.get(this.activeTabId);
    if (!managed) return;

    void this.pollForegroundProcess();
    this.foregroundProcessPollTimer = window.setInterval(() => {
      void this.pollForegroundProcess();
    }, FALLBACK_FOREGROUND_PROCESS_POLL_MS);
  }

  private scheduleForegroundProcessRefresh(delayMs: number): void {
    window.setTimeout(() => {
      void this.pollForegroundProcess();
    }, delayMs);
  }

  setActiveTab(tabId: string | null): void {
    if (this.activeTabId && this.activeTabId !== tabId) {
      const previous = this.terminals.get(this.activeTabId);
      if (previous) {
        this.clearPredictiveEcho(previous);
      }
    }

    this.activeTabId = tabId;
    this.syncActiveProcessInfo();
    this.startForegroundProcessPolling();
  }

  async create(
    tabId: string,
    container: HTMLElement,
    config: {
      fontFamily: string;
      fontSize: number;
      scrollback: number;
      latencyMode: TerminalLatencyMode;
    },
    onExit?: (code: number | null) => void,
    onFirstOutput?: () => void
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
      latencyMode: config.latencyMode,
      pendingInput: [],
      inputFlushTimer: null,
      pendingOutput: [],
      outputWriteInFlight: false,
      receivedOutput: false,
      predictiveEcho: {
        foregroundProcessInfo: null,
        foregroundProcessPolled: false,
        integrationSeen: false,
        promptReady: false,
        pendingText: "",
        visible: false,
        anchorX: null,
        marker: null,
        decoration: null,
        decorationRenderDisposable: null,
        revealTimer: null,
        renderFrame: null,
        outputBuffer: "",
      },
      subscribed: false,
    };

    // Wire user input → PTY
    const dataDisposable = terminal.onData((data) => {
      this.handlePredictiveInput(managed, data);
      const bytes = textEncoder.encode(data);
      const immediate = shouldFlushInputImmediately(data, managed.latencyMode);
      if (hasPromptSubmitInput(data)) {
        this.setPromptReady(managed, false);
        if (managed.tabId === this.activeTabId) {
          this.scheduleForegroundProcessRefresh(120);
          this.scheduleForegroundProcessRefresh(360);
        }
      } else if (!isPredictiveEchoCharacter(data) && !BACKSPACE_INPUTS.has(data)) {
        this.clearPredictiveEcho(managed);
      }
      this.queueInput(managed, "text", bytes, immediate);
    });
    this.addCleanup(managed, () => dataDisposable.dispose());

    const binaryDisposable = terminal.onBinary((data) => {
      this.clearPredictiveEcho(managed);
      this.setPromptReady(managed, false);
      const bytes = Uint8Array.from(data, (c) => c.charCodeAt(0));
      this.queueInput(managed, "binary", bytes, true);
    });
    this.addCleanup(managed, () => binaryDisposable.dispose());

    // Wire resize
    const resizeDisposable = terminal.onResize(({ cols, rows }) => {
      this.flushPendingInput(managed);
      this.clearPredictiveEcho(managed);
      terminalResize(tabId, cols, rows).catch(console.error);
    });
    this.addCleanup(managed, () => resizeDisposable.dispose());

    const bufferChangeDisposable = terminal.buffer.onBufferChange(() => {
      this.clearPredictiveEcho(managed);
    });
    this.addCleanup(managed, () => bufferChangeDisposable.dispose());

    const selectionChangeDisposable = terminal.onSelectionChange(() => {
      this.clearPredictiveEcho(managed);
    });
    this.addCleanup(managed, () => selectionChangeDisposable.dispose());

    const cursorMoveDisposable = terminal.onCursorMove(() => {
      if (managed.predictiveEcho.pendingText.length > 0) {
        this.clearPredictiveEcho(managed);
      }
    });
    this.addCleanup(managed, () => cursorMoveDisposable.dispose());

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
          const visibleData = this.consumeShellIntegration(managed, output.data);
          if (visibleData.length === 0) return;
          if (!managed.receivedOutput) {
            managed.receivedOutput = true;
            onFirstOutput?.();
          }
          this.clearPredictiveEcho(managed);
          this.completeInputTrace(managed, tabId, visibleData.length);
          this.queueOutput(managed, visibleData);
        } else if (output.kind === "Exited") {
          this.stopPendingInputFlush(managed);
          managed.pendingInput = [];
          managed.pendingOutput = [];
          managed.outputWriteInFlight = false;
          this.clearPredictiveEcho(managed);
          this.updateForegroundProcessName(managed, null);
          managed.pendingInputTrace = [];
          onExit?.(output.code);
        }
      });
      managed.subscribed = true;
      // Sync PTY to actual xterm.js viewport size (PTY starts at 80×24)
      terminalResize(tabId, terminal.cols, terminal.rows).catch(console.error);
      // Prime foreground process now that PTY is alive (initial poll races with spawn)
      if (this.activeTabId === tabId) {
        terminalGetForegroundProcess(tabId)
          .then((info) => {
            if (this.activeTabId === tabId) {
              this.updateForegroundProcessName(managed, info ?? null);
            }
          })
          .catch(() => {});
      }
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
      this.setActiveProcessInfo(null);
    }

    this.stopPendingInputFlush(managed);
    managed.pendingInput = [];
    managed.pendingOutput = [];
    managed.outputWriteInFlight = false;
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

  hasReceivedOutput(tabId: string): boolean {
    return this.terminals.get(tabId)?.receivedOutput ?? false;
  }

  getActiveProcessInfo(): ProcessInfo | null {
    return this.activeProcessInfo;
  }

  subscribeActiveProcess(listener: ActiveProcessListener): () => void {
    this.activeProcessListeners.add(listener);
    listener(this.activeProcessInfo);

    return () => {
      this.activeProcessListeners.delete(listener);
    };
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

  updateAllLatencyMode(mode: TerminalLatencyMode): void {
    for (const [, managed] of this.terminals) {
      managed.latencyMode = mode;
      if (mode === "compatible") {
        this.flushPendingInput(managed);
      }
    }
  }

  destroyAll(): void {
    this.activeTabId = null;
    this.setActiveProcessInfo(null);
    this.stopForegroundProcessPolling();
    for (const tabId of this.terminals.keys()) {
      this.destroy(tabId);
    }
  }
}

export const terminalManager = new TerminalManager();
