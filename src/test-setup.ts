import { vi } from "vitest";

const mockWindow = {
  startDragging: vi.fn(),
  minimize: vi.fn(),
  toggleMaximize: vi.fn(),
  close: vi.fn(),
  onCloseRequested: vi.fn().mockResolvedValue(() => {}),
  scaleFactor: vi.fn().mockResolvedValue(1),
  outerSize: vi.fn().mockResolvedValue({ width: 1200, height: 800 }),
  outerPosition: vi.fn().mockResolvedValue({ x: 40, y: 60 }),
  isMaximized: vi.fn().mockResolvedValue(false),
};

// Mock @tauri-apps/api/core
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
  Channel: vi.fn().mockImplementation(() => ({
    onmessage: null,
  })),
}));

// Mock @tauri-apps/api/event
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: vi.fn(() => mockWindow),
}));

// Mock @tauri-apps/plugin-dialog
vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: vi.fn(),
}));

// Mock @tauri-apps/plugin-shell
vi.mock("@tauri-apps/plugin-shell", () => ({
  open: vi.fn(),
}));

// Mock xterm.js and addons
let markerId = 0;

function createDisposable() {
  return { dispose: vi.fn() };
}

vi.mock("@xterm/xterm", () => ({
  Terminal: vi.fn().mockImplementation(() => ({
    loadAddon: vi.fn(),
    open: vi.fn(),
    onData: vi.fn(() => createDisposable()),
    onBinary: vi.fn(() => createDisposable()),
    onResize: vi.fn(() => createDisposable()),
    write: vi.fn(),
    focus: vi.fn(),
    dispose: vi.fn(),
    refresh: vi.fn(),
    hasSelection: vi.fn().mockReturnValue(false),
    registerMarker: vi.fn(() => ({
      id: ++markerId,
      line: 23,
      isDisposed: false,
      onDispose: vi.fn(() => createDisposable()),
      dispose: vi.fn(),
    })),
    registerDecoration: vi.fn(({ marker }) => {
      const element = document.createElement("div");
      return {
        marker,
        element,
        options: {},
        isDisposed: false,
        onDispose: vi.fn(() => createDisposable()),
        onRender: vi.fn((listener: (el: HTMLElement) => void) => {
          listener(element);
          return createDisposable();
        }),
        dispose: vi.fn(),
      };
    }),
    cols: 80,
    rows: 24,
    options: {
      theme: {},
    },
    buffer: {
      active: {
        type: "normal",
        cursorY: 23,
        cursorX: 0,
        viewportY: 0,
        baseY: 0,
        length: 24,
        getLine: vi.fn(),
        getNullCell: vi.fn(),
      },
      normal: {},
      alternate: {},
      onBufferChange: vi.fn(() => createDisposable()),
    },
    modes: {
      applicationCursorKeysMode: false,
      applicationKeypadMode: false,
      bracketedPasteMode: false,
      insertMode: false,
      mouseTrackingMode: "none",
      originMode: false,
      reverseWraparoundMode: false,
      sendFocusMode: false,
      wraparoundMode: true,
    },
    element: document.createElement("div"),
    textarea: document.createElement("textarea"),
    unicode: { activeVersion: "11" },
  })),
}));

vi.mock("@xterm/addon-fit", () => ({
  FitAddon: vi.fn().mockImplementation(() => ({
    fit: vi.fn(),
    dispose: vi.fn(),
  })),
}));

vi.mock("@xterm/addon-webgl", () => ({
  WebglAddon: vi.fn().mockImplementation(() => ({
    onContextLoss: vi.fn(),
    dispose: vi.fn(),
  })),
}));

vi.mock("@xterm/addon-search", () => ({
  SearchAddon: vi.fn().mockImplementation(() => ({
    findNext: vi.fn().mockReturnValue(false),
    dispose: vi.fn(),
  })),
}));

vi.mock("@xterm/addon-web-links", () => ({
  WebLinksAddon: vi.fn().mockImplementation(() => ({
    dispose: vi.fn(),
  })),
}));

vi.mock("@xterm/addon-unicode11", () => ({
  Unicode11Addon: vi.fn().mockImplementation(() => ({
    dispose: vi.fn(),
  })),
}));

vi.stubGlobal(
  "ResizeObserver",
  vi.fn().mockImplementation(() => ({
    observe: vi.fn(),
    unobserve: vi.fn(),
    disconnect: vi.fn(),
  }))
);

vi.stubGlobal("requestAnimationFrame", (callback: FrameRequestCallback) => {
  callback(0);
  return 0;
});

vi.stubGlobal("cancelAnimationFrame", vi.fn());
