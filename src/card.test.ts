import { afterEach, describe, expect, it, vi } from "vitest";
import { CARD_HEIGHT, CARD_WIDTH, drawCard, loadImage, mixColor } from "./card";
import type { CardInfo } from "./card";

const info: CardInfo = { name: "ごま", look: "きつね・きらきら", generation: 3, daysTogether: 4, together: "2時間30分", type: "夜型", charm: "🎀", tintDegrees: 130, bodyColor: "#4b5fb8", souvenirs: ["🐚", "🍀"], souvenirsFound: 7, souvenirsTotal: 40 };

function fakeContext() {
  const texts: string[] = [];
  const ctx = {
    fillStyle: "",
    font: "",
    textBaseline: "",
    filter: "",
    imageSmoothingEnabled: true,
    createLinearGradient: vi.fn(() => ({ addColorStop: vi.fn() })),
    fillRect: vi.fn(),
    fillText: vi.fn((text: string) => void texts.push(text)),
    drawImage: vi.fn(),
  };
  return { ctx: ctx as unknown as CanvasRenderingContext2D, raw: ctx, texts };
}

describe("mixColor", () => {
  it("moves a colour towards another one", () => {
    expect(mixColor("#000000", "#ffffff", 0.5)).toBe("#808080");
    expect(mixColor("#4b5fb8", "#000000", 0)).toBe("#4b5fb8");
    expect(mixColor("#4b5fb8", "#000000", 1)).toBe("#000000");
  });

  it("keeps the amount within 0 to 1", () => {
    expect(mixColor("#102030", "#ffffff", 5)).toBe("#ffffff");
    expect(mixColor("#102030", "#ffffff", -5)).toBe("#102030");
  });

  it("treats a malformed colour as the default blue", () => {
    expect(mixColor("red", "#4b5fb8", 1)).toBe("#4b5fb8");
    expect(mixColor("#4b5fb8", "nonsense", 1)).toBe("#4b5fb8");
  });
});

describe("drawCard", () => {
  it("paints the background in shades of the body colour", () => {
    const stops: [number, string][] = [];
    const { ctx, raw } = fakeContext();
    raw.createLinearGradient.mockReturnValue({ addColorStop: vi.fn((offset: number, color: string) => void stops.push([offset, color])) });
    drawCard(ctx, { ...info, bodyColor: "#2f9e8f" }, null);
    expect(stops).toEqual([
      [0, mixColor("#2f9e8f", "#ffffff", 0.12)],
      [1, mixColor("#2f9e8f", "#000000", 0.4)],
    ]);
  });

  it("shows how many souvenirs were collected and the displayed ones, but nothing before the first", () => {
    const withSouvenirs = fakeContext();
    drawCard(withSouvenirs.ctx, info, null);
    expect(withSouvenirs.texts).toContain("おみやげ 7 / 40");
    const none = fakeContext();
    drawCard(none.ctx, { ...info, souvenirsFound: 0, souvenirs: [] }, null);
    expect(none.texts.some((t) => t.startsWith("おみやげ"))).toBe(false);
  });

  it("paints the whole card and writes the pet's name, look and history", () => {
    const { ctx, raw, texts } = fakeContext();
    drawCard(ctx, info, {} as CanvasImageSource);
    expect(raw.fillRect).toHaveBeenCalledWith(0, 0, CARD_WIDTH, CARD_HEIGHT);
    expect(texts).toEqual(["🎀 ごま", "きつね・きらきら", "3代目", "いっしょに 4 日目", "いっしょに作業 2時間30分（夜型）", "おみやげ 7 / 40", "🐚 🍀", "PokkePet"]);
  });

  it("draws the sprite crisp (no smoothing) and tinted with the inherited hue, then resets the filter", () => {
    const { ctx, raw } = fakeContext();
    const filters: string[] = [];
    Object.defineProperty(raw, "filter", { get: () => filters[filters.length - 1] ?? "", set: (v: string) => void filters.push(v) });
    drawCard(ctx, info, {} as CanvasImageSource);
    expect(raw.imageSmoothingEnabled).toBe(false);
    expect(raw.drawImage).toHaveBeenCalledTimes(1);
    expect(filters).toEqual(["hue-rotate(130deg)", "none"]);
  });

  it("copes with no sprite, no name, no charm, no tint and no working time", () => {
    const { ctx, raw, texts } = fakeContext();
    drawCard(ctx, { ...info, name: null, charm: null, tintDegrees: 0, together: null, type: null, souvenirsFound: 0, souvenirs: [] }, null);
    expect(raw.drawImage).not.toHaveBeenCalled();
    expect(texts).toEqual(["なまえなし", "きつね・きらきら", "3代目", "いっしょに 4 日目", "PokkePet"]);
  });

  it("leaves out the type when only the time is known, and uses no filter without a tint", () => {
    const { ctx, raw, texts } = fakeContext();
    drawCard(ctx, { ...info, type: null, tintDegrees: 0 }, {} as CanvasImageSource);
    expect(texts).toContain("いっしょに作業 2時間30分");
    expect(raw.filter).toBe("none");
  });
});

describe("loadImage", () => {
  const original = globalThis.Image;
  afterEach(() => {
    globalThis.Image = original;
  });

  it("resolves with the image once it has loaded", async () => {
    class OkImage {
      onload: (() => void) | null = null;
      onerror: (() => void) | null = null;
      set src(_: string) {
        queueMicrotask(() => this.onload?.());
      }
    }
    globalThis.Image = OkImage as unknown as typeof Image;
    await expect(loadImage("/sprites/egg.png")).resolves.toBeInstanceOf(OkImage);
  });

  it("rejects when the image cannot be loaded", async () => {
    class BadImage {
      onload: (() => void) | null = null;
      onerror: (() => void) | null = null;
      set src(_: string) {
        queueMicrotask(() => this.onerror?.());
      }
    }
    globalThis.Image = BadImage as unknown as typeof Image;
    await expect(loadImage("/nope.png")).rejects.toThrow("画像を読み込めませんでした");
  });
});
