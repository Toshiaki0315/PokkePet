import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { SouvenirEntry, SouvenirView } from "./api";

const api = vi.hoisted(() => ({
  getSouvenirs: vi.fn(),
  toggleSouvenir: vi.fn(),
  onStateChanged: vi.fn(),
}));
vi.mock("./api", () => api);

import Souvenirs from "./Souvenirs";

const entry = (id: string, emoji: string, name: string, rarity: number, count: number, displayed = false): SouvenirEntry => ({ id, emoji, name, rarity, count, displayed });
const book = (displayed = false): SouvenirView => ({
  total: 24,
  found: 2,
  maxDisplayed: 3,
  entries: [entry("shell", "🐚", "かいがら", 0, 3, displayed), entry("acorn", "🌰", "どんぐり", 0, 0), entry("gem", "💎", "ほうせき", 2, 1)],
});

let emit: () => void;
const unlisten = vi.fn();

beforeEach(() => {
  vi.resetAllMocks();
  api.getSouvenirs.mockResolvedValue(book());
  api.onStateChanged.mockImplementation((h: () => void) => {
    emit = h;
    return Promise.resolve(unlisten);
  });
});

describe("Souvenirs window", () => {
  it("shows how many have been found", async () => {
    render(<Souvenirs />);
    expect(await screen.findByText("集めた 2 / 24")).toBeInTheDocument();
    expect(screen.getByRole("progressbar", { name: "集めた数" })).toHaveAttribute("aria-valuenow", "2");
  });

  it("shows owned items by name and rarity, with counts, and unknown ones as question marks", async () => {
    render(<Souvenirs />);
    const list = await screen.findByRole("list", { name: "おみやげ" });
    expect(within(list).getByRole("button", { name: "かいがら（よくある）" })).toHaveTextContent("🐚×3");
    expect(within(list).getByRole("button", { name: "ほうせき（とてもめずらしい）" })).toHaveTextContent("💎");
    const unknown = within(list).getByRole("button", { name: "まだ見つけていない" });
    expect(unknown).toBeDisabled();
    expect(unknown).toHaveTextContent("？");
  });

  it("displays and puts away an item with a click", async () => {
    api.toggleSouvenir.mockResolvedValue(book(true));
    render(<Souvenirs />);
    const shell = await screen.findByRole("button", { name: "かいがら（よくある）" });
    expect(shell).toHaveAttribute("aria-pressed", "false");
    await userEvent.click(shell);
    expect(api.toggleSouvenir).toHaveBeenCalledWith("shell");
    await waitFor(() => expect(screen.getByRole("button", { name: "かいがら（よくある）" })).toHaveAttribute("aria-pressed", "true"));
  });

  it("shows the reason when the shelf is full", async () => {
    api.toggleSouvenir.mockRejectedValue(new Error("only 3 souvenirs can be displayed at once"));
    render(<Souvenirs />);
    await userEvent.click(await screen.findByRole("button", { name: "かいがら（よくある）" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("only 3");
  });

  it("reloads when the game state changes (new souvenirs after an outing)", async () => {
    render(<Souvenirs />);
    await screen.findByText("集めた 2 / 24");
    api.getSouvenirs.mockResolvedValue({ ...book(), found: 3 });
    await waitFor(() => expect(api.onStateChanged).toHaveBeenCalled());
    emit();
    expect(await screen.findByText("集めた 3 / 24")).toBeInTheDocument();
  });

  it("shows an error when the collection cannot be loaded", async () => {
    api.getSouvenirs.mockRejectedValue(new Error("unreadable"));
    render(<Souvenirs />);
    expect(await screen.findByRole("alert")).toHaveTextContent("unreadable");
  });

  it("stops listening when closed", async () => {
    const { unmount } = render(<Souvenirs />);
    await screen.findByText("集めた 2 / 24");
    await waitFor(() => expect(api.onStateChanged).toHaveBeenCalled());
    unmount();
    await waitFor(() => expect(unlisten).toHaveBeenCalled());
  });
});
