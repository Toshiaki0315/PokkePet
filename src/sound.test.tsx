import { fireEvent, renderHook } from "@testing-library/react";
import { act } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Personality } from "./api";
import { ContextKeeper, cryNotes, MASTER_GAIN, PITCH_STEP_SEMITONES, playNotes, useSound, WAKE_TIMEOUT_MS } from "./sound";
import type { CryKind, Voice } from "./sound";

const PERSONALITIES: Personality[] = ["calm", "playful", "gluttonous", "tidy"];
const KINDS: CryKind[] = ["happy", "neutral", "sad", "eat", "tap", "hatch"];
const voice = (personality: Personality, stage: number): Voice => ({ personality, stage });

describe("cryNotes", () => {
  it("gives every cry audible, short and well-formed notes", () => {
    for (const kind of KINDS) {
      for (const personality of PERSONALITIES) {
        for (const stage of [1, 2, 5, 10]) {
          const notes = cryNotes(kind, voice(personality, stage));
          expect(notes.length, `${kind}/${personality}/${stage}`).toBeGreaterThan(0);
          for (const n of notes) {
            expect(n.duration).toBeGreaterThan(0);
            expect(n.start).toBeGreaterThanOrEqual(0);
            expect(n.from).toBeGreaterThan(20);
            expect(n.to).toBeGreaterThan(20);
            expect(Math.max(n.from, n.to)).toBeLessThan(4000);
            expect(n.gain).toBeGreaterThan(0);
            expect(n.gain).toBeLessThanOrEqual(1);
          }
          const end = Math.max(...notes.map((n) => n.start + n.duration));
          expect(end).toBeLessThan(1);
        }
      }
    }
  });

  it("makes a happy cry out of three rising notes", () => {
    const notes = cryNotes("happy", voice("calm", 5));
    expect(notes).toHaveLength(3);
    for (const n of notes) expect(n.to).toBeGreaterThan(n.from);
    expect(notes[1].from).toBeGreaterThan(notes[0].from);
    expect(notes[2].from).toBeGreaterThan(notes[1].from);
    expect(notes[1].start).toBeGreaterThan(notes[0].start);
  });

  it("makes a sad cry out of falling notes", () => {
    const notes = cryNotes("sad", voice("playful", 5));
    expect(notes.length).toBeGreaterThanOrEqual(2);
    for (const n of notes) expect(n.to).toBeLessThan(n.from);
  });

  it("makes an ordinary cry out of a single gentle rise", () => {
    const notes = cryNotes("neutral", voice("tidy", 5));
    expect(notes).toHaveLength(1);
    expect(notes[0].to).toBeGreaterThan(notes[0].from);
  });

  it("makes eating sound like quick low munches", () => {
    const notes = cryNotes("eat", voice("gluttonous", 5));
    expect(notes).toHaveLength(3);
    for (const n of notes) expect(n.duration).toBeLessThan(0.1);
    expect(notes[0].from).toBeLessThan(cryNotes("neutral", voice("gluttonous", 5))[0].from);
  });

  it("lowers the voice as the pet grows", () => {
    for (const personality of PERSONALITIES) {
      const small = cryNotes("neutral", voice(personality, 2))[0].from;
      const mid = cryNotes("neutral", voice(personality, 6))[0].from;
      const big = cryNotes("neutral", voice(personality, 10))[0].from;
      expect(small).toBeGreaterThan(mid);
      expect(mid).toBeGreaterThan(big);
    }
  });

  it("gives each species its own voice", () => {
    const first = (p: Personality) => cryNotes("neutral", voice(p, 5))[0];
    expect(first("playful").from).toBeGreaterThan(first("tidy").from);
    expect(first("tidy").from).toBeGreaterThan(first("calm").from);
    expect(first("calm").from).toBeGreaterThan(first("gluttonous").from);
    expect(new Set(PERSONALITIES.map((p) => first(p).type)).size).toBe(4);
  });

  it("lets an egg only tap, whatever it is asked to do", () => {
    const reference = cryNotes("tap", voice("calm", 1));
    for (const kind of ["happy", "neutral", "sad", "eat", "tap"] as CryKind[]) {
      for (const personality of PERSONALITIES) expect(cryNotes(kind, voice(personality, 1))).toEqual(reference);
    }
    expect(reference).toHaveLength(2);
    for (const n of reference) expect(n.duration).toBeLessThan(0.06);
  });

  it("still lets the newborn cheer when it hatches, even if asked with the egg's stage", () => {
    const notes = cryNotes("hatch", voice("calm", 1));
    expect(notes.length).toBeGreaterThan(1);
    expect(notes[0].to).toBeGreaterThan(notes[0].from * 1.5); // 低い声から高い声へのびる
  });

  it("does not depend on anything but its arguments", () => {
    expect(cryNotes("happy", voice("tidy", 7))).toEqual(cryNotes("happy", voice("tidy", 7)));
  });
});

