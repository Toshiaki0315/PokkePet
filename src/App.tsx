import { useEffect, useRef, useState } from "react";
import type { CSSProperties } from "react";
import {
  ackHatch,
  buryPet,
  graduate,
  playMinigame,
  setName,
  togglePower,
  getSettings,
  getState,
  onPopupVisibility,
  onSettingsChanged,
  onStateChanged,
  performAction,
  saveSettings,
  strokePet,
} from "./api";
import type {
  ActionName,
  MiniGameReport,
  Settings,
  Side,
  StateView,
} from "./api";
import { DEFAULT_SETTINGS } from "./defaults";
import { Sprite } from "./SpriteView";
import { BUBBLE_TEXT } from "./bubble";
import { Farewell } from "./Farewell";
import { Grave } from "./Grave";
import { MiniGame } from "./MiniGame";
import { NamePrompt } from "./NamePrompt";
import { useHiddenCommand } from "./hiddenCommand";
import { useHeldKey } from "./heldKey";
import { HatchSequence } from "./hatch";
import {
  cryForGesture,
  gesturesFor,
  IDLE_CRY_CHANCE,
  IDLE_CRY_COOLDOWN_MS,
  useIdleGesture,
} from "./idle";
import { useSound } from "./sound";
import type { CryKind } from "./sound";
import { strokeReaction, usePetting } from "./petting";
import { useListener } from "./useListener";
import "./App.css";

// 隠しコマンド: ↑ ↑ ↓ ↓ でゲージの表示を切り替える。
const GAUGE_COMMAND = ["ArrowUp", "ArrowUp", "ArrowDown", "ArrowDown"] as const;

const GAUGES: ReadonlyArray<readonly [string, keyof StateView]> = [
  ["ご機嫌", "mood"],
  ["親密度", "affinity"],
  ["空腹度", "hunger"],
  ["汚れ度", "dirtiness"],
  ["しつけ度", "discipline"],
  ["体重", "weight"],
];

const actions = (
  view: StateView,
): ReadonlyArray<{ name: ActionName; label: string; icon: string }> => [
  { name: "feed", label: "ご飯", icon: "🍙" },
  { name: "snack", label: "おやつ", icon: "🍪" },
  // 卵の間は「撫でる」。送るアクションは同じ "play" で、効果の違いは Rust 側が決める。
  view.playAction === "pet"
    ? { name: "play", label: "撫でる", icon: "🤚" }
    : { name: "play", label: "遊ぶ", icon: "🎾" },
  { name: "clean", label: "掃除", icon: "🧹" },
  { name: "scold", label: "しつけ", icon: "😠" },
  ...(view.sick ? [{ name: "cure" as const, label: "治療", icon: "💉" }] : []),
];

const errorText = (e: unknown) => (e instanceof Error ? e.message : String(e));

