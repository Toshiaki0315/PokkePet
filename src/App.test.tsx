import { fireEvent, render, screen, waitFor } from "@testing-library/react";
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

const base: StateView = { mood: 50, affinity: 10, hunger: 80, dirtiness: 5, stage: 2, branch: "normal", personality: "calm" };
let emit: (v: StateView) => void;
const unlisten = vi.fn();

beforeEach(() => {
  vi.resetAllMocks();
  api.getState.mockResolvedValue(base);
  api.performAction.mockResolvedValue(base);
  api.onStateChanged.mockImplementation((h: (v: StateView) => void) => {
    emit = h;
    return Promise.resolve(unlisten);
  });
});

describe("App", () => {
  it("renders four gauges from get_state", async () => {
    render(<App />);
    for (const [name, now] of [["ご機嫌", "50"], ["親密度", "10"], ["空腹度", "80"], ["汚れ度", "5"]]) {
      expect(await screen.findByRole("progressbar", { name })).toHaveAttribute("aria-valuenow", now);
    }
  });

  it("shows the stage and the sprite", async () => {
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
    api.performAction.mockResolvedValue({ ...base, hunger: 100 });
    render(<App />);
    await userEvent.click(await screen.findByRole("button", { name: "ご飯" }));
    await waitFor(() => expect(screen.getByRole("progressbar", { name: "空腹度" })).toHaveAttribute("aria-valuenow", "100"));
  });

  it("updates the view when a state-changed event arrives", async () => {
    render(<App />);
    await screen.findByRole("progressbar", { name: "ご機嫌" });
    await waitFor(() => expect(api.onStateChanged).toHaveBeenCalled());
    emit({ ...base, mood: 77 });
    await waitFor(() => expect(screen.getByRole("progressbar", { name: "ご機嫌" })).toHaveAttribute("aria-valuenow", "77"));
  });

  it("unsubscribes the listener on unmount", async () => {
    const { unmount } = render(<App />);
    await waitFor(() => expect(api.onStateChanged).toHaveBeenCalled());
    await screen.findByRole("progressbar", { name: "ご機嫌" });
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
});
