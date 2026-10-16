import { useEffect, useState } from "react";
import { getState, onStateChanged, performAction } from "./api";
import type { ActionName, StateView } from "./api";
import { Sprite } from "./SpriteView";
import "./App.css";

const GAUGES: ReadonlyArray<readonly [string, keyof StateView]> = [
  ["ご機嫌", "mood"],
  ["親密度", "affinity"],
  ["空腹度", "hunger"],
  ["汚れ度", "dirtiness"],
];

const ACTIONS: ReadonlyArray<readonly [string, ActionName]> = [
  ["ご飯", "feed"],
  ["遊ぶ", "play"],
  ["掃除", "clean"],
];

const errorText = (e: unknown) => (e instanceof Error ? e.message : String(e));

export default function App() {
  const [view, setView] = useState<StateView | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    getState().then(setView, (e) => setError(errorText(e)));
    onStateChanged(setView).then((off) => {
      if (disposed) return off();
      unlisten = off;
    });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  const act = (action: ActionName) =>
    performAction(action).then(
      (next) => {
        setError(null);
        setView(next);
      },
      (e) => setError(errorText(e)),
    );

  if (!view) {
    return (
      <main className="container">
        {error ? <p role="alert">{error}</p> : <p>読み込み中…</p>}
      </main>
    );
  }

  return (
    <main className="container">
      <Sprite view={view} />
      <p className="stage">段階 {view.stage} / 10</p>
      <div className="gauges">
        {GAUGES.map(([label, key]) => (
          <label key={key} className="gauge">
            <span>{label}</span>
            <progress aria-label={label} role="progressbar" aria-valuenow={Number(view[key])} max={100} value={Number(view[key])} />
          </label>
        ))}
      </div>
      <div className="actions">
        {ACTIONS.map(([label, action]) => (
          <button key={action} type="button" onClick={() => act(action)}>
            {label}
          </button>
        ))}
      </div>
      {error && <p role="alert">{error}</p>}
    </main>
  );
}
