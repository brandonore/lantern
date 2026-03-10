import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { terminalWrite } from "./tauriCommands";

const mockInvoke = invoke as ReturnType<typeof vi.fn>;

describe("tauriCommands", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
    mockInvoke.mockResolvedValue(undefined);
  });

  it("sends terminal input as a raw bytes payload with session metadata headers", async () => {
    const bytes = new Uint8Array([97, 98]);

    await terminalWrite("tab-1", bytes, 42);

    expect(mockInvoke).toHaveBeenCalledWith("terminal_write_raw", bytes, {
      headers: {
        "content-type": "application/octet-stream",
        "x-lantern-session-id": "tab-1",
        "x-lantern-input-seq": "42",
      },
    });
  });

  it("omits the sequence header when latency tracing is disabled", async () => {
    const bytes = new Uint8Array([97]);

    await terminalWrite("tab-2", bytes);

    expect(mockInvoke).toHaveBeenCalledWith("terminal_write_raw", bytes, {
      headers: {
        "content-type": "application/octet-stream",
        "x-lantern-session-id": "tab-2",
      },
    });
  });
});
