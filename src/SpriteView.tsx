import { useState } from "react";
import type { ReactNode } from "react";
import type { StateView } from "./api";
import { spritePath } from "./sprite";

interface SpriteProps {
  view: StateView;
  /** 演出用の追加クラス（wobble / hop / appear / shake / pop / しぐさ） */
  extraClass?: string;
  /** ゆっくり呼吸するような常時の動き（眠っているときや演出中は付けない）。 */
  breathing?: boolean;
  onAnimationEnd?: () => void;
}

// ドット絵アセットが無い間は絵文字のプレースホルダを表示する。
export function Sprite({ view, extraClass, breathing = false, onAnimationEnd }: SpriteProps) {
  const [broken, setBroken] = useState(false);
  const label = `stage ${view.stage} ${view.branch} ${view.personality}`;
  const mood = view.sleeping ? "sleeping" : view.expression.hunger === "low" ? "hungry" : "";
  const classes = ["sprite", mood, extraClass ?? ""].filter(Boolean).join(" ");
  const wrap = (child: ReactNode) => <span className={breathing ? "sprite-wrap breathing" : "sprite-wrap"}>{child}</span>;
  if (broken) {
    return wrap(
      <div className={`${classes} placeholder`} role="img" aria-label={label} onAnimationEnd={onAnimationEnd}>
        {view.stage <= 1 ? "🥚" : "🐣"}
      </div>,
    );
  }
  return wrap(<img className={classes} src={spritePath(view)} alt={label} onError={() => setBroken(true)} onAnimationEnd={onAnimationEnd} />);
}
