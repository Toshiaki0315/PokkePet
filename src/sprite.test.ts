import { describe, expect, it } from "vitest";
import { spritePath } from "./sprite";

describe("spritePath", () => {
  it("maps stage/branch/personality to an asset path", () => {
    expect(spritePath({ stage: 3, branch: "good", personality: "playful" })).toBe("/sprites/3-good-playful.png");
  });

  it("uses the egg sprite for stage 1 regardless of branch and personality", () => {
    expect(spritePath({ stage: 1, branch: "poor", personality: "tidy" })).toBe("/sprites/egg.png");
  });
});