class FakeParam {
  calls: Array<[string, number, number?]> = [];
  setValueAtTime = vi.fn((v: number, t: number) => this.calls.push(["set", v, t]));
  exponentialRampToValueAtTime = vi.fn((v: number, t: number) => this.calls.push(["ramp", v, t]));
}
class FakeOscillator {
  type = "sine";
  frequency = new FakeParam();
  connect = vi.fn();
  start = vi.fn();
  stop = vi.fn();
}
class FakeGain {
  gain = new FakeParam();
  connect = vi.fn();
}
class FakeContext {
  static instances: FakeContext[] = [];
  currentTime = 10;
  state: "running" | "suspended" | "interrupted" | "closed" = "running";
  onstatechange: (() => void) | null = null;
  destination = { id: "destination" };
  oscillators: FakeOscillator[] = [];
  gains: FakeGain[] = [];
  resume = vi.fn(() => Promise.resolve());
  close = vi.fn(() => {
    this.state = "closed";
    return Promise.resolve();
  });
  constructor() {
    FakeContext.instances.push(this);
  }
  createOscillator() {
    const o = new FakeOscillator();
    this.oscillators.push(o);
    return o;
  }
  createGain() {
    const g = new FakeGain();
    this.gains.push(g);
    return g;
  }
}

describe("playNotes", () => {
  it("builds an oscillator and an envelope for every note", () => {
    const ctx = new FakeContext();
    const notes = cryNotes("happy", voice("calm", 5));
    playNotes(ctx as never, notes);
    expect(ctx.oscillators).toHaveLength(3);
    expect(ctx.gains).toHaveLength(3);
    ctx.oscillators.forEach((osc, i) => {
      expect(osc.type).toBe(notes[i].type);
      expect(osc.connect).toHaveBeenCalledWith(ctx.gains[i]);
      expect(ctx.gains[i].connect).toHaveBeenCalledWith(ctx.destination);
      const startAt = osc.start.mock.calls[0][0];
      expect(startAt).toBeGreaterThanOrEqual(ctx.currentTime);
      expect(osc.stop.mock.calls[0][0]).toBeGreaterThan(startAt + notes[i].duration);
      expect(osc.frequency.setValueAtTime).toHaveBeenCalledWith(notes[i].from, startAt);
      expect(osc.frequency.exponentialRampToValueAtTime).toHaveBeenCalledWith(notes[i].to, startAt + notes[i].duration);
    });
  });

  it("fades each note in and out so that it never clicks, and stays quiet", () => {
    const ctx = new FakeContext();
    playNotes(ctx as never, cryNotes("neutral", voice("playful", 5)));
    const g = ctx.gains[0].gain;
    const peak = Math.max(...g.calls.map((c) => c[1]));
    expect(peak).toBeLessThanOrEqual(MASTER_GAIN);
    expect(g.calls[0][1]).toBeLessThan(0.001); // 無音から始まる
    expect(g.calls[g.calls.length - 1][1]).toBeLessThan(0.001); // 無音で終わる
  });

  it("schedules later notes later", () => {
    const ctx = new FakeContext();
    playNotes(ctx as never, cryNotes("happy", voice("calm", 5)));
    const starts = ctx.oscillators.map((o) => o.start.mock.calls[0][0] as number);
    expect(starts[1]).toBeGreaterThan(starts[0]);
    expect(starts[2]).toBeGreaterThan(starts[1]);
  });
});

