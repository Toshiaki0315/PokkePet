import { useEffect, useRef, useState } from "react";
import type { StateView } from "./api";
import { Sprite } from "./SpriteView";

export const HATCH_SHAKE_MS = 1400;
export const HATCH_FLASH_MS = 500;

/**
 * 孵化の演出: 卵が揺れる → フラッシュ → 生まれたキャラが飛び出す。
 * 最後に `onDone` を 1 回だけ呼ぶ（呼び出し側が「演出済み」を記録する）。
 */
export function HatchSequence({ view, onDone }: { view: StateView; onDone: () => void }) {
  const [phase, setPhase] = useState<"shake" | "flash" | "born">("shake");
  const done = useRef(onDone);
  useEffect(() => {
    done.current = onDone;
  });

  useEffect(() => {
    const toFlash = setTimeout(() => setPhase("flash"), HATCH_SHAKE_MS);
    const toBorn = setTimeout(() => {
      setPhase("born");
      done.current();
    }, HATCH_SHAKE_MS + HATCH_FLASH_MS);
    return () => {
      clearTimeout(toFlash);
      clearTimeout(toBorn);
    };
  }, []);

  if (phase === "born") return <Sprite view={view} extraClass="pop" />;
  return (
    <>
      <Sprite view={{ ...view, stage: 1 }} extraClass="shake" />
      {phase === "flash" && <div className="flash" data-testid="hatch-flash" />}
    </>
  );
}
