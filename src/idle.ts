import { useEffect, useState } from "react";
import type { StateView } from "./api";
import type { CryKind } from "./sound";

/** しぐさの間隔（最短〜最長）と、1回のしぐさの長さ。 */
export const IDLE_MIN_MS = 2500;
export const IDLE_MAX_MS = 6000;
export const IDLE_GESTURE_MS = 1400;

/** しぐさに合わせて鳴くのは、そのうちの一部だけ（うるさくならないように）。 */
export const IDLE_CRY_CHANCE = 0.35;
/** しぐさで鳴いたあと、次にしぐさで鳴くまでの最短間隔。 */
export const IDLE_CRY_COOLDOWN_MS = 20_000;

/** しぐさに合う鳴き声。 */
export function cryForGesture(gesture: string): CryKind {
  switch (gesture) {
    case "dance":
    case "bounce":
    case "spin":
      return "happy";
    case "sigh":
      return "sad";
    case "rock":
      return "tap";
    default:
      return "neutral";
  }
}

/** 見た目に合わせたしぐさの種類。卵は揺れるだけ。機嫌が良いほど活発に、悪いと控えめに動く。 */
export function gesturesFor({ stage, expression }: Pick<StateView, "stage" | "expression">): readonly string[] {
  if (stage <= 1) return ["rock"];
  if (expression.mood === "high") return ["dance", "bounce", "walk", "spin", "stretch"];
  if (expression.mood === "low") return ["sigh", "look", "nod"];
  return ["bounce", "walk", "look", "stretch", "nod"];
}

/**
 * 待機中のしぐさ。`enabled` の間、ランダムな間隔で `kinds` のどれかを 1 回ずつ返す（null は静止）。
 * 見た目だけの演出で、ゲームの状態には影響しない。
 */
export function useIdleGesture(kinds: readonly string[], enabled: boolean, random: () => number = Math.random): string | null {
  const [gesture, setGesture] = useState<string | null>(null);
  const kindsKey = kinds.join("|");

  useEffect(() => {
    if (!enabled || kinds.length === 0) {
      setGesture(null);
      return;
    }
    let timer: ReturnType<typeof setTimeout>;
    const scheduleNext = () => {
      timer = setTimeout(() => {
        setGesture(kinds[Math.floor(random() * kinds.length)]);
        timer = setTimeout(() => {
          setGesture(null);
          scheduleNext();
        }, IDLE_GESTURE_MS);
      }, IDLE_MIN_MS + random() * (IDLE_MAX_MS - IDLE_MIN_MS));
    };
    scheduleNext();
    return () => {
      clearTimeout(timer);
      setGesture(null);
    };
    // kinds は中身（kindsKey）が変わったときだけ作り直す。random は安定した関数を渡す。
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [enabled, kindsKey]);

  return gesture;
}
