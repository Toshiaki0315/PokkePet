import type { Settings } from "./api";

// Rust 側の既定値 (src-tauri/src/settings.rs) と同じ。起動直後に設定が届くまでの表示に使う。
export const DEFAULT_SETTINGS: Settings = {
  showStage: false,
  showGauges: false,
  notifyCalls: true,
  bodyColor: "#4b5fb8",
  buttonColor: "#f280a8",
};

export interface Preset {
  name: string;
  bodyColor: string;
  buttonColor: string;
}

export const PRESETS: readonly Preset[] = [
  { name: "ブルー", bodyColor: "#4b5fb8", buttonColor: "#f280a8" },
  { name: "ミント", bodyColor: "#2f9e8f", buttonColor: "#ffb454" },
  { name: "サンセット", bodyColor: "#d9644a", buttonColor: "#ffd166" },
  { name: "グレープ", bodyColor: "#7a4fc2", buttonColor: "#7be0c3" },
  { name: "モノクロ", bodyColor: "#444b57", buttonColor: "#e8e8ee" },
];
