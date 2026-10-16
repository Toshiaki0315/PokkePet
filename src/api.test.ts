import { beforeEach, describe, expect, it, vi } from "vitest";

const invoke = vi.fn();
const listen = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({ invoke: (...a: unknown[]) => invoke(...a) }));
vi.mock("@tauri-apps/api/event", () => ({ listen: (...a: unknown[]) => listen(...a) }));

import { ackHatch, getDex, setName, getSettings, getState, onPopupVisibility, onSettingsChanged, onStateChanged, performAction, saveSettings } from "./api";

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

  it("getSettings invokes get_settings", async () => {
    invoke.mockResolvedValue({ showStage: false });
    await expect(getSettings()).resolves.toEqual({ showStage: false });
    expect(invoke).toHaveBeenCalledWith("get_settings");
  });

  it("saveSettings invokes save_settings with the settings", async () => {
    const settings = { showStage: true, showGauges: false, notifyCalls: true, bodyColor: "#112233", buttonColor: "#445566" };
    invoke.mockResolvedValue(settings);
    await expect(saveSettings(settings)).resolves.toEqual(settings);
    expect(invoke).toHaveBeenCalledWith("save_settings", { settings });
  });

  it("onSettingsChanged listens to settings-changed and forwards the payload", async () => {
    const unlisten = vi.fn();
    listen.mockImplementation((_name: string, cb: (e: { payload: unknown }) => void) => {
      cb({ payload: { showStage: true } });
      return Promise.resolve(unlisten);
    });
    const handler = vi.fn();
    const off = await onSettingsChanged(handler);
    expect(listen).toHaveBeenCalledWith("settings-changed", expect.any(Function));
    expect(handler).toHaveBeenCalledWith({ showStage: true });
    expect(off).toBe(unlisten);
  });

  it("ackHatch invokes ack_hatch", async () => {
    invoke.mockResolvedValue(view);
    await expect(ackHatch()).resolves.toEqual(view);
    expect(invoke).toHaveBeenCalledWith("ack_hatch");
  });

  it("onPopupVisibility listens to popup-visibility and forwards the boolean", async () => {
    const unlisten = vi.fn();
    listen.mockImplementation((_name: string, cb: (e: { payload: unknown }) => void) => {
      cb({ payload: true });
      return Promise.resolve(unlisten);
    });
    const handler = vi.fn();
    const off = await onPopupVisibility(handler);
    expect(listen).toHaveBeenCalledWith("popup-visibility", expect.any(Function));
    expect(handler).toHaveBeenCalledWith(true);
    expect(off).toBe(unlisten);
  });

  it("setName invokes set_name with the name", async () => {
    invoke.mockResolvedValue(view);
    await expect(setName("ごま")).resolves.toEqual(view);
    expect(invoke).toHaveBeenCalledWith("set_name", { name: "ごま" });
  });

  it("getDex invokes get_dex", async () => {
    invoke.mockResolvedValue({ total: 109, discovered: 1, entries: [] });
    await expect(getDex()).resolves.toEqual({ total: 109, discovered: 1, entries: [] });
    expect(invoke).toHaveBeenCalledWith("get_dex");
  });
});
