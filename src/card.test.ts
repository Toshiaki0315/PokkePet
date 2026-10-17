import { afterEach, describe, expect, it, vi } from "vitest";
import { CARD_HEIGHT, CARD_WIDTH, drawCard, loadImage } from "./card";
import type { CardInfo } from "./card";

const info: CardInfo = { name: "ごま", look: "きつね・きらきら", generation: 3, daysTogether: 4, together: "2時間30分", type: "夜型", charm: "🎀", tintDegrees: 130 };

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

describe("drawCard", () => {
  it("paints the whole card and writes the pet's name, look and history", () => {
    const { ctx, raw, texts } = fakeContext();
    drawCard(ctx, info, {} as CanvasImageSource);
    expect(raw.fillRect).toHaveBeenCalledWith(0, 0, CARD_WIDTH, CARD_HEIGHT);
    expect(texts).toEqual(["🎀 ごま", "きつね・きらきら", "3代目", "いっしょに 4 日目", "いっしょに作業 2時間30分（夜型）", "PokkePet"]);
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
    drawCard(ctx, { ...info, name: null, charm: null, tintDegrees: 0, together: null, type: null }, null);
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
