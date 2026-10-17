import type { DayStats } from "./api";

/** 日記の材料（きょうの数え方 `DayStats` から、時間を分にしたもの）。 */
export type DiaryCounts = Pick<DayStats, "feeds" | "snacks" | "plays" | "cleans" | "strokes" | "souvenirs"> & { togetherMinutes: number };

/** 「2時間30分」「45分」。 */
export const formatDuration = (minutes: number) => (minutes >= 60 ? `${Math.floor(minutes / 60)}時間${minutes % 60 === 0 ? "" : `${minutes % 60}分`}` : `${minutes}分`);

/** ペットの目線の、その日の絵日記。何もない日は null。 */
export function diaryLine(stats: DiaryCounts): string | null {
  const parts: string[] = [];
  if (stats.togetherMinutes > 0) parts.push(`${formatDuration(stats.togetherMinutes)}、いっしょにいたよ。`);
  if (stats.feeds + stats.snacks > 0) {
    const meals = [stats.feeds > 0 && `ごはんを ${stats.feeds}かい`, stats.snacks > 0 && `おやつを ${stats.snacks}かい`].filter(Boolean).join("、");
    parts.push(`${meals} たべたよ。`);
  }
  if (stats.plays > 0) parts.push(`あそんだのは ${stats.plays}かい。`);
  if (stats.cleans > 0) parts.push(`おそうじは ${stats.cleans}かい。`);
  if (stats.strokes > 0) parts.push(`なでてもらったのは ${stats.strokes}かい。`);
  if (stats.souvenirs > 0) parts.push(`おでかけで おみやげを ${stats.souvenirs}こ もらったよ。`);
  return parts.length === 0 ? null : parts.join(" ");
}
