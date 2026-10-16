import { useEffect, useState } from "react";

/** しぐさの間隔（最短〜最長）と、1回のしぐさの長さ。 */
export const IDLE_MIN_MS = 4000;
export const IDLE_MAX_MS = 9000;
export const IDLE_GESTURE_MS = 1400;

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
