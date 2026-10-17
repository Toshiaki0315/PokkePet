import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export type Branch = "poor" | "normal" | "good";
export type Personality = "calm" | "playful" | "gluttonous" | "tidy";
export type Level = "low" | "mid" | "high";
export type BubbleKind = "hungry" | "dirty" | "gloomy" | "happy" | "welcome" | "sleeping" | "morning" | "noon" | "evening" | "drowsy";
export type PlayAction = "play" | "pet";
export type ActionName = "feed" | "snack" | "play" | "clean";

export interface StateView {
  mood: number;
  affinity: number;
  hunger: number;
  dirtiness: number;
  stage: number;
  branch: Branch;
  personality: Personality;
  expression: Expression;
  /** 卵の間は "pet"（ボタンの表示が「撫でる」になる） */
  playAction: PlayAction;
  /** 初回オープニング中（最初の操作まで） */
  intro: boolean;
  /** 孵化したが、演出をまだ見せていない */
  hatching: boolean;
  /** おやすみ時間中（ローカル時刻） */
  sleeping: boolean;
  /** ペットの名前（未設定なら null） */
  name: string | null;
  /** 孵化の演出が終わったのに、まだ名前がない */
  needsName: boolean;
  /** 旅立ちの時を迎えている（見送るまで一緒にいられる） */
  graduating: boolean;
  /** 何代目か */
  generation: number;
}

export interface Expression {
  mood: Level;
  /** low = おなかがすいている */
  hunger: Level;
  /** high = 部屋がとても汚れている */
  dirtiness: Level;
  bubble: BubbleKind | null;
}

export function getState(): Promise<StateView> {
  return invoke<StateView>("get_state");
}

export function performAction(action: ActionName): Promise<StateView> {
  return invoke<StateView>("perform_action", { action });
}

/** マウスでなでた。ほんの少しご機嫌が上がる（おやすみ中は変わらない）。 */
export function strokePet(): Promise<StateView> {
  return invoke<StateView>("stroke_pet");
}

/** ポップアップが開いた(true)/閉じた(false)ことの通知。 */
export function onPopupVisibility(handler: (visible: boolean) => void): Promise<() => void> {
  return listen<boolean>("popup-visibility", (event) => handler(event.payload));
}

/** 名前を付ける（付け直す）。長さなどの検証は Rust 側。 */
export function setName(name: string): Promise<StateView> {
  return invoke<StateView>("set_name", { name });
}

/** 旅立ちを見送り、次の世代の卵を迎える。 */
export function graduate(): Promise<StateView> {
  return invoke<StateView>("graduate");
}

/** 孵化の演出を見せ終えたことを記録する。 */
export function ackHatch(): Promise<StateView> {
  return invoke<StateView>("ack_hatch");
}

export function onStateChanged(handler: (view: StateView) => void): Promise<() => void> {
  return listen<StateView>("state-changed", (event) => handler(event.payload));
}

export interface Settings {
  showGauges: boolean;
  /** お世話の呼びかけ（空腹・汚れ・つまらない）をmacOSの通知で知らせる */
  notifyCalls: boolean;
  /** 鳴き声（サウンド）を鳴らす */
  soundEnabled: boolean;
  /** おやすみ時間の開始・終了（0〜23時）。同じ値ならおやすみなし */
  sleepStartHour: number;
  sleepEndHour: number;
  bodyColor: string;
  buttonColor: string;
}

export function getSettings(): Promise<Settings> {
  return invoke<Settings>("get_settings");
}

export function saveSettings(settings: Settings): Promise<Settings> {
  return invoke<Settings>("save_settings", { settings });
}

export function onSettingsChanged(handler: (settings: Settings) => void): Promise<() => void> {
  return listen<Settings>("settings-changed", (event) => handler(event.payload));
}

export interface DexEntry {
  key: string;
  stage: number;
  branch: Branch;
  personality: Personality;
  discovered: boolean;
}

export interface DexView {
  total: number;
  discovered: number;
  entries: DexEntry[];
}

/** 図鑑（全 109 種と発見済みの印）。 */
export function getDex(): Promise<DexView> {
  return invoke<DexView>("get_dex");
}

export type DiaryEvent =
  | { kind: "born" }
  | { kind: "hatched" }
  | { kind: "evolved"; stage: number; branch: Branch; personality: Personality }
  | { kind: "named"; name: string }
  | { kind: "careMilestone"; count: number }
  | { kind: "anniversary"; days: number }
  | { kind: "graduated"; name: string | null; personality: Personality };

export interface DiaryEntry {
  atUnix: number;
  event: DiaryEvent;
}

export interface DiaryView {
  /** いっしょに過ごした日数（生まれた日が 1 日目） */
  daysTogether: number;
  /** 新しい順 */
  entries: DiaryEntry[];
}

/** 思い出（新しい順）といっしょの日数。 */
export function getDiary(): Promise<DiaryView> {
  return invoke<DiaryView>("get_diary");
}

export type Side = "left" | "right";

export interface MiniGameReport {
  view: StateView;
  win: boolean;
  /** ペットがおやつを隠していた手 */
  answer: Side;
}

/** ミニゲーム「どっちの手？」。勝敗の判定と効果は Rust 側。 */
export function playMinigame(guess: Side): Promise<MiniGameReport> {
  return invoke<MiniGameReport>("play_minigame", { guess });
}
