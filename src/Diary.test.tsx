import { render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { DiaryView } from "./api";

const api = vi.hoisted(() => ({
  getDiary: vi.fn(),
  onStateChanged: vi.fn(),
}));
vi.mock("./api", () => api);

import Diary from "./Diary";

/** ローカル時刻 `2026/10/17 07:05` の UNIX 秒（表示もローカル時刻なので環境によらず一致する）。 */
const at = (month: number, day: number, hour: number, minute: number) => new Date(2026, month - 1, day, hour, minute).getTime() / 1000;

const sample: DiaryView = {
  daysTogether: 3,
  // 新しい順
  entries: [
    { atUnix: at(10, 19, 21, 30), event: { kind: "careMilestone", count: 50 } },
    { atUnix: at(10, 18, 9, 0), event: { kind: "evolved", stage: 3, branch: "good", personality: "playful" } },
    { atUnix: at(10, 17, 8, 15), event: { kind: "named", name: "ごま" } },
    { atUnix: at(10, 17, 7, 25), event: { kind: "hatched" } },
    { atUnix: at(10, 17, 7, 5), event: { kind: "born" } },
  ],
};

let emit: () => void;
const unlisten = vi.fn();

beforeEach(() => {
  vi.resetAllMocks();
  api.getDiary.mockResolvedValue(sample);
  api.onStateChanged.mockImplementation((h: () => void) => {
    emit = h;
    return Promise.resolve(unlisten);
  });
});

describe("Diary window", () => {
  it("shows how many days the player has spent together", async () => {
    render(<Diary />);
    expect(await screen.findByText("いっしょに 3 日目")).toBeInTheDocument();
  });

  it("lists the memories newest first", async () => {
    render(<Diary />);
    const items = within(await screen.findByRole("list", { name: "思い出" })).getAllByRole("listitem");
    expect(items).toHaveLength(5);
    expect(items[0]).toHaveTextContent("お世話が 50 回になった");
    expect(items[4]).toHaveTextContent("卵が届いた");
  });

  it("describes every kind of memory", async () => {
    render(<Diary />);
    await screen.findByText("いっしょに 3 日目");
    expect(screen.getByText("卵が届いた")).toBeInTheDocument();
    expect(screen.getByText("卵が孵った！")).toBeInTheDocument();
    expect(screen.getByText("「ごま」と名づけた")).toBeInTheDocument();
    expect(screen.getByText("段階 3 に進化した（きらきら・きつね）")).toBeInTheDocument();
    expect(screen.getByText("お世話が 50 回になった")).toBeInTheDocument();
  });

  it("names the branch and species of every evolution", async () => {
    api.getDiary.mockResolvedValue({
      daysTogether: 1,
      entries: [
        { atUnix: at(10, 17, 12, 0), event: { kind: "evolved", stage: 4, branch: "poor", personality: "gluttonous" } },
        { atUnix: at(10, 17, 11, 0), event: { kind: "evolved", stage: 5, branch: "normal", personality: "tidy" } },
        { atUnix: at(10, 17, 10, 0), event: { kind: "evolved", stage: 6, branch: "normal", personality: "calm" } },
      ],
    });
    render(<Diary />);
    expect(await screen.findByText("段階 4 に進化した（しょんぼり・ぽっちゃり）")).toBeInTheDocument();
    expect(screen.getByText("段階 5 に進化した（ふつう・うさぎ）")).toBeInTheDocument();
    expect(screen.getByText("段階 6 に進化した（ふつう・ふわもち）")).toBeInTheDocument();
  });

  it("shows when each memory happened in local time", async () => {
    render(<Diary />);
    await screen.findByText("いっしょに 3 日目");
    expect(screen.getByText("10/17 07:05")).toBeInTheDocument();
    expect(screen.getByText("10/19 21:30")).toBeInTheDocument();
  });

  it("says so when there are no memories yet", async () => {
    api.getDiary.mockResolvedValue({ daysTogether: 1, entries: [] });
    render(<Diary />);
    expect(await screen.findByText("まだ思い出がありません")).toBeInTheDocument();
  });

  it("shows an error when the diary cannot be loaded", async () => {
    api.getDiary.mockRejectedValue(new Error("unreadable"));
    render(<Diary />);
    expect(await screen.findByRole("alert")).toHaveTextContent("unreadable");
  });

  it("reloads when the game state changes", async () => {
    render(<Diary />);
    await screen.findByText("いっしょに 3 日目");
    await waitFor(() => expect(api.onStateChanged).toHaveBeenCalled());
    api.getDiary.mockResolvedValue({ ...sample, daysTogether: 4 });
    emit();
    expect(await screen.findByText("いっしょに 4 日目")).toBeInTheDocument();
  });

  it("shows an error when reloading fails and unsubscribes on unmount", async () => {
    const { unmount } = render(<Diary />);
    await screen.findByText("いっしょに 3 日目");
    await waitFor(() => expect(api.onStateChanged).toHaveBeenCalled());
    api.getDiary.mockRejectedValue("boom");
    emit();
    expect(await screen.findByRole("alert")).toHaveTextContent("boom");
    unmount();
    await waitFor(() => expect(unlisten).toHaveBeenCalled());
  });
});
