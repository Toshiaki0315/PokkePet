import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export type Branch = "poor" | "normal" | "good";
export type Personality = "calm" | "playful" | "gluttonous" | "tidy";
export type ActionName = "feed" | "play" | "clean";

export interface StateView {
  mood: number;
  affinity: number;
  hunger: number;
  dirtiness: number;
  stage: number;
  branch: Branch;
  personality: Personality;
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
