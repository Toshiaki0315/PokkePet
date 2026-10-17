import { useEffect, useState } from "react";

/** 吹き出しを出している長さ（ミリ秒）。そのあと、すっと消える。 */
export const BUBBLE_SHOW_MS = 8000;
/** 消えるまでの、うすくなる時間。 */
export const BUBBLE_FADE_MS = 500;

export type BubblePhase = "show" | "fade" | "gone";

/**
 * 吹き出しは、言葉が変わったとき、または `resetKey` が変わったとき（ポップアップを開いたとき）から、
 * `showMs`（既定は `BUBBLE_SHOW_MS`。設定で変えられる。0 以下なら消さない）だけ出して、うすくなって消える。困りごとが続いていても、ずっと出しっぱなしにはしない
 * （背景・ペットの様子・メニューバーのマークで、状態は伝わる）。
 */
export function useBubblePhase(text: string | null, resetKey: unknown, showMs: number = BUBBLE_SHOW_MS): BubblePhase {
  const [phase, setPhase] = useState<BubblePhase>("show");

  useEffect(() => {
    if (!text) return;
    setPhase("show");
    if (showMs <= 0) return; // ずっと出しておく
    const fade = setTimeout(() => setPhase("fade"), showMs);
    const gone = setTimeout(() => setPhase("gone"), showMs + BUBBLE_FADE_MS);
    return () => {
      clearTimeout(fade);
      clearTimeout(gone);
    };
  }, [text, resetKey, showMs]);

  return phase;
}
