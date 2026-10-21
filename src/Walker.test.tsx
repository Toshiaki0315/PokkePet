import { act, fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const api = vi.hoisted(() => ({
  getState: vi.fn(),
  onStateChanged: vi.fn(),
  onWalkFrame: vi.fn(),
  walkerMouse: vi.fn(),
}));
vi.mock("./api", () => api);

import { spritePath } from "./sprite";
import Walker from "./Walker";

const view = { stage: 3, branch: "normal", personality: "calm" };
const still = {
  dir: 1,
  moving: false,
  held: false,
  flying: false,
  action: null,
  seq: 0,
};
let emitFrame: (f: typeof still | Record<string, unknown>) => void;

beforeEach(() => {
  vi.resetAllMocks();
  api.getState.mockResolvedValue(view);
  api.onStateChanged.mockResolvedValue(() => {});
  api.walkerMouse.mockResolvedValue(undefined);
  api.onWalkFrame.mockImplementation((h: typeof emitFrame) => {
    emitFrame = h;
    return Promise.resolve(() => {});
  });
});

const shown = async () => {
  const utils = render(<Walker />);
  const img = await screen.findByAltText("おでかけ中のペット");
  return {
    ...utils,
    img,
    walker: utils.container.querySelector(".walker") as HTMLElement,
  };
};

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
    const { walker } = await shown();
    const flip = walker.querySelector(".walker-flip") as HTMLElement;
    expect(walker).not.toHaveClass("walking");
    expect(flip.style.transform).toBe("scaleX(1)");
    act(() => emitFrame({ ...still, dir: -1, moving: true }));
    expect(walker).toHaveClass("walking");
    expect(flip.style.transform).toBe("scaleX(-1)");
    act(() => emitFrame(still));
    expect(walker).not.toHaveClass("walking");
  });

  it("shows being held, flying and each gesture (a hop also shows a heart)", async () => {
    const { walker, container } = await shown();
    act(() => emitFrame({ ...still, held: true }));
    expect(walker).toHaveClass("held");
    act(() => emitFrame({ ...still, flying: true }));
    expect(walker).toHaveClass("flying");
    expect(walker).not.toHaveClass("held");
    act(() => emitFrame({ ...still, action: "hop", seq: 1 }));
    expect(container.querySelector(".walker-move")).toHaveClass("hop");
    expect(container.querySelector(".walker-heart")).not.toBeNull();
    act(() => emitFrame({ ...still, action: "spin", seq: 2 }));
    expect(container.querySelector(".walker-move")).toHaveClass("spin");
    expect(container.querySelector(".walker-heart")).toBeNull();
    act(() => emitFrame(still));
    expect(container.querySelector(".walker-move")).not.toHaveClass("spin");
  });

  it("shows what the pet says in a bubble that is not mirrored, and removes it afterwards", async () => {
    const { walker } = await shown();
    expect(walker.querySelector(".bubble")).toBeNull();
    act(() =>
      emitFrame({ ...still, dir: -1, say: "いいてんき！", say_seq: 1 }),
    );
    const bubble = walker.querySelector(".bubble") as HTMLElement;
    expect(bubble).toHaveTextContent("いいてんき！");
    expect(bubble.closest(".walker-flip")).toBeNull();
    act(() => emitFrame({ ...still, say: null, say_seq: 1 }));
    expect(walker.querySelector(".bubble")).toBeNull();
  });

  it("tells Rust about presses, releases and double clicks (and ignores a failure)", async () => {
    const { walker } = await shown();
    fireEvent.mouseDown(walker);
    fireEvent.mouseUp(walker);
    fireEvent.doubleClick(walker);
    expect(api.walkerMouse.mock.calls).toEqual([
      ["down"],
      ["up"],
      ["dblclick"],
    ]);
    api.walkerMouse.mockRejectedValue(new Error("x"));
    fireEvent.mouseDown(walker);
    await act(async () => {});
    expect(screen.getByAltText("おでかけ中のペット")).toBeInTheDocument();
  });

  it("copes with the state not being readable", async () => {
    api.getState.mockRejectedValue(new Error("x"));
    const { container } = render(<Walker />);
    await act(async () => {});
    expect(container).toBeEmptyDOMElement();
  });
});