describe("tuning", () => {
  it("raising the pitch by one step multiplies every frequency by the same ratio", () => {
    const base = cryNotes("happy", voice("calm", 5));
    const up = cryNotes("happy", voice("calm", 5), 1);
    const ratio = 2 ** (PITCH_STEP_SEMITONES / 12);
    up.forEach((n, i) => {
      expect(n.from).toBeCloseTo(base[i].from * ratio, 6);
      expect(n.to).toBeCloseTo(base[i].to * ratio, 6);
      expect(n.start).toBe(base[i].start);
    });
  });

  it("lowering the pitch makes the voice lower, and 0 changes nothing", () => {
    const base = cryNotes("sad", voice("tidy", 4));
    expect(cryNotes("sad", voice("tidy", 4), 0)).toEqual(base);
    expect(cryNotes("sad", voice("tidy", 4), -2)[0].from).toBeLessThan(base[0].from);
  });

  it("the volume scales the loudness: 100 is twice the standard, 0 is silent", () => {
    const peakAt = (volume: number) => {
      const ctx = new FakeContext();
      playNotes(ctx as never, cryNotes("neutral", voice("playful", 5)), volume);
      return ctx.gains.length === 0 ? 0 : Math.max(...ctx.gains[0].gain.calls.map((c) => c[1]));
    };
    expect(peakAt(50)).toBeCloseTo(MASTER_GAIN * 0.9, 6);
    expect(peakAt(100)).toBeCloseTo(MASTER_GAIN * 0.9 * 2, 6);
    expect(peakAt(25)).toBeCloseTo(MASTER_GAIN * 0.9 * 0.5, 6);
    expect(peakAt(0)).toBe(0);
  });

  it("useSound plays with the given volume and pitch", () => {
    FakeContext.instances = [];
    vi.stubGlobal("AudioContext", FakeContext);
    const { result } = renderHook(() => useSound(true, { volume: 100, pitch: 1 }));
    result.current("neutral", voice("calm", 5));
    const [ctx] = FakeContext.instances;
    const expected = cryNotes("neutral", voice("calm", 5), 1)[0].from;
    expect(ctx.oscillators[0].frequency.setValueAtTime.mock.calls[0][0]).toBeCloseTo(expected, 6);
    expect(Math.max(...ctx.gains[0].gain.calls.map((c) => c[1]))).toBeCloseTo(MASTER_GAIN * 2, 6);
    vi.unstubAllGlobals();
  });
});

