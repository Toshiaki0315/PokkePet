import { describe, expect, it } from "vitest";
import { diaryLine, formatDuration } from "./pictureDiary";

const none = { feeds: 0, snacks: 0, plays: 0, cleans: 0, strokes: 0, togetherMinutes: 0, souvenirs: 0 };

describe("formatDuration", () => {
  it("writes hours and minutes, whole hours, and short times", () => {
    expect(formatDuration(150)).toBe("2時間30分");
    expect(formatDuration(120)).toBe("2時間");
    expect(formatDuration(45)).toBe("45分");
  });
});

describe("diaryLine", () => {
  it("has nothing to say about an empty day", () => {
    expect(diaryLine(none)).toBeNull();
  });

  it("tells the day in the pet's voice, part by part", () => {
    expect(diaryLine({ ...none, togetherMinutes: 150 })).toBe("2時間30分、いっしょにいたよ。");
    expect(diaryLine({ ...none, feeds: 3 })).toBe("ごはんを 3かい たべたよ。");
    expect(diaryLine({ ...none, snacks: 2 })).toBe("おやつを 2かい たべたよ。");
    expect(diaryLine({ ...none, feeds: 3, snacks: 2 })).toBe("ごはんを 3かい、おやつを 2かい たべたよ。");
    expect(diaryLine({ ...none, plays: 4 })).toBe("あそんだのは 4かい。");
    expect(diaryLine({ ...none, cleans: 1 })).toBe("おそうじは 1かい。");
    expect(diaryLine({ ...none, strokes: 7 })).toBe("なでてもらったのは 7かい。");
    expect(diaryLine({ ...none, souvenirs: 2 })).toBe("おでかけで おみやげを 2こ もらったよ。");
  });

  it("joins everything that happened", () => {
    expect(diaryLine({ feeds: 1, snacks: 0, plays: 2, cleans: 1, strokes: 3, togetherMinutes: 60, souvenirs: 1 })).toBe(
      "1時間、いっしょにいたよ。 ごはんを 1かい たべたよ。 あそんだのは 2かい。 おそうじは 1かい。 なでてもらったのは 3かい。 おでかけで おみやげを 1こ もらったよ。",
    );
  });
});
