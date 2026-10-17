import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { STROKE_COOLDOWN_MS, STROKE_DISTANCE, strokeReaction, usePetting } from "./petting";

function Pad({
  onStroke,
  clock,
}: {
  onStroke: () => void;
  clock: () => number;
}) {
  const handlers = usePetting(onStroke, clock);
  return <div data-testid="pad" {...handlers} />;
}

const setup = () => {
  const onStroke = vi.fn();
  let t = 10_000;
  const clock = () => t;
  render(<Pad onStroke={onStroke} clock={clock} />);
  return {
    onStroke,
    pad: screen.getByTestId("pad"),
    advance: (ms: number) => (t += ms),
  };
};
const move = (el: Element, x: number, y = 0) =>
  fireEvent.mouseMove(el, { clientX: x, clientY: y });

describe("usePetting", () => {
  it("a click strokes the pet", () => {
    const { onStroke, pad } = setup();
    fireEvent.click(pad);
    expect(onStroke).toHaveBeenCalledTimes(1);
  });

  it("clicking again too soon does not stroke twice", () => {
    const { onStroke, pad, advance } = setup();
    fireEvent.click(pad);
    advance(STROKE_COOLDOWN_MS - 1);
    fireEvent.click(pad);
    expect(onStroke).toHaveBeenCalledTimes(1);
    advance(1);
    fireEvent.click(pad);
    expect(onStroke).toHaveBeenCalledTimes(2);
  });

  it("rubbing back and forth over the pet strokes it once enough distance is covered", () => {
    const { onStroke, pad } = setup();
    move(pad, 0); // 最初の位置（距離にはならない）
    move(pad, STROKE_DISTANCE / 2);
    expect(onStroke).not.toHaveBeenCalled();
    move(pad, 0); // 往復も距離に数える
    expect(onStroke).toHaveBeenCalledTimes(1);
  });

  it("counts vertical movement too", () => {
    const { onStroke, pad } = setup();
    move(pad, 0, 0);
    move(pad, 0, STROKE_DISTANCE);
    expect(onStroke).toHaveBeenCalledTimes(1);
  });

  it("forgets the position when the cursor leaves", () => {
    const { onStroke, pad } = setup();
    move(pad, 0);
    fireEvent.mouseLeave(pad);
    move(pad, STROKE_DISTANCE * 5, 0); // 離れていた間の移動は数えない
    expect(onStroke).not.toHaveBeenCalled();
  });

  it("keeps the distance while cooling down but strokes only after the cooldown", () => {
    const { onStroke, pad, advance } = setup();
    fireEvent.click(pad);
    move(pad, 0);
    move(pad, STROKE_DISTANCE);
    expect(onStroke).toHaveBeenCalledTimes(1);
    advance(STROKE_COOLDOWN_MS);
    move(pad, 0);
    expect(onStroke).toHaveBeenCalledTimes(2);
  });
});

describe("strokeReaction", () => {
  it.each([
    ["calm", "nod"],
    ["playful", "dance"],
    ["gluttonous", "flop"],
    ["tidy", "spin"],
  ] as const)("a %s pet reacts with %s", (personality, reaction) => {
    expect(strokeReaction(personality, 5)).toBe(reaction);
  });

  it("an egg just wobbles", () => {
    expect(strokeReaction("playful", 1)).toBe("wobble");
  });
});
