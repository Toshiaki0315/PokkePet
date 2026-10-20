import { useEffect, useState } from "react";
import { getState, onStateChanged, onWalkFrame } from "./api";
import type { StateView, WalkFrame } from "./api";
import { spritePath } from "./sprite";
import { useListener } from "./useListener";
import "./Walker.css";

/** おでかけ中にデスクトップを歩き回る、小さなペット。位置の計算は Rust、ここは絵を出すだけ。 */
export default function Walker() {
  const [view, setView] = useState<StateView | null>(null);
  const [frame, setFrame] = useState<WalkFrame>({ dir: 1, moving: false });

  useEffect(() => {
    getState().then(setView, () => {});
  }, []);
  useListener(onStateChanged, setView);
  useListener(onWalkFrame, setFrame);

  if (!view) return null;
  return (
    <div
      className={frame.moving ? "walker walking" : "walker"}
      style={{ transform: `scaleX(${frame.dir < 0 ? -1 : 1})` }}
    >
      <img
        className="walker-sprite"
        src={spritePath(view)}
        alt="おでかけ中のペット"
      />
    </div>
  );
}
