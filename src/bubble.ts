import type { BubbleKind, StateView } from "./api";

export const BUBBLE_TEXT: Record<BubbleKind, string> = {
  hungry: "おなかすいたよ…",
  dirty: "おへや、よごれてるよ…",
  gloomy: "つまんないな…",
  happy: "たのしい！",
  sleeping: "Zzz…",
  morning: "おはよう！",
  noon: "おひるだね",
  evening: "こんばんは",
  drowsy: "ねむくなってきた…",
  selfish: "ねえねえ、かまって！",
  sick: "きもちわるい…",
  dying: "たすけて…もう、げんかい…",
  rest: "ちょっと休もうよ",
  welcomeback: "おかえり！",
  batterylow: "でんち、すくないよ…",
  offline: "ネットが、つながらないよ…",
  event: "",
  welcome: "卵が届いたよ！撫でてあげてね",
};

/** 吹き出しに出す言葉。季節のできごとは、その日ごとの言葉（`event.message`）を言う。 */
export function bubbleMessage(view: Pick<StateView, "expression" | "event">): string | null {
  const kind = view.expression.bubble;
  if (!kind) return null;
  return kind === "event" ? (view.event?.message ?? "") : BUBBLE_TEXT[kind];
}
