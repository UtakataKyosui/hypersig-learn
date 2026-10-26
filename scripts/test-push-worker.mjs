// Run the built production Worker against ephemeral D1 and an in-process
// outbound HTTP adapter. Nothing is sent to an external Push service.
import assert from "node:assert/strict";
import { readFile, readdir, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { execFileSync } from "node:child_process";
import { createECDH, createDecipheriv, hkdfSync, randomBytes, createPublicKey, verify, createHash } from "node:crypto";
import { Miniflare, Response, Log, LogLevel } from "miniflare";

const bundle = resolve(".cloudflare/output/v0/workers/default/bundle");
const built = JSON.parse(await readFile(resolve(bundle, "../worker.config.json"), "utf8"));
const modules = {};
for (const [name, module] of Object.entries(built.manifest.modules)) {
  if (module.type === "sourcemap") continue;
  modules[name] = { type: module.type, contents: await readFile(resolve(bundle, name), module.type === "esm" ? "utf8" : undefined) };
}
// This wrapper exists only in the simulator. The production wasm/module is
// unchanged. worker::Date::now() uses `new Date()`, so inject the clock at
// the scheduled boundary to exercise JST midnight and lease/retry cutoffs.
modules["push-test-clock.js"] = { type: "esm", contents: `
import App from './${built.manifest.mainModule}';
export { Hub } from './${built.manifest.mainModule}';
const NativeDate = globalThis.Date;
let clock = 0;
globalThis.Date = class extends NativeDate {
  constructor(...args) { super(...(args.length ? args : [clock])); }
  static now() { return clock; }
};
export default class extends App {
  async scheduled(controller) {
    clock = Number(controller.scheduledTime);
    return super.scheduled(controller);
  }
}
` };
const receiver = createECDH("prime256v1");
const receiverPublic = receiver.generateKeys();
const auth = randomBytes(16);
const requests = [];
const adapterErrors = [];
const statuses = new Map();

function decrypt(body) {
  const salt = body.subarray(0, 16);
  assert.equal(body.readUInt32BE(16), 4096);
  assert.equal(body[20], 65);
  const sender = body.subarray(21, 86);
  const ikm = hkdfSync("sha256", receiver.computeSecret(sender), auth,
    Buffer.concat([Buffer.from("WebPush: info\0"), receiverPublic, sender]), 32);
  const key = hkdfSync("sha256", ikm, salt, Buffer.from("Content-Encoding: aes128gcm\0"), 16);
  const nonce = hkdfSync("sha256", ikm, salt, Buffer.from("Content-Encoding: nonce\0"), 12);
  const cipher = createDecipheriv("aes-128-gcm", key, nonce);
  cipher.setAuthTag(body.subarray(-16));
  const plaintext = Buffer.concat([cipher.update(body.subarray(86, -16)), cipher.final()]);
  assert.equal(plaintext.at(-1), 2);
  return JSON.parse(plaintext.subarray(0, -1).toString());
}

async function outbound(request) {
  try {
    const url = new URL(request.url);
    assert.equal(url.origin, "https://fcm.googleapis.com");
    assert.equal(request.method, "POST");
    assert.equal(request.headers.get("Content-Encoding"), "aes128gcm");
    const authorization = request.headers.get("Authorization");
    const [, token, publicText] = /^vapid t=([^,]+), k=(.+)$/.exec(authorization);
    const [header, claimsText, signature] = token.split(".");
    const publicBytes = Buffer.from(publicText, "base64url");
    const publicKey = createPublicKey({ format: "jwk", key: {
      kty: "EC", crv: "P-256", x: publicBytes.subarray(1, 33).toString("base64url"), y: publicBytes.subarray(33).toString("base64url"),
    } });
    assert(verify("sha256", Buffer.from(`${header}.${claimsText}`), { key: publicKey, dsaEncoding: "ieee-p1363" }, Buffer.from(signature, "base64url")));
    const claims = JSON.parse(Buffer.from(claimsText, "base64url").toString());
    assert.equal(claims.aud, url.origin);
    assert.equal(claims.sub, "mailto:ci@example.invalid");
    const payload = decrypt(Buffer.from(await request.arrayBuffer()));
    assert.deepEqual(Object.keys(payload).sort(), ["tag", "url"]);
    assert.equal(payload.url, "/medications");
    assert.equal(typeof payload.tag, "string");
    const ttl = Number(request.headers.get("TTL"));
    assert(Number.isInteger(ttl) && ttl >= 0 && ttl <= 900);
    assert(statuses.has(url.pathname), "Unplanned outbound request");
    requests.push({ path: url.pathname, ttl, payload, claims });
    return new Response(null, { status: statuses.get(url.pathname) });
  } catch (error) {
    adapterErrors.push(error);
    return new Response(null, { status: 500 });
  }
}

const options = {
  log: new Log(LogLevel.ERROR),
  unsafeTriggerHandlers: true,
  workers: [{
    config: {
      name: "app", compatibilityDate: built.compatibilityDate, exports: built.exports,
      manifest: { mainModule: "push-test-clock.js", modulesRoot: bundle, modules },
      env: {
      DB: { type: "d1", name: "push-test", id: "00000000-0000-4000-8000-000000000001" },
      HUB: built.env.HUB,
      // RFC 8291 example sender key, never a deployed VAPID secret.
      VAPID_PRIVATE_KEY: { type: "text", value: "yfWPiYE-n46HLnH0KqZOF1fJJU3MYrct3AELtAQ-oRw" },
      VAPID_SUBJECT: { type: "text", value: "mailto:ci@example.invalid" },
      GOOGLE_CLIENT_ID: { type: "text", value: "ci-client.apps.example" },
      GOOGLE_CLIENT_SECRET: { type: "text", value: "ci-client-secret" },
      GOOGLE_REDIRECT_URL: { type: "text", value: "http://localhost/auth/google/callback" },
      },
    },
    dev: { outboundService: { type: "fetcher", handler: outbound } },
  }],
};
const mf = new Miniflare(options);

try {
  let db = await mf.getD1Database("DB", "app");
  const statements = JSON.parse(execFileSync("python3", ["-c", `
import json, sqlite3
from pathlib import Path
statements=[]
for path in sorted(Path('migrations').glob('*.sql')):
    buffer=''
    for line in path.read_text().splitlines(keepends=True):
        buffer+=line
        if sqlite3.complete_statement(buffer):
            statements.append(buffer.strip())
            buffer=''
    assert not buffer.strip(), path
print(json.dumps(statements))
`], { encoding: "utf8" }));
  for (const sql of statements) await db.prepare(sql).run();
  const execute = (sql, ...args) => db.prepare(sql).bind(...args).run();
  const query = (sql, ...args) => db.prepare(sql).bind(...args).all().then((result) => result.results);
  const clock = Date.UTC(2026, 9, 2, 15); // 2026-10-03 00:00 JST.
  const due = clock - 60_000; // Previous JST day's 23:59.
  async function cron(at = clock) {
    const response = await mf.dispatchFetch(`http://localhost/cdn-cgi/local/scheduled?format=json&time=${at}`);
    assert.equal(response.status, 200, await response.text());
    assert.deepEqual(adapterErrors, []);
  }
  async function seed(kind, name, status, scheduled = due) {
    const owner = `${kind}-${name}`;
    const endpoint = `https://fcm.googleapis.com/${owner}`;
    statuses.set(`/${owner}`, status);
    await execute("INSERT INTO users VALUES(?1,'active',1,?2)", owner, scheduled - 86_400_000);
    await execute("INSERT INTO push_subscriptions VALUES(?1,?1,?2,?3,?4,1,?5)", owner, endpoint, receiverPublic.toString("base64url"), auth.toString("base64url"), scheduled - 86_400_000);
    if (kind === "probe") {
      await execute("INSERT INTO push_probe(owner,endpoint,p256dh,auth,probe_at,next_attempt_at) VALUES(?1,?2,?3,?4,?5,?5)", owner, endpoint, receiverPublic.toString("base64url"), auth.toString("base64url"), scheduled);
    } else {
      const time = new Date(scheduled + 9 * 60 * 60 * 1000).toISOString().slice(11, 16);
      const editor = { id: owner, revision: 1, enabled: true,
        medication: { name: "private medicine", start: "2026-10-01", end: "", repeat: "daily", interval: 1, weekdays: 0 },
        slots: [{ id: `${owner}-slot`, input: { time, amount: "0.5", unit: "tablet", timing: "after_meal", note: "private note" } }], pauses: [],
      };
      await execute("INSERT INTO medications VALUES(?1,?1,'private medicine',1,1,?2,?2,?3)", owner, scheduled - 86_400_000, JSON.stringify(editor));
      await execute("INSERT INTO schedules VALUES(?1,?1,'2026-10-01',NULL,'daily',1,0,'Asia/Tokyo')", owner);
      await execute("INSERT INTO dose_slots VALUES(?1,?2,?3,50,'tablet','after_meal','private note')", `${owner}-slot`, owner, time);
    }
    return owner;
  }
  const matrix = [201, 404, 410, 429, 503, 403];
  for (const kind of ["dose", "probe"]) for (const status of matrix) await seed(kind, String(status), status);
  await Promise.all([cron(), cron()]);
  assert.equal(requests.length, 12);
  for (const kind of ["dose", "probe"]) for (const status of matrix) {
    const owner = `${kind}-${status}`;
    const [state] = kind === "dose"
      ? await query("SELECT state,attempt_count,next_attempt_at FROM notification_deliveries WHERE subscription_id=?1", owner)
      : await query("SELECT state,attempt_count,next_attempt_at FROM push_probe WHERE owner=?1", owner);
    assert.equal(state.state, status === 201 ? "sent" : [429, 503].includes(status) ? "pending" : "cancelled", owner);
    assert.equal(state.attempt_count, 1, owner);
    if ([429, 503].includes(status)) assert.equal(state.next_attempt_at, clock + 60_000);
    const [subscription] = await query("SELECT enabled FROM push_subscriptions WHERE id=?1", owner);
    assert.equal(subscription.enabled, [404, 410].includes(status) ? 0 : 1, owner);
    const sent = requests.find((request) => request.path === `/${owner}`);
    assert.equal(sent.ttl, 840);
    assert.equal(sent.claims.exp, clock / 1000 + 12 * 60 * 60);
  }
  await cron();
  assert.equal(requests.length, 12, "Duplicate cron must not repeat accepted or backoff-pending deliveries");
  for (const kind of ["dose", "probe"]) statuses.set(`/${kind}-429`, 201);
  await cron(clock + 60_000);
  assert.equal(requests.length, 16);
  for (const kind of ["dose", "probe"]) {
    const retry = requests.filter((request) => request.path === `/${kind}-429`);
    assert.equal(retry.length, 2);
    assert.deepEqual(retry[0].payload, retry[1].payload, "Retries reuse the same notification tag");
    assert.equal(retry[1].ttl, 780);
    const [recovered] = kind === "dose"
      ? await query("SELECT state,attempt_count FROM notification_deliveries WHERE subscription_id=?1", `${kind}-429`)
      : await query("SELECT state,attempt_count FROM push_probe WHERE owner=?1", `${kind}-429`);
    assert.deepEqual(recovered, { state: "sent", attempt_count: 2 });
  }
  await execute("UPDATE push_subscriptions SET enabled=0 WHERE id IN ('dose-503','probe-503')");

  // Kill the completion transaction after the simulated Push service accepts.
  // Recovery must wait for lease expiry, and may send the same tag again.
  for (const kind of ["dose", "probe"]) await seed(kind, "crash", 201);
  await db.prepare("CREATE TRIGGER fail_dose_completion BEFORE UPDATE OF state ON notification_deliveries WHEN NEW.subscription_id='dose-crash' AND NEW.state='sent' BEGIN SELECT RAISE(ABORT,'simulated completion outage'); END;").run();
  await db.prepare("CREATE TRIGGER fail_probe_completion BEFORE UPDATE OF state ON push_probe WHEN NEW.owner='probe-crash' AND NEW.state='sent' BEGIN SELECT RAISE(ABORT,'simulated completion outage'); END;").run();
  await cron(clock + 60_000);
  assert.equal(requests.length, 18);
  assert.deepEqual(await query("SELECT state,attempt_count FROM notification_deliveries WHERE subscription_id='dose-crash'"), [{ state: "sending", attempt_count: 1 }]);
  assert.deepEqual(await query("SELECT state,attempt_count FROM push_probe WHERE owner='probe-crash'"), [{ state: "sending", attempt_count: 1 }]);
  await cron(clock + 179_999);
  assert.equal(requests.length, 18, "An unexpired lease blocks another send after an accepted-but-unsaved response");
  await db.prepare("DROP TRIGGER fail_dose_completion").run();
  await db.prepare("DROP TRIGGER fail_probe_completion").run();
  await cron(clock + 180_000);
  assert.equal(requests.length, 20);
  for (const kind of ["dose", "probe"]) {
    const sent = requests.filter((request) => request.path === `/${kind}-crash`);
    assert.equal(sent.length, 2);
    assert.deepEqual(sent[0].payload, sent[1].payload);
    assert.equal(sent[1].ttl, 660);
  }
  assert.deepEqual(await query("SELECT state,attempt_count FROM notification_deliveries WHERE subscription_id='dose-crash'"), [{ state: "sent", attempt_count: 2 }]);
  assert.deepEqual(await query("SELECT state,attempt_count FROM push_probe WHERE owner='probe-crash'"), [{ state: "sent", attempt_count: 2 }]);

  // A prolonged outage must expire old pending/sending rows without a burst.
  const old = clock - 16 * 60_000;
  await seed("dose", "outage", 201, old);
  await seed("probe", "outage", 201, old);
  await execute("INSERT INTO notification_jobs VALUES('outage-job','dose-outage','dose-outage-slot',?1,1,'pending',?2)", old, old + 900_000);
  await execute("INSERT INTO notification_deliveries(job_id,subscription_id,state,attempt_count,next_attempt_at,lease_until) VALUES('outage-job','dose-outage','sending',1,?1,?1)", old);
  await cron(clock + 180_000);
  assert.equal(requests.length, 20);
  assert.deepEqual(await query("SELECT state FROM notification_deliveries WHERE subscription_id='dose-outage'"), [{ state: "expired" }]);
  assert.deepEqual(await query("SELECT state FROM push_probe WHERE owner='probe-outage'"), [{ state: "expired" }]);

  // Generate candidates from the production domain against D1 at fixed JST
  // midnight, including a pause that starts after yesterday's delayed dose.
  for (const reason of ["pause", "disabled", "ended", "first-save"]) {
    const owner = await seed("dose", reason, 201);
    const [row] = await query("SELECT document FROM medications WHERE id=?1", owner);
    const editor = JSON.parse(row.document);
    let saved = due - 86_400_000;
    if (reason === "pause") {
      editor.pauses = [{ id: "pause-midnight", input: { start: "2026-10-03", end: "2026-10-03" } }];
      await execute("INSERT INTO pause_periods VALUES('pause-midnight',?1,'2026-10-03','2026-10-03')", owner);
    } else if (reason === "disabled") {
      editor.enabled = false;
    } else if (reason === "ended") {
      editor.medication.end = "2026-10-01";
      await execute("UPDATE schedules SET end_date='2026-10-01' WHERE id=?1", owner);
    } else {
      saved = due + 1;
    }
    await execute("UPDATE medications SET document=?2,enabled=?3,updated_at=?4 WHERE id=?1", owner, JSON.stringify(editor), Number(editor.enabled), saved);
  }
  await cron(clock + 180_000);
  assert.equal(requests.length, 20);
  assert.equal((await query("SELECT id FROM notification_jobs WHERE medication_id IN ('dose-pause','dose-disabled','dose-ended','dose-first-save')")).length, 0);
  const boundary = clock + 180_000 - 900_000;
  for (const kind of ["dose", "probe"]) await seed(kind, "boundary", 201, boundary);
  await cron(clock + 180_000);
  assert.equal(requests.length, 22);
  for (const kind of ["dose", "probe"]) assert.equal(requests.find((request) => request.path === `/${kind}-boundary`).ttl, 0);
  for (const kind of ["dose", "probe"]) await seed(kind, "past-boundary", 201, boundary);
  await cron(clock + 180_001);
  assert.equal(requests.length, 22, "One millisecond beyond the 15-minute deadline must not send");
  assert.equal((await query("SELECT id FROM notification_jobs WHERE medication_id='dose-past-boundary'")).length, 0);
  assert.deepEqual(await query("SELECT state FROM push_probe WHERE owner='probe-past-boundary'"), [{ state: "expired" }]);

  // Use the actual protected Actions for invalidation, with disposable
  // server-side sessions. No test route or bypass is added to the app.
  let sessionId = 100;
  async function session(owner) {
    const id = ++sessionId;
    const bytes = Buffer.alloc(16);
    bytes.writeBigUInt64LE(BigInt(id));
    const cookie = bytes.toString("base64url");
    const expiry = clock + 86_400_000;
    const expires = new Date(expiry);
    const ordinal = Math.floor((expiry - Date.UTC(expires.getUTCFullYear(), 0, 1)) / 86_400_000) + 1;
    const record = { id, data: { identity: { user_id: owner, session_version: 1, level: "Google", expires_at: expiry / 1000 } }, expiry_date: [expires.getUTCFullYear(), ordinal, expires.getUTCHours(), expires.getUTCMinutes(), expires.getUTCSeconds(), 0, 0, 0, 0] };
    await execute("INSERT INTO sessions VALUES(?1,?2,?3)", createHash("sha256").update(cookie).digest("base64url"), JSON.stringify(record), expiry);
    return `__Host-session=${cookie}`;
  }
  async function action(owner, path, input) {
    const response = await mf.dispatchFetch(`http://localhost${path}`, {
      method: "POST", headers: { Accept: "text/event-stream", "Content-Type": "application/json", "Sec-Fetch-Site": "same-origin", Cookie: await session(owner) }, body: JSON.stringify(input),
    });
    const body = await response.text();
    assert.equal(response.status, 200, body);
    return body;
  }
  const reasons = ["edit", "delete", "disable", "pause-action", "unsubscribe"];
  for (const reason of reasons) await seed("dose", reason, 503);
  await cron(clock + 240_000);
  assert.equal(requests.length, 27);
  for (const reason of reasons) {
    const owner = `dose-${reason}`;
    if (reason === "delete") {
      await action(owner, "/medications/remove", [owner, 1]);
    } else if (reason === "unsubscribe") {
      await action(owner, "/notifications/unsubscribe", `https://fcm.googleapis.com/${owner}`);
    } else {
      const [row] = await query("SELECT document FROM medications WHERE id=?1", owner);
      const editor = JSON.parse(row.document);
      if (reason === "edit") editor.medication.name = "edited medicine";
      if (reason === "disable") editor.enabled = false;
      if (reason === "pause-action") editor.pauses.push({ id: `${owner}-pause`, input: { start: "2026-10-03", end: "2026-10-03" } });
      await action(owner, "/medications/save", editor);
    }
    const rows = await query("SELECT state FROM notification_deliveries WHERE subscription_id=?1", owner);
    assert.deepEqual(rows, reason === "delete" ? [] : [{ state: "cancelled" }], reason);
  }
  await cron(clock + 300_000);
  assert.equal(requests.length, 27, "Authenticated editing/deletion/disable/pause/unsubscribe must suppress queued retries");
  const beforeReload = await query("SELECT id,revision,document FROM medications ORDER BY id");
  const worker = options.workers[0];
  const wrapper = modules["push-test-clock.js"];
  await mf.setOptions({ ...options, workers: [{ ...worker, config: { ...worker.config,
    manifest: { ...worker.config.manifest, modules: { ...modules, "push-test-clock.js": { ...wrapper, contents: wrapper.contents + "\n// Force a fresh Worker isolate for persistence verification.\n" } } },
  } }] });
  db = await mf.getD1Database("DB", "app");
  await cron(clock + 300_000);
  assert.deepEqual(await query("SELECT id,revision,document FROM medications ORDER BY id"), beforeReload);
  assert((await action("dose-edit", "/medications/load", "dose-edit")).includes("edited medicine"));
  assert.equal(requests.length, 27, "Reloaded Worker must preserve D1 completion/invalidation and not repeat sends");
  if (process.env.PUSH_FIXTURE_PATH) {
    // Only generic, decrypted test payload goes to the browser fixture. No
    // VAPID material, subscription keys, cookies, or medication text.
    await writeFile(process.env.PUSH_FIXTURE_PATH, JSON.stringify(requests.find((request) => request.path === "/dose-201").payload));
  }
  console.log("Push Worker HTTP integration: dose/test status matrix, VAPID/decryption, TTL, concurrent cron, retry/lease recovery, expired backlog, fixed-clock boundaries, authenticated invalidation and Worker reload persistence OK");
} finally {
  await mf.dispose();
}
