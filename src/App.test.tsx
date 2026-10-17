import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Settings, StateView } from "./api";
import { DEFAULT_SETTINGS } from "./defaults";
import { HATCH_FLASH_MS, HATCH_SHAKE_MS } from "./hatch";
import { IDLE_CRY_COOLDOWN_MS, IDLE_MAX_MS, IDLE_MIN_MS } from "./idle";

const api = vi.hoisted(() => ({
  getState: vi.fn(),
  performAction: vi.fn(),
  strokePet: vi.fn(),
  onStateChanged: vi.fn(),
  ackHatch: vi.fn(),
  graduate: vi.fn(),
  buryPet: vi.fn(),
  playMinigame: vi.fn(),
  setName: vi.fn(),
  getSettings: vi.fn(),
  saveSettings: vi.fn(),
  onSettingsChanged: vi.fn(),
  onPopupVisibility: vi.fn(),
}));
vi.mock("./api", () => api);

const sound = vi.hoisted(() => ({ play: vi.fn(), useSound: vi.fn() }));
vi.mock("./sound", () => ({
  useSound: (enabled: boolean, tuning: unknown) => {
    sound.useSound(enabled, tuning);
    return sound.play;
  },
}));

import App from "./App";

const base: StateView = {
  mood: 50,
  affinity: 10,
  hunger: 80,
  dirtiness: 5,
  weight: 50,
  discipline: 0,
  poops: 0,
  sick: false,
  danger: false,
  dead: false,
  stage: 2,
  branch: "normal",
  personality: "calm",
  expression: { mood: "mid", hunger: "high", dirtiness: "low", bubble: null },
  playAction: "play",
  intro: false,
  hatching: false,
  sleeping: false,
  name: null,
  needsName: false,
  graduating: false,
  generation: 1,
};
const withExpression = (e: Partial<StateView["expression"]>): StateView => ({ ...base, expression: { ...base.expression, ...e } });
const gaugesOn: Settings = { ...DEFAULT_SETTINGS, showGauges: true };
const enableGauges = () => api.getSettings.mockResolvedValue(gaugesOn);
const konami = async () => {
  await userEvent.keyboard("{ArrowUp}{ArrowUp}{ArrowDown}{ArrowDown}");
};
let emit: (v: StateView) => void;
let emitSettings: (s: Settings) => void;
let emitVisible: (visible: boolean) => void;
const unlisten = vi.fn();
const unlistenSettings = vi.fn();
const unlistenVisible = vi.fn();

beforeEach(() => {
  vi.resetAllMocks();
  api.getSettings.mockResolvedValue(DEFAULT_SETTINGS);
  api.saveSettings.mockImplementation((s: Settings) => Promise.resolve(s));
  api.onPopupVisibility.mockImplementation((h: (v: boolean) => void) => {
    emitVisible = h;
    return Promise.resolve(unlistenVisible);
  });
  api.onSettingsChanged.mockImplementation((h: (s: Settings) => void) => {
    emitSettings = h;
    return Promise.resolve(unlistenSettings);
  });
  api.getState.mockResolvedValue(base);
  api.performAction.mockResolvedValue(base);
  api.onStateChanged.mockImplementation((h: (v: StateView) => void) => {
    emit = h;
    return Promise.resolve(unlisten);
  });
});

