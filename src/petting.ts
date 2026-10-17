import { useRef } from "react";
import type { MouseEvent } from "react";

/** この距離（px）だけ、なでるように動かすと 1 回なでたことになる。 */
export const STROKE_DISTANCE = 80;
/** なでる反応が続けて起きないようにする間隔。 */
export const STROKE_COOLDOWN_MS = 1200;

/**
 * クリックやなぞる動きを「なでた」として知らせる。
 * 反応の間隔を空けるため、直前から STROKE_COOLDOWN_MS 経つまでは数えない。
 */
export function usePetting(onStroke: () => void, now: () => number = Date.now) {
  const last = useRef<{ x: number; y: number } | null>(null);
  const travelled = useRef(0);
  const lastStroke = useRef<number | null>(null);

  const stroke = () => {
    const t = now();
    if (
      lastStroke.current !== null &&
      t - lastStroke.current < STROKE_COOLDOWN_MS
    )
      return;
    lastStroke.current = t;
    travelled.current = 0;
    onStroke();
  };

  return {
    onClick: stroke,
    onMouseMove: (e: MouseEvent) => {
      if (last.current)
        travelled.current += Math.hypot(
          e.clientX - last.current.x,
          e.clientY - last.current.y,
        );
      last.current = { x: e.clientX, y: e.clientY };
      if (travelled.current >= STROKE_DISTANCE) stroke();
    },
    onMouseLeave: () => {
      last.current = null;
    },
  };
}
