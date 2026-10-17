import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export type Branch = "poor" | "normal" | "good" | "legend";
export type Personality = "calm" | "playful" | "gluttonous" | "tidy";
export type Level = "low" | "mid" | "high";
export type BubbleKind = "hungry" | "dirty" | "gloomy" | "happy" | "welcome" | "sleeping" | "morning" | "noon" | "evening" | "drowsy" | "selfish" | "sick" | "dying" | "rest" | "welcomeback" | "batterylow" | "offline" | "event";
export type Chronotype = "morning" | "night" | "neutral";
export type PlayAction = "play" | "pet";
export type ActionName = "feed" | "snack" | "play" | "clean" | "scold" | "cure";

/** 持ち帰ったおみやげ 1 つ。rarity は 0=よくある 1=めずらしい 2=とてもめずらしい */
export interface GiftItem {
  emoji: string;
  name: string;
  rarity: number;
}

export interface StateView {
  mood: number;
  affinity: number;
  hunger: number;
  dirtiness: number;
  /** 体重（標準は 50、適正は 35〜65） */
  weight: number;
  /** しつけ度（0〜100） */
  discipline: number;
  /** 画面に出るうんちの数（0〜4） */
  poops: number;
  /** 病気か（治療のボタンが出る） */
  sick: boolean;
  /** 命が危ない（きびしいモード） */
  danger: boolean;
  /** 亡くなっている（きびしいモード）。新しい卵を迎えるまで操作できない */
  dead: boolean;
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
  /** 電源ボタンで寝かされている（おやすみ時間とは関係なく）。もう一度押すと起こせる */
  forcedSleep: boolean;
  /** おでかけ中（飼い主が席を離れていて、ペットは起きている）。画面にはいない */
  out: boolean;
  /** 親から受け継いだ色あい（色相を回す角度。0 ならもとの色）と、飾り（絵文字。なければ null） */
  tintDegrees: number;
  charm: string | null;
  /** 持ち帰ったばかりで、まだ見せていないおみやげ */
  gift: GiftItem[];
  /** 画面に飾っているおみやげ（絵文字） */
  displayed: string[];
  /** Mac の電池が少ない（電源につながっていないとき） */
  batteryLow: boolean;
  /** Mac が充電中 */
  charging: boolean;
  /** インターネットにつながっていない */
  offline: boolean;
  /** Mac がダークモード */
  darkMode: boolean;
  /** 今日の季節のできごと（ない日は null） */
  event: { emoji: string; message: string } | null;
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
/** 最初からやり直す（隠しコマンド）。今のペットとお別れして、新しい卵を迎える。図鑑と思い出は残る。 */
export function restartGame(): Promise<StateView> {
  return invoke<StateView>("restart_game");
}

/** 電源ボタン。ペットを無理やり寝かせる（ご機嫌が上がるか下がるかはランダム）。寝ている間にもう一度押すと起こす。 */
export function togglePower(): Promise<StateView> {
  return invoke<StateView>("toggle_power");
}

/** 亡くなったペットを見送り、新しい卵を迎える。 */
export function buryPet(): Promise<StateView> {
  return invoke<StateView>("bury_pet");
}

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
  /** 鳴き声の大きさ（0〜100。50 が標準） */
  soundVolume: number;
  /** 鳴き声の高さ（-2〜2。0 が標準） */
  soundPitch: number;
  /** ログイン時にアプリを自動で起動する */
  launchAtLogin: boolean;
  /** 作業に寄り添う: Mac を操作している時間（操作の有無だけ）を、いっしょにいた時間として数える */
  companionEnabled: boolean;
  /** Mac の状態（電池・ネット・ダークモード）にペットが反応する */
  systemReactions: boolean;
  /** 長く休憩なしで作業しているとき、「ちょっと休もうよ」と声をかける */
  breakReminder: boolean;
  /** 操作する時間帯から生活リズムを学んで、おやすみ時間を自動で合わせる */
  autoSleep: boolean;
  /** きびしいモード: 病気や衰弱を放っておくと、お別れがある */
  strictMode: boolean;
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

/** 図鑑（全 113 種と発見済みの印）。 */
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
  | { kind: "eggReceived"; name: string | null; personality: Personality }
  | { kind: "dayLog"; month: number; day: number; feeds: number; snacks: number; plays: number; cleans: number; strokes: number; togetherMinutes: number; souvenirs: number }
  | { kind: "souvenir"; name: string; emoji: string }
  | { kind: "restarted"; name: string | null; personality: Personality }
  | { kind: "died"; name: string | null; personality: Personality }
  | { kind: "graduated"; name: string | null; personality: Personality };

export interface DiaryEntry {
  atUnix: number;
  event: DiaryEvent;
}

/** 家系図の 1 人。 */
export interface Ancestor {
  generation: number;
  name: string | null;
  personality: Personality;
  branch: Branch;
  tintDegrees: number;
  charm: string | null;
  /** 友だちのたまごコードの血筋か */
  fromFriend: boolean;
}

/** その日の出来事の数（きょうのページ）。 */
export interface DayStats {
  feeds: number;
  snacks: number;
  plays: number;
  cleans: number;
  strokes: number;
  /** いっしょに作業した時間（秒） */
  togetherSecs: number;
  souvenirs: number;
}

export interface DiaryView {
  /** いっしょに過ごした日数（生まれた日が 1 日目） */
  daysTogether: number;
  /** いっしょに作業した時間の合計（分） */
  togetherMinutes: number;
  /** 生活リズムから学んだ朝型・夜型（まだ学べていなければ null） */
  chronotype: Chronotype | null;
  /** 家系図（旅立った先祖。古い順） */
  lineage: Ancestor[];
  /** きょうの、ここまでの出来事 */
  today: DayStats;
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

export interface SouvenirEntry {
  id: string;
  name: string;
  emoji: string;
  rarity: number;
  count: number;
  displayed: boolean;
}

export interface SouvenirView {
  total: number;
  found: number;
  maxDisplayed: number;
  entries: SouvenirEntry[];
}

/** おみやげのコレクション。 */
export function getSouvenirs(): Promise<SouvenirView> {
  return invoke<SouvenirView>("get_souvenirs");
}

/** おみやげを飾る／しまう（飾れるのは `maxDisplayed` 個まで）。 */
export function toggleSouvenir(id: string): Promise<SouvenirView> {
  return invoke<SouvenirView>("toggle_souvenir", { id });
}

/** 持ち帰ったおみやげを見せた。 */
export function ackGift(): Promise<StateView> {
  return invoke<StateView>("ack_gift");
}

/** いまのペットの血筋の「たまごコード」（友だちに送れる）。 */
export function getEggCode(): Promise<string> {
  return invoke<string>("get_egg_code");
}

/** 友だちのたまごコードのたまごを迎える（いまのペットとはお別れ）。 */
export function receiveEgg(code: string): Promise<StateView> {
  return invoke<StateView>("receive_egg", { code });
}

/** 成長カード（PNG の data URL）をピクチャフォルダに保存する。保存した場所を返す。 */
export function saveCard(dataUrl: string): Promise<string> {
  return invoke<string>("save_card", { dataUrl });
}