export default function App() {
  const [view, setView] = useState<StateView | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [settings, setSettings] = useState<Settings>(DEFAULT_SETTINGS);
  // ボタンを押したときのキャラの反応。id を変えて同じ演出でも再生し直す。
  const [reaction, setReaction] = useState<{
    name: string;
    id: number;
  } | null>(null);
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

  // 隠しコマンドでゲージ表示を切り替える。
  // ⌘P を押している間だけ、画面の上にゲージを重ねて見せる。
  const peekGauges = useHeldKey("p", true);

  useHiddenCommand(GAUGE_COMMAND, () => {
    saveSettings({ ...settings, showGauges: !settings.showGauges }).then(
      setSettings,
      (e) => setError(errorText(e)),
    );
  });

  const idle = useIdleGesture(
    view ? gesturesFor(view) : [],
    popupOpen &&
      view !== null &&
      !view.sleeping &&
      !view.intro &&
      !view.hatching &&
      view.expression.hunger !== "low" &&
      reaction === null,
  );

  // 鳴き声（設定でオフにできる）。声は、そのときのペットの性格と段階で決まる。
  const play = useSound(settings.soundEnabled, {
    volume: settings.soundVolume,
    pitch: settings.soundPitch,
  });
  const cry = (kind: CryKind, who: StateView) =>
    play(kind, { personality: who.personality, stage: who.stage });
  const latest = useRef({ view, cry });
  useEffect(() => {
    latest.current = { view, cry };
  });
  const lastIdleCry = useRef(0);

  // ポップアップを開いたときのあいさつ。おやすみ中は鳴かない。
  useEffect(() => {
    const { view: current, cry: speak } = latest.current;
    if (!popupOpen || !current || current.sleeping) return;
    const e = current.expression;
    speak(
      e.hunger === "low" || e.mood === "low"
        ? "sad"
        : e.mood === "high"
          ? "happy"
          : "neutral",
      current,
    );
  }, [popupOpen]);

  // しぐさに合わせて、ときどき鳴く（短い間隔では鳴かない）。
  useEffect(() => {
    const { view: current, cry: speak } = latest.current;
    if (!idle || !current) return;
    const now = Date.now();
    if (
      Math.random() >= IDLE_CRY_CHANCE ||
      now - lastIdleCry.current < IDLE_CRY_COOLDOWN_MS
    )
      return;
    lastIdleCry.current = now;
    speak(cryForGesture(idle), current);
  }, [idle]);

  const act = (action: ActionName, petting: boolean) =>
    performAction(action).then(
      (next) => {
        setError(null);
        setView(next);
        setReaction((prev) => ({
          name:
            petting || action === "scold" || action === "cure"
              ? "wobble"
              : "hop",
          id: (prev?.id ?? 0) + 1,
        }));
        cry(
          petting
            ? "tap"
            : action === "scold" || action === "cure"
              ? "sad"
              : action === "clean"
                ? "neutral"
                : "eat",
          next,
        );
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
        cry(report.win ? "happy" : "sad", report.view);
        return report;
      },
      (e) => {
        setError(errorText(e));
        throw e;
      },
    );

  // マウスでなでられたとき: ハートを浮かべて喜ぶ。おやすみ中は起こさない。
  const [hearts, setHearts] = useState<number | null>(null);
  const petting = usePetting(() => {
    strokePet().then(
      (next) => {
        setError(null);
        setView(next);
        if (next.sleeping) return;
        setHearts((prev) => (prev ?? 0) + 1);
        setReaction((prev) => ({
          name: strokeReaction(next.personality, next.stage),
          id: (prev?.id ?? 0) + 1,
        }));
        cry("happy", next);
      },
      (e) => setError(errorText(e)),
    );
  });

  const seeOff = () => {
    graduate().then(
      (next) => {
        setError(null);
        setView(next);
      },
      (e) => setError(errorText(e)),
    );
  };

  // 電源ボタン: ペットを無理やり寝かせる（寝ている間に押すと起こす）。
  const power = () => {
    togglePower().then(
      (next) => {
        setError(null);
        setView(next);
      },
      (e) => setError(errorText(e)),
    );
  };

  const bury = () => {
    buryPet().then(
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
      style={
        {
          "--body-color": settings.bodyColor,
          "--button-color": settings.buttonColor,
        } as CSSProperties
      }
    >
      <div className="device">
        <section
          className={`screen${settings.showGauges ? " compact" : ""}`}
          aria-label="画面"
        >
          <div className="titlebar">
            {view.name && <p className="name">{view.name}</p>}
            <button
              type="button"
              className="power"
              aria-label="電源"
              aria-pressed={view.forcedSleep}
              title={view.forcedSleep ? "電源（起こす）" : "電源（寝かせる）"}
              disabled={view.stage <= 1 || view.dead || (view.sleeping && !view.forcedSleep)}
              onClick={power}
            >
              ⏻
            </button>
          </div>
          {view.graduating && stayedGeneration !== view.generation && (
            <Farewell
              name={view.name}
              onSeeOff={seeOff}
              onStay={() => setStayedGeneration(view.generation)}
            />
          )}
          {view.dead && <Grave name={view.name} onBury={bury} />}
          {gameOpen && (
            <MiniGame onPlay={playGame} onClose={() => setGameOpen(false)} />
          )}
          {view.needsName && !namingLater && (
            <NamePrompt
              onSubmit={submitName}
              onLater={() => setNamingLater(true)}
            />
          )}
          <div className="stage-area">
            {view.poops > 0 && (
              <div
                className="poops"
                role="img"
                aria-label={`うんち ${view.poops}個`}
              >
                {"💩".repeat(view.poops)}
              </div>
            )}
            {view.sick && (
              <div className="sick-mark" role="img" aria-label="病気">
                🤒
              </div>
            )}
            {view.expression.bubble && (
              <div className="bubble" role="status">
                {BUBBLE_TEXT[view.expression.bubble]}
              </div>
            )}
            {view.hatching && popupOpen ? (
              <HatchSequence
                view={view}
                onDone={finishHatch}
                onCry={(kind) => cry(kind, view)}
              />
            ) : (
              <div className="petting-area" {...petting}>
                {hearts !== null && (
                  <span
                    key={`hearts-${hearts}`}
                    className="hearts"
                    data-testid="hearts"
                    aria-hidden="true"
                    onAnimationEnd={() => setHearts(null)}
                  >
                    <i>♥</i>
                    <i>♥</i>
                    <i>♥</i>
                  </span>
                )}
                <Sprite
                  key={reaction?.id}
                  view={view}
                  breathing={!view.sleeping}
                  extraClass={
                    reaction?.name ??
                    (view.intro ? "appear" : (idle ?? undefined))
                  }
                  onAnimationEnd={() => setReaction(null)}
                />
              </div>
            )}
          </div>
          {(settings.showGauges || peekGauges) && (
            <div className={settings.showGauges ? "gauges" : "gauges peek"}>
              {GAUGES.map(([label, key]) => (
                <label key={key} className="gauge">
                  <span>{label}</span>
                  <progress
                    aria-label={label}
                    role="progressbar"
                    aria-valuenow={Number(view[key])}
                    max={100}
                    value={Number(view[key])}
                  />
                </label>
              ))}
            </div>
          )}
        </section>
        <div className="controls" role="group" aria-label="ボタン">
          {actions(view).map(({ name, label, icon }) => (
            <div key={label} className="control">
              <button
                type="button"
                aria-label={label}
                onClick={() =>
                  name === "play" && view.playAction === "play"
                    ? setGameOpen(true)
                    : act(name, name === "play")
                }
              >
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
