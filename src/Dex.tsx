import { useCallback, useEffect, useState } from "react";
import { getDex, onStateChanged } from "./api";
import type { DexEntry, DexView } from "./api";
import { BRANCH_NAMES, SPECIES_NAMES } from "./names";
import { spritePath } from "./sprite";
import { useListener } from "./useListener";
import "./Dex.css";

const errorText = (e: unknown) => (e instanceof Error ? e.message : String(e));

// 発見していない姿は名前を出さない（アクセシビリティ名も「未発見」）。
const label = (e: DexEntry) => {
  if (!e.discovered) return "未発見";
  if (e.stage <= 1) return "卵";
  return `段階 ${e.stage}・${BRANCH_NAMES[e.branch]}・${SPECIES_NAMES[e.personality]}`;
};

/** メニューバーの「図鑑…」から開く、出会った姿の一覧。 */
export default function Dex() {
  const [dex, setDex] = useState<DexView | null>(null);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(() => {
    getDex().then(
      (next) => {
        setError(null);
        setDex(next);
      },
      (e) => setError(errorText(e)),
    );
  }, []);

  useEffect(load, [load]);
  // 進化すると新しい姿が増えるので、状態が変わるたびに読み直す。
  useListener(onStateChanged, load);

  const stages = dex ? [...new Set(dex.entries.map((e) => e.stage))] : [];

  return (
    <main className="dex">
      <h1>図鑑</h1>
      {error && <p role="alert">{error}</p>}
      {dex && (
        <>
          <div className="summary">
            <span>
              発見 {dex.discovered} / {dex.total}
            </span>
            <progress aria-label="発見数" role="progressbar" aria-valuenow={dex.discovered} max={dex.total} value={dex.discovered} />
            {dex.discovered === dex.total && <strong>コンプリート！</strong>}
          </div>
          {stages.map((stage) => (
            <section key={stage} aria-label={`段階 ${stage}`} className="stage-row">
              <h2>{stage <= 1 ? "卵" : `段階 ${stage}`}</h2>
              <div className="cells">
                {dex.entries
                  .filter((e) => e.stage === stage)
                  .map((e) => (
                    <span key={e.key} className="cell-box">
                      <img className={e.discovered ? "cell" : "cell unknown"} src={spritePath(e)} alt={label(e)} title={label(e)} />
                    </span>
                  ))}
              </div>
            </section>
          ))}
        </>
      )}
    </main>
  );
}
