// Client-side leaderboard + nickname helpers.

import { api } from "../base";

const NICK_KEY = "webrace.nickname";
const TOKEN_KEY = "webrace.token";

/** Get the stored nickname + token, or null if not registered. */
export function getIdentity(): { nickname: string; token: string } | null {
  try {
    const nickname = localStorage.getItem(NICK_KEY);
    const token = localStorage.getItem(TOKEN_KEY);
    if (nickname && token) return { nickname, token };
  } catch {
    /* localStorage unavailable */
  }
  return null;
}

/** Register (or look up) a nickname, storing the returned token. */
export async function registerNickname(nickname: string): Promise<{ nickname: string; token: string }> {
  const res = await fetch(api("/api/nickname"), {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ nickname }),
  });
  const data = await res.json();
  if (!res.ok) {
    throw new Error(data.error ?? `nickname failed (${res.status})`);
  }
  try {
    localStorage.setItem(NICK_KEY, data.nickname);
    localStorage.setItem(TOKEN_KEY, data.token);
  } catch {
    /* ignore */
  }
  return { nickname: data.nickname, token: data.token };
}

/** Submit a finished race time to the leaderboard. */
export async function submitTime(token: string, map: string, timeMs: number, splits: number[]): Promise<void> {
  const res = await fetch(api("/api/times"), {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ token, map, timeMs, splits }),
  });
  if (!res.ok) {
    const data = await res.json().catch(() => ({}));
    console.warn("submit time failed:", data.error ?? res.status);
  }
}

/** Fetch the leaderboard for a map. */
export async function fetchLeaderboard(map: string): Promise<Array<{ nickname: string; time_ms: number }>> {
  const res = await fetch(api(`/api/leaderboard?map=${encodeURIComponent(map)}`));
  const data = await res.json();
  return data.entries ?? [];
}
