import { useEffect, useState } from "react";
import type { CSSProperties } from "react";
import { getSettings, getState, onSettingsChanged, onStateChanged, performAction, saveSettings } from "./api";
import type { ActionName, Settings, StateView } from "./api";
import { DEFAULT_SETTINGS } from "./defaults";
import { Sprite } from "./SpriteView";
import { BUBBLE_TEXT } from "./bubble";
import { useHiddenCommand } from "./hiddenCommand";
import { useListener } from "./useListener";
import "./App.css";

// 隠しコマンド: ↑ ↑ ↓ ↓ で段階とゲージの表示を切り替える。
const GAUGE_COMMAND = ["ArrowUp", "ArrowUp", "ArrowDown", "ArrowDown"] as const;

const GAUGES: ReadonlyArray<readonly [string, keyof StateView]> = [
  ["ご機嫌", "mood"],
  ["親密度", "affinity"],
  ["空腹度", "hunger"],
  ["汚れ度", "dirtiness"],
];

const actions = (view: StateView): ReadonlyArray<{ name: ActionName; label: string; icon: string }> => [
  { name: "feed", label: "ご飯", icon: "🍙" },
  // 卵の間は「撫でる」。送るアクションは同じ "play" で、効果の違いは Rust 側が決める。
  view.playAction === "pet" ? { name: "play", label: "撫でる", icon: "🤚" } : { name: "play", label: "遊ぶ", icon: "🎾" },
  { name: "clean", label: "掃除", icon: "🧹" },
];

const errorText = (e: unknown) => (e instanceof Error ? e.message : String(e));

export default function App() {
  const [view, setView] = useState<StateView | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [settings, setSettings] = useState<Settings>(DEFAULT_SETTINGS);

  useEffect(() => {
    getState().then(setView, (e) => setError(errorText(e)));
    getSettings().then(setSettings, (e) => setError(errorText(e)));
  }, []);
  useListener(onStateChanged, setView);
  useListener(onSettingsChanged, setSettings);

  // 隠しコマンドは段階表示とゲージ表示をまとめて切り替える（どちらかがオンならオフにする）。
  useHiddenCommand(GAUGE_COMMAND, () => {
    const next = !(settings.showStage || settings.showGauges);
    saveSettings({ ...settings, showStage: next, showGauges: next }).then(setSettings, (e) => setError(errorText(e)));
  });

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
    <main
      className="container"
      data-mood={view.expression.mood}
      data-dirt={view.expression.dirtiness}
      data-hunger={view.expression.hunger}
      style={{ "--body-color": settings.bodyColor, "--button-color": settings.buttonColor } as CSSProperties}
    >
      <div className="device">
        <section className={`screen${settings.showGauges ? " compact" : ""}`} aria-label="画面">
          {settings.showStage && <p className="stage">段階 {view.stage} / 10</p>}
          <div className="stage-area">
            {view.expression.bubble && (
              <div className="bubble" role="status">
                {BUBBLE_TEXT[view.expression.bubble]}
              </div>
            )}
            <Sprite view={view} />
          </div>
          {settings.showGauges && (
            <div className="gauges">
              {GAUGES.map(([label, key]) => (
                <label key={key} className="gauge">
                  <span>{label}</span>
                  <progress aria-label={label} role="progressbar" aria-valuenow={Number(view[key])} max={100} value={Number(view[key])} />
                </label>
              ))}
            </div>
          )}
        </section>
        <div className="controls" role="group" aria-label="ボタン">
          {actions(view).map(({ name, label, icon }) => (
            <div key={label} className="control">
              <button type="button" aria-label={label} onClick={() => act(name)}>
                <span aria-hidden="true">{icon}</span>
              </button>
              <span className="caption" aria-hidden="true">
                {label}
              </span>
            </div>
          ))}
        </div>
        {error && <p role="alert">{error}</p>}
      </div>
    </main>
  );
}
