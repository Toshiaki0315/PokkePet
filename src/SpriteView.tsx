import { useState } from "react";
import type { StateView } from "./api";
import { spritePath } from "./sprite";

interface SpriteProps {
  view: StateView;
  /** 演出用の追加クラス（wobble / hop / appear / shake / pop） */
  extraClass?: string;
  onAnimationEnd?: () => void;
}

// ドット絵アセットが無い間は絵文字のプレースホルダを表示する。
export function Sprite({ view, extraClass, onAnimationEnd }: SpriteProps) {
  const [broken, setBroken] = useState(false);
  const label = `stage ${view.stage} ${view.branch} ${view.personality}`;
  const mood = view.sleeping ? "sleeping" : view.expression.hunger === "low" ? "hungry" : "";
  const classes = ["sprite", mood, extraClass ?? ""].filter(Boolean).join(" ");
  if (broken) {
    return (
      <div className={`${classes} placeholder`} role="img" aria-label={label} onAnimationEnd={onAnimationEnd}>
        {view.stage <= 1 ? "🥚" : "🐣"}
      </div>
    );
  }
  return <img className={classes} src={spritePath(view)} alt={label} onError={() => setBroken(true)} onAnimationEnd={onAnimationEnd} />;
}
