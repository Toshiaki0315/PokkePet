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

/** 設定の音量（0〜100）の標準値。このとき音量は MASTER_GAIN そのもの。 */
export const STANDARD_VOLUME = 50;
/** 設定の高さ 1 段ぶん（半音いくつか）。 */
export const PITCH_STEP_SEMITONES = 3;

/** 鳴き声の調整（設定ダイアログで変えられる）。 */
export interface Tuning {
  /** 0〜100。0 は鳴らさない。 */
  volume: number;
  /** -2〜2 段。 */
  pitch: number;
}

export const STANDARD_TUNING: Tuning = { volume: STANDARD_VOLUME, pitch: 0 };

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
export function cryNotes(kind: CryKind, { personality, stage }: Voice, pitch = 0): Note[] {
  const notes = baseNotes(kind, { personality, stage });
  if (pitch === 0) return notes;
  const ratio = 2 ** ((pitch * PITCH_STEP_SEMITONES) / 12);
  return notes.map((n) => ({ ...n, from: n.from * ratio, to: n.to * ratio }));
}

function baseNotes(kind: CryKind, { personality, stage }: Voice): Note[] {
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
export function playNotes(ctx: AudioContext, notes: readonly Note[], volume = STANDARD_VOLUME): void {
  if (volume <= 0) return;
  const level = MASTER_GAIN * (volume / STANDARD_VOLUME);
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
    amp.gain.exponentialRampToValueAtTime(level * n.gain, t0 + Math.min(0.01, n.duration / 3));
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

/** 止まっている AudioContext を動かすのを待つ上限（ミリ秒）。これを過ぎても動かなければ、あきらめる。 */
export const WAKE_TIMEOUT_MS = 800;

/**
 * AudioContext の持ち主。ずっと放っておくと、WebView の AudioContext は止まったり（suspended）、
 * 割り込まれたり（interrupted。スリープ明け・音の出力先が変わったとき）、閉じられたり（closed）して、
 * そのまま動かなくなることがある。そこで、鳴らす前に必ず動かしてから鳴らし、動かない・応答がないものは捨てて、
 * 次の機会に作り直す。
 */
export class ContextKeeper {
  private ctx: AudioContext | null = null;

  /** いま持っている AudioContext（作らない）。 */
  peek(): AudioContext | null {
    return this.ctx;
  }

  /** 使える AudioContext を返す（無ければ作る。閉じられていたら作り直す）。Web Audio が無い環境では null。 */
  get(): AudioContext | null {
    const Ctor = audioContextCtor();
    if (!Ctor) return null;
    if (this.ctx?.state === "closed") this.ctx = null;
    if (!this.ctx) {
      const ctx = new Ctor();
      ctx.onstatechange = () => {
        if (ctx.state === "closed" && this.ctx === ctx) this.ctx = null;
      };
      this.ctx = ctx;
    }
    return this.ctx;
  }

  /** 動かす。動いたら true。動かない・応答がないときは、その AudioContext を捨てて false。 */
  async wake(ctx: AudioContext): Promise<boolean> {
    if (ctx.state === "running") return true;
    try {
      await Promise.race([ctx.resume(), new Promise((resolve) => setTimeout(resolve, WAKE_TIMEOUT_MS))]);
    } catch {
      // 動かせなかった。下で、捨てる。
    }
    if ((ctx.state as AudioContextState) === "running") return true; // 待っている間に、動いた
    this.discard(ctx);
    return false;
  }

  private discard(ctx: AudioContext) {
    if (this.ctx === ctx) this.ctx = null;
    try {
      void ctx.close().catch(() => undefined);
    } catch {
      // すでに閉じている。
    }
  }
}

/**
 * 鳴き声を鳴らす関数を返す。`enabled` が false の間は何もしない（AudioContext も作らない）。
 * 音が出せない環境（Web Audio が無い・自動再生が拒否された）でも例外は外へ出さず、静かに無視する。
 *
 * WebView は、ユーザーが操作していないページの音を許可しないことがあり、放っておくと音の出口が止まることもある。
 * そこで、クリック・キー入力・ウィンドウが前に来たとき（開いたとき）に、AudioContext を動かしておく
 * （音は出さない）。鳴らすときも、止まっていたら動かしてから鳴らす。
 */
export function useSound(enabled: boolean, tuning: Tuning = STANDARD_TUNING): (kind: CryKind, voice: Voice) => void {
  const { volume, pitch } = tuning;
  const keeper = useRef<ContextKeeper | null>(null);
  keeper.current ??= new ContextKeeper();

  useEffect(() => {
    if (!enabled) return;
    const k = keeper.current as ContextKeeper;
    // 操作のとき: 作って、動かす。
    const unlock = () => {
      try {
        const ctx = k.get();
        if (ctx) void k.wake(ctx);
      } catch {
        // 準備できなくても、鳴らすときにもう一度試す。
      }
    };
    // ウィンドウが前に来たとき: すでにあるものだけ、動かす（作らない）。
    const refresh = () => {
      const ctx = k.peek();
      if (ctx) void k.wake(ctx);
    };
    window.addEventListener("pointerdown", unlock);
    window.addEventListener("keydown", unlock);
    window.addEventListener("focus", refresh);
    document.addEventListener("visibilitychange", refresh);
    return () => {
      window.removeEventListener("pointerdown", unlock);
      window.removeEventListener("keydown", unlock);
      window.removeEventListener("focus", refresh);
      document.removeEventListener("visibilitychange", refresh);
    };
  }, [enabled]);

  return useCallback(
    (kind, voice) => {
      if (!enabled) return;
      const k = keeper.current as ContextKeeper;
      try {
        const ctx = k.get();
        if (!ctx) return;
        const notes = cryNotes(kind, voice, pitch);
        if (ctx.state === "running") {
          playNotes(ctx, notes, volume);
          return;
        }
        // 止まっていた（長く放っておいたあとなど）。動かしてから鳴らす。
        void k.wake(ctx).then((ok) => {
          if (!ok) return;
          try {
            playNotes(ctx, notes, volume);
          } catch {
            // 音が鳴らなくてもゲームの動作には影響しない。
          }
        });
      } catch {
        // 音が鳴らなくてもゲームの動作には影響しない。
      }
    },
    [enabled, volume, pitch],
  );
}
