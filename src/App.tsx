import { useEffect, useState } from "react";
import type { CSSProperties } from "react";
import { ackHatch, graduate, playMinigame, setName, getSettings, getState, onPopupVisibility, onSettingsChanged, onStateChanged, performAction, saveSettings } from "./api";
import type { ActionName, MiniGameReport, Settings, Side, StateView } from "./api";
import { DEFAULT_SETTINGS } from "./defaults";
import { Sprite } from "./SpriteView";
import { bubbleText } from "./bubble";
import { Farewell } from "./Farewell";
import { MiniGame } from "./MiniGame";
import { NamePrompt } from "./NamePrompt";
import { useHiddenCommand } from "./hiddenCommand";
import { HatchSequence } from "./hatch";
import { useIdleGesture } from "./idle";
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
  { name: "snack", label: "おやつ", icon: "🍪" },
  // 卵の間は「撫でる」。送るアクションは同じ "play" で、効果の違いは Rust 側が決める。
  view.playAction === "pet" ? { name: "play", label: "撫でる", icon: "🤚" } : { name: "play", label: "遊ぶ", icon: "🎾" },
  { name: "clean", label: "掃除", icon: "🧹" },
];

// 待機中のしぐさ。卵はゆっくり揺れるだけ。
const EGG_GESTURES = ["rock"] as const;
const GESTURES = ["bounce", "walk", "look", "stretch"] as const;

const errorText = (e: unknown) => (e instanceof Error ? e.message : String(e));

export default function App() {
  const [view, setView] = useState<StateView | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [settings, setSettings] = useState<Settings>(DEFAULT_SETTINGS);
  // ボタンを押したときのキャラの反応。id を変えて同じ演出でも再生し直す。
  const [reaction, setReaction] = useState<{ name: "wobble" | "hop"; id: number } | null>(null);
  // ポップアップが開いている間だけ孵化の演出を再生する（Rust が表示/非表示を通知する）。
  const [popupOpen, setPopupOpen] = useState(false);
  const [namingLater, setNamingLater] = useState(false);
  const [gameOpen, setGameOpen] = useState(false);
  // 「もう少しいっしょにいる」を選んだ世代（その世代の間だけお別れ画面を隠す）。
  const [stayedGeneration, setStayedGeneration] = useState<number | null>(null);

  useEffect(() => {
    getState().then(setView, (e) => setError(errorText(e)));
    getSettings().then(setSettings, (e) => setError(errorText(e)));
  }, []);
  useListener(onStateChanged, setView);
  useListener(onSettingsChanged, setSettings);
  useListener(onPopupVisibility, setPopupOpen);

  // 隠しコマンドは段階表示とゲージ表示をまとめて切り替える（どちらかがオンならオフにする）。
  useHiddenCommand(GAUGE_COMMAND, () => {
    const next = !(settings.showStage || settings.showGauges);
    saveSettings({ ...settings, showStage: next, showGauges: next }).then(setSettings, (e) => setError(errorText(e)));
  });

  const idle = useIdleGesture(
    view?.stage === 1 ? EGG_GESTURES : GESTURES,
    popupOpen && view !== null && !view.sleeping && !view.intro && !view.hatching && view.expression.hunger !== "low" && reaction === null,
  );

  const act = (action: ActionName, petting: boolean) =>
    performAction(action).then(
      (next) => {
        setError(null);
        setView(next);
        setReaction((prev) => ({ name: petting ? "wobble" : "hop", id: (prev?.id ?? 0) + 1 }));
      },
      (e) => setError(errorText(e)),
    );

  // ミニゲーム: 結果が届いたら状態を更新してキャラを跳ねさせる。失敗はエラー欄に出して、選び直せるようにする。
  const playGame = (guess: Side): Promise<MiniGameReport> =>
    playMinigame(guess).then(
      (report) => {
        setError(null);
        setView(report.view);
        setReaction((prev) => ({ name: "hop", id: (prev?.id ?? 0) + 1 }));
        return report;
      },
      (e) => {
        setError(errorText(e));
        throw e;
      },
    );

  const seeOff = () => {
    graduate().then(
      (next) => {
        setError(null);
        setView(next);
      },
      (e) => setError(errorText(e)),
    );
  };

  const submitName = (name: string) => {
    setName(name).then(
      (next) => {
        setError(null);
        setView(next);
      },
      (e) => setError(errorText(e)),
    );
  };

  const finishHatch = () => {
    ackHatch().then(setView, (e) => setError(errorText(e)));
  };

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
      data-intro={view.intro}
      data-sleep={view.sleeping}
      style={{ "--body-color": settings.bodyColor, "--button-color": settings.buttonColor } as CSSProperties}
    >
      <div className="device">
        <section className={`screen${settings.showGauges ? " compact" : ""}`} aria-label="画面">
          {settings.showStage && <p className="stage">段階 {view.stage} / 10</p>}
          {settings.showStage && view.generation > 1 && <p className="generation">{view.generation}代目</p>}
          {view.name && <p className="name">{view.name}</p>}
          {view.graduating && stayedGeneration !== view.generation && (
            <Farewell name={view.name} onSeeOff={seeOff} onStay={() => setStayedGeneration(view.generation)} />
          )}
          {gameOpen && <MiniGame onPlay={playGame} onClose={() => setGameOpen(false)} />}
          {view.needsName && !namingLater && <NamePrompt onSubmit={submitName} onLater={() => setNamingLater(true)} />}
          <div className="stage-area">
            {view.expression.bubble && (
              <div className="bubble" role="status">
                {bubbleText(view.expression.bubble, view.name)}
              </div>
            )}
            {view.hatching && popupOpen ? (
              <HatchSequence view={view} onDone={finishHatch} />
            ) : (
              <Sprite
                key={reaction?.id}
                view={view}
                extraClass={reaction?.name ?? (view.intro ? "appear" : (idle ?? undefined))}
                onAnimationEnd={() => setReaction(null)}
              />
            )}
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
              <button type="button" aria-label={label} onClick={() => (name === "play" && view.playAction === "play" ? setGameOpen(true) : act(name, name === "play"))}>
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
