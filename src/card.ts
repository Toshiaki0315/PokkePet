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
}

/** 成長カードを描く。`sprite` は、ペットのドット絵（まだ読み込めていなければ null）。 */
export function drawCard(ctx: CanvasRenderingContext2D, info: CardInfo, sprite: CanvasImageSource | null): void {
  const bg = ctx.createLinearGradient(0, 0, CARD_WIDTH, CARD_HEIGHT);
  bg.addColorStop(0, "#4b5fb8");
  bg.addColorStop(1, "#2a3270");
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
  ctx.font = "bold 22px sans-serif";
  ctx.fillStyle = "#ffd88a";
  ctx.fillText("PokkePet", 356, 292);
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
