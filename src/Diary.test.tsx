import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { DayStats, DiaryView } from "./api";

const noToday: DayStats = { feeds: 0, snacks: 0, plays: 0, cleans: 0, strokes: 0, togetherSecs: 0, souvenirs: 0 };

const api = vi.hoisted(() => ({
  getDiary: vi.fn(),
  getState: vi.fn(),
  getSettings: vi.fn(),
  getSouvenirs: vi.fn(),
  saveCard: vi.fn(),
  onStateChanged: vi.fn(),
}));
vi.mock("./api", () => api);

import Diary from "./Diary";

/** ローカル時刻 `2026/10/17 07:05` の UNIX 秒（表示もローカル時刻なので環境によらず一致する）。 */
const at = (month: number, day: number, hour: number, minute: number) => new Date(2026, month - 1, day, hour, minute).getTime() / 1000;

const sample: DiaryView = {
  daysTogether: 3, togetherMinutes: 0, chronotype: null, lineage: [], today: noToday,
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

describe("Diary window (picture diary and card)", () => {
  const page = (today: Partial<DayStats>, entries: DiaryView["entries"] = []): DiaryView => ({ daysTogether: 2, togetherMinutes: 90, chronotype: "night", lineage: [], today: { ...noToday, ...today }, entries });

  it("shows today's page so far, in the pet's voice", async () => {
    api.getDiary.mockResolvedValue(page({ feeds: 2, plays: 1, strokes: 4, togetherSecs: 5_400 }));
    render(<Diary />);
    const section = await screen.findByRole("region", { name: "きょうのページ" });
    expect(section).toHaveTextContent("1時間30分、いっしょにいたよ。 ごはんを 2かい たべたよ。 あそんだのは 1かい。 なでてもらったのは 4かい。");
  });

  it("says nothing has happened yet on a quiet day", async () => {
    api.getDiary.mockResolvedValue(page({}));
    render(<Diary />);
    expect(await screen.findByRole("region", { name: "きょうのページ" })).toHaveTextContent("まだ、なにも ないよ。");
  });

  it("writes finished days into the memories as a picture-diary line", async () => {
    api.getDiary.mockResolvedValue(page({}, [{ atUnix: at(10, 18, 0, 5), event: { kind: "dayLog", month: 10, day: 17, feeds: 3, snacks: 1, plays: 2, cleans: 1, strokes: 0, togetherMinutes: 45, souvenirs: 0 } }]));
    render(<Diary />);
    expect(await screen.findByText("10/17 きょうは… 45分、いっしょにいたよ。 ごはんを 3かい、おやつを 1かい たべたよ。 あそんだのは 2かい。 おそうじは 1かい。")).toBeInTheDocument();
  });

  describe("saving the growth card", () => {
    const pet = { name: "ごま", personality: "playful", branch: "good", stage: 5, generation: 2, charm: "🎀", tintDegrees: 80, displayed: ["🐚", "🍀"] };
    const ctx = { createLinearGradient: () => ({ addColorStop: vi.fn() }), fillRect: vi.fn(), fillText: vi.fn(), drawImage: vi.fn() };
    const originalImage = globalThis.Image;

    beforeEach(() => {
      api.getState.mockResolvedValue(pet);
      api.getSettings.mockResolvedValue({ bodyColor: "#2f9e8f" });
      api.getSouvenirs.mockResolvedValue({ total: 40, found: 9, maxDisplayed: 3, entries: [] });
      api.saveCard.mockResolvedValue("/Users/me/Pictures/PokkePet-ごま-20261017-153045.png");
      vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue(ctx as never);
      vi.spyOn(HTMLCanvasElement.prototype, "toDataURL").mockReturnValue("data:image/png;base64,AAAA");
      class OkImage {
        onload: (() => void) | null = null;
        set src(_: string) {
          queueMicrotask(() => this.onload?.());
        }
      }
      globalThis.Image = OkImage as unknown as typeof Image;
      api.getDiary.mockResolvedValue(page({}));
    });
    afterEach(() => {
      vi.restoreAllMocks();
      globalThis.Image = originalImage;
    });

    it("draws the card for the current pet, saves it and says where", async () => {
      render(<Diary />);
      await userEvent.click(await screen.findByRole("button", { name: "成長カードを保存" }));
      expect(await screen.findByText(/ピクチャフォルダに保存しました: \/Users\/me\/Pictures\/PokkePet-ごま-20261017-153045.png/)).toBeInTheDocument();
      expect(api.saveCard).toHaveBeenCalledWith("data:image/png;base64,AAAA");
      expect(ctx.fillText).toHaveBeenCalledWith("🎀 ごま", expect.any(Number), expect.any(Number));
      expect(ctx.fillText).toHaveBeenCalledWith("きつね・きらきら", expect.any(Number), expect.any(Number));
      expect(ctx.fillText).toHaveBeenCalledWith("いっしょに作業 1時間30分（夜型）", expect.any(Number), expect.any(Number));
      expect(ctx.fillText).toHaveBeenCalledWith("おみやげ 9 / 40", expect.any(Number), expect.any(Number));
      expect(ctx.fillText).toHaveBeenCalledWith("🐚 🍀", expect.any(Number), expect.any(Number));
    });

    it("leaves out the working time and type when there is none", async () => {
      api.getDiary.mockResolvedValue({ ...page({}), togetherMinutes: 0, chronotype: null });
      render(<Diary />);
      await userEvent.click(await screen.findByRole("button", { name: "成長カードを保存" }));
      await screen.findByText(/ピクチャフォルダに保存しました/);
      expect(ctx.fillText).not.toHaveBeenCalledWith(expect.stringContaining("いっしょに作業"), expect.any(Number), expect.any(Number));
    });

    it("shows the error when the card cannot be saved", async () => {
      api.saveCard.mockRejectedValue(new Error("disk full"));
      render(<Diary />);
      await userEvent.click(await screen.findByRole("button", { name: "成長カードを保存" }));
      expect(await screen.findByRole("alert")).toHaveTextContent("disk full");
      expect(screen.queryByText(/ピクチャフォルダに保存しました/)).toBeNull();
    });

    it("explains when no drawing surface is available", async () => {
      vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue(null);
      render(<Diary />);
      await userEvent.click(await screen.findByRole("button", { name: "成長カードを保存" }));
      expect(await screen.findByRole("alert")).toHaveTextContent("画像をつくれませんでした");
    });
  });
});

describe("Diary window (family tree)", () => {
  const tree: DiaryView = {
    daysTogether: 5,
    togetherMinutes: 0,
    chronotype: null,
    today: noToday,
    entries: [{ atUnix: at(10, 18, 7, 5), event: { kind: "eggReceived", name: "ごま", personality: "calm" } }, { atUnix: at(10, 17, 7, 5), event: { kind: "eggReceived", name: null, personality: "tidy" } }],
    lineage: [
      { generation: 1, name: "ごま", personality: "playful", branch: "good", tintDegrees: 0, charm: null, fromFriend: false },
      { generation: 2, name: null, personality: "tidy", branch: "normal", tintDegrees: 130, charm: "🎀", fromFriend: true },
    ],
  };

  it("lists the ancestors oldest first with their species, charm and colour, and marks a friend's blood", async () => {
    api.getDiary.mockResolvedValue(tree);
    render(<Diary />);
    const section = await screen.findByRole("region", { name: "家系図" });
    const items = within(section).getAllByRole("listitem");
    expect(items).toHaveLength(2);
    expect(items[0]).toHaveTextContent("1代目 ごま（きつね・きらきら）");
    expect(items[1]).toHaveTextContent("2代目 なまえなし（うさぎ・ふつう）🎀");
    expect(items[1]).toHaveTextContent("友だちのたまごの血筋");
    expect(items[1].style.filter).toBe("hue-rotate(130deg)");
    expect(items[0].style.filter).toBe("");
  });

  it("has no family tree section for the first generation", async () => {
    api.getDiary.mockResolvedValue({ ...tree, lineage: [] });
    render(<Diary />);
    await screen.findByText("いっしょに 5 日目");
    expect(screen.queryByRole("region", { name: "家系図" })).toBeNull();
  });

  it("describes receiving a friend's egg, with and without a name", async () => {
    api.getDiary.mockResolvedValue(tree);
    render(<Diary />);
    expect(await screen.findByText("「ごま」とお別れして、友だちのたまごを迎えた")).toBeInTheDocument();
    expect(screen.getByText("友だちのたまごを迎えた")).toBeInTheDocument();
  });
});

describe("Diary window (souvenir)", () => {
  it("describes a rare find", async () => {
    api.getDiary.mockResolvedValue({ daysTogether: 1, togetherMinutes: 0, chronotype: null, lineage: [], today: noToday, entries: [{ atUnix: at(10, 18, 7, 5), event: { kind: "souvenir", name: "ほうせき", emoji: "💎" } }] });
    render(<Diary />);
    expect(await screen.findByText("おでかけで 💎ほうせき を見つけた！")).toBeInTheDocument();
  });
});

describe("Diary window (together)", () => {
  const withTime = (togetherMinutes: number, chronotype: DiaryView["chronotype"]): DiaryView => ({ daysTogether: 2, togetherMinutes, chronotype, lineage: [], today: noToday, entries: [] });

  it("shows the working time together, in hours and minutes, with the learned type", async () => {
    api.getDiary.mockResolvedValue(withTime(150, "night"));
    render(<Diary />);
    expect(await screen.findByText("いっしょに作業した時間 2時間30分（夜型）")).toBeInTheDocument();
  });

  it("formats whole hours and short times", async () => {
    api.getDiary.mockResolvedValue(withTime(120, null));
    const { unmount } = render(<Diary />);
    expect(await screen.findByText("いっしょに作業した時間 2時間")).toBeInTheDocument();
    unmount();
    api.getDiary.mockResolvedValue(withTime(45, "morning"));
    render(<Diary />);
    expect(await screen.findByText("いっしょに作業した時間 45分（朝型）")).toBeInTheDocument();
  });

  it("says nothing about working together before any time has been counted", async () => {
    api.getDiary.mockResolvedValue(withTime(0, null));
    render(<Diary />);
    await screen.findByText("いっしょに 2 日目");
    expect(screen.queryByText(/いっしょに作業した時間/)).toBeNull();
  });
});

describe("Diary window (restart)", () => {
  it("describes starting over, with and without a name", async () => {
    api.getDiary.mockResolvedValue({
      daysTogether: 1, togetherMinutes: 0, chronotype: null, lineage: [], today: noToday,
      entries: [
        { atUnix: at(10, 18, 7, 5), event: { kind: "restarted", name: "ごま", personality: "calm" } },
        { atUnix: at(10, 17, 7, 5), event: { kind: "restarted", name: null, personality: "tidy" } },
      ],
    });
    render(<Diary />);
    expect(await screen.findByText("「ごま」とお別れして、卵からやり直した")).toBeInTheDocument();
    expect(screen.getByText("卵からやり直した")).toBeInTheDocument();
  });
});

describe("Diary window (death)", () => {
  it("describes a death, with and without a name", async () => {
    api.getDiary.mockResolvedValue({
      daysTogether: 3, togetherMinutes: 0, chronotype: null, lineage: [], today: noToday,
      entries: [
        { atUnix: at(10, 18, 7, 5), event: { kind: "died", name: "ごま", personality: "calm" } },
        { atUnix: at(10, 17, 7, 5), event: { kind: "died", name: null, personality: "tidy" } },
      ],
    });
    render(<Diary />);
    expect(await screen.findByText("「ごま」が天国へ旅立った（ふわもち）")).toBeInTheDocument();
    expect(screen.getByText("天国へ旅立った（うさぎ）")).toBeInTheDocument();
  });
});

describe("Diary window (anniversaries)", () => {
  it("describes an anniversary", async () => {
    api.getDiary.mockResolvedValue({ daysTogether: 2, togetherMinutes: 0, chronotype: null, lineage: [], today: noToday, entries: [{ atUnix: at(10, 18, 7, 5), event: { kind: "anniversary", days: 1 } }] });
    render(<Diary />);
    expect(await screen.findByText("いっしょに 1 日が経った")).toBeInTheDocument();
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

  it("remembers farewells, with or without a name", async () => {
    api.getDiary.mockResolvedValue({
      daysTogether: 1, togetherMinutes: 0, chronotype: null, lineage: [], today: noToday,
      entries: [
        { atUnix: at(10, 17, 12, 0), event: { kind: "graduated", name: "ごま", personality: "playful" } },
        { atUnix: at(10, 17, 11, 0), event: { kind: "graduated", name: null, personality: "calm" } },
      ],
    });
    render(<Diary />);
    expect(await screen.findByText("「ごま」が旅立った（きつね）")).toBeInTheDocument();
    expect(screen.getByText("旅立ちを見送った（ふわもち）")).toBeInTheDocument();
  });

  it("names the branch and species of every evolution", async () => {
    api.getDiary.mockResolvedValue({
      daysTogether: 1, togetherMinutes: 0, chronotype: null, lineage: [], today: noToday,
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
    api.getDiary.mockResolvedValue({ daysTogether: 1, togetherMinutes: 0, chronotype: null, lineage: [], today: noToday, entries: [] });
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
