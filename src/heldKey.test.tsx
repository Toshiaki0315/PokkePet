import { act, fireEvent, renderHook } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { useHeldKey } from "./heldKey";

const down = (init: KeyboardEventInit) => act(() => void fireEvent.keyDown(window, init));
const up = (init: KeyboardEventInit) => act(() => void fireEvent.keyUp(window, init));

describe("useHeldKey (cmd+p)", () => {
  it("is false until the shortcut is pressed", () => {
    const { result } = renderHook(() => useHeldKey("p", true));
    expect(result.current).toBe(false);
  });

  it("is true while cmd+p is held and false after p is released", () => {
    const { result } = renderHook(() => useHeldKey("p", true));
    down({ key: "p", metaKey: true });
    expect(result.current).toBe(true);
    up({ key: "p" });
    expect(result.current).toBe(false);
  });

  it("also lets go when cmd is released first (macOS drops the p key-up while cmd is held)", () => {
    const { result } = renderHook(() => useHeldKey("p", true));
    down({ key: "p", metaKey: true });
    up({ key: "Meta" });
    expect(result.current).toBe(false);
  });

  it("ignores p without cmd, other keys, and cmd with other keys", () => {
    const { result } = renderHook(() => useHeldKey("p", true));
    down({ key: "p" });
    expect(result.current).toBe(false);
    down({ key: "o", metaKey: true });
    expect(result.current).toBe(false);
  });

  it("works without cmd when asked to", () => {
    const { result } = renderHook(() => useHeldKey("g"));
    down({ key: "G" });
    expect(result.current).toBe(true);
  });

  it("stops the browser's default action (printing)", () => {
    renderHook(() => useHeldKey("p", true));
    const event = new KeyboardEvent("keydown", { key: "p", metaKey: true, cancelable: true });
    act(() => void window.dispatchEvent(event));
    expect(event.defaultPrevented).toBe(true);
  });

  it("lets go when the window loses focus", () => {
    const { result } = renderHook(() => useHeldKey("p", true));
    down({ key: "p", metaKey: true });
    act(() => void fireEvent.blur(window));
    expect(result.current).toBe(false);
  });

  it("stops listening after unmount", () => {
    const { result, unmount } = renderHook(() => useHeldKey("p", true));
    unmount();
    down({ key: "p", metaKey: true });
    expect(result.current).toBe(false);
  });
});
