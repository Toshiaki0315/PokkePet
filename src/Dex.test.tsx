import { render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Branch, DexView, Personality } from "./api";

const api = vi.hoisted(() => ({
  getDex: vi.fn(),
  onStateChanged: vi.fn(),
}));
vi.mock("./api", () => api);

import Dex from "./Dex";

const BRANCHES: Branch[] = ["poor", "normal", "good"];
const PERSONALITIES: Personality[] = ["calm", "playful", "gluttonous", "tidy"];

/** Rust の `dex::build_view` と同じ並び（卵→段階順→分岐→性格）で 113 件を作る。 */
function dexWith(found: string[]): DexView {
  const entries: DexView["entries"] = [{ key: "egg", stage: 1, branch: "normal", personality: "calm", discovered: found.includes("egg") }];
  for (let stage = 2; stage <= 10; stage++)
    for (const branch of stage === 10 ? [...BRANCHES, "legend" as Branch] : BRANCHES)
      for (const personality of PERSONALITIES) {
        const key = `${stage}-${branch}-${personality}`;
        entries.push({ key, stage, branch, personality, discovered: found.includes(key) });
      }
  return { total: entries.length, discovered: entries.filter((e) => e.discovered).length, entries };
}

let emit: () => void;
const unlisten = vi.fn();

beforeEach(() => {
  vi.resetAllMocks();
  api.getDex.mockResolvedValue(dexWith(["egg", "2-normal-calm", "5-good-playful"]));
  api.onStateChanged.mockImplementation((h: () => void) => {
    emit = h;
    return Promise.resolve(unlisten);
  });
});

describe("Dex window", () => {
  it("shows how many forms have been discovered", async () => {
    render(<Dex />);
    expect(await screen.findByText("発見 3 / 113")).toBeInTheDocument();
    const bar = screen.getByRole("progressbar", { name: "発見数" });
    expect(bar).toHaveAttribute("aria-valuenow", "3");
    expect(bar).toHaveAttribute("max", "113");
  });

  it("has one section per growth stage", async () => {
    render(<Dex />);
    await screen.findByText("発見 3 / 113");
    for (let stage = 1; stage <= 10; stage++) expect(screen.getByRole("region", { name: `段階 ${stage}` })).toBeInTheDocument();
    expect(within(screen.getByRole("region", { name: "段階 1" })).getAllByRole("img")).toHaveLength(1);
    expect(within(screen.getByRole("region", { name: "段階 7" })).getAllByRole("img")).toHaveLength(12);
  });

  it("shows discovered forms with their sprite and name", async () => {
    render(<Dex />);
    await screen.findByText("発見 3 / 113");
    const egg = screen.getByRole("img", { name: "卵" });
    expect(egg).toHaveAttribute("src", "/sprites/egg.png");
    const fox = screen.getByRole("img", { name: "段階 5・きらきら・きつね" });
    expect(fox).toHaveAttribute("src", "/sprites/5-good-playful.png");
    expect(fox).not.toHaveClass("unknown");
    expect(screen.getByRole("img", { name: "段階 2・ふつう・ふわもち" })).toBeInTheDocument();
  });

  it("shows the forms not met yet as silhouettes without naming them", async () => {
    render(<Dex />);
    await screen.findByText("発見 3 / 113");
    const unknown = screen.getAllByRole("img", { name: "未発見" });
    expect(unknown).toHaveLength(110);
    expect(unknown[0]).toHaveClass("unknown");
    expect(screen.queryByRole("img", { name: "段階 10・きらきら・うさぎ" })).toBeNull();
  });

  it("names the hidden form once it has been met", async () => {
    api.getDex.mockResolvedValue(dexWith(["10-legend-playful"]));
    render(<Dex />);
    await screen.findByText("発見 1 / 113");
    expect(screen.getByRole("img", { name: "段階 10・でんせつ・きつね" })).toBeInTheDocument();
  });

  it("names every branch and species", async () => {
    api.getDex.mockResolvedValue(dexWith(["3-poor-gluttonous", "3-normal-tidy", "3-good-calm"]));
    render(<Dex />);
    await screen.findByText("発見 3 / 113");
    expect(screen.getByRole("img", { name: "段階 3・しょんぼり・ぽっちゃり" })).toBeInTheDocument();
    expect(screen.getByRole("img", { name: "段階 3・ふつう・うさぎ" })).toBeInTheDocument();
    expect(screen.getByRole("img", { name: "段階 3・きらきら・ふわもち" })).toBeInTheDocument();
  });

  it("shows a hint when nothing but the egg has been found", async () => {
    api.getDex.mockResolvedValue(dexWith(["egg"]));
    render(<Dex />);
    expect(await screen.findByText("発見 1 / 113")).toBeInTheDocument();
  });

  it("congratulates when everything has been collected", async () => {
    const all = dexWith([]);
    api.getDex.mockResolvedValue(dexWith(all.entries.map((e) => e.key)));
    render(<Dex />);
    expect(await screen.findByText("発見 113 / 113")).toBeInTheDocument();
    expect(screen.getByText("コンプリート！")).toBeInTheDocument();
  });

  it("does not congratulate before that", async () => {
    render(<Dex />);
    await screen.findByText("発見 3 / 113");
    expect(screen.queryByText("コンプリート！")).toBeNull();
  });

  it("shows an error when the dex cannot be loaded", async () => {
    api.getDex.mockRejectedValue(new Error("unreadable"));
    render(<Dex />);
    expect(await screen.findByRole("alert")).toHaveTextContent("unreadable");
  });

  it("reloads when the game state changes (a new form may have been found)", async () => {
    render(<Dex />);
    await screen.findByText("発見 3 / 113");
    await waitFor(() => expect(api.onStateChanged).toHaveBeenCalled());
    api.getDex.mockResolvedValue(dexWith(["egg", "2-normal-calm", "3-normal-calm", "5-good-playful"]));
    emit();
    expect(await screen.findByText("発見 4 / 113")).toBeInTheDocument();
  });

  it("shows an error when reloading fails", async () => {
    render(<Dex />);
    await screen.findByText("発見 3 / 113");
    await waitFor(() => expect(api.onStateChanged).toHaveBeenCalled());
    api.getDex.mockRejectedValue("boom");
    emit();
    expect(await screen.findByRole("alert")).toHaveTextContent("boom");
  });

  it("unsubscribes on unmount", async () => {
    const { unmount } = render(<Dex />);
    await screen.findByText("発見 3 / 113");
    await waitFor(() => expect(api.onStateChanged).toHaveBeenCalled());
    unmount();
    await waitFor(() => expect(unlisten).toHaveBeenCalled());
  });
});
