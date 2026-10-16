import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Settings, StateView } from "./api";
import { DEFAULT_SETTINGS } from "./defaults";
import { HATCH_FLASH_MS, HATCH_SHAKE_MS } from "./hatch";
import { IDLE_MIN_MS } from "./idle";

const api = vi.hoisted(() => ({
  getState: vi.fn(),
  performAction: vi.fn(),
  onStateChanged: vi.fn(),
  ackHatch: vi.fn(),
  setName: vi.fn(),
  getSettings: vi.fn(),
  saveSettings: vi.fn(),
  onSettingsChanged: vi.fn(),
  onPopupVisibility: vi.fn(),
}));
vi.mock("./api", () => api);

import App from "./App";

const base: StateView = {
  mood: 50,
  affinity: 10,
  hunger: 80,
  dirtiness: 5,
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
};
const withExpression = (e: Partial<StateView["expression"]>): StateView => ({ ...base, expression: { ...base.expression, ...e } });
const gaugesOn: Settings = { ...DEFAULT_SETTINGS, showStage: true, showGauges: true };
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

  it("shows the stage (with the gauges) and the sprite", async () => {
    enableGauges();
    render(<App />);
    expect(await screen.findByText("段階 2 / 10")).toBeInTheDocument();
    expect(screen.getByRole("img")).toHaveAttribute("alt", "stage 2 normal calm");
  });

  it.each([["ご飯", "feed"], ["遊ぶ", "play"], ["掃除", "clean"]])("%s button calls performAction(%s)", async (label, action) => {
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
    await userEvent.click(await screen.findByRole("button", { name: "遊ぶ" }));
    await screen.findByRole("alert");
    await userEvent.click(screen.getByRole("button", { name: "遊ぶ" }));
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
    await userEvent.click(await screen.findByRole("button", { name: "遊ぶ" }));
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
      expect(screen.getAllByRole("progressbar")).toHaveLength(4);
      await konami();
      expect(screen.queryByRole("progressbar")).toBeNull();
    });

    it("ignores wrong sequences and resets on a stray key", async () => {
      render(<App />);
      await screen.findByRole("img");
      await userEvent.keyboard("{ArrowUp}{ArrowDown}{ArrowDown}{ArrowDown}");
      expect(screen.queryByRole("progressbar")).toBeNull();
      await userEvent.keyboard("{ArrowUp}{ArrowUp}{ArrowUp}{ArrowDown}{ArrowDown}");
      expect(screen.getAllByRole("progressbar")).toHaveLength(4);
    });

    it("saves both switches through the settings API when the command is used", async () => {
      render(<App />);
      await screen.findByRole("img");
      await konami();
      await waitFor(() => expect(api.saveSettings).toHaveBeenCalledWith({ ...DEFAULT_SETTINGS, showStage: true, showGauges: true }));
      await konami();
      await waitFor(() => expect(api.saveSettings).toHaveBeenLastCalledWith({ ...DEFAULT_SETTINGS, showStage: false, showGauges: false }));
    });

    it("turns both off when only one of them was on", async () => {
      api.getSettings.mockResolvedValue({ ...DEFAULT_SETTINGS, showStage: true });
      render(<App />);
      await screen.findByText("段階 2 / 10");
      await konami();
      await waitFor(() => expect(api.saveSettings).toHaveBeenCalledWith({ ...DEFAULT_SETTINGS, showStage: false, showGauges: false }));
    });

    it("shows an error when saving the settings fails", async () => {
      api.saveSettings.mockRejectedValue(new Error("disk full"));
      render(<App />);
      await screen.findByRole("img");
      await konami();
      expect(await screen.findByRole("alert")).toHaveTextContent("disk full");
    });

    it("shows the stage and the gauges independently according to the settings", async () => {
      api.getSettings.mockResolvedValue({ ...DEFAULT_SETTINGS, showStage: true });
      render(<App />);
      expect(await screen.findByText("段階 2 / 10")).toBeInTheDocument();
      expect(screen.queryByRole("progressbar")).toBeNull();
    });

    it("follows settings-changed events from the settings dialog", async () => {
      render(<App />);
      await screen.findByRole("img");
      await waitFor(() => expect(api.onSettingsChanged).toHaveBeenCalled());
      emitSettings(gaugesOn);
      expect(await screen.findAllByRole("progressbar")).toHaveLength(4);
      expect(screen.getByText("段階 2 / 10")).toBeInTheDocument();
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
    it("hides the stage label together with the gauges and toggles it with the hidden command", async () => {
      render(<App />);
      await screen.findByRole("img");
      expect(screen.queryByText("段階 2 / 10")).toBeNull();
      await konami();
      expect(screen.getByText("段階 2 / 10")).toBeInTheDocument();
      await konami();
      expect(screen.queryByText("段階 2 / 10")).toBeNull();
    });

    it("keeps the screen (character, bubble) apart from the buttons", async () => {
      api.getState.mockResolvedValue(withExpression({ bubble: "happy" }));
      render(<App />);
      const display = await screen.findByRole("region", { name: "画面" });
      const controls = screen.getByRole("group", { name: "ボタン" });
      expect(within(display).getByRole("img")).toBeInTheDocument();
      expect(within(display).getByRole("status")).toHaveTextContent("たのしい！");
      expect(within(display).queryByRole("button")).toBeNull();
      expect(within(controls).getAllByRole("button")).toHaveLength(3);
      expect(within(controls).queryByRole("img")).toBeNull();
      expect(within(controls).queryByRole("status")).toBeNull();
    });

    it("shows the gauges and the stage label on the screen when enabled", async () => {
      enableGauges();
      render(<App />);
      const display = await screen.findByRole("region", { name: "画面" });
      expect(within(display).getAllByRole("progressbar")).toHaveLength(4);
      expect(within(display).getByText("段階 2 / 10")).toBeInTheDocument();
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
});
