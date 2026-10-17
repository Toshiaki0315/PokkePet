import { useEffect, useRef, useState } from "react";
import type { StateView } from "./api";
import type { CryKind } from "./sound";
import { Sprite } from "./SpriteView";

export const HATCH_SHAKE_MS = 1400;
export const HATCH_FLASH_MS = 500;

/**
 * 孵化の演出: 卵が揺れる → フラッシュ → 生まれたキャラが飛び出す。
 * 最後に `onDone` を 1 回だけ呼ぶ（呼び出し側が「演出済み」を記録する）。
 */
export function HatchSequence({ view, onDone, onCry }: { view: StateView; onDone: () => void; onCry?: (kind: CryKind) => void }) {
  const [phase, setPhase] = useState<"shake" | "flash" | "born">("shake");
  const done = useRef(onDone);
  const cry = useRef(onCry);
  useEffect(() => {
    done.current = onDone;
    cry.current = onCry;
  });

  useEffect(() => {
    cry.current?.("tap"); // 卵がコツコツ揺れ始める
    const toFlash = setTimeout(() => {
      setPhase("flash");
      cry.current?.("hatch"); // 生まれた！
    }, HATCH_SHAKE_MS);
    const toBorn = setTimeout(() => {
      setPhase("born");
      done.current();
    }, HATCH_SHAKE_MS + HATCH_FLASH_MS);
    return () => {
      clearTimeout(toFlash);
      clearTimeout(toBorn);
    };
  }, []);

  // 生まれたては、夜でも目を開けている。
  if (phase === "born") return <Sprite view={{ ...view, sleeping: false }} extraClass="pop" />;
  return (
    <>
      <Sprite view={{ ...view, stage: 1 }} extraClass="shake" />
      {phase === "flash" && <div className="flash" data-testid="hatch-flash" />}
    </>
  );
}
