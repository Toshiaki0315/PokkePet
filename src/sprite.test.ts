import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
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

  it("uses an eyes-closed sprite while sleeping, except for the egg", () => {
    const pet = { stage: 5, branch: "good", personality: "playful", sleeping: true } as const;
    expect(spritePath(pet)).toBe("/sprites/5-good-playful-sleep.png");
    expect(spritePath({ ...pet, sleeping: false })).toBe("/sprites/5-good-playful.png");
    expect(spritePath({ ...pet, stage: 1 })).toBe("/sprites/egg.png");
  });

  it("has a sleeping sprite for every stage 2-10 / branch / personality combination (and the hidden form)", () => {
    const missing: string[] = [];
    for (let stage = 2; stage <= 10; stage++)
      for (const branch of stage === 10 ? [...branches, "legend" as Branch] : branches)
        for (const personality of personalities) {
          const path = spritePath({ stage, branch, personality, sleeping: true });
          if (!files.has(path)) missing.push(path);
        }
    expect(missing).toEqual([]);
  });

  it("has the hidden form for every personality at the final stage", () => {
    for (const personality of personalities) expect(files.has(spritePath({ stage: 10, branch: "legend", personality }))).toBe(true);
  });

  it("uses 32x32 PNGs for every sprite", () => {
    const dir = join(process.cwd(), "public", "sprites");
    const names = readdirSync(dir).filter((n) => n.endsWith(".png"));
    expect(names.length).toBe(225); // 113 + ねむっているときの絵 112
    const wrong = names.filter((n) => {
      const png = readFileSync(join(dir, n));
      return png.readUInt32BE(16) !== 32 || png.readUInt32BE(20) !== 32; // IHDR width/height
    });
    expect(wrong).toEqual([]);
  });
});
