import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { BUBBLE_FADE_MS, BUBBLE_SHOW_MS, useBubblePhase } from "./bubbleTimeout";

beforeEach(() => {
  vi.useFakeTimers();
});
afterEach(() => {
  vi.useRealTimers();
});

const tick = (ms: number) => act(() => void vi.advanceTimersByTime(ms));

describe("useBubblePhase", () => {
  it("shows the bubble, fades it, then removes it", () => {
    const { result } = renderHook(() => useBubblePhase("おなかすいたよ…", false));
    expect(result.current).toBe("show");
    tick(BUBBLE_SHOW_MS - 1);
    expect(result.current).toBe("show");
    tick(1);
    expect(result.current).toBe("fade");
    tick(BUBBLE_FADE_MS - 1);
    expect(result.current).toBe("fade");
    tick(1);
    expect(result.current).toBe("gone");
  });

  it("starts over when the words change", () => {
    const { result, rerender } = renderHook(({ text }) => useBubblePhase(text, false), { initialProps: { text: "おなかすいたよ…" } });
    tick(BUBBLE_SHOW_MS + BUBBLE_FADE_MS);
    expect(result.current).toBe("gone");
    rerender({ text: "つまんないな…" });
    expect(result.current).toBe("show");
    tick(BUBBLE_SHOW_MS - 1);
    expect(result.current).toBe("show");
  });

  it("starts over when the reset key changes (the popup is opened again)", () => {
    const { result, rerender } = renderHook(({ open }) => useBubblePhase("おなかすいたよ…", open), { initialProps: { open: false } });
    tick(BUBBLE_SHOW_MS + BUBBLE_FADE_MS);
    expect(result.current).toBe("gone");
    rerender({ open: true });
    expect(result.current).toBe("show");
  });

  it("does not restart the timer when nothing changed", () => {
    const { result, rerender } = renderHook(() => useBubblePhase("たのしい！", false));
    tick(BUBBLE_SHOW_MS - 100);
    rerender();
    tick(100);
    expect(result.current).toBe("fade");
  });

  it("has nothing to time when there is no bubble", () => {
    const { result } = renderHook(() => useBubblePhase(null, false));
    tick(BUBBLE_SHOW_MS * 2);
    expect(result.current).toBe("show");
  });

  it("uses the length it is given", () => {
    const { result } = renderHook(() => useBubblePhase("たのしい！", false, 3000));
    tick(2999);
    expect(result.current).toBe("show");
    tick(1);
    expect(result.current).toBe("fade");
    tick(BUBBLE_FADE_MS);
    expect(result.current).toBe("gone");
  });

  it("never removes the bubble when the length is 0", () => {
    const { result } = renderHook(() => useBubblePhase("たのしい！", false, 0));
    tick(10 * 60 * 1000);
    expect(result.current).toBe("show");
    expect(vi.getTimerCount()).toBe(0);
  });

  it("applies a new length straight away", () => {
    const { result, rerender } = renderHook(({ ms }) => useBubblePhase("たのしい！", false, ms), { initialProps: { ms: 3000 } });
    tick(3000 + BUBBLE_FADE_MS);
    expect(result.current).toBe("gone");
    rerender({ ms: 0 });
    expect(result.current).toBe("show");
  });

  it("stops its timers when removed", () => {
    const { unmount } = renderHook(() => useBubblePhase("たのしい！", false));
    unmount();
    expect(vi.getTimerCount()).toBe(0);
  });
});
