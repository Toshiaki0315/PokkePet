import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export type Branch = "poor" | "normal" | "good";
export type Personality = "calm" | "playful" | "gluttonous" | "tidy";
export type Level = "low" | "mid" | "high";
export type BubbleKind = "hungry" | "dirty" | "gloomy" | "happy";
export type PlayAction = "play" | "pet";
export type ActionName = "feed" | "play" | "clean";

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

export function onStateChanged(handler: (view: StateView) => void): Promise<() => void> {
  return listen<StateView>("state-changed", (event) => handler(event.payload));
}

export interface Settings {
  showStage: boolean;
  showGauges: boolean;
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
