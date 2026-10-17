import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { StateView } from "./api";
import { cryForGesture, gesturesFor, IDLE_CRY_CHANCE, IDLE_CRY_COOLDOWN_MS, IDLE_GESTURE_MS, IDLE_MAX_MS, IDLE_MIN_MS, useIdleGesture } from "./idle";

const KINDS = ["bounce", "walk", "look"] as const;

beforeEach(() => {
  vi.useFakeTimers();
});
afterEach(() => {
  vi.useRealTimers();
});

const tick = (ms: number) => act(() => vi.advanceTimersByTime(ms));

describe("useIdleGesture", () => {
  it("starts with no gesture", () => {
    const { result } = renderHook(() => useIdleGesture(KINDS, true, () => 0));
    expect(result.current).toBeNull();
  });

  it("performs a gesture after the shortest delay, then settles down", () => {
    const { result } = renderHook(() => useIdleGesture(KINDS, true, () => 0));
    tick(IDLE_MIN_MS - 1);
    expect(result.current).toBeNull();
    tick(1);
    expect(result.current).toBe("bounce");
    tick(IDLE_GESTURE_MS - 1);
    expect(result.current).toBe("bounce");
    tick(1);
    expect(result.current).toBeNull();
  });

  it("waits longer when the random number is large", () => {
    const { result } = renderHook(() => useIdleGesture(KINDS, true, () => 0.999));
    tick(IDLE_MAX_MS - 20);
    expect(result.current).toBeNull();
    tick(30);
    expect(result.current).toBe("look"); // 末尾の種類
  });

  it("picks the gesture from the random number", () => {
    const values = [0, 0.5, 0.99, 0, 0.5, 0.99];
    let i = 0;
    const { result } = renderHook(() => useIdleGesture(KINDS, true, () => values[i++ % values.length]));
    const seen: Array<string | null> = [];
    for (let n = 0; n < 3; n++) {
      while (result.current === null) tick(100); // 次のしぐさが始まるまで進める
      seen.push(result.current);
      while (result.current !== null) tick(100); // しぐさが終わるまで進める
    }
    expect(seen.every((g) => KINDS.includes(g as (typeof KINDS)[number]))).toBe(true);
    expect(new Set(seen).size).toBeGreaterThan(1);
  });

  it("keeps going with the next gesture after a pause", () => {
    const { result } = renderHook(() => useIdleGesture(KINDS, true, () => 0));
    tick(IDLE_MIN_MS + IDLE_GESTURE_MS);
    expect(result.current).toBeNull();
    tick(IDLE_MIN_MS);
    expect(result.current).toBe("bounce");
  });

  it("does nothing while disabled", () => {
    const { result } = renderHook(() => useIdleGesture(KINDS, false, () => 0));
    tick(IDLE_MAX_MS * 3);
    expect(result.current).toBeNull();
    expect(vi.getTimerCount()).toBe(0);
  });

  it("stops at once when it becomes disabled in the middle of a gesture", () => {
    const { result, rerender } = renderHook(({ on }) => useIdleGesture(KINDS, on, () => 0), { initialProps: { on: true } });
    tick(IDLE_MIN_MS);
    expect(result.current).toBe("bounce");
    rerender({ on: false });
    expect(result.current).toBeNull();
    tick(IDLE_MAX_MS * 3);
    expect(result.current).toBeNull();
  });

  it("starts again when it is enabled later", () => {
    const { result, rerender } = renderHook(({ on }) => useIdleGesture(KINDS, on, () => 0), { initialProps: { on: false } });
    tick(IDLE_MAX_MS);
    rerender({ on: true });
    tick(IDLE_MIN_MS);
    expect(result.current).toBe("bounce");
  });

  it("has no gestures to perform without kinds", () => {
    const { result } = renderHook(() => useIdleGesture([], true, () => 0));
    tick(IDLE_MAX_MS * 3);
    expect(result.current).toBeNull();
    expect(vi.getTimerCount()).toBe(0);
  });

  it("switches to the new set of gestures when the kinds change", () => {
    const { result, rerender } = renderHook(({ kinds }) => useIdleGesture(kinds, true, () => 0), { initialProps: { kinds: ["rock"] as readonly string[] } });
    tick(IDLE_MIN_MS);
    expect(result.current).toBe("rock");
    rerender({ kinds: ["bounce", "walk"] });
    tick(IDLE_MIN_MS);
    expect(result.current).toBe("bounce");
  });

  it("leaves no timers behind after unmount", () => {
    const { unmount } = renderHook(() => useIdleGesture(KINDS, true, () => 0));
    tick(IDLE_MIN_MS);
    unmount();
    expect(vi.getTimerCount()).toBe(0);
  });
});

type Look = Pick<StateView, "stage" | "expression">;
const look = (stage: number, mood: "low" | "mid" | "high"): Look => ({
  stage,
  expression: { mood, hunger: "mid", dirtiness: "low", bubble: null },
});

describe("gesturesFor", () => {
  it("lets an egg only rock, whatever its mood", () => {
    for (const mood of ["low", "mid", "high"] as const) expect(gesturesFor(look(1, mood))).toEqual(["rock"]);
  });

  it("gives a cheerful pet the liveliest moves", () => {
    const kinds = gesturesFor(look(5, "high"));
    expect(kinds).toEqual(expect.arrayContaining(["dance", "bounce", "walk", "spin"]));
    expect(kinds).not.toContain("sigh");
  });

  it("gives an ordinary pet a calm mix", () => {
    const kinds = gesturesFor(look(5, "mid"));
    expect(kinds).toEqual(expect.arrayContaining(["bounce", "walk", "look", "stretch", "nod"]));
    expect(kinds).not.toContain("dance");
  });

  it("keeps a gloomy pet subdued", () => {
    const kinds = gesturesFor(look(5, "low"));
    expect(kinds).toEqual(expect.arrayContaining(["sigh", "look"]));
    expect(kinds).not.toContain("dance");
    expect(kinds).not.toContain("bounce");
  });

  it("never lists a move twice and always has something to do", () => {
    for (const stage of [1, 2, 10]) {
      for (const mood of ["low", "mid", "high"] as const) {
        const kinds = gesturesFor(look(stage, mood));
        expect(kinds.length).toBeGreaterThan(0);
        expect(new Set(kinds).size).toBe(kinds.length);
      }
    }
  });

  it("moves more often than before", () => {
    expect(IDLE_MIN_MS).toBeLessThanOrEqual(3000);
    expect(IDLE_MAX_MS).toBeLessThanOrEqual(6500);
  });
});

describe("cryForGesture", () => {
  it("cheers while dancing, bouncing and spinning", () => {
    for (const g of ["dance", "bounce", "spin"]) expect(cryForGesture(g)).toBe("happy");
  });

  it("sighs sadly when sighing", () => {
    expect(cryForGesture("sigh")).toBe("sad");
  });

  it("only taps when an egg rocks", () => {
    expect(cryForGesture("rock")).toBe("tap");
  });

  it("makes an ordinary cry for the calm moves", () => {
    for (const g of ["walk", "look", "stretch", "nod"]) expect(cryForGesture(g)).toBe("neutral");
  });

  it("does not cry all the time: only now and then, and not again right away", () => {
    expect(IDLE_CRY_CHANCE).toBeGreaterThan(0);
    expect(IDLE_CRY_CHANCE).toBeLessThanOrEqual(0.5);
    expect(IDLE_CRY_COOLDOWN_MS).toBeGreaterThanOrEqual(15_000);
  });
});
