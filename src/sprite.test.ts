import { describe, expect, it } from "vitest";
import { spritePath } from "./sprite";
import type { Branch, Personality } from "./api";

describe("spritePath", () => {
  it("maps stage/branch/personality to an asset path", () => {
    expect(spritePath({ stage: 3, branch: "good", personality: "playful" })).toBe("/sprites/3-good-playful.png");
  });

  it("uses the egg sprite for stage 1 regardless of branch and personality", () => {
    expect(spritePath({ stage: 1, branch: "poor", personality: "tidy" })).toBe("/sprites/egg.png");
  });
});

describe("sprite assets", () => {
  const files = new Set(Object.keys(import.meta.glob("../public/sprites/*.png")).map((p) => p.replace("../public", "")));
  const branches: Branch[] = ["poor", "normal", "good"];
  const personalities: Personality[] = ["calm", "playful", "gluttonous", "tidy"];

  it("has the egg sprite", () => {
    expect(files.has(spritePath({ stage: 1, branch: "normal", personality: "calm" }))).toBe(true);
  });

  it("has a sprite for every stage 2-10 / branch / personality combination", () => {
    const missing: string[] = [];
    for (let stage = 2; stage <= 10; stage++)
      for (const branch of branches)
        for (const personality of personalities) {
          const path = spritePath({ stage, branch, personality });
          if (!files.has(path)) missing.push(path);
        }
    expect(missing).toEqual([]);
  });
});
