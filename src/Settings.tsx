import { useEffect, useState } from "react";
import { getSettings, getState, onSettingsChanged, onStateChanged, saveSettings, setName } from "./api";
import type { Settings as SettingsModel } from "./api";
import { DEFAULT_SETTINGS, PRESETS } from "./defaults";
import { NAME_INPUT_MAX } from "./NamePrompt";
import { useListener } from "./useListener";
import "./Settings.css";

const HOURS = Array.from({ length: 24 }, (_, hour) => hour);

/** 「22時〜翌6時」のような表示。開始が終了より後なら翌日にまたがる。 */
const describeWindow = (start: number, end: number) => (start > end ? `${start}時〜翌${end}時` : `${start}時〜${end}時`);

const errorText = (e: unknown) => (e instanceof Error ? e.message : String(e));

/** メニューバーの「設定…」から開く設定ダイアログ。変更は即座に保存・反映される。 */
export default function Settings() {
  const [settings, setSettings] = useState<SettingsModel>(DEFAULT_SETTINGS);
  const [error, setError] = useState<string | null>(null);
  const [petName, setPetName] = useState("");

  useEffect(() => {
    getSettings().then(setSettings, (e) => setError(errorText(e)));
    getState().then((v) => setPetName(v.name ?? ""), (e) => setError(errorText(e)));
  }, []);
  useListener(onSettingsChanged, setSettings);
  useListener(onStateChanged, (v: { name: string | null }) => setPetName(v.name ?? ""));

  const rename = () =>
    setName(petName).then(
      () => setError(null), // 入力欄の値は state-changed の通知（と、いま入力した値）で揃う
      (e) => setError(errorText(e)),
    );

  const change = (next: SettingsModel) =>
    saveSettings(next).then(
      (saved) => {
        setError(null);
        setSettings(saved);
      },
      (e) => setError(errorText(e)),
    );
  const patch = (partial: Partial<SettingsModel>) => change({ ...settings, ...partial });

  return (
    <main className="settings">
      <h1>設定</h1>

      <fieldset>
        <legend>ペット</legend>
        <div className="row">
          <label>
            名前
            <input type="text" value={petName} maxLength={NAME_INPUT_MAX} onChange={(e) => setPetName(e.target.value)} />
          </label>
          <button type="button" aria-label="名前を変更" disabled={petName.trim() === ""} onClick={rename}>
            変更
          </button>
        </div>
      </fieldset>

      <fieldset>
        <legend>表示</legend>
        <label className="row">
          <input type="checkbox" checked={settings.showStage} onChange={(e) => patch({ showStage: e.target.checked })} />
          段階を表示
        </label>
        <label className="row">
          <input type="checkbox" checked={settings.showGauges} onChange={(e) => patch({ showGauges: e.target.checked })} />
          ゲージを表示
        </label>
      </fieldset>

      <fieldset>
        <legend>おやすみ時間</legend>
        <p className="hint">
          この間はキャラが眠り、状態の減りがゆっくりになります（今: <strong>{describeWindow(settings.sleepStartHour, settings.sleepEndHour)}</strong>）。
        </p>
        <div className="row">
          <label>
            開始
            <select aria-label="おやすみ開始" value={settings.sleepStartHour} onChange={(e) => patch({ sleepStartHour: Number(e.target.value) })}>
              {HOURS.map((hour) => (
                <option key={hour} value={hour}>
                  {hour}時
                </option>
              ))}
            </select>
          </label>
          <label>
            終了
            <select aria-label="おやすみ終了" value={settings.sleepEndHour} onChange={(e) => patch({ sleepEndHour: Number(e.target.value) })}>
              {HOURS.map((hour) => (
                <option key={hour} value={hour}>
                  {hour}時
                </option>
              ))}
            </select>
          </label>
        </div>
        {settings.sleepStartHour === settings.sleepEndHour && <p className="hint">開始と終了が同じ時刻なので、おやすみしません</p>}
      </fieldset>

      <fieldset>
        <legend>通知</legend>
        <label className="row">
          <input type="checkbox" checked={settings.notifyCalls} onChange={(e) => patch({ notifyCalls: e.target.checked })} />
          お世話の呼びかけを通知
        </label>
      </fieldset>

      <fieldset>
        <legend>配色</legend>
        <div className="presets">
          {PRESETS.map(({ name, bodyColor, buttonColor }) => (
            <button key={name} type="button" onClick={() => patch({ bodyColor, buttonColor })}>
              <span className="swatch" style={{ background: bodyColor }} />
              <span className="swatch" style={{ background: buttonColor }} />
              {name}
            </button>
          ))}
        </div>
        <label className="row">
          本体の色
          <input type="color" value={settings.bodyColor} onChange={(e) => patch({ bodyColor: e.target.value })} />
        </label>
        <label className="row">
          ボタンの色
          <input type="color" value={settings.buttonColor} onChange={(e) => patch({ buttonColor: e.target.value })} />
        </label>
      </fieldset>

      <button type="button" className="reset" onClick={() => change(DEFAULT_SETTINGS)}>
        初期設定に戻す
      </button>
      {error && <p role="alert">{error}</p>}
    </main>
  );
}
