import { useState } from "react";
import type { StateView } from "./api";
import { spritePath } from "./sprite";

// ドット絵アセットが無い間は絵文字のプレースホルダを表示する。
export function Sprite({ view }: { view: StateView }) {
  const [broken, setBroken] = useState(false);
  const label = `stage ${view.stage} ${view.branch} ${view.personality}`;
  if (broken) {
    return (
      <div className="sprite placeholder" role="img" aria-label={label}>
        {view.stage <= 1 ? "🥚" : "🐣"}
      </div>
    );
  }
  return <img className="sprite" src={spritePath(view)} alt={label} onError={() => setBroken(true)} />;
}
