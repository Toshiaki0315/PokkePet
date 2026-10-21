import { useEffect, useState } from "react";
import {
  getSettings,
  onSettingsChanged,
  revealDebugLog,
  saveSettings,
} from "./api";
import type { Settings as SettingsModel } from "./api";
import {
  BACKGROUNDS,
  BUBBLE_SECONDS_CHOICES,
  DEFAULT_SETTINGS,
  LEGACY_STRENGTHS,
  PRESETS,
} from "./defaults";
import { useSound } from "./sound";
import { useListener } from "./useListener";
import "./Settings.css";
import "./scenes.css";

/** 月ごとの日数（2 月は 29 日まで選べる）。 */
const DAYS_IN_MONTH = [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
const MONTHS = Array.from({ length: 12 }, (_, i) => i + 1);

const HOURS = Array.from({ length: 24 }, (_, hour) => hour);

/** 「22時〜翌6時」のような表示。開始が終了より後なら翌日にまたがる。 */
const describeWindow = (start: number, end: number) =>
  start > end ? `${start}時〜翌${end}時` : `${start}時〜${end}時`;

const errorText = (e: unknown) => (e instanceof Error ? e.message : String(e));

/** メニューバーの「設定…」から開く設定ダイアログ。変更は即座に保存・反映される。 */
export default function Settings() {
  const [settings, setSettings] = useState<SettingsModel>(DEFAULT_SETTINGS);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    getSettings().then(setSettings, (e) => setError(errorText(e)));
  }, []);
  useListener(onSettingsChanged, setSettings);
  // 鳴き声の聞き比べ用。調整中の音量・高さでそのまま鳴る。
  const preview = useSound(settings.soundEnabled, {
    volume: settings.soundVolume,
    pitch: settings.soundPitch,
  });

  const change = (next: SettingsModel) =>
    saveSettings(next).then(
      (saved) => {
        setError(null);
        setSettings(saved);
      },
      (e) => setError(errorText(e)),
    );
  const patch = (partial: Partial<SettingsModel>) =>
    change({ ...settings, ...partial });

  return (
    <main className="settings">
      <h1>設定</h1>

      <fieldset>
        <legend>表示</legend>
        <label className="row">
          <input
            type="checkbox"
            checked={settings.showGauges}
            onChange={(e) => patch({ showGauges: e.target.checked })}
          />
          ゲージを表示
        </label>
        <label className="row">
          吹き出しを出す長さ
          <select
            aria-label="吹き出しを出す長さ"
            value={settings.bubbleSeconds}
            onChange={(e) => patch({ bubbleSeconds: Number(e.target.value) })}
          >
            {BUBBLE_SECONDS_CHOICES.map(({ seconds, name }) => (
              <option key={seconds} value={seconds}>
                {name}
              </option>
            ))}
          </select>
        </label>
        <p className="hint">
          ペットのひとことが、これだけ出てから、すっと消えます。
        </p>
      </fieldset>

      <fieldset>
        <legend>お誕生日</legend>
        <p className="hint">
          あなたのお誕生日を入れると、その日、ペットがお祝いしてくれます。
        </p>
        <div className="row">
          <label>
            月
            <select
              aria-label="誕生日の月"
              value={settings.birthdayMonth}
              onChange={(e) =>
                patch(
                  e.target.value === "0"
                    ? { birthdayMonth: 0, birthdayDay: 0 }
                    : {
                        birthdayMonth: Number(e.target.value),
                        birthdayDay: Math.max(
                          1,
                          Math.min(
                            settings.birthdayDay || 1,
                            DAYS_IN_MONTH[Number(e.target.value) - 1],
                          ),
                        ),
                      },
                )
              }
            >
              <option value={0}>なし</option>
              {MONTHS.map((month) => (
                <option key={month} value={month}>
                  {month}月
                </option>
              ))}
            </select>
          </label>
          <label>
            日
            <select
              aria-label="誕生日の日"
              value={settings.birthdayDay}
              disabled={settings.birthdayMonth === 0}
              onChange={(e) => patch({ birthdayDay: Number(e.target.value) })}
            >
              {settings.birthdayMonth === 0 && <option value={0}>-</option>}
              {Array.from(
                {
                  length:
                    settings.birthdayMonth === 0
                      ? 0
                      : DAYS_IN_MONTH[settings.birthdayMonth - 1],
                },
                (_, i) => i + 1,
              ).map((day) => (
                <option key={day} value={day}>
                  {day}日
                </option>
              ))}
            </select>
          </label>
        </div>
      </fieldset>

      <fieldset>
        <legend>おやすみ時間</legend>
        <p className="hint">
          この間はキャラが眠り、状態の減りがゆっくりになります（今:{" "}
          <strong>
            {describeWindow(settings.sleepStartHour, settings.sleepEndHour)}
          </strong>
          ）。
        </p>
        <div className="row">
          <label>
            開始
            <select
              aria-label="おやすみ開始"
              value={settings.sleepStartHour}
              onChange={(e) =>
                patch({ sleepStartHour: Number(e.target.value) })
              }
            >
              {HOURS.map((hour) => (
                <option key={hour} value={hour}>
                  {hour}時
                </option>
              ))}
            </select>
          </label>
          <label>
            終了
            <select
              aria-label="おやすみ終了"
              value={settings.sleepEndHour}
              onChange={(e) => patch({ sleepEndHour: Number(e.target.value) })}
            >
              {HOURS.map((hour) => (
                <option key={hour} value={hour}>
                  {hour}時
                </option>
              ))}
            </select>
          </label>
        </div>
        {settings.sleepStartHour === settings.sleepEndHour && (
          <p className="hint">開始と終了が同じ時刻なので、おやすみしません</p>
        )}
      </fieldset>

      <fieldset>
        <legend>サウンド</legend>
        <label className="row">
          <input
            type="checkbox"
            checked={settings.soundEnabled}
            onChange={(e) => patch({ soundEnabled: e.target.checked })}
          />
          鳴き声を鳴らす
        </label>
        <p className="hint">
          ポップアップを開いている間に、お世話やしぐさに合わせて鳴きます（おやすみ中は鳴きません）。
        </p>
        <label className="slider">
          大きさ
          <input
            type="range"
            aria-label="鳴き声の大きさ"
            min={0}
            max={100}
            step={10}
            value={settings.soundVolume}
            disabled={!settings.soundEnabled}
            onChange={(e) => patch({ soundVolume: Number(e.target.value) })}
          />
        </label>
        <label className="slider">
          高さ
          <input
            type="range"
            aria-label="鳴き声の高さ"
            min={-2}
            max={2}
            step={1}
            value={settings.soundPitch}
            disabled={!settings.soundEnabled}
            onChange={(e) => patch({ soundPitch: Number(e.target.value) })}
          />
        </label>
        <button
          type="button"
          disabled={!settings.soundEnabled}
          onClick={() => preview("happy", { personality: "calm", stage: 5 })}
        >
          ためしに鳴かせる
        </button>
      </fieldset>

      <fieldset>
        <legend>いっしょに</legend>
        <label className="row">
          <input
            type="checkbox"
            checked={settings.companionEnabled}
            onChange={(e) => patch({ companionEnabled: e.target.checked })}
          />
          作業に寄り添う
        </label>
        <p className="hint">
          Mac を操作している時間を、いっしょにいた時間として数えます（30
          分ごとに親密度が少し上がり、席を離れて戻ると「おかえり」と言います）。見るのは「操作しているか」だけで、何を打ったか・どのアプリかは見ません。
        </p>
        <label className="row">
          <input
            type="checkbox"
            checked={settings.breakReminder}
            disabled={!settings.companionEnabled}
            onChange={(e) => patch({ breakReminder: e.target.checked })}
          />
          休憩の声かけ
        </label>
        <p className="hint">
          1 時間半、休まずに作業していると、「ちょっと休もうよ」と言います。
        </p>
        <label className="row">
          <input
            type="checkbox"
            checked={settings.systemReactions}
            onChange={(e) => patch({ systemReactions: e.target.checked })}
          />
          Mac の状態に反応
        </label>
        <p className="hint">
          電池が少ない・充電中・ネットにつながっていない・ダークモード、に反応します（電池やネットの状態を読むだけです）。
        </p>
        <label className="row">
          <input
            type="checkbox"
            checked={settings.autoSleep}
            disabled={!settings.companionEnabled}
            onChange={(e) => patch({ autoSleep: e.target.checked })}
          />
          おやすみ時間を自動で合わせる
        </label>
        <p className="hint">
          操作する時間帯から生活リズムを学んで、上の設定の近く（4
          時間以内）で、いちばん操作しない 8
          時間を、おやすみ時間にします（学べるまでは、上の設定を使います）。
        </p>
      </fieldset>

      <fieldset>
        <legend>ゲーム</legend>
        <label className="row">
          <input
            type="checkbox"
            checked={settings.strictMode}
            onChange={(e) => patch({ strictMode: e.target.checked })}
          />
          きびしいモード
        </label>
        <label className="row">
          引き継ぎの強さ
          <select
            aria-label="引き継ぎの強さ"
            value={settings.legacyStrength}
            onChange={(e) => patch({ legacyStrength: Number(e.target.value) })}
          >
            {LEGACY_STRENGTHS.map(({ value, name }) => (
              <option key={value} value={value}>
                {name}
              </option>
            ))}
          </select>
        </label>
        <p className="hint">
          旅立った親の性格（種族）を、次の世代がどれだけ受け継ぐか。強いほど、親に似た子が生まれ、違う種族にするには、たくさん世話の仕方を変える必要があります。
        </p>
        <p className="hint">
          病気を治さずに放っておいたり、おなかもご機嫌もゼロのまま長く放置すると、お別れ（死）があります。ふだんは、こまめにお世話すれば心配いりません。
        </p>
      </fieldset>

      <fieldset>
        <legend>起動</legend>
        <label className="row">
          <input
            type="checkbox"
            checked={settings.launchAtLogin}
            onChange={(e) => patch({ launchAtLogin: e.target.checked })}
          />
          ログイン時に自動で起動
        </label>
        <p className="hint">
          インストールしたアプリだけに効きます（開発中の起動では登録されません）。
        </p>
      </fieldset>

      <fieldset>
        <legend>おでかけ</legend>
        <label className="row">
          <input
            type="checkbox"
            checked={settings.walkOnDesktop}
            onChange={(e) => patch({ walkOnDesktop: e.target.checked })}
          />
          おでかけ中、デスクトップを歩き回る
        </label>
        <p className="hint">
          おでかけしている間、小さなペットがデスクトップの上をうろうろします（クリックは通り抜けます）。
        </p>
      </fieldset>

      <fieldset>
        <legend>開発（デバッグ）</legend>
        <label className="row">
          <input
            type="checkbox"
            checked={settings.debugLog}
            onChange={(e) => patch({ debugLog: e.target.checked })}
          />
          デバッグログを残す
        </label>
        <p className="hint">
          ご飯・遊び・眠る・おでかけ・うんち・ご機嫌の変化などを、1
          行ずつ詳しく、データフォルダの debug.log に書き足します（1MB
          をこえると、古いものは debug.log.1
          になります）。ふだんはオフのままで大丈夫です。
        </p>
        <button
          type="button"
          onClick={() => revealDebugLog().catch((e) => setError(errorText(e)))}
        >
          ログの場所を開く
        </button>
      </fieldset>

      <fieldset>
        <legend>通知</legend>
        <label className="row">
          <input
            type="checkbox"
            checked={settings.notifyCalls}
            onChange={(e) => patch({ notifyCalls: e.target.checked })}
          />
          お世話の呼びかけを通知
        </label>
      </fieldset>

      <fieldset>
        <legend>背景</legend>
        <div className="scenes" role="radiogroup" aria-label="背景">
          {BACKGROUNDS.map(({ id, name }) => (
            <label
              key={id}
              className={`scene-choice${settings.background === id ? " selected" : ""}`}
            >
              <input
                type="radio"
                name="background"
                value={id}
                checked={settings.background === id}
                onChange={() => patch({ background: id })}
              />
              <span className={`scene-thumb scene-${id}`} aria-hidden="true" />
              <span>{name}</span>
            </label>
          ))}
        </div>
        <p className="hint">
          「きもち（いつもの）」は、ご機嫌で色が変わります。ほかの背景は、ご機嫌やおやすみで、色みが変わります。
        </p>
      </fieldset>

      <fieldset>
        <legend>配色</legend>
        <div className="presets">
          {PRESETS.map(({ name, bodyColor, buttonColor }) => (
            <button
              key={name}
              type="button"
              onClick={() => patch({ bodyColor, buttonColor })}
            >
              <span className="swatch" style={{ background: bodyColor }} />
              <span className="swatch" style={{ background: buttonColor }} />
              {name}
            </button>
          ))}
        </div>
        <label className="row">
          本体の色
          <input
            type="color"
            value={settings.bodyColor}
            onChange={(e) => patch({ bodyColor: e.target.value })}
          />
        </label>
        <label className="row">
          ボタンの色
          <input
            type="color"
            value={settings.buttonColor}
            onChange={(e) => patch({ buttonColor: e.target.value })}
          />
        </label>
      </fieldset>

      <button
        type="button"
        className="reset"
        onClick={() => change(DEFAULT_SETTINGS)}
      >
        初期設定に戻す
      </button>
      {error && <p role="alert">{error}</p>}
    </main>
  );
}