describe("App", () => {
  it("renders four gauges from get_state when enabled", async () => {
    enableGauges();
    render(<App />);
    for (const [name, now] of [["ご機嫌", "50"], ["親密度", "10"], ["空腹度", "80"], ["汚れ度", "5"]]) {
      expect(await screen.findByRole("progressbar", { name })).toHaveAttribute("aria-valuenow", now);
    }
  });

  it("shows the sprite, and never the stage number (the design tells the stage)", async () => {
    enableGauges();
    render(<App />);
    expect(await screen.findByRole("img")).toHaveAttribute("alt", "stage 2 normal calm");
    expect(screen.queryByText(/段階 \d/)).toBeNull();
  });

  it.each([["ご飯", "feed"], ["おやつ", "snack"], ["掃除", "clean"], ["しつけ", "scold"]])("%s button calls performAction(%s)", async (label, action) => {
    render(<App />);
    await userEvent.click(await screen.findByRole("button", { name: label }));
    expect(api.performAction).toHaveBeenCalledWith(action);
  });

  it("falls back to a placeholder when the sprite file is missing", async () => {
    render(<App />);
    fireEvent.error(await screen.findByRole("img"));
    expect(screen.getByRole("img")).toHaveTextContent("🐣");
  });

  it("uses the egg placeholder at stage 1", async () => {
    api.getState.mockResolvedValue({ ...base, stage: 1 });
    render(<App />);
    fireEvent.error(await screen.findByRole("img"));
    expect(screen.getByRole("img")).toHaveTextContent("🥚");
  });

  it("clears a previous error after a successful action", async () => {
    api.performAction.mockRejectedValueOnce(new Error("x")).mockResolvedValue(base);
    render(<App />);
    await userEvent.click(await screen.findByRole("button", { name: "ご飯" }));
    await screen.findByRole("alert");
    await userEvent.click(screen.getByRole("button", { name: "ご飯" }));
    await waitFor(() => expect(screen.queryByRole("alert")).toBeNull());
  });

  it("shows the state returned by an action", async () => {
    enableGauges();
    api.performAction.mockResolvedValue({ ...base, hunger: 100 });
    render(<App />);
    await userEvent.click(await screen.findByRole("button", { name: "ご飯" }));
    await waitFor(() => expect(screen.getByRole("progressbar", { name: "空腹度" })).toHaveAttribute("aria-valuenow", "100"));
  });

  it("updates the view when a state-changed event arrives", async () => {
    enableGauges();
    render(<App />);
    await screen.findByRole("progressbar", { name: "ご機嫌" });
    await waitFor(() => expect(api.onStateChanged).toHaveBeenCalled());
    emit({ ...base, mood: 77 });
    await waitFor(() => expect(screen.getByRole("progressbar", { name: "ご機嫌" })).toHaveAttribute("aria-valuenow", "77"));
  });

  it("unsubscribes the listener on unmount", async () => {
    const { unmount } = render(<App />);
    await waitFor(() => expect(api.onStateChanged).toHaveBeenCalled());
    await screen.findByRole("img");
    unmount();
    await waitFor(() => expect(unlisten).toHaveBeenCalled());
  });

  it("unsubscribes immediately if unmounted before listen resolves", async () => {
    let resolve!: (f: () => void) => void;
    api.onStateChanged.mockReturnValue(new Promise((r) => (resolve = r)));
    const { unmount } = render(<App />);
    unmount();
    resolve(unlisten);
    await waitFor(() => expect(unlisten).toHaveBeenCalled());
  });

  it("shows an error message when performAction rejects", async () => {
    api.performAction.mockRejectedValue(new Error("save failed"));
    render(<App />);
    await userEvent.click(await screen.findByRole("button", { name: "ご飯" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("save failed");
  });

  it("shows an error message when getState rejects", async () => {
    api.getState.mockRejectedValue("boom");
    render(<App />);
    expect(await screen.findByRole("alert")).toHaveTextContent("boom");
  });

  it("renders a loading state before the first snapshot", () => {
    api.getState.mockReturnValue(new Promise(() => {}));
    render(<App />);
    expect(screen.getByText("読み込み中…")).toBeInTheDocument();
  });

  describe("hidden gauges", () => {
    it("hides the numeric gauges by default", async () => {
      render(<App />);
      await screen.findByRole("img");
      expect(screen.queryByRole("progressbar")).toBeNull();
    });

    it("toggles the gauges with the hidden command (up, up, down, down)", async () => {
      render(<App />);
      await screen.findByRole("img");
      await konami();
      expect(screen.getAllByRole("progressbar")).toHaveLength(6);
      await konami();
      expect(screen.queryByRole("progressbar")).toBeNull();
    });

    it("ignores wrong sequences and resets on a stray key", async () => {
      render(<App />);
      await screen.findByRole("img");
      await userEvent.keyboard("{ArrowUp}{ArrowDown}{ArrowDown}{ArrowDown}");
      expect(screen.queryByRole("progressbar")).toBeNull();
      await userEvent.keyboard("{ArrowUp}{ArrowUp}{ArrowUp}{ArrowDown}{ArrowDown}");
      expect(screen.getAllByRole("progressbar")).toHaveLength(6);
    });

    it("saves the gauge switch through the settings API when the command is used", async () => {
      render(<App />);
      await screen.findByRole("img");
      await konami();
      await waitFor(() => expect(api.saveSettings).toHaveBeenCalledWith({ ...DEFAULT_SETTINGS, showGauges: true }));
      await konami();
      await waitFor(() => expect(api.saveSettings).toHaveBeenLastCalledWith({ ...DEFAULT_SETTINGS, showGauges: false }));
    });

    it("shows an error when saving the settings fails", async () => {
      api.saveSettings.mockRejectedValue(new Error("disk full"));
      render(<App />);
      await screen.findByRole("img");
      await konami();
      expect(await screen.findByRole("alert")).toHaveTextContent("disk full");
    });

    it("follows settings-changed events from the settings dialog", async () => {
      render(<App />);
      await screen.findByRole("img");
      await waitFor(() => expect(api.onSettingsChanged).toHaveBeenCalled());
      emitSettings(gaugesOn);
      expect(await screen.findAllByRole("progressbar")).toHaveLength(6);
    });
  });

  describe("colour scheme", () => {
    it("shows an error when the settings cannot be loaded and keeps the defaults", async () => {
      api.getSettings.mockRejectedValue(new Error("unreadable"));
      render(<App />);
      expect(await screen.findByRole("alert")).toHaveTextContent("unreadable");
      expect(screen.getByRole("main").style.getPropertyValue("--body-color")).toBe(DEFAULT_SETTINGS.bodyColor);
    });

    it("applies the configured colours as CSS variables", async () => {
      api.getSettings.mockResolvedValue({ ...DEFAULT_SETTINGS, bodyColor: "#112233", buttonColor: "#445566" });
      render(<App />);
      await waitFor(() => expect(screen.getByRole("main").style.getPropertyValue("--body-color")).toBe("#112233"));
      expect(screen.getByRole("main").style.getPropertyValue("--button-color")).toBe("#445566");
    });

    it("updates the colours live when settings change", async () => {
      render(<App />);
      await screen.findByRole("img");
      await waitFor(() => expect(api.onSettingsChanged).toHaveBeenCalled());
      emitSettings({ ...DEFAULT_SETTINGS, bodyColor: "#abcdef" });
      await waitFor(() => expect(screen.getByRole("main").style.getPropertyValue("--body-color")).toBe("#abcdef"));
    });

    it("unsubscribes from settings events on unmount", async () => {
      const { unmount } = render(<App />);
      await screen.findByRole("img");
      await waitFor(() => expect(api.onSettingsChanged).toHaveBeenCalled());
      unmount();
      await waitFor(() => expect(unlistenSettings).toHaveBeenCalled());
    });
  });

  describe("egg stage", () => {
    it("calls the play button 撫でる while the pet is an egg", async () => {
      api.getState.mockResolvedValue({ ...base, stage: 1, playAction: "pet" });
      render(<App />);
      expect(await screen.findByRole("button", { name: "撫でる" })).toBeInTheDocument();
      expect(screen.queryByRole("button", { name: "遊ぶ" })).toBeNull();
    });

    it("still sends the play action from the 撫でる button (Rust decides what it means)", async () => {
      api.getState.mockResolvedValue({ ...base, stage: 1, playAction: "pet" });
      render(<App />);
      await userEvent.click(await screen.findByRole("button", { name: "撫でる" }));
      expect(api.performAction).toHaveBeenCalledWith("play");
    });

    it("switches the label back to 遊ぶ after hatching", async () => {
      api.getState.mockResolvedValue({ ...base, stage: 1, playAction: "pet" });
      render(<App />);
      await screen.findByRole("button", { name: "撫でる" });
      await waitFor(() => expect(api.onStateChanged).toHaveBeenCalled());
      emit({ ...base, stage: 2, playAction: "play" });
      expect(await screen.findByRole("button", { name: "遊ぶ" })).toBeInTheDocument();
    });
  });

  describe("expression", () => {
    it.each([
      ["hungry", "おなかすいたよ…"],
      ["dirty", "おへや、よごれてるよ…"],
      ["gloomy", "つまんないな…"],
      ["happy", "たのしい！"],
      ["morning", "おはよう！"],
      ["noon", "おひるだね"],
      ["evening", "こんばんは"],
      ["drowsy", "ねむくなってきた…"],
      ["selfish", "ねえねえ、かまって！"],
      ["sick", "きもちわるい…"],
      ["dying", "たすけて…もう、げんかい…"],
    ] as const)("shows a speech bubble for %s", async (bubble, text) => {
      api.getState.mockResolvedValue(withExpression({ bubble }));
      render(<App />);
      expect(await screen.findByRole("status")).toHaveTextContent(text);
    });

    it("shows no bubble when nothing stands out", async () => {
      render(<App />);
      await screen.findByRole("img");
      expect(screen.queryByRole("status")).toBeNull();
    });

    it("exposes mood / dirt / hunger levels for the background", async () => {
      api.getState.mockResolvedValue(withExpression({ mood: "low", dirtiness: "high", hunger: "low" }));
      render(<App />);
      const main = await screen.findByRole("main");
      expect(main).toHaveAttribute("data-mood", "low");
      expect(main).toHaveAttribute("data-dirt", "high");
      expect(main).toHaveAttribute("data-hunger", "low");
    });

    it("updates the background when a state-changed event arrives", async () => {
      render(<App />);
      const main = await screen.findByRole("main");
      await waitFor(() => expect(api.onStateChanged).toHaveBeenCalled());
      emit(withExpression({ mood: "high", bubble: "happy" }));
      await waitFor(() => expect(main).toHaveAttribute("data-mood", "high"));
      expect(screen.getByRole("status")).toHaveTextContent("たのしい！");
    });

    it("makes the sprite droop when the pet is hungry", async () => {
      api.getState.mockResolvedValue(withExpression({ hunger: "low", bubble: "hungry" }));
      render(<App />);
      expect(await screen.findByRole("img")).toHaveClass("hungry");
    });

    it("keeps the sprite upright when not hungry", async () => {
      render(<App />);
      expect(await screen.findByRole("img")).not.toHaveClass("hungry");
    });
  });

  describe("device layout", () => {
    it("never shows a stage label, with or without the gauges", async () => {
      render(<App />);
      await screen.findByRole("img");
      expect(screen.queryByText(/段階 \d/)).toBeNull();
      await konami();
      expect(screen.getAllByRole("progressbar")).toHaveLength(6);
      expect(screen.queryByText(/段階 \d/)).toBeNull();
    });

    it("keeps the screen (character, bubble) apart from the buttons", async () => {
      api.getState.mockResolvedValue(withExpression({ bubble: "happy" }));
      render(<App />);
      const display = await screen.findByRole("region", { name: "画面" });
      const controls = screen.getByRole("group", { name: "ボタン" });
      expect(within(display).getByRole("img")).toBeInTheDocument();
      expect(within(display).getByRole("status")).toHaveTextContent("たのしい！");
      expect(within(display).queryByRole("button")).toBeNull();
      expect(within(controls).getAllByRole("button")).toHaveLength(5);
      expect(within(controls).queryByRole("img")).toBeNull();
      expect(within(controls).queryByRole("status")).toBeNull();
    });

    it("shows the gauges on the screen when enabled", async () => {
      enableGauges();
      render(<App />);
      const display = await screen.findByRole("region", { name: "画面" });
      expect(within(display).getAllByRole("progressbar")).toHaveLength(6);
      expect(display).toHaveClass("compact"); // ゲージ分の高さを確保するためキャラを小さくする
    });

    it("uses the full-size character when the gauges are hidden", async () => {
      render(<App />);
      expect(await screen.findByRole("region", { name: "画面" })).not.toHaveClass("compact");
    });

    it("shows action errors outside the screen", async () => {
      api.performAction.mockRejectedValue(new Error("save failed"));
      render(<App />);
      await userEvent.click(await screen.findByRole("button", { name: "ご飯" }));
      const alert = await screen.findByRole("alert");
      expect(within(screen.getByRole("region", { name: "画面" })).queryByRole("alert")).toBeNull();
      expect(alert).toHaveTextContent("save failed");
    });
  });

  describe("opening", () => {
    const intro = { ...base, stage: 1, playAction: "pet", intro: true, expression: { ...base.expression, bubble: "welcome" } } as StateView;

    it("welcomes the player with a speech bubble on the first launch", async () => {
      api.getState.mockResolvedValue(intro);
      render(<App />);
      expect(await screen.findByRole("status")).toHaveTextContent("卵が届いたよ！撫でてあげてね");
    });

    it("fades the egg in during the opening only", async () => {
      api.getState.mockResolvedValue(intro);
      render(<App />);
      expect(await screen.findByRole("img")).toHaveClass("appear");
      expect(screen.getByRole("main")).toHaveAttribute("data-intro", "true");
    });

    it("has no opening effect afterwards", async () => {
      render(<App />);
      expect(await screen.findByRole("img")).not.toHaveClass("appear");
      expect(screen.getByRole("main")).toHaveAttribute("data-intro", "false");
    });
  });

  describe("reactions to the buttons", () => {
    it("makes the egg wobble when it is stroked", async () => {
      api.getState.mockResolvedValue({ ...base, stage: 1, playAction: "pet" });
      api.performAction.mockResolvedValue({ ...base, stage: 1, playAction: "pet" });
      render(<App />);
      await userEvent.click(await screen.findByRole("button", { name: "撫でる" }));
      await waitFor(() => expect(screen.getByRole("img")).toHaveClass("wobble"));
    });

    it("makes the pet hop for the other buttons", async () => {
      render(<App />);
      await userEvent.click(await screen.findByRole("button", { name: "ご飯" }));
      await waitFor(() => expect(screen.getByRole("img")).toHaveClass("hop"));
    });

    it("clears the reaction when the animation ends so that other animations can continue", async () => {
      render(<App />);
      await userEvent.click(await screen.findByRole("button", { name: "掃除" }));
      await waitFor(() => expect(screen.getByRole("img")).toHaveClass("hop"));
      // jsdom の React は webkit 接頭辞付きの名前で待つので、両方の名前で発火させる。
      for (const name of ["animationend", "webkitAnimationEnd"]) fireEvent(screen.getByRole("img"), new Event(name, { bubbles: true }));
      expect(screen.getByRole("img")).not.toHaveClass("hop");
    });

    it("restarts the animation on every press", async () => {
      render(<App />);
      await userEvent.click(await screen.findByRole("button", { name: "ご飯" }));
      await waitFor(() => expect(screen.getByRole("img")).toHaveClass("hop"));
      const first = screen.getByRole("img");
      await userEvent.click(screen.getByRole("button", { name: "ご飯" }));
      await waitFor(() => expect(screen.getByRole("img")).not.toBe(first));
      expect(screen.getByRole("img")).toHaveClass("hop");
    });

    it("does not react when the action fails", async () => {
      api.performAction.mockRejectedValue(new Error("x"));
      render(<App />);
      await userEvent.click(await screen.findByRole("button", { name: "ご飯" }));
      await screen.findByRole("alert");
      expect(screen.getByRole("img")).not.toHaveClass("hop");
    });
  });

  describe("hatching", () => {
    const hatching = { ...base, hatching: true };
    const settled = { ...base, hatching: false };
    const showPopup = (visible: boolean) => {
      act(() => emitVisible(visible));
    };
    const tick = async (ms: number) => {
      await act(async () => {
        vi.advanceTimersByTime(ms);
      });
    };

    beforeEach(() => {
      vi.useFakeTimers({ shouldAdvanceTime: true });
      api.getState.mockResolvedValue(hatching);
      api.ackHatch.mockResolvedValue(settled);
    });
    afterEach(() => {
      vi.useRealTimers();
    });

    it("shakes the egg first, then flashes, then shows the newborn and reports it", async () => {
      render(<App />);
      await screen.findByRole("img");
      await waitFor(() => expect(api.onPopupVisibility).toHaveBeenCalled());
      showPopup(true);
      const egg = screen.getByRole("img");
      expect(egg).toHaveAttribute("alt", expect.stringContaining("stage 1"));
      expect(egg).toHaveClass("shake");
      expect(screen.queryByTestId("hatch-flash")).toBeNull();
      expect(api.ackHatch).not.toHaveBeenCalled();

      await tick(HATCH_SHAKE_MS + 10);
      expect(screen.getByTestId("hatch-flash")).toBeInTheDocument();
      expect(api.ackHatch).not.toHaveBeenCalled();

      await tick(HATCH_FLASH_MS + 10);
      expect(api.ackHatch).toHaveBeenCalledTimes(1);
      await waitFor(() => expect(screen.getByRole("img")).toHaveAttribute("alt", expect.stringContaining("stage 2")));
      expect(screen.queryByTestId("hatch-flash")).toBeNull();
    });

    it("taps while the egg shakes and cheers when the newborn appears", async () => {
      render(<App />);
      await screen.findByRole("img");
      await waitFor(() => expect(api.onPopupVisibility).toHaveBeenCalled());
      showPopup(true);
      expect(sound.play).toHaveBeenCalledWith("tap", { personality: "calm", stage: 2 });
      expect(sound.play).not.toHaveBeenCalledWith("hatch", expect.anything());
      await tick(HATCH_SHAKE_MS + 10);
      expect(sound.play).toHaveBeenCalledWith("hatch", { personality: "calm", stage: 2 });
    });

    it("plays the sequence only once", async () => {
      render(<App />);
      await screen.findByRole("img");
      await waitFor(() => expect(api.onPopupVisibility).toHaveBeenCalled());
      showPopup(true);
      await tick(HATCH_SHAKE_MS + HATCH_FLASH_MS + 500);
      await tick(5000);
      expect(api.ackHatch).toHaveBeenCalledTimes(1);
    });

    it("waits until the popup is actually opened before playing", async () => {
      render(<App />);
      const sprite = await screen.findByRole("img");
      expect(sprite).toHaveAttribute("alt", expect.stringContaining("stage 2"));
      await tick(HATCH_SHAKE_MS + HATCH_FLASH_MS + 500);
      expect(api.ackHatch).not.toHaveBeenCalled();

      await waitFor(() => expect(api.onPopupVisibility).toHaveBeenCalled());
      showPopup(true);
      expect(screen.getByRole("img")).toHaveClass("shake");
    });

    it("is interrupted, not lost, when the popup is closed midway", async () => {
      render(<App />);
      await screen.findByRole("img");
      await waitFor(() => expect(api.onPopupVisibility).toHaveBeenCalled());
      showPopup(true);
      expect(screen.getByRole("img")).toHaveClass("shake");
      showPopup(false);
      expect(screen.getByRole("img")).not.toHaveClass("shake");
      await tick(HATCH_SHAKE_MS + HATCH_FLASH_MS + 500);
      expect(api.ackHatch).not.toHaveBeenCalled();
      showPopup(true);
      expect(screen.getByRole("img")).toHaveClass("shake");
    });

    it("shows an error when the hatch cannot be recorded", async () => {
      api.ackHatch.mockRejectedValue(new Error("disk full"));
      render(<App />);
      await screen.findByRole("img");
      await waitFor(() => expect(api.onPopupVisibility).toHaveBeenCalled());
      showPopup(true);
      await tick(HATCH_SHAKE_MS + HATCH_FLASH_MS + 10);
      expect(await screen.findByRole("alert")).toHaveTextContent("disk full");
    });

    it("does not hatch anything for an egg that is still an egg", async () => {
      api.getState.mockResolvedValue({ ...base, stage: 1, hatching: false });
      render(<App />);
      await screen.findByRole("img");
      await waitFor(() => expect(api.onPopupVisibility).toHaveBeenCalled());
      showPopup(true);
      expect(screen.getByRole("img")).not.toHaveClass("shake");
    });

    it("unsubscribes from popup events on unmount", async () => {
      const { unmount } = render(<App />);
      await screen.findByRole("img");
      await waitFor(() => expect(api.onPopupVisibility).toHaveBeenCalled());
      unmount();
      await waitFor(() => expect(unlistenVisible).toHaveBeenCalled());
    });
  });

  describe("sleeping", () => {
    const asleep = (e: Partial<StateView["expression"]> = {}): StateView => ({
      ...base,
      sleeping: true,
      expression: { ...base.expression, bubble: "sleeping", ...e },
    });

    it("shows a Zzz bubble while the pet sleeps", async () => {
      api.getState.mockResolvedValue(asleep());
      render(<App />);
      expect(await screen.findByRole("status")).toHaveTextContent("Zzz…");
    });

    it("darkens the screen and lets the sprite sleep", async () => {
      api.getState.mockResolvedValue(asleep());
      render(<App />);
      const sprite = await screen.findByRole("img");
      expect(sprite).toHaveClass("sleeping");
      expect(screen.getByRole("main")).toHaveAttribute("data-sleep", "true");
    });

    it("does not droop with hunger while asleep", async () => {
      api.getState.mockResolvedValue(asleep({ hunger: "low" }));
      render(<App />);
      const sprite = await screen.findByRole("img");
      expect(sprite).toHaveClass("sleeping");
      expect(sprite).not.toHaveClass("hungry");
    });

    it("is awake by default", async () => {
      render(<App />);
      const sprite = await screen.findByRole("img");
      expect(sprite).not.toHaveClass("sleeping");
      expect(screen.getByRole("main")).toHaveAttribute("data-sleep", "false");
    });

    it("wakes up visually when a state-changed event says so", async () => {
      api.getState.mockResolvedValue(asleep());
      render(<App />);
      await screen.findByRole("img");
      await waitFor(() => expect(api.onStateChanged).toHaveBeenCalled());
      emit(base);
      await waitFor(() => expect(screen.getByRole("main")).toHaveAttribute("data-sleep", "false"));
      expect(screen.getByRole("img")).not.toHaveClass("sleeping");
    });
  });

  describe("naming", () => {
    const unnamed: StateView = { ...base, needsName: true };

    it("asks for a name once the pet has hatched and has none", async () => {
      api.getState.mockResolvedValue(unnamed);
      render(<App />);
      expect(await screen.findByRole("textbox", { name: "名前" })).toBeInTheDocument();
      expect(screen.getByText("名前をつけてあげてね")).toBeInTheDocument();
    });

    it("does not ask when no name is needed", async () => {
      render(<App />);
      await screen.findByRole("img");
      expect(screen.queryByRole("textbox", { name: "名前" })).toBeNull();
    });

    it("sends the typed name and shows it afterwards", async () => {
      api.getState.mockResolvedValue(unnamed);
      api.setName.mockResolvedValue({ ...base, name: "ごま", needsName: false });
      render(<App />);
      await userEvent.type(await screen.findByRole("textbox", { name: "名前" }), "ごま");
      await userEvent.click(screen.getByRole("button", { name: "決定" }));
      expect(api.setName).toHaveBeenCalledWith("ごま");
      expect(await screen.findByText("ごま")).toBeInTheDocument();
      expect(screen.queryByRole("textbox", { name: "名前" })).toBeNull();
    });

    it("limits the length of the input", async () => {
      api.getState.mockResolvedValue(unnamed);
      render(<App />);
      expect(await screen.findByRole("textbox", { name: "名前" })).toHaveAttribute("maxlength", "8");
    });

    it("also submits with the Enter key", async () => {
      api.getState.mockResolvedValue(unnamed);
      api.setName.mockResolvedValue({ ...base, name: "もち" });
      render(<App />);
      await userEvent.type(await screen.findByRole("textbox", { name: "名前" }), "もち{Enter}");
      expect(api.setName).toHaveBeenCalledWith("もち");
    });

    it("does not send an empty name", async () => {
      api.getState.mockResolvedValue(unnamed);
      render(<App />);
      await screen.findByRole("textbox", { name: "名前" });
      expect(screen.getByRole("button", { name: "決定" })).toBeDisabled();
      await userEvent.type(screen.getByRole("textbox", { name: "名前" }), "   ");
      expect(screen.getByRole("button", { name: "決定" })).toBeDisabled();
      expect(api.setName).not.toHaveBeenCalled();
    });

    it("ignores Enter on a blank input", async () => {
      api.getState.mockResolvedValue(unnamed);
      render(<App />);
      await userEvent.type(await screen.findByRole("textbox", { name: "名前" }), "  {Enter}");
      expect(api.setName).not.toHaveBeenCalled();
    });

    it("shows the reason when the name is rejected and keeps asking", async () => {
      api.getState.mockResolvedValue(unnamed);
      api.setName.mockRejectedValue(new Error("name must be at most 8 characters"));
      render(<App />);
      await userEvent.type(await screen.findByRole("textbox", { name: "名前" }), "ごま");
      await userEvent.click(screen.getByRole("button", { name: "決定" }));
      expect(await screen.findByRole("alert")).toHaveTextContent("at most 8");
      expect(screen.getByRole("textbox", { name: "名前" })).toBeInTheDocument();
    });

    it("lets the player decide later", async () => {
      api.getState.mockResolvedValue(unnamed);
      render(<App />);
      await userEvent.click(await screen.findByRole("button", { name: "あとで" }));
      expect(screen.queryByRole("textbox", { name: "名前" })).toBeNull();
      expect(screen.getByRole("img")).toBeInTheDocument();
    });

    it("shows the pet's name on the screen", async () => {
      api.getState.mockResolvedValue({ ...base, name: "ごま" });
      render(<App />);
      expect(await screen.findByText("ごま")).toHaveClass("name");
    });

    it("speaks with the name in the speech bubble", async () => {
      api.getState.mockResolvedValue({ ...base, name: "ごま", expression: { ...base.expression, bubble: "hungry" } });
      render(<App />);
      expect(await screen.findByRole("status")).toHaveTextContent("ごま「おなかすいたよ…」");
    });

    it("keeps the bubble unchanged for an unnamed pet", async () => {
      api.getState.mockResolvedValue({ ...base, expression: { ...base.expression, bubble: "happy" } });
      render(<App />);
      expect((await screen.findByRole("status")).textContent).toBe("たのしい！");
    });
  });

  describe("idle gestures", () => {
    beforeEach(() => {
      vi.useFakeTimers({ shouldAdvanceTime: true });
      vi.spyOn(Math, "random").mockReturnValue(0);
    });
    afterEach(() => {
      vi.useRealTimers();
      vi.restoreAllMocks();
    });

    const openPopup = async () => {
      await screen.findByRole("img");
      await waitFor(() => expect(api.onPopupVisibility).toHaveBeenCalled());
      act(() => emitVisible(true));
    };
    const wait = async (ms: number) => {
      await act(async () => {
        vi.advanceTimersByTime(ms);
      });
    };

    it("lets an awake pet move now and then while the popup is open", async () => {
      render(<App />);
      await openPopup();
      expect(screen.getByRole("img")).not.toHaveClass("bounce");
      await wait(IDLE_MIN_MS + 10);
      expect(screen.getByRole("img")).toHaveClass("bounce");
    });

    it("lets a cheerful pet dance", async () => {
      api.getState.mockResolvedValue(withExpression({ mood: "high", bubble: "happy" }));
      render(<App />);
      await openPopup();
      await wait(IDLE_MIN_MS + 10);
      expect(screen.getByRole("img")).toHaveClass("dance");
    });

    it("lets a gloomy pet only sigh and nod", async () => {
      api.getState.mockResolvedValue(withExpression({ mood: "low", bubble: "gloomy" }));
      render(<App />);
      await openPopup();
      await wait(IDLE_MIN_MS + 10);
      expect(screen.getByRole("img")).toHaveClass("sigh");
      expect(screen.getByRole("img")).not.toHaveClass("bounce");
    });

    it("lets an egg only rock gently", async () => {
      api.getState.mockResolvedValue({ ...base, stage: 1, playAction: "pet" });
      render(<App />);
      await openPopup();
      await wait(IDLE_MIN_MS + 10);
      expect(screen.getByRole("img")).toHaveClass("rock");
    });

    it("does not move while the popup is closed", async () => {
      render(<App />);
      await screen.findByRole("img");
      await wait(IDLE_MIN_MS * 3);
      expect(screen.getByRole("img")).not.toHaveClass("bounce");
    });

    it("does not move while asleep", async () => {
      api.getState.mockResolvedValue({ ...base, sleeping: true, expression: { ...base.expression, bubble: "sleeping" } });
      render(<App />);
      await openPopup();
      await wait(IDLE_MIN_MS * 3);
      expect(screen.getByRole("img")).not.toHaveClass("bounce");
    });

    it("does not move while hungry (it droops instead)", async () => {
      api.getState.mockResolvedValue({ ...base, expression: { ...base.expression, hunger: "low", bubble: "hungry" } });
      render(<App />);
      await openPopup();
      await wait(IDLE_MIN_MS * 3);
      expect(screen.getByRole("img")).not.toHaveClass("bounce");
      expect(screen.getByRole("img")).toHaveClass("hungry");
    });

    it("does not move during the opening", async () => {
      api.getState.mockResolvedValue({ ...base, stage: 1, playAction: "pet", intro: true, expression: { ...base.expression, bubble: "welcome" } });
      render(<App />);
      await openPopup();
      await wait(IDLE_MIN_MS * 3);
      expect(screen.getByRole("img")).not.toHaveClass("rock");
    });

    it("lets a reaction to a button take priority over a gesture", async () => {
      render(<App />);
      await openPopup();
      await wait(IDLE_MIN_MS + 10);
      expect(screen.getByRole("img")).toHaveClass("bounce");
      await userEvent.click(screen.getByRole("button", { name: "ご飯" }));
      await waitFor(() => expect(screen.getByRole("img")).toHaveClass("hop"));
      expect(screen.getByRole("img")).not.toHaveClass("bounce");
    });
  });

  describe("snack button", () => {
    it("sits between the meal and play buttons", async () => {
      render(<App />);
      await screen.findByRole("img");
      const names = within(screen.getByRole("group", { name: "ボタン" }))
        .getAllByRole("button")
        .map((b) => b.getAttribute("aria-label"));
      expect(names).toEqual(["ご飯", "おやつ", "遊ぶ", "掃除", "しつけ"]);
    });

    it("makes the pet hop like the other care buttons", async () => {
      render(<App />);
      await userEvent.click(await screen.findByRole("button", { name: "おやつ" }));
      await waitFor(() => expect(screen.getByRole("img")).toHaveClass("hop"));
    });

    it("is also available for an egg", async () => {
      api.getState.mockResolvedValue({ ...base, stage: 1, playAction: "pet" });
      render(<App />);
      expect(await screen.findByRole("button", { name: "おやつ" })).toBeInTheDocument();
    });
  });

  describe("mini game", () => {
    const report = (win: boolean, answer: "left" | "right") => ({ view: { ...base, mood: 90 }, win, answer });
    const openGame = async () => {
      await userEvent.click(await screen.findByRole("button", { name: "遊ぶ" }));
      return screen.findByRole("dialog", { name: "どっちの手？" });
    };

    it("opens instead of playing right away", async () => {
      render(<App />);
      const dialog = await openGame();
      expect(within(dialog).getByText("どっちの手に入ってるかな？")).toBeInTheDocument();
      expect(within(dialog).getByRole("button", { name: "左の手" })).toBeInTheDocument();
      expect(within(dialog).getByRole("button", { name: "右の手" })).toBeInTheDocument();
      expect(api.performAction).not.toHaveBeenCalled();
    });

    it("shows the game on the screen, apart from the buttons", async () => {
      render(<App />);
      const dialog = await openGame();
      expect(within(screen.getByRole("region", { name: "画面" })).getByRole("dialog")).toBe(dialog);
    });

    it("sends the chosen hand and celebrates a win", async () => {
      api.playMinigame.mockResolvedValue(report(true, "left"));
      render(<App />);
      const dialog = await openGame();
      await userEvent.click(within(dialog).getByRole("button", { name: "左の手" }));
      expect(api.playMinigame).toHaveBeenCalledWith("left");
      expect(await within(dialog).findByText("あたり！🎉")).toBeInTheDocument();
      expect(within(dialog).getByText("左の手でした")).toBeInTheDocument();
    });

    it("consoles the player after a loss and shows where the treat was", async () => {
      api.playMinigame.mockResolvedValue(report(false, "right"));
      render(<App />);
      const dialog = await openGame();
      await userEvent.click(within(dialog).getByRole("button", { name: "左の手" }));
      expect(await within(dialog).findByText("ざんねん…")).toBeInTheDocument();
      expect(within(dialog).getByText("右の手でした")).toBeInTheDocument();
    });

    it("updates the pet with the state returned by the game and makes it hop", async () => {
      localStorage.clear();
      enableGauges();
      api.playMinigame.mockResolvedValue(report(true, "right"));
      render(<App />);
      const dialog = await openGame();
      await userEvent.click(within(dialog).getByRole("button", { name: "右の手" }));
      await within(dialog).findByText("あたり！🎉");
      expect(screen.getByRole("progressbar", { name: "ご機嫌" })).toHaveAttribute("aria-valuenow", "90");
      expect(screen.getByRole("img")).toHaveClass("hop");
    });

    it("closes after the result", async () => {
      api.playMinigame.mockResolvedValue(report(true, "left"));
      render(<App />);
      const dialog = await openGame();
      await userEvent.click(within(dialog).getByRole("button", { name: "左の手" }));
      await userEvent.click(await within(dialog).findByRole("button", { name: "とじる" }));
      expect(screen.queryByRole("dialog")).toBeNull();
    });

    it("can be cancelled before choosing", async () => {
      render(<App />);
      const dialog = await openGame();
      await userEvent.click(within(dialog).getByRole("button", { name: "やめる" }));
      expect(screen.queryByRole("dialog")).toBeNull();
      expect(api.playMinigame).not.toHaveBeenCalled();
    });

    it("lets the player play again", async () => {
      api.playMinigame.mockResolvedValue(report(false, "right"));
      render(<App />);
      const dialog = await openGame();
      await userEvent.click(within(dialog).getByRole("button", { name: "左の手" }));
      await userEvent.click(await within(dialog).findByRole("button", { name: "もういちど" }));
      expect(within(dialog).getByText("どっちの手に入ってるかな？")).toBeInTheDocument();
      await userEvent.click(within(dialog).getByRole("button", { name: "右の手" }));
      expect(api.playMinigame).toHaveBeenLastCalledWith("right");
    });

    it("shows the reason when the game fails and keeps the choice open", async () => {
      api.playMinigame.mockRejectedValue(new Error("disk full"));
      render(<App />);
      const dialog = await openGame();
      await userEvent.click(within(dialog).getByRole("button", { name: "左の手" }));
      expect(await screen.findByRole("alert")).toHaveTextContent("disk full");
      expect(within(dialog).getByRole("button", { name: "左の手" })).toBeInTheDocument();
    });

    it("is not offered to an egg, which is just petted", async () => {
      api.getState.mockResolvedValue({ ...base, stage: 1, playAction: "pet" });
      api.performAction.mockResolvedValue({ ...base, stage: 1, playAction: "pet" });
      render(<App />);
      await userEvent.click(await screen.findByRole("button", { name: "撫でる" }));
      expect(api.performAction).toHaveBeenCalledWith("play");
      expect(screen.queryByRole("dialog")).toBeNull();
    });
  });

  describe("farewell", () => {
    const elder: StateView = { ...base, stage: 10, name: "ごま", graduating: true };
    const nextEgg: StateView = { ...base, stage: 1, playAction: "pet", generation: 2, intro: true, name: null, expression: { ...base.expression, bubble: "welcome" } };

    it("announces that it is time to say goodbye", async () => {
      api.getState.mockResolvedValue(elder);
      render(<App />);
      const dialog = await screen.findByRole("dialog", { name: "旅立ち" });
      expect(within(dialog).getByText("旅立ちの時がきたよ")).toBeInTheDocument();
      expect(within(dialog).getByText("ごまはりっぱに育ちました。ありがとう！")).toBeInTheDocument();
    });

    it("works for a pet that was never named", async () => {
      api.getState.mockResolvedValue({ ...elder, name: null });
      render(<App />);
      const dialog = await screen.findByRole("dialog", { name: "旅立ち" });
      expect(within(dialog).getByText("りっぱに育ちました。ありがとう！")).toBeInTheDocument();
    });

    it("stays out of the way before the time comes", async () => {
      render(<App />);
      await screen.findByRole("img");
      expect(screen.queryByRole("dialog", { name: "旅立ち" })).toBeNull();
    });

    it("shows it on the screen, apart from the buttons", async () => {
      api.getState.mockResolvedValue(elder);
      render(<App />);
      const dialog = await screen.findByRole("dialog", { name: "旅立ち" });
      expect(within(screen.getByRole("region", { name: "画面" })).getByRole("dialog", { name: "旅立ち" })).toBe(dialog);
    });

    it("sees the pet off and welcomes the next egg", async () => {
      api.getState.mockResolvedValue(elder);
      api.graduate.mockResolvedValue(nextEgg);
      render(<App />);
      await userEvent.click(await screen.findByRole("button", { name: "見送る" }));
      expect(api.graduate).toHaveBeenCalledTimes(1);
      await waitFor(() => expect(screen.queryByRole("dialog", { name: "旅立ち" })).toBeNull());
      expect(await screen.findByRole("status")).toHaveTextContent("卵が届いたよ！撫でてあげてね");
      expect(screen.getByRole("img")).toHaveAttribute("alt", expect.stringContaining("stage 1"));
    });

    it("lets the player stay a little longer", async () => {
      api.getState.mockResolvedValue(elder);
      render(<App />);
      await userEvent.click(await screen.findByRole("button", { name: "もう少しいっしょにいる" }));
      expect(screen.queryByRole("dialog", { name: "旅立ち" })).toBeNull();
      expect(api.graduate).not.toHaveBeenCalled();
    });

    it("asks again for the next generation even after staying longer once", async () => {
      api.getState.mockResolvedValue(elder);
      api.graduate.mockResolvedValue(nextEgg);
      render(<App />);
      await userEvent.click(await screen.findByRole("button", { name: "もう少しいっしょにいる" }));
      await waitFor(() => expect(api.onStateChanged).toHaveBeenCalled());
      emit({ ...elder, generation: 2 }); // 次の世代が旅立ちの時を迎えた
      // 「もう少しいっしょにいる」は、その世代にだけ効く
      expect(await screen.findByRole("dialog", { name: "旅立ち" })).toBeInTheDocument();
    });

    it("shows the reason when the goodbye fails and keeps the dialog", async () => {
      api.getState.mockResolvedValue(elder);
      api.graduate.mockRejectedValue(new Error("not yet"));
      render(<App />);
      await userEvent.click(await screen.findByRole("button", { name: "見送る" }));
      expect(await screen.findByRole("alert")).toHaveTextContent("not yet");
      expect(screen.getByRole("dialog", { name: "旅立ち" })).toBeInTheDocument();
    });

    it("does not put the generation on the screen either", async () => {
      enableGauges();
      api.getState.mockResolvedValue({ ...base, generation: 3 });
      render(<App />);
      await screen.findByRole("img");
      expect(screen.queryByText(/代目/)).toBeNull();
    });
  });

  describe("breathing", () => {
    it("breathes slowly all the time while awake", async () => {
      render(<App />);
      const sprite = await screen.findByRole("img");
      expect(sprite.parentElement).toHaveClass("breathing");
    });

    it("keeps breathing even when the popup is closed or the pet is hungry", async () => {
      api.getState.mockResolvedValue(withExpression({ hunger: "low", bubble: "hungry" }));
      render(<App />);
      expect((await screen.findByRole("img")).parentElement).toHaveClass("breathing");
    });

    it("does not breathe like that while asleep (it has its own slow motion)", async () => {
      api.getState.mockResolvedValue({ ...base, sleeping: true, expression: { ...base.expression, bubble: "sleeping" } });
      render(<App />);
      const sprite = await screen.findByRole("img");
      expect(sprite.parentElement).not.toHaveClass("breathing");
      expect(sprite).toHaveClass("sleeping");
    });

    it("does not breathe during the hatching show", async () => {
      vi.useFakeTimers({ shouldAdvanceTime: true });
      api.getState.mockResolvedValue({ ...base, hatching: true });
      render(<App />);
      await screen.findByRole("img");
      await waitFor(() => expect(api.onPopupVisibility).toHaveBeenCalled());
      act(() => emitVisible(true));
      expect(screen.getByRole("img")).toHaveClass("shake");
      expect(screen.getByRole("img").parentElement).not.toHaveClass("breathing");
      vi.useRealTimers();
    });
  });

  describe("sound", () => {
    const calmVoice = { personality: "calm", stage: 2 };

    it("follows the sound setting", async () => {
      render(<App />);
      await screen.findByRole("img");
      expect(sound.useSound).toHaveBeenLastCalledWith(true, { volume: 50, pitch: 0 });
    });

    it("is switched off by the setting", async () => {
      api.getSettings.mockResolvedValue({ ...DEFAULT_SETTINGS, soundEnabled: false });
      render(<App />);
      await screen.findByRole("img");
      await waitFor(() => expect(sound.useSound).toHaveBeenLastCalledWith(false, { volume: 50, pitch: 0 }));
    });

    it.each([["ご飯"], ["おやつ"]])("munches when given %s", async (label) => {
      render(<App />);
      await userEvent.click(await screen.findByRole("button", { name: label }));
      await waitFor(() => expect(sound.play).toHaveBeenCalledWith("eat", calmVoice));
    });

    it("gives an ordinary cry when the room is cleaned", async () => {
      render(<App />);
      await userEvent.click(await screen.findByRole("button", { name: "掃除" }));
      await waitFor(() => expect(sound.play).toHaveBeenCalledWith("neutral", calmVoice));
    });

    it("taps when an egg is stroked", async () => {
      const egg = { ...base, stage: 1, playAction: "pet" } as StateView;
      api.getState.mockResolvedValue(egg);
      api.performAction.mockResolvedValue(egg);
      render(<App />);
      await userEvent.click(await screen.findByRole("button", { name: "撫でる" }));
      await waitFor(() => expect(sound.play).toHaveBeenCalledWith("tap", { personality: "calm", stage: 1 }));
    });

    it("uses the voice of the pet as it is after the action", async () => {
      api.performAction.mockResolvedValue({ ...base, stage: 5, personality: "playful" });
      render(<App />);
      await userEvent.click(await screen.findByRole("button", { name: "ご飯" }));
      await waitFor(() => expect(sound.play).toHaveBeenCalledWith("eat", { personality: "playful", stage: 5 }));
    });

    it("stays quiet when the action fails", async () => {
      api.performAction.mockRejectedValue(new Error("x"));
      render(<App />);
      await userEvent.click(await screen.findByRole("button", { name: "ご飯" }));
      await screen.findByRole("alert");
      expect(sound.play).not.toHaveBeenCalled();
    });

    it("cheers when the mini game is won and sighs when it is lost", async () => {
      api.playMinigame.mockResolvedValueOnce({ view: base, win: true, answer: "left" }).mockResolvedValueOnce({ view: base, win: false, answer: "right" });
      render(<App />);
      await userEvent.click(await screen.findByRole("button", { name: "遊ぶ" }));
      const dialog = await screen.findByRole("dialog", { name: "どっちの手？" });
      await userEvent.click(within(dialog).getByRole("button", { name: "左の手" }));
      await waitFor(() => expect(sound.play).toHaveBeenCalledWith("happy", calmVoice));
      await userEvent.click(await within(dialog).findByRole("button", { name: "もういちど" }));
      await userEvent.click(within(dialog).getByRole("button", { name: "左の手" }));
      await waitFor(() => expect(sound.play).toHaveBeenCalledWith("sad", calmVoice));
    });

    describe("greeting when the popup is opened", () => {
      const open = async (state: StateView) => {
        api.getState.mockResolvedValue(state);
        render(<App />);
        await screen.findByRole("img");
        await waitFor(() => expect(api.onPopupVisibility).toHaveBeenCalled());
        act(() => emitVisible(true));
      };

      it("is cheerful for a happy pet", async () => {
        await open(withExpression({ mood: "high", bubble: "happy" }));
        expect(sound.play).toHaveBeenCalledWith("happy", calmVoice);
      });

      it("is ordinary for an ordinary pet", async () => {
        await open(base);
        expect(sound.play).toHaveBeenCalledWith("neutral", calmVoice);
      });

      it("is sad for a gloomy pet", async () => {
        await open(withExpression({ mood: "low", bubble: "gloomy" }));
        expect(sound.play).toHaveBeenCalledWith("sad", calmVoice);
      });

      it("is sad for a hungry pet, even when its mood is fine", async () => {
        await open(withExpression({ mood: "high", hunger: "low", bubble: "hungry" }));
        expect(sound.play).toHaveBeenCalledWith("sad", calmVoice);
      });

      it("is silent while the pet sleeps", async () => {
        await open({ ...base, sleeping: true, expression: { ...base.expression, bubble: "sleeping" } });
        expect(sound.play).not.toHaveBeenCalled();
      });

      it("says nothing when the popup is closed", async () => {
        await open(base);
        sound.play.mockClear();
        act(() => emitVisible(false));
        expect(sound.play).not.toHaveBeenCalled();
      });

      it("greets again every time the popup is opened", async () => {
        await open(base);
        act(() => emitVisible(false));
        act(() => emitVisible(true));
        expect(sound.play).toHaveBeenCalledTimes(2);
      });
    });

    describe("idle cries", () => {
      beforeEach(() => {
        vi.useFakeTimers({ shouldAdvanceTime: true });
        vi.setSystemTime(new Date("2026-10-17T12:00:00"));
        vi.spyOn(Math, "random").mockReturnValue(0);
      });
      afterEach(() => {
        vi.useRealTimers();
        vi.restoreAllMocks();
      });
      const openAndWait = async () => {
        render(<App />);
        await screen.findByRole("img");
        await waitFor(() => expect(api.onPopupVisibility).toHaveBeenCalled());
        act(() => emitVisible(true));
        sound.play.mockClear(); // あいさつは別のテストで確認する
      };
      const advance = async (ms: number) => {
        await act(async () => {
          vi.advanceTimersByTime(ms);
        });
      };

      it("sometimes cries along with a gesture", async () => {
        await openAndWait();
        await advance(IDLE_MIN_MS + 10);
        expect(sound.play).toHaveBeenCalledWith("happy", calmVoice); // bounce → happy
      });

      it("usually stays quiet during a gesture", async () => {
        vi.spyOn(Math, "random").mockReturnValue(0.99);
        await openAndWait();
        await advance(IDLE_MAX_MS + 10);
        expect(sound.play).not.toHaveBeenCalled();
      });

      it("does not cry again within the cooldown", async () => {
        await openAndWait();
        await advance(IDLE_MIN_MS + 10);
        for (let elapsed = 0; elapsed < 8000; elapsed += 500) await advance(500); // 次のしぐさが何度か始まる
        expect(sound.play).toHaveBeenCalledTimes(1);
      });

      it("cries again once the cooldown has passed", async () => {
        await openAndWait();
        await advance(IDLE_MIN_MS + 10);
        // 一度にまとめて進めると、途中のしぐさが画面に反映されないので、少しずつ進める。
        for (let elapsed = 0; elapsed < IDLE_CRY_COOLDOWN_MS + IDLE_MAX_MS; elapsed += 500) await advance(500);
        expect(sound.play.mock.calls.length).toBeGreaterThanOrEqual(2);
      });
    });
  });
});

describe("stroking the character", () => {
  const stroked = { ...base, mood: 52 };

  it("clicking the character cheers it up: the state is updated, hearts float and it cries happily", async () => {
    api.strokePet.mockResolvedValue(stroked);
    render(<App />);
    await userEvent.click(await screen.findByRole("img"));
    expect(api.strokePet).toHaveBeenCalledTimes(1);
    expect(await screen.findByTestId("hearts")).toBeInTheDocument();
    expect(sound.play).toHaveBeenCalledWith("happy", { personality: "calm", stage: 2 });
  });

  it("rubbing the character also counts", async () => {
    api.strokePet.mockResolvedValue(stroked);
    render(<App />);
    const sprite = await screen.findByRole("img");
    fireEvent.mouseMove(sprite, { clientX: 0, clientY: 0 });
    fireEvent.mouseMove(sprite, { clientX: 100, clientY: 0 });
    await waitFor(() => expect(api.strokePet).toHaveBeenCalledTimes(1));
  });

  it("the hearts disappear when their animation ends", async () => {
    api.strokePet.mockResolvedValue(stroked);
    render(<App />);
    await userEvent.click(await screen.findByRole("img"));
    const hearts = await screen.findByTestId("hearts");
    for (const name of ["animationend", "webkitAnimationEnd"]) fireEvent(hearts, new Event(name, { bubbles: true }));
    await waitFor(() => expect(screen.queryByTestId("hearts")).toBeNull());
  });

  it("a sleeping character is not woken by a cry or hearts", async () => {
    const asleep = { ...base, sleeping: true };
    api.getState.mockResolvedValue(asleep);
    api.strokePet.mockResolvedValue(asleep);
    render(<App />);
    await userEvent.click(await screen.findByRole("img"));
    await waitFor(() => expect(api.strokePet).toHaveBeenCalled());
    expect(screen.queryByTestId("hearts")).toBeNull();
    expect(sound.play).not.toHaveBeenCalled();
  });

  it("shows the error when stroking fails", async () => {
    api.strokePet.mockRejectedValue(new Error("save failed"));
    render(<App />);
    await userEvent.click(await screen.findByRole("img"));
    expect(await screen.findByRole("alert")).toHaveTextContent("save failed");
  });
});

describe("stroke reaction by species", () => {
  it.each([
    ["calm", "nod"],
    ["playful", "dance"],
    ["gluttonous", "flop"],
    ["tidy", "spin"],
  ] as const)("a %s pet reacts with %s", async (personality, reaction) => {
    const pet = { ...base, stage: 5, personality };
    api.getState.mockResolvedValue(pet);
    api.strokePet.mockResolvedValue(pet);
    render(<App />);
    await userEvent.click(await screen.findByRole("img"));
    await waitFor(() => expect(screen.getByRole("img")).toHaveClass(reaction));
  });
});

describe("sound tuning", () => {
  it("passes the saved volume and pitch to the sound", async () => {
    api.getSettings.mockResolvedValue({ ...DEFAULT_SETTINGS, soundVolume: 90, soundPitch: -2 });
    render(<App />);
    await waitFor(() => expect(sound.useSound).toHaveBeenLastCalledWith(true, { volume: 90, pitch: -2 }));
  });
});

describe("care quality", () => {
  it("shows the discipline and weight gauges when the gauges are on", async () => {
    api.getSettings.mockResolvedValue(gaugesOn);
    api.getState.mockResolvedValue({ ...base, discipline: 30, weight: 62 });
    render(<App />);
    expect(await screen.findByRole("progressbar", { name: "しつけ度" })).toHaveAttribute("aria-valuenow", "30");
    expect(screen.getByRole("progressbar", { name: "体重" })).toHaveAttribute("aria-valuenow", "62");
  });

  it("scolding makes the pet cry sadly", async () => {
    render(<App />);
    await userEvent.click(await screen.findByRole("button", { name: "しつけ" }));
    await waitFor(() => expect(sound.play).toHaveBeenCalledWith("sad", { personality: "calm", stage: 2 }));
  });
});

describe("poop and sickness", () => {
  it("shows no poop and no sickness mark for a clean, healthy pet", async () => {
    render(<App />);
    await screen.findByRole("img", { name: /stage/ });
    expect(screen.queryByRole("img", { name: /うんち/ })).toBeNull();
    expect(screen.queryByRole("img", { name: "病気" })).toBeNull();
    expect(screen.queryByRole("button", { name: "治療" })).toBeNull();
  });

  it("shows as many poops as the view says", async () => {
    api.getState.mockResolvedValue({ ...base, poops: 3 });
    render(<App />);
    expect(await screen.findByRole("img", { name: "うんち 3個" })).toHaveTextContent("💩💩💩");
  });

  it("a sick pet shows the mark and a treatment button that sends the cure action", async () => {
    api.getState.mockResolvedValue({ ...base, sick: true });
    render(<App />);
    expect(await screen.findByRole("img", { name: "病気" })).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "治療" }));
    expect(api.performAction).toHaveBeenCalledWith("cure");
    await waitFor(() => expect(sound.play).toHaveBeenCalledWith("sad", { personality: "calm", stage: 2 }));
  });
});

