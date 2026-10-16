import { useEffect, useState } from "react";
import { getSettings, onSettingsChanged, saveSettings } from "./api";
import type { Settings as SettingsModel } from "./api";
import { DEFAULT_SETTINGS, PRESETS } from "./defaults";
import { useListener } from "./useListener";
import "./Settings.css";

const errorText = (e: unknown) => (e instanceof Error ? e.message : String(e));

/** メニューバーの「設定…」から開く設定ダイアログ。変更は即座に保存・反映される。 */
export default function Settings() {
  const [settings, setSettings] = useState<SettingsModel>(DEFAULT_SETTINGS);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    getSettings().then(setSettings, (e) => setError(errorText(e)));
  }, []);
  useListener(onSettingsChanged, setSettings);

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
