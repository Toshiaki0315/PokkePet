import { beforeEach, describe, expect, it, vi } from "vitest";

const invoke = vi.fn();
const listen = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({ invoke: (...a: unknown[]) => invoke(...a) }));
vi.mock("@tauri-apps/api/event", () => ({ listen: (...a: unknown[]) => listen(...a) }));

import { getState, onStateChanged, performAction } from "./api";

const view = { mood: 1, affinity: 2, hunger: 3, dirtiness: 4, stage: 1, branch: "normal", personality: "calm" };

beforeEach(() => {
  invoke.mockReset();
  listen.mockReset();
});

describe("api", () => {
  it("getState invokes get_state", async () => {
    invoke.mockResolvedValue(view);
    await expect(getState()).resolves.toEqual(view);
    expect(invoke).toHaveBeenCalledWith("get_state");
  });

  it("performAction invokes perform_action with the action name", async () => {
    invoke.mockResolvedValue(view);
    await performAction("feed");
    expect(invoke).toHaveBeenCalledWith("perform_action", { action: "feed" });
  });

  it("onStateChanged forwards the payload and returns the unlisten function", async () => {
    const unlisten = vi.fn();
    listen.mockImplementation((_name: string, cb: (e: { payload: unknown }) => void) => {
      cb({ payload: view });
      return Promise.resolve(unlisten);
    });
    const handler = vi.fn();
    const off = await onStateChanged(handler);
    expect(listen).toHaveBeenCalledWith("state-changed", expect.any(Function));
    expect(handler).toHaveBeenCalledWith(view);
    expect(off).toBe(unlisten);
  });
});