describe("useSound", () => {
  beforeEach(() => {
    FakeContext.instances = [];
    vi.stubGlobal("AudioContext", FakeContext);
  });
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("plays a cry through a lazily created audio context", () => {
    const { result } = renderHook(() => useSound(true));
    expect(FakeContext.instances).toHaveLength(0);
    result.current("happy", voice("calm", 5));
    expect(FakeContext.instances).toHaveLength(1);
    expect(FakeContext.instances[0].oscillators).toHaveLength(3);
  });

  it("reuses the same audio context for every cry", () => {
    const { result } = renderHook(() => useSound(true));
    result.current("happy", voice("calm", 5));
    result.current("sad", voice("calm", 5));
    expect(FakeContext.instances).toHaveLength(1);
    expect(FakeContext.instances[0].oscillators.length).toBeGreaterThan(3);
  });

  it("makes no sound and creates no audio context while disabled", () => {
    const { result } = renderHook(() => useSound(false));
    result.current("happy", voice("calm", 5));
    expect(FakeContext.instances).toHaveLength(0);
  });

  it("stops making sound as soon as it is switched off", () => {
    const { result, rerender } = renderHook(({ on }) => useSound(on), { initialProps: { on: true } });
    result.current("neutral", voice("calm", 5));
    const count = FakeContext.instances[0].oscillators.length;
    rerender({ on: false });
    result.current("neutral", voice("calm", 5));
    expect(FakeContext.instances[0].oscillators).toHaveLength(count);
  });

  it("wakes a suspended audio context before playing", () => {
    class Suspended extends FakeContext {
      state = "suspended" as const;
    }
    vi.stubGlobal("AudioContext", Suspended);
    const { result } = renderHook(() => useSound(true));
    result.current("neutral", voice("calm", 5));
    expect(FakeContext.instances[0].resume).toHaveBeenCalled();
  });

  it("falls back to the prefixed WebKit audio context", () => {
    vi.stubGlobal("AudioContext", undefined);
    vi.stubGlobal("webkitAudioContext", FakeContext);
    const { result } = renderHook(() => useSound(true));
    result.current("neutral", voice("calm", 5));
    expect(FakeContext.instances).toHaveLength(1);
  });

  it("is silent, and does not throw, when there is no Web Audio at all", () => {
    vi.stubGlobal("AudioContext", undefined);
    vi.stubGlobal("webkitAudioContext", undefined);
    const { result } = renderHook(() => useSound(true));
    expect(() => result.current("happy", voice("calm", 5))).not.toThrow();
  });

  it("is silent, and does not throw, when the audio context cannot be created", () => {
    vi.stubGlobal("AudioContext", function Broken() {
      throw new Error("blocked");
    });
    const { result } = renderHook(() => useSound(true));
    expect(() => result.current("happy", voice("calm", 5))).not.toThrow();
  });

  it("does not let a failure while playing escape", () => {
    class Failing extends FakeContext {
      createOscillator(): never {
        throw new Error("boom");
      }
    }
    vi.stubGlobal("AudioContext", Failing);
    const { result } = renderHook(() => useSound(true));
    expect(() => result.current("happy", voice("calm", 5))).not.toThrow();
  });

  it("prepares the audio context at the first user gesture, so that later cries are allowed", () => {
    class Suspended extends FakeContext {
      state = "suspended" as const;
    }
    vi.stubGlobal("AudioContext", Suspended);
    renderHook(() => useSound(true));
    expect(FakeContext.instances).toHaveLength(0);
    fireEvent.pointerDown(window);
    expect(FakeContext.instances).toHaveLength(1);
    expect(FakeContext.instances[0].resume).toHaveBeenCalled();
    expect(FakeContext.instances[0].oscillators).toHaveLength(0); // 準備だけで、音は出さない
  });

  it("reuses that prepared context for the cries", () => {
    const { result } = renderHook(() => useSound(true));
    fireEvent.pointerDown(window);
    result.current("neutral", voice("calm", 5));
    expect(FakeContext.instances).toHaveLength(1);
    expect(FakeContext.instances[0].oscillators).toHaveLength(1);
  });

  it("also prepares it at a key press", () => {
    renderHook(() => useSound(true));
    fireEvent.keyDown(window, { key: "ArrowUp" });
    expect(FakeContext.instances).toHaveLength(1);
  });

  it("does not prepare anything while the sound is off", () => {
    renderHook(() => useSound(false));
    fireEvent.pointerDown(window);
    expect(FakeContext.instances).toHaveLength(0);
  });

  it("stops listening for gestures when unmounted", () => {
    const { unmount } = renderHook(() => useSound(true));
    unmount();
    fireEvent.pointerDown(window);
    expect(FakeContext.instances).toHaveLength(0);
  });

  it("survives a gesture in an environment without Web Audio", () => {
    vi.stubGlobal("AudioContext", undefined);
    vi.stubGlobal("webkitAudioContext", undefined);
    renderHook(() => useSound(true));
    expect(() => fireEvent.pointerDown(window)).not.toThrow();
  });
});