describe("strict mode: the end", () => {
  it("shows the grave for a dead pet and lets you welcome a new egg", async () => {
    api.getState.mockResolvedValue({ ...base, dead: true, name: "ごま" });
    api.buryPet.mockResolvedValue({ ...base, stage: 1, generation: 2 });
    render(<App />);
    expect(await screen.findByRole("dialog", { name: "お別れ" })).toHaveTextContent("ごまは天国へ旅立ちました");
    await userEvent.click(screen.getByRole("button", { name: "新しい卵を迎える" }));
    expect(api.buryPet).toHaveBeenCalledTimes(1);
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "お別れ" })).toBeNull());
  });

  it("does not show the grave for a living pet", async () => {
    render(<App />);
    await screen.findByRole("img");
    expect(screen.queryByRole("dialog", { name: "お別れ" })).toBeNull();
  });

  it("an unnamed pet's grave still reads well", async () => {
    api.getState.mockResolvedValue({ ...base, dead: true });
    render(<App />);
    expect(await screen.findByRole("dialog", { name: "お別れ" })).toHaveTextContent("天国へ旅立ちました");
  });

  it("shows the error when welcoming a new egg fails", async () => {
    api.getState.mockResolvedValue({ ...base, dead: true });
    api.buryPet.mockRejectedValue(new Error("save failed"));
    render(<App />);
    await userEvent.click(await screen.findByRole("button", { name: "新しい卵を迎える" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("save failed");
  });
});
