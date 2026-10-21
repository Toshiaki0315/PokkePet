import { beforeEach, describe, expect, it, vi } from "vitest";

const invoke = vi.fn();
const listen = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({ invoke: (...a: unknown[]) => invoke(...a) }));
vi.mock("@tauri-apps/api/event", () => ({ listen: (...a: unknown[]) => listen(...a) }));

import { onWalkFrame, toggleOutingNow, revealDebugLog, saveCard, getEggCode, receiveEgg, ackGift, getSouvenirs, toggleSouvenir, ackHatch, buryPet, restartGame, togglePower, graduate, getDex, getDiary, playMinigame, setName, getSettings, getState, onPopupVisibility, onSettingsChanged, onStateChanged, performAction, saveSettings, strokePet } from "./api";

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
    invoke.mockResolvedValue({ showGauges: false });
    await expect(getSettings()).resolves.toEqual({ showGauges: false });
    expect(invoke).toHaveBeenCalledWith("get_settings");
  });

  it("saveSettings invokes save_settings with the settings", async () => {
    const settings = { showGauges: false, notifyCalls: true, soundEnabled: true, soundVolume: 50, soundPitch: 0, launchAtLogin: false, strictMode: false, companionEnabled: true, systemReactions: true, background: "classic" as const, bubbleSeconds: 8, debugLog: false, walkOnDesktop: false, legacyStrength: 2, birthdayMonth: 0, birthdayDay: 0, breakReminder: true, autoSleep: true, sleepStartHour: 23, sleepEndHour: 7, bodyColor: "#112233", buttonColor: "#445566" };
    invoke.mockResolvedValue(settings);
    await expect(saveSettings(settings)).resolves.toEqual(settings);
    expect(invoke).toHaveBeenCalledWith("save_settings", { settings });
  });

  it("onSettingsChanged listens to settings-changed and forwards the payload", async () => {
    const unlisten = vi.fn();
    listen.mockImplementation((_name: string, cb: (e: { payload: unknown }) => void) => {
      cb({ payload: { showGauges: true } });
      return Promise.resolve(unlisten);
    });
    const handler = vi.fn();
    const off = await onSettingsChanged(handler);
    expect(listen).toHaveBeenCalledWith("settings-changed", expect.any(Function));
    expect(handler).toHaveBeenCalledWith({ showGauges: true });
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

  it("strokePet invokes stroke_pet", async () => {
    invoke.mockResolvedValue(view);
    await expect(strokePet()).resolves.toEqual(view);
    expect(invoke).toHaveBeenCalledWith("stroke_pet");
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

  it("getDiary invokes get_diary", async () => {
    invoke.mockResolvedValue({ daysTogether: 1, togetherMinutes: 0, chronotype: null, lineage: [], today: { feeds: 0, snacks: 0, plays: 0, cleans: 0, strokes: 0, togetherSecs: 0, souvenirs: 0 }, entries: [] });
    await expect(getDiary()).resolves.toEqual({ daysTogether: 1, togetherMinutes: 0, chronotype: null, lineage: [], today: { feeds: 0, snacks: 0, plays: 0, cleans: 0, strokes: 0, togetherSecs: 0, souvenirs: 0 }, entries: [] });
    expect(invoke).toHaveBeenCalledWith("get_diary");
  });

  it("playMinigame invokes play_minigame with the guess", async () => {
    const report = { view, win: true, answer: "left" };
    invoke.mockResolvedValue(report);
    await expect(playMinigame("left")).resolves.toEqual(report);
    expect(invoke).toHaveBeenCalledWith("play_minigame", { guess: "left" });
  });

  it("togglePower invokes toggle_power", async () => {
    invoke.mockResolvedValue(view);
    await expect(togglePower()).resolves.toEqual(view);
    expect(invoke).toHaveBeenCalledWith("toggle_power");
  });

  it("onWalkFrame listens to walker-frame and forwards the payload", async () => {
    const unlisten = vi.fn();
    listen.mockImplementation((_name: string, cb: (e: { payload: unknown }) => void) => {
      cb({ payload: { dir: -1, moving: true } });
      return Promise.resolve(unlisten);
    });
    const handler = vi.fn();
    const off = await onWalkFrame(handler);
    expect(listen).toHaveBeenCalledWith("walker-frame", expect.any(Function));
    expect(handler).toHaveBeenCalledWith({ dir: -1, moving: true });
    expect(off).toBe(unlisten);
  });

  it("toggleOutingNow invokes toggle_outing", async () => {
    invoke.mockResolvedValue(view);
    await expect(toggleOutingNow()).resolves.toEqual(view);
    expect(invoke).toHaveBeenCalledWith("toggle_outing");
  });

  it("revealDebugLog invokes reveal_debug_log", async () => {
    invoke.mockResolvedValue(undefined);
    await expect(revealDebugLog()).resolves.toBeUndefined();
    expect(invoke).toHaveBeenCalledWith("reveal_debug_log");
  });

  it("saveCard invokes save_card with the image", async () => {
    invoke.mockResolvedValue("/Users/me/Pictures/card.png");
    await expect(saveCard("data:image/png;base64,AAAA")).resolves.toBe("/Users/me/Pictures/card.png");
    expect(invoke).toHaveBeenCalledWith("save_card", { dataUrl: "data:image/png;base64,AAAA" });
  });

  it("egg code commands invoke the matching Tauri commands", async () => {
    invoke.mockResolvedValue("ABCDE-FGHJK");
    await expect(getEggCode()).resolves.toBe("ABCDE-FGHJK");
    expect(invoke).toHaveBeenCalledWith("get_egg_code");
    invoke.mockResolvedValue(view);
    await expect(receiveEgg("ABCDE-FGHJK")).resolves.toEqual(view);
    expect(invoke).toHaveBeenCalledWith("receive_egg", { code: "ABCDE-FGHJK" });
  });

  it("souvenir commands invoke the matching Tauri commands", async () => {
    invoke.mockResolvedValue({ total: 24, found: 0, maxDisplayed: 3, entries: [] });
    await getSouvenirs();
    expect(invoke).toHaveBeenCalledWith("get_souvenirs");
    await toggleSouvenir("shell");
    expect(invoke).toHaveBeenCalledWith("toggle_souvenir", { id: "shell" });
    invoke.mockResolvedValue(view);
    await expect(ackGift()).resolves.toEqual(view);
    expect(invoke).toHaveBeenCalledWith("ack_gift");
  });

  it("restartGame invokes restart_game", async () => {
    invoke.mockResolvedValue(view);
    await expect(restartGame()).resolves.toEqual(view);
    expect(invoke).toHaveBeenCalledWith("restart_game");
  });

  it("buryPet invokes bury_pet", async () => {
    invoke.mockResolvedValue(view);
    await expect(buryPet()).resolves.toEqual(view);
    expect(invoke).toHaveBeenCalledWith("bury_pet");
  });

  it("graduate invokes graduate", async () => {
    invoke.mockResolvedValue(view);
    await expect(graduate()).resolves.toEqual(view);
    expect(invoke).toHaveBeenCalledWith("graduate");
  });
});
