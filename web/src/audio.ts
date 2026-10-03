// Sound system: maps logical game events to files under `snd/` and plays them.
//
// The file list is fetched live from the map-server's `/sounds` endpoint, so the
// user can drop new files into `snd/` and hit "Reload" in the Sound menu tab.
// Assignment (event -> file) lives in Settings and is persisted with the rest.
//
// Playback uses the Web Audio API: each one-shot is a fresh `AudioBufferSource`
// fed from a decoded+cached `AudioBuffer`. That avoids the `<audio>` element
// state/autoplay pitfalls and makes overlapping sounds (footsteps + jump)
// reliable. The `AudioContext` is created/resumed on the first user gesture.

import { api } from "./base";

export interface SoundEventDef {
  id: string;
  label: string;
}

/**
 * Every assignable sound effect. The first block is emitted by the current sim;
 * the weapon/pickup ones are placeholders until those systems land, but stay
 * configurable so files can be auditioned now.
 */
export const SOUND_EVENTS: SoundEventDef[] = [
  { id: "jump", label: "Jump" },
  { id: "dash", label: "Dash" },
  { id: "walljump", label: "Wall dash / walljump" },
  { id: "land", label: "Land" },
  { id: "footstep", label: "Footstep" },
  { id: "jumppad", label: "Jump pad" },
  { id: "teleport", label: "Teleport" },
  { id: "rocket_fire", label: "Rocket — fire" },
  { id: "rocket_explode", label: "Rocket — explode" },
  { id: "plasma_fire", label: "Plasma — fire" },
  { id: "plasma_explode", label: "Plasma — explode" },
  { id: "grenade_fire", label: "Grenade — fire" },
  { id: "grenade_explode", label: "Grenade — explode" },
  { id: "lightning_fire", label: "Lightning — fire" },
  { id: "hit", label: "Hit" },
  { id: "pain", label: "Pain" },
  { id: "death", label: "Death" },
  { id: "checkpoint", label: "Checkpoint" },
  { id: "finish", label: "Finish" },
];

/**
 * Gameplay event bits -> sound event ids. Must stay in sync with the `EV_*`
 * constants in `core/src/lib.rs`.
 */
const EVENT_BITS: Array<[string, number]> = [
  ["jump", 1 << 0],
  ["dash", 1 << 1],
  ["walljump", 1 << 2],
  ["land", 1 << 3],
  ["footstep", 1 << 4],
  ["jumppad", 1 << 5],
  ["teleport", 1 << 6],
];

export class SoundManager {
  private assignments: Record<string, string> = {};
  private volume = 1.0;
  private ctx: AudioContext | null = null;
  private master: GainNode | null = null;
  /** Decoded buffers, cached by file name. */
  private buffers = new Map<string, AudioBuffer>();
  /** In-flight decodes (so repeated plays don't fetch twice). */
  private pending = new Map<string, Promise<AudioBuffer | null>>();
  private failed = new Set<string>();

  setAssignments(a: Record<string, string>) {
    this.assignments = { ...a };
  }
  getAssignments(): Record<string, string> {
    return { ...this.assignments };
  }
  setVolume(v: number) {
    this.volume = Math.min(1, Math.max(0, v));
    if (this.master) this.master.gain.value = this.volume;
  }

  /** Fetch the current list of playable files under `snd/`. */
  async listFiles(): Promise<string[]> {
    try {
      const res = await fetch(api("/sounds"), { cache: "no-store" });
      if (!res.ok) return [];
      const json = (await res.json()) as { files?: unknown };
      return Array.isArray(json.files) ? (json.files as string[]) : [];
    } catch {
      return [];
    }
  }

  /**
   * Create/resume the `AudioContext`. Call this from a user gesture (pointer or
   * key) so browsers allow audio to start.
   */
  unlock() {
    const ctx = this.ensureCtx();
    if (ctx && ctx.state === "suspended") void ctx.resume();
  }

  private ensureCtx(): AudioContext | null {
    if (this.ctx) {
      if (this.ctx.state === "suspended") void this.ctx.resume();
      return this.ctx;
    }
    const Ctor =
      window.AudioContext ??
      (window as unknown as { webkitAudioContext?: typeof AudioContext }).webkitAudioContext;
    if (!Ctor) return null;
    try {
      this.ctx = new Ctor();
      this.master = this.ctx.createGain();
      this.master.gain.value = this.volume;
      this.master.connect(this.ctx.destination);
    } catch (e) {
      console.warn("[sound] AudioContext unavailable:", (e as Error).message);
      this.ctx = null;
    }
    return this.ctx;
  }

  /** Play the sound assigned to a logical event, if one is set. */
  play(eventId: string) {
    const file = this.assignments[eventId];
    if (file) this.playFile(file);
  }

  /** Play a specific `snd/` file directly (menu preview + gameplay). */
  playFile(file: string) {
    if (!file || this.failed.has(file)) return;
    const ctx = this.ensureCtx();
    if (!ctx || !this.master) return;
    const cached = this.buffers.get(file);
    if (cached) {
      this.start(ctx, cached);
      return;
    }
    void this.loadBuffer(ctx, file).then((buf) => {
      if (buf) this.start(ctx, buf);
    });
  }

  private start(ctx: AudioContext, buf: AudioBuffer) {
    try {
      const src = ctx.createBufferSource();
      src.buffer = buf;
      src.connect(this.master!);
      src.start();
    } catch (e) {
      console.warn("[sound] play failed:", (e as Error).message);
    }
  }

  private loadBuffer(ctx: AudioContext, file: string): Promise<AudioBuffer | null> {
    const existing = this.pending.get(file);
    if (existing) return existing;
    const p = (async () => {
      try {
        const res = await fetch(api(`/snd/${encodeURIComponent(file)}`), { cache: "no-store" });
        if (!res.ok) throw new Error(`HTTP ${res.status}`);
        const bytes = await res.arrayBuffer();
        const buf = await ctx.decodeAudioData(bytes);
        this.buffers.set(file, buf);
        return buf;
      } catch (e) {
        this.failed.add(file);
        console.warn(`[sound] failed to load "${file}":`, (e as Error).message);
        return null;
      } finally {
        this.pending.delete(file);
      }
    })();
    this.pending.set(file, p);
    return p;
  }

  /** Play all sounds whose event bit is set in `bits`. */
  handleBits(bits: number) {
    if (!bits) return;
    for (const [id, bit] of EVENT_BITS) {
      if (bits & bit) this.play(id);
    }
  }
}
