import { act, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const api = vi.hoisted(() => ({
  getState: vi.fn(),
  onStateChanged: vi.fn(),
  onWalkFrame: vi.fn(),
}));
vi.mock("./api", () => api);

import { spritePath } from "./sprite";
import Walker from "./Walker";

const view = { stage: 3, branch: "normal", personality: "calm" };
let emitFrame: (f: { dir: number; moving: boolean }) => void;

beforeEach(() => {
  vi.resetAllMocks();
  api.getState.mockResolvedValue(view);
  api.onStateChanged.mockResolvedValue(() => {});
  api.onWalkFrame.mockImplementation((h: typeof emitFrame) => {
    emitFrame = h;
    return Promise.resolve(() => {});
  });
});

describe("Walker", () => {
  it("shows nothing until the state arrives, then the pet", async () => {
    const { container } = render(<Walker />);
    expect(container).toBeEmptyDOMElement();
    expect(await screen.findByAltText("おでかけ中のペット")).toHaveAttribute(
      "src",
      spritePath(view as never),
    );
  });

  it("walks and turns to face where it goes", async () => {
    const { container } = render(<Walker />);
    await screen.findByAltText("おでかけ中のペット");
    const walker = container.querySelector(".walker") as HTMLElement;
    expect(walker).not.toHaveClass("walking");
    expect(walker.style.transform).toBe("scaleX(1)");
    act(() => emitFrame({ dir: -1, moving: true }));
    expect(walker).toHaveClass("walking");
    expect(walker.style.transform).toBe("scaleX(-1)");
    act(() => emitFrame({ dir: 1, moving: false }));
    expect(walker).not.toHaveClass("walking");
  });

  it("copes with the state not being readable", async () => {
    api.getState.mockRejectedValue(new Error("x"));
    const { container } = render(<Walker />);
    await act(async () => {});
    expect(container).toBeEmptyDOMElement();
  });
});
