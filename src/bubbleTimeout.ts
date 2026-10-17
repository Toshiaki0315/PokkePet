import { useEffect, useState } from "react";

/** 吹き出しを出している長さ（ミリ秒）。そのあと、すっと消える。 */
export const BUBBLE_SHOW_MS = 8000;
/** 消えるまでの、うすくなる時間。 */
export const BUBBLE_FADE_MS = 500;

export type BubblePhase = "show" | "fade" | "gone";

/**
 * 吹き出しは、言葉が変わったとき、または `resetKey` が変わったとき（ポップアップを開いたとき）から、
 * `BUBBLE_SHOW_MS` だけ出して、うすくなって消える。困りごとが続いていても、ずっと出しっぱなしにはしない
 * （背景・ペットの様子・メニューバーのマークで、状態は伝わる）。
 */
export function useBubblePhase(text: string | null, resetKey: unknown): BubblePhase {
  const [phase, setPhase] = useState<BubblePhase>("show");

  useEffect(() => {
    if (!text) return;
    setPhase("show");
    const fade = setTimeout(() => setPhase("fade"), BUBBLE_SHOW_MS);
    const gone = setTimeout(() => setPhase("gone"), BUBBLE_SHOW_MS + BUBBLE_FADE_MS);
    return () => {
      clearTimeout(fade);
      clearTimeout(gone);
    };
  }, [text, resetKey]);

  return phase;
}
