import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { StateView } from "./api";

const api = vi.hoisted(() => ({
  getState: vi.fn(),
  performAction: vi.fn(),
  onStateChanged: vi.fn(),
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
};
const withExpression = (e: Partial<StateView["expression"]>): StateView => ({ ...base, expression: { ...base.expression, ...e } });
const KEY = "pokkepet.showGauges";
const konami = async () => {
  await userEvent.keyboard("{ArrowUp}{ArrowUp}{ArrowDown}{ArrowDown}");
};
let emit: (v: StateView) => void;
const unlisten = vi.fn();

beforeEach(() => {
  vi.resetAllMocks();
  localStorage.clear();
  api.getState.mockResolvedValue(base);
  api.performAction.mockResolvedValue(base);
  api.onStateChanged.mockImplementation((h: (v: StateView) => void) => {
    emit = h;
    return Promise.resolve(unlisten);
  });
});

describe("App", () => {
  it("renders four gauges from get_state when enabled", async () => {
    localStorage.setItem(KEY, "1");
    render(<App />);
    for (const [name, now] of [["ご機嫌", "50"], ["親密度", "10"], ["空腹度", "80"], ["汚れ度", "5"]]) {
      expect(await screen.findByRole("progressbar", { name })).toHaveAttribute("aria-valuenow", now);
    }
  });

  it("shows the stage (with the gauges) and the sprite", async () => {
    localStorage.setItem(KEY, "1");
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
    localStorage.setItem(KEY, "1");
    api.performAction.mockResolvedValue({ ...base, hunger: 100 });
    render(<App />);
    await userEvent.click(await screen.findByRole("button", { name: "ご飯" }));
    await waitFor(() => expect(screen.getByRole("progressbar", { name: "空腹度" })).toHaveAttribute("aria-valuenow", "100"));
  });

  it("updates the view when a state-changed event arrives", async () => {
    localStorage.setItem(KEY, "1");
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

    it("remembers the setting across reloads", async () => {
      const { unmount } = render(<App />);
      await screen.findByRole("img");
      await konami();
      expect(localStorage.getItem(KEY)).toBe("1");
      unmount();
      render(<App />);
      expect(await screen.findAllByRole("progressbar")).toHaveLength(4);
    });

    it("still works when localStorage is unavailable", async () => {
      const get = vi.spyOn(Storage.prototype, "getItem").mockImplementation(() => {
        throw new Error("blocked");
      });
      const set = vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => {
        throw new Error("blocked");
      });
      render(<App />);
      await screen.findByRole("img");
      await konami();
      expect(screen.getAllByRole("progressbar")).toHaveLength(4);
      get.mockRestore();
      set.mockRestore();
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
      localStorage.setItem(KEY, "1");
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
});
