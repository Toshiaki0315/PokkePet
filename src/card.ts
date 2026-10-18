export const CARD_WIDTH = 640;
export const CARD_HEIGHT = 360;

/** 成長カードに書くこと。 */
export interface CardInfo {
  /** なまえ（なければ null） */
  name: string | null;
  /** 「きつね・きらきら」のような姿の説明 */
  look: string;
  generation: number;
  daysTogether: number;
  /** 「2時間30分」など（なければ null） */
  together: string | null;
  /** 朝型・夜型（なければ null） */
  type: string | null;
  charm: string | null;
  tintDegrees: number;
  /** 本体の色（設定の「本体の色」。#rrggbb）。カードの背景の色になる */
  bodyColor: string;
  /** 画面に飾っているおみやげ（絵文字） */
  souvenirs: string[];
  /** 集めたおみやげの種類の数と、全部の数 */
  souvenirsFound: number;
  souvenirsTotal: number;
}

const parseHex = (hex: string): [number, number, number] => {
  const m = /^#([0-9a-f]{2})([0-9a-f]{2})([0-9a-f]{2})$/i.exec(hex);
  return m ? [parseInt(m[1], 16), parseInt(m[2], 16), parseInt(m[3], 16)] : [0x4b, 0x5f, 0xb8];
};

/** `from` の色を、`to` の色へ `amount`（0〜1）だけ近づける。#rrggbb で返す。形式が違う色は、既定の青として扱う。 */
export function mixColor(from: string, to: string, amount: number): string {
  const a = parseHex(from);
  const b = parseHex(to);
  const channel = (i: number) =>
    Math.round(a[i] + (b[i] - a[i]) * Math.max(0, Math.min(1, amount)))
      .toString(16)
      .padStart(2, "0");
  return `#${channel(0)}${channel(1)}${channel(2)}`;
}

/** 成長カードを描く。`sprite` は、ペットのドット絵（まだ読み込めていなければ null）。 */
export function drawCard(ctx: CanvasRenderingContext2D, info: CardInfo, sprite: CanvasImageSource | null): void {
  const bg = ctx.createLinearGradient(0, 0, CARD_WIDTH, CARD_HEIGHT);
  bg.addColorStop(0, mixColor(info.bodyColor, "#ffffff", 0.12));
  bg.addColorStop(1, mixColor(info.bodyColor, "#000000", 0.4));
  ctx.fillStyle = bg;
  ctx.fillRect(0, 0, CARD_WIDTH, CARD_HEIGHT);

  // 画面（ペットの枠）
  ctx.fillStyle = "#fbfbfe";
  ctx.fillRect(28, 28, 300, 304);
  if (sprite) {
    ctx.imageSmoothingEnabled = false; // ドットをにじませない
    ctx.filter = info.tintDegrees ? `hue-rotate(${info.tintDegrees}deg)` : "none";
    ctx.drawImage(sprite, 48, 60, 256, 256);
    ctx.filter = "none";
  }

  ctx.fillStyle = "#ffffff";
  ctx.textBaseline = "top";
  ctx.font = "bold 40px sans-serif";
  ctx.fillText(`${info.charm ? `${info.charm} ` : ""}${info.name ?? "なまえなし"}`, 356, 40);
  ctx.font = "20px sans-serif";
  ctx.fillStyle = "#dfe4ff";
  ctx.fillText(info.look, 356, 100);
  ctx.fillText(`${info.generation}代目`, 356, 140);
  ctx.fillText(`いっしょに ${info.daysTogether} 日目`, 356, 180);
  if (info.together) ctx.fillText(`いっしょに作業 ${info.together}${info.type ? `（${info.type}）` : ""}`, 356, 220);
  if (info.souvenirsFound > 0) {
    ctx.font = "18px sans-serif";
    ctx.fillStyle = "#dfe4ff";
    ctx.fillText(`おみやげ ${info.souvenirsFound} / ${info.souvenirsTotal}`, 356, 262);
    ctx.font = "30px sans-serif";
    ctx.fillText(info.souvenirs.join(" "), 356, 288);
  }
  ctx.font = "bold 18px sans-serif";
  ctx.fillStyle = "#ffd88a";
  ctx.fillText("PokkePet", 540, 326);
}

/** 画像を読み込む。 */
export function loadImage(src: string): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const img = new Image();
    img.onload = () => resolve(img);
    img.onerror = () => reject(new Error("画像を読み込めませんでした"));
    img.src = src;
  });
}
