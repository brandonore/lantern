import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { WebglAddon } from "@xterm/addon-webgl";
import { SearchAddon } from "@xterm/addon-search";
import { WebLinksAddon } from "@xterm/addon-web-links";
import { Unicode11Addon } from "@xterm/addon-unicode11";
import { getTerminalTheme } from "./theme";
import type { ThemeVariant } from "./themes/index";
import {
  terminalWrite,
  terminalResize,
  terminalSubscribe,
} from "./tauriCommands";
import type { TerminalOutputData } from "../types";

interface ManagedTerminal {
  terminal: Terminal;
  fitAddon: FitAddon;
  searchAddon: SearchAddon;
  container: HTMLElement;
  resizeObserver: ResizeObserver;
  subscribed: boolean;
}

class TerminalManager {
  private terminals = new Map<string, ManagedTerminal>();

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

    // Wire user input → PTY
    terminal.onData((data) => {
      const bytes = Array.from(new TextEncoder().encode(data));
      terminalWrite(tabId, bytes).catch(console.error);
    });

    terminal.onBinary((data) => {
      const bytes = Array.from(data, (c) => c.charCodeAt(0));
      terminalWrite(tabId, bytes).catch(console.error);
    });

    // Wire resize
    terminal.onResize(({ cols, rows }) => {
      terminalResize(tabId, cols, rows).catch(console.error);
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

    const managed: ManagedTerminal = {
      terminal,
      fitAddon,
      searchAddon,
      container,
      resizeObserver,
      subscribed: false,
    };

    this.terminals.set(tabId, managed);

    // Subscribe to PTY output
    try {
      await terminalSubscribe(tabId, (output: TerminalOutputData) => {
        if (output.kind === "Data") {
          terminal.write(output.data);
        } else if (output.kind === "Exited") {
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
    for (const tabId of this.terminals.keys()) {
      this.destroy(tabId);
    }
  }
}

export const terminalManager = new TerminalManager();