describe("after a long idle time (the audio context may have stopped)", () => {
  beforeEach(() => {
    FakeContext.instances = [];
  });
  afterEach(() => {
    vi.unstubAllGlobals();
    vi.useRealTimers();
  });

  /** 止まった状態で作られ、`resume` で動く AudioContext。 */
  const sleepyThatWakes = (state: "suspended" | "interrupted") =>
    class extends FakeContext {
      constructor() {
        super();
        this.state = state;
        this.resume = vi.fn(() => {
          this.state = "running";
          return Promise.resolve();
        });
      }
    };

  it.each(["suspended", "interrupted"] as const)("wakes a %s context and then plays the cry (instead of losing it)", async (state) => {
    vi.stubGlobal("AudioContext", sleepyThatWakes(state));
    const { result } = renderHook(() => useSound(true));
    result.current("happy", voice("calm", 5));
    expect(FakeContext.instances[0].oscillators).toHaveLength(0); // まだ動いていない
    await act(async () => {});
    expect(FakeContext.instances[0].resume).toHaveBeenCalled();
    expect(FakeContext.instances[0].oscillators).toHaveLength(3);
  });

  it("gives up on a context that will not wake, throws it away, and the next cry uses a fresh one", async () => {
    class Stuck extends FakeContext {
      constructor() {
        super();
        this.state = "interrupted";
      }
    }
    vi.stubGlobal("AudioContext", Stuck);
    const { result } = renderHook(() => useSound(true));
    result.current("happy", voice("calm", 5));
    await act(async () => {});
    expect(FakeContext.instances[0].oscillators).toHaveLength(0);
    expect(FakeContext.instances[0].close).toHaveBeenCalled();
    vi.stubGlobal("AudioContext", FakeContext); // 今度は、ふつうに動く
    result.current("happy", voice("calm", 5));
    expect(FakeContext.instances).toHaveLength(2);
    expect(FakeContext.instances[1].oscillators).toHaveLength(3);
  });

  it("does not wait for ever when resuming never answers", async () => {
    vi.useFakeTimers();
    class Silent extends FakeContext {
      constructor() {
        super();
        this.state = "suspended";
        this.resume = vi.fn(() => new Promise<void>(() => {}));
      }
    }
    vi.stubGlobal("AudioContext", Silent);
    const { result } = renderHook(() => useSound(true));
    result.current("happy", voice("calm", 5));
    await act(async () => {
      vi.advanceTimersByTime(WAKE_TIMEOUT_MS + 1);
    });
    expect(FakeContext.instances[0].close).toHaveBeenCalled();
    expect(FakeContext.instances[0].oscillators).toHaveLength(0);
  });

  it("survives a context whose resume() throws", async () => {
    class Refusing extends FakeContext {
      constructor() {
        super();
        this.state = "suspended";
        this.resume = vi.fn(() => Promise.reject(new Error("not allowed")));
      }
    }
    vi.stubGlobal("AudioContext", Refusing);
    const { result } = renderHook(() => useSound(true));
    expect(() => result.current("happy", voice("calm", 5))).not.toThrow();
    await act(async () => {});
    expect(FakeContext.instances[0].close).toHaveBeenCalled();
  });

  it("does not let a failure while playing after waking escape", async () => {
    class WakesButFails extends FakeContext {
      constructor() {
        super();
        this.state = "suspended";
        this.resume = vi.fn(() => {
          this.state = "running";
          return Promise.resolve();
        });
      }
      createOscillator(): never {
        throw new Error("boom");
      }
    }
    vi.stubGlobal("AudioContext", WakesButFails);
    const { result } = renderHook(() => useSound(true));
    result.current("happy", voice("calm", 5));
    await expect(act(async () => {})).resolves.not.toThrow();
  });

  it("replaces a context that was closed behind its back", () => {
    vi.stubGlobal("AudioContext", FakeContext);
    const { result } = renderHook(() => useSound(true));
    result.current("happy", voice("calm", 5));
    FakeContext.instances[0].state = "closed";
    result.current("happy", voice("calm", 5));
    expect(FakeContext.instances).toHaveLength(2);
    expect(FakeContext.instances[1].oscillators).toHaveLength(3);
  });

  it("forgets a context that reports being closed", () => {
    vi.stubGlobal("AudioContext", FakeContext);
    const { result } = renderHook(() => useSound(true));
    result.current("happy", voice("calm", 5));
    const first = FakeContext.instances[0];
    first.state = "closed";
    first.onstatechange?.();
    result.current("happy", voice("calm", 5));
    expect(FakeContext.instances).toHaveLength(2);
  });

  it("keeps waking the context at every click or key press (not only the first one)", async () => {
    vi.stubGlobal("AudioContext", sleepyThatWakes("suspended"));
    renderHook(() => useSound(true));
    fireEvent.pointerDown(window);
    await act(async () => {});
    expect(FakeContext.instances).toHaveLength(1);
    FakeContext.instances[0].state = "suspended"; // 放っておいたら、また止まった
    fireEvent.keyDown(window, { key: "ArrowUp" });
    await act(async () => {});
    expect(FakeContext.instances[0].resume).toHaveBeenCalledTimes(2);
    expect(FakeContext.instances[0].state).toBe("running");
  });

  it("wakes an existing context when the window comes to the front, but never creates one", async () => {
    vi.stubGlobal("AudioContext", sleepyThatWakes("suspended"));
    const { result } = renderHook(() => useSound(true));
    fireEvent.focus(window);
    document.dispatchEvent(new Event("visibilitychange"));
    expect(FakeContext.instances).toHaveLength(0);
    FakeContext.instances.length = 0;
    result.current("happy", voice("calm", 5));
    await act(async () => {});
    FakeContext.instances[0].state = "interrupted";
    fireEvent.focus(window);
    await act(async () => {});
    expect(FakeContext.instances[0].state).toBe("running");
    FakeContext.instances[0].state = "suspended";
    document.dispatchEvent(new Event("visibilitychange"));
    await act(async () => {});
    expect(FakeContext.instances[0].state).toBe("running");
  });

  it("stops listening for focus and visibility changes when unmounted or switched off", () => {
    vi.stubGlobal("AudioContext", FakeContext);
    const { unmount } = renderHook(() => useSound(true));
    unmount();
    fireEvent.focus(window);
    document.dispatchEvent(new Event("visibilitychange"));
    expect(FakeContext.instances).toHaveLength(0);
  });
});

