import type { Settings } from "./api";

// Rust 側の既定値 (src-tauri/src/settings.rs) と同じ。起動直後に設定が届くまでの表示に使う。
export const DEFAULT_SETTINGS: Settings = {
  showGauges: false,
  notifyCalls: true,
  soundEnabled: true,
  soundVolume: 50,
  soundPitch: 0,
  launchAtLogin: false,
  strictMode: false,
  companionEnabled: true,
  systemReactions: true,
  background: "classic",
  bubbleSeconds: 8,
  debugLog: false,
  walkOnDesktop: false,
  legacyStrength: 2,
  birthdayMonth: 0,
  birthdayDay: 0,
  breakReminder: true,
  autoSleep: true,
  sleepStartHour: 23,
  sleepEndHour: 7,
  bodyColor: "#4b5fb8",
  buttonColor: "#f280a8",
};

/** 選べる背景（`id` は設定の値）。見た目は `scenes.css`。 */
export const BACKGROUNDS: readonly { id: Settings["background"]; name: string }[] = [
  { id: "classic", name: "きもち（いつもの）" },
  { id: "season", name: "季節にまかせる" },
  { id: "room", name: "おへや" },
  { id: "tatami", name: "わしつ" },
  { id: "meadow", name: "そうげん" },
  { id: "forest", name: "もり" },
  { id: "beach", name: "うみべ" },
  { id: "sakura", name: "よざくら" },
  { id: "snow", name: "ゆきげしき" },
  { id: "autumn", name: "あき" },
  { id: "stars", name: "ほしぞら" },
];

/** 吹き出しを出しておく長さの選びかた（秒。0 はずっと出しておく）。 */
export const BUBBLE_SECONDS_CHOICES: readonly { seconds: number; name: string }[] = [
  { seconds: 3, name: "3 秒" },
  { seconds: 5, name: "5 秒" },
  { seconds: 8, name: "8 秒" },
  { seconds: 15, name: "15 秒" },
  { seconds: 30, name: "30 秒" },
  { seconds: 0, name: "ずっと出しておく" },
];

/** 親の性格を受け継ぐ強さの選びかた。 */
export const LEGACY_STRENGTHS: readonly { value: number; name: string }[] = [
  { value: 0, name: "受け継がない" },
  { value: 1, name: "弱い" },
  { value: 2, name: "ふつう" },
  { value: 3, name: "強い" },
];

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
