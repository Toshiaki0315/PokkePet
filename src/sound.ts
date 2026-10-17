import { useCallback, useEffect, useRef } from "react";
import type { Personality } from "./api";

/** 鳴き声の種類。 */
export type CryKind = "happy" | "neutral" | "sad" | "eat" | "tap" | "hatch";

/** 声のもと: 性格（種族）で高さと音色が、段階（大きさ）で高さが変わる。 */
export interface Voice {
  personality: Personality;
  stage: number;
}

/** 1 つの音。周波数は `from` から `to` へなめらかに動く。時刻は鳴らし始めからの秒。 */
export interface Note {
  type: OscillatorType;
  from: number;
  to: number;
  start: number;
  duration: number;
  /** 音色ごとの音量の補正（0〜1）。 */
  gain: number;
}

/** 全体の音量。小さめにして、作業中に鳴ってもうるさくならないようにする。 */
export const MASTER_GAIN = 0.18;

// 種族ごとの基本の高さ(Hz)と音色。
const SPECIES: Record<Personality, { base: number; type: OscillatorType; gain: number }> = {
  calm: { base: 520, type: "sine", gain: 1 },
  playful: { base: 760, type: "triangle", gain: 0.9 },
  gluttonous: { base: 380, type: "sawtooth", gain: 0.45 },
  tidy: { base: 640, type: "square", gain: 0.3 },
};

const note = (type: OscillatorType, from: number, to: number, start: number, duration: number, gain: number): Note => ({ type, from, to, start, duration, gain });

/** 卵のコツコツ（2 回の短いクリック）。 */
const taps = (): Note[] => [note("square", 900, 700, 0, 0.03, 0.5), note("square", 900, 700, 0.12, 0.03, 0.5)];

/**
 * 鳴き声の音符を作る。純粋な関数（乱数や時刻は使わない）。
 * 段階が大きいほど低く、卵はコツコツ（孵化の鳴き声だけは例外）。
 */
export function cryNotes(kind: CryKind, { personality, stage }: Voice): Note[] {
  if (stage <= 1 && kind !== "hatch") return taps();
  const { base, type, gain } = SPECIES[personality];
  const f = base / (1 + (Math.max(stage, 2) - 2) * 0.05);
  switch (kind) {
    case "happy":
      return [note(type, f, f * 1.25, 0, 0.09, gain), note(type, f * 1.25, f * 1.5, 0.09, 0.09, gain), note(type, f * 1.5, f * 1.9, 0.18, 0.12, gain)];
    case "sad":
      return [note(type, f * 1.1, f * 0.9, 0, 0.28, gain), note(type, f * 0.9, f * 0.7, 0.26, 0.34, gain)];
    case "eat":
      return [0, 0.1, 0.2].map((start) => note("square", f * 0.55, f * 0.5, start, 0.05, Math.min(gain, 0.4)));
    case "tap":
      return taps();
    case "hatch":
      return [note(type, f * 0.7, f * 1.8, 0, 0.35, gain), note("sine", f * 2, f * 2.4, 0.3, 0.12, 0.6)];
    case "neutral":
      return [note(type, f, f * 1.12, 0, 0.16, gain)];
  }
}

/** 音符を Web Audio で鳴らす。各音は無音から立ち上がり無音で終わる（プチッという音を出さない）。 */
export function playNotes(ctx: AudioContext, notes: readonly Note[]): void {
  const origin = ctx.currentTime + 0.02;
  for (const n of notes) {
    const t0 = origin + n.start;
    const t1 = t0 + n.duration;
    const osc = ctx.createOscillator();
    const amp = ctx.createGain();
    osc.type = n.type;
    osc.frequency.setValueAtTime(n.from, t0);
    osc.frequency.exponentialRampToValueAtTime(n.to, t1);
    amp.gain.setValueAtTime(0.0001, t0);
    amp.gain.exponentialRampToValueAtTime(MASTER_GAIN * n.gain, t0 + Math.min(0.01, n.duration / 3));
    amp.gain.exponentialRampToValueAtTime(0.0001, t1);
    osc.connect(amp);
    amp.connect(ctx.destination);
    osc.start(t0);
    osc.stop(t1 + 0.02);
  }
}

type AudioContextCtor = new () => AudioContext;

/** この環境の AudioContext（無ければ null）。 */
function audioContextCtor(): AudioContextCtor | null {
  const g = globalThis as unknown as { AudioContext?: AudioContextCtor; webkitAudioContext?: AudioContextCtor };
  return g.AudioContext ?? g.webkitAudioContext ?? null;
}

/**
 * 鳴き声を鳴らす関数を返す。`enabled` が false の間は何もしない（AudioContext も作らない）。
 * 音が出せない環境（Web Audio が無い・自動再生が拒否された）でも例外は外へ出さず、静かに無視する。
 *
 * WebView は、ユーザーが一度も操作していないページの音を許可しないことがある。そこで、最初のクリックや
 * キー入力のときに AudioContext を作って有効にしておく（音は出さない）。
 */
export function useSound(enabled: boolean): (kind: CryKind, voice: Voice) => void {
  const context = useRef<AudioContext | null>(null);

  /** AudioContext を用意して、止まっていれば動かす。使えない環境では null。 */
  const ensureContext = useCallback((): AudioContext | null => {
    const Ctor = audioContextCtor();
    if (!Ctor) return null;
    context.current ??= new Ctor();
    if (context.current.state === "suspended") void context.current.resume();
    return context.current;
  }, []);

  useEffect(() => {
    if (!enabled) return;
    const unlock = () => {
      try {
        ensureContext();
      } catch {
        // 準備できなくても、鳴らすときにもう一度試す。
      }
    };
    window.addEventListener("pointerdown", unlock, { once: true });
    window.addEventListener("keydown", unlock, { once: true });
    return () => {
      window.removeEventListener("pointerdown", unlock);
      window.removeEventListener("keydown", unlock);
    };
  }, [enabled, ensureContext]);

  return useCallback(
    (kind, voice) => {
      if (!enabled) return;
      try {
        const ctx = ensureContext();
        if (ctx) playNotes(ctx, cryNotes(kind, voice));
      } catch {
        // 音が鳴らなくてもゲームの動作には影響しない。
      }
    },
    [enabled, ensureContext],
  );
}