describe("ContextKeeper", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("has nothing until asked, and then keeps the one it made", () => {
    FakeContext.instances = [];
    vi.stubGlobal("AudioContext", FakeContext);
    const keeper = new ContextKeeper();
    expect(keeper.peek()).toBeNull();
    const ctx = keeper.get();
    expect(keeper.get()).toBe(ctx);
    expect(keeper.peek()).toBe(ctx);
  });

  it("says a running context is awake without touching it", async () => {
    vi.stubGlobal("AudioContext", FakeContext);
    const keeper = new ContextKeeper();
    const ctx = keeper.get() as unknown as FakeContext;
    await expect(keeper.wake(ctx as unknown as AudioContext)).resolves.toBe(true);
    expect(ctx.resume).not.toHaveBeenCalled();
  });

  it("keeps a context that woke up while it was being waited for", async () => {
    vi.stubGlobal("AudioContext", FakeContext);
    const keeper = new ContextKeeper();
    const ctx = keeper.get() as unknown as FakeContext;
    ctx.state = "suspended";
    ctx.resume = vi.fn(() => {
      ctx.state = "running";
      return Promise.resolve();
    });
    await expect(keeper.wake(ctx as unknown as AudioContext)).resolves.toBe(true);
    expect(keeper.peek()).toBe(ctx);
  });

  it("copes with a context that cannot even be closed", async () => {
    vi.stubGlobal("AudioContext", FakeContext);
    const keeper = new ContextKeeper();
    const ctx = keeper.get() as unknown as FakeContext;
    ctx.state = "suspended";
    ctx.close = vi.fn(() => {
      throw new Error("already closed");
    });
    await expect(keeper.wake(ctx as unknown as AudioContext)).resolves.toBe(false);
    expect(keeper.peek()).toBeNull();
    const other = new ContextKeeper();
    const ctx2 = other.get() as unknown as FakeContext;
    ctx2.state = "suspended";
    ctx2.close = vi.fn(() => Promise.reject(new Error("nope")));
    await expect(other.wake(ctx2 as unknown as AudioContext)).resolves.toBe(false);
  });
});
