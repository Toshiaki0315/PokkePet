import { useEffect, useState } from "react";
import { getState, onStateChanged, onWalkFrame, walkerMouse } from "./api";
import type { StateView, WalkFrame } from "./api";
import { spritePath } from "./sprite";
import { useListener } from "./useListener";
import "./Walker.css";

const STILL: WalkFrame = {
  dir: 1,
  moving: false,
  held: false,
  flying: false,
  action: null,
  seq: 0,
};

/** おでかけ中にデスクトップを歩き回る、小さなペット。位置・動きの計算は Rust、ここは絵と、マウス操作を伝えるだけ。 */
export default function Walker() {
  const [view, setView] = useState<StateView | null>(null);
  const [frame, setFrame] = useState<WalkFrame>(STILL);

  useEffect(() => {
    getState().then(setView, () => {});
  }, []);
  useListener(onStateChanged, setView);
  useListener(onWalkFrame, setFrame);

  if (!view) return null;
  const classes = [
    "walker",
    frame.moving && "walking",
    frame.held && "held",
    frame.flying && "flying",
  ]
    .filter(Boolean)
    .join(" ");
  // クリックやつかむ操作は Rust が受け取る（ペットの上にマウスがあるときだけ、このウィンドウがクリックを受ける）。
  return (
    <div
      className={classes}
      style={{ transform: `scaleX(${frame.dir < 0 ? -1 : 1})` }}
      onMouseDown={() => void walkerMouse("down").catch(() => {})}
      onMouseUp={() => void walkerMouse("up").catch(() => {})}
      onDoubleClick={() => void walkerMouse("dblclick").catch(() => {})}
    >
      <div
        key={frame.seq}
        className={frame.action ? `walker-move ${frame.action}` : "walker-move"}
      >
        <img
          className="walker-sprite"
          src={spritePath(view)}
          alt="おでかけ中のペット"
          draggable={false}
        />
        {frame.action === "hop" && <span className="walker-heart">❤</span>}
      </div>
    </div>
  );
}
