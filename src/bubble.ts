import type { BubbleKind } from "./api";

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
  welcome: "卵が届いたよ！撫でてあげてね",
};

/** 名前があれば「ごま「おなかすいたよ…」」の形にする。 */
export function bubbleText(kind: BubbleKind, name: string | null): string {
  const text = BUBBLE_TEXT[kind];
  return name ? `${name}「${text}」` : text;
}
