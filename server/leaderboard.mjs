// Leaderboard + nickname server.
//
// Uses Node's built-in `node:sqlite` (no external deps). Stores nicknames and
// per-map race times so we can build leaderboards.
//
// API:
//   POST /api/nickname           { nickname } -> { token }
//   POST /api/times              { token, map, timeMs, splits } -> { ok }
//   GET  /api/leaderboard?map=   -> { entries: [{ nickname, timeMs }] }
//   GET  /api/personal?token=    -> { nickname, times: [...] }
//
// Run: node server/leaderboard.mjs [port]

import { DatabaseSync } from "node:sqlite";
import http from "node:http";
import { randomBytes } from "node:crypto";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const __dirname = dirname(fileURLToPath(import.meta.url));
const PORT = Number(process.env.LB_PORT || process.env.PORT || 4174);
const DB_PATH = process.env.DB_PATH || join(__dirname, "data.db");

const db = new DatabaseSync(DB_PATH);
db.exec(`
  CREATE TABLE IF NOT EXISTS users (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    nickname TEXT UNIQUE NOT NULL,
    token TEXT UNIQUE NOT NULL,
    created_at INTEGER NOT NULL
  );
  CREATE TABLE IF NOT EXISTS times (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL,
    map TEXT NOT NULL,
    time_ms INTEGER NOT NULL,
    splits TEXT,
    created_at INTEGER NOT NULL,
    FOREIGN KEY(user_id) REFERENCES users(id)
  );
  CREATE INDEX IF NOT EXISTS idx_times_map_time ON times(map, time_ms);
`);

function readBody(req) {
  return new Promise((resolve, reject) => {
    let data = "";
    req.on("data", (c) => (data += c));
    req.on("end", () => resolve(data));
    req.on("error", reject);
  });
}

function json(res, status, obj) {
  res.writeHead(status, { "Content-Type": "application/json" });
  res.end(JSON.stringify(obj));
}

function findUserByToken(token) {
  return db.prepare("SELECT id, nickname FROM users WHERE token = ?").get(token);
}

async function handleNickname(body) {
  let nickname = "";
  try {
    nickname = String(JSON.parse(body).nickname ?? "").trim();
  } catch {
    return null;
  }
  if (!/^[A-Za-z0-9_-]{2,20}$/.test(nickname)) {
    return { status: 400, err: "nickname must be 2-20 chars: letters, digits, - and _" };
  }
  const token = randomBytes(16).toString("hex");
  try {
    db.prepare("INSERT INTO users (nickname, token, created_at) VALUES (?, ?, ?)").run(
      nickname,
      token,
      Date.now(),
    );
  } catch (e) {
    if (String(e.message).includes("UNIQUE")) {
      return { status: 409, err: "nickname already taken" };
    }
    throw e;
  }
  return { status: 200, obj: { token, nickname } };
}

async function handleSubmit(body) {
  let token, map, timeMs, splits;
  try {
    const o = JSON.parse(body);
    token = String(o.token ?? "");
    map = String(o.map ?? "").toLowerCase();
    timeMs = Number(o.timeMs);
    splits = Array.isArray(o.splits) ? o.splits : [];
  } catch {
    return { status: 400, err: "bad request" };
  }
  if (!token || !map || !Number.isFinite(timeMs) || timeMs <= 0) {
    return { status: 400, err: "missing fields" };
  }
  const user = findUserByToken(token);
  if (!user) return { status: 401, err: "invalid token" };

  db.prepare(
    "INSERT INTO times (user_id, map, time_ms, splits, created_at) VALUES (?, ?, ?, ?, ?)",
  ).run(user.id, map, Math.round(timeMs), JSON.stringify(splits), Date.now());
  return { status: 200, obj: { ok: true } };
}

function handleLeaderboard(url) {
  const map = String(url.searchParams.get("map") ?? "").toLowerCase();
  if (!map) return { status: 400, err: "missing map" };
  const rows = db
    .prepare(
      `SELECT u.nickname, MIN(t.time_ms) AS time_ms
       FROM times t JOIN users u ON u.id = t.user_id
       WHERE t.map = ?
       GROUP BY u.id
       ORDER BY time_ms ASC
       LIMIT 100`,
    )
    .all(map);
  return { status: 200, obj: { entries: rows } };
}

function handlePersonal(url) {
  const token = String(url.searchParams.get("token") ?? "");
  const user = findUserByToken(token);
  if (!user) return { status: 401, err: "invalid token" };
  const times = db
    .prepare("SELECT map, time_ms, splits FROM times WHERE user_id = ? ORDER BY created_at DESC LIMIT 100")
    .all(user.id);
  return { status: 200, obj: { nickname: user.nickname, times } };
}

const server = http.createServer(async (req, res) => {
  try {
    const url = new URL(req.url, "http://localhost");
    if (req.method === "POST" && url.pathname === "/api/nickname") {
      const r = await handleNickname(await readBody(req));
      if (r === null) return json(res, 400, { error: "bad request" });
      return "obj" in r ? json(res, r.status, r.obj) : json(res, r.status, { error: r.err });
    }
    if (req.method === "POST" && url.pathname === "/api/times") {
      const r = await handleSubmit(await readBody(req));
      return "obj" in r ? json(res, r.status, r.obj) : json(res, r.status, { error: r.err });
    }
    if (req.method === "GET" && url.pathname === "/api/leaderboard") {
      const r = handleLeaderboard(url);
      return "obj" in r ? json(res, r.status, r.obj) : json(res, r.status, { error: r.err });
    }
    if (req.method === "GET" && url.pathname === "/api/personal") {
      const r = handlePersonal(url);
      return "obj" in r ? json(res, r.status, r.obj) : json(res, r.status, { error: r.err });
    }
    json(res, 404, { error: "not found" });
  } catch (e) {
    json(res, 500, { error: String(e?.message ?? e) });
  }
});

server.listen(PORT, () => {
  console.log(`[leaderboard] listening on http://127.0.0.1:${PORT}`);
});
