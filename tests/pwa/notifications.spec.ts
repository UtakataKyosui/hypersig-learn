import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { expect, test } from "@playwright/test";

// Only a disposable local Worker started by test-worker.sh is supported.
test.beforeEach(async ({ context }) => {
  if (!process.env.PERSIST_DIR) throw new Error("Run npm run test:worker");
  const { cookie } = JSON.parse(execFileSync("python3", ["scripts/pwa-fixture.py"], { encoding: "utf8" }));
  const [name, value] = cookie.split("=");
  await context.addCookies([{ name, value, domain: "localhost", path: "/", secure: true, httpOnly: true, sameSite: "Lax" }]);
});

test("manifest and icons are public, and the page links the manifest", async ({ page, request }) => {
  await page.goto("/notifications");
  await expect(page.locator('head link[rel="manifest"]')).toHaveAttribute("href", "/manifest.webmanifest");
  const response = await request.get("/manifest.webmanifest");
  expect(response.status()).toBe(200);
  expect(response.headers()["content-type"]).toContain("application/manifest+json");
  const manifest = await response.json();
  expect(manifest).toMatchObject({ scope: "/", start_url: "/notifications", display: "standalone" });
  for (const icon of manifest.icons) {
    const image = await request.get(icon.src);
    expect(image.status()).toBe(200);
    expect(image.headers()["content-type"]).toBe("image/png");
    expect((await image.body()).subarray(0, 8).toString("hex")).toBe("89504e470d0a1a0a");
  }
  const sw = await request.get("/sw.js");
  expect(sw.status()).toBe(200);
  expect(sw.headers()["service-worker-allowed"]).toBe("/");
  expect(sw.headers()["cache-control"]).toBe("no-cache");
});

test("permission alone is not saved; subscribing uses the real action and SSE patch", async ({ context, page }) => {
  await context.grantPermissions(["notifications"]);
  // Replace the external Push service only. SW registration, custom events,
  // hypersig action, validation, D1 write and the returned patch remain real.
  await page.addInitScript(() => {
    PushManager.prototype.getSubscription = async () => null;
    PushManager.prototype.subscribe = async function (options) {
      if (!options?.userVisibleOnly || !(options.applicationServerKey instanceof Uint8Array)) {
        throw new Error("Missing Push subscription options");
      }
      return {
        toJSON: () => ({
          endpoint: "https://fcm.googleapis.com/fcm/send/pwa-ci",
          keys: {
            p256dh: "BCVxsr7N_eNgVRqvHtD0zTZsEc6-VV-JvLexhqUzORcxaOzi6-AYWXvTBHm4bjyPjs7Vd8pZGH6SRpkNtoIAiw4",
            // Disposable test auth material; never a real Push subscription secret.
            auth: btoa(String.fromCharCode(...crypto.getRandomValues(new Uint8Array(16))))
              .replaceAll("+", "-").replaceAll("/", "_").replace(/=+$/, ""),
          },
        }),
      } as PushSubscription;
    };
  });
  await page.goto("/notifications");
  await expect(page.getByText("未保存", { exact: true })).toBeVisible();
  await expect(page.getByText("許可済み", { exact: true })).toBeVisible();
  await page.getByRole("button", { name: "通知を有効にする", exact: true }).click();
  await expect(page.getByText("保存済み", { exact: true })).toBeVisible();
  const registration = await page.evaluate(async () => {
    const registration = await navigator.serviceWorker.ready;
    return { scope: registration.scope, state: registration.active?.state };
  });
  expect(registration).toEqual({ scope: "http://localhost:8791/", state: "activated" });
  // Query server state, so a UI-only success cannot pass this check.
  const saved = await page.request.post("/notifications/subscription/status", {
    headers: { Accept: "text/event-stream", "Sec-Fetch-Site": "same-origin", "Content-Type": "application/json" },
    data: JSON.stringify("https://fcm.googleapis.com/fcm/send/pwa-ci"),
  });
  expect(saved.status()).toBe(200);
  expect(await saved.text()).toContain('"saved":true');
});

test("denied permission does not register a worker or save a subscription", async ({ page }) => {
  await page.addInitScript(() => {
    Notification.requestPermission = async () => "denied";
  });
  await page.goto("/notifications");
  await page.getByRole("button", { name: "通知を有効にする", exact: true }).click();
  await expect(page.getByText("拒否", { exact: true })).toBeVisible();
  await expect(page.getByText("未保存", { exact: true })).toBeVisible();
  expect(await page.evaluate(async () => (await navigator.serviceWorker.getRegistrations()).length)).toBe(0);
});

test("unsupported Push keeps support separate from permission and server save", async ({ page }) => {
  await page.addInitScript(() => { Reflect.deleteProperty(window, "PushManager"); });
  await page.goto("/notifications");
  await expect(page.getByText("非対応", { exact: true })).toBeVisible();
  await expect(page.getByText("未保存", { exact: true })).toBeVisible();
  await expect(page.getByRole("heading", { name: "通知の準備ができていません", exact: true })).toBeVisible();
  await page.getByRole("button", { name: "通知を有効にする", exact: true }).click();
  await expect(page.getByText("非対応", { exact: true })).toBeVisible();
  expect(await page.evaluate(async () => (await navigator.serviceWorker.getRegistrations()).length)).toBe(0);
});

test("a browser operation failure preserves granted permission and shows its own error", async ({ context, page }) => {
  await context.grantPermissions(["notifications"]);
  await page.addInitScript(() => {
    navigator.serviceWorker.register = async () => { throw new Error("Disposable registration failure"); };
  });
  await page.goto("/notifications");
  await page.getByRole("button", { name: "通知を有効にする", exact: true }).click();
  await expect(page.getByText("対応", { exact: true })).toBeVisible();
  await expect(page.getByText("許可済み", { exact: true })).toBeVisible();
  await expect(page.getByText("未保存", { exact: true })).toBeVisible();
  await expect(page.getByText("端末の通知操作に失敗しました。通信とブラウザの設定を確認し、再設定または解除をやり直してください。", { exact: true })).toBeVisible();
});

test("standalone context is reported independently of subscription readiness", async ({ page }) => {
  // WebKit's standalone flag is simulated; this does not claim an iOS test.
  await page.addInitScript(() => Object.defineProperty(navigator, "standalone", { value: true }));
  await page.goto("/notifications");
  await expect(page.getByText("ホーム画面アプリ", { exact: true })).toBeVisible();
  await expect(page.getByText("未保存", { exact: true })).toBeVisible();
  await expect(page.getByRole("heading", { name: "通知の準備ができていません", exact: true })).toBeVisible();
  await expect(page.getByText("ブラウザから利用中です。iPhone／iPadでは、ホーム画面に追加したアイコンから開いてください。", { exact: true })).toBeHidden();
});

test("saved subscriptions reconcile after reload and removal repairs a lost response", async ({ context, page }) => {
  await context.grantPermissions(["notifications"]);
  await page.addInitScript(() => {
    const subscription = {
      endpoint: "https://fcm.googleapis.com/fcm/send/pwa-removal",
      toJSON: () => ({ endpoint: subscription.endpoint, keys: {
        p256dh: "BCVxsr7N_eNgVRqvHtD0zTZsEc6-VV-JvLexhqUzORcxaOzi6-AYWXvTBHm4bjyPjs7Vd8pZGH6SRpkNtoIAiw4",
        auth: "BTBZMqHH6r4Tts7J_aSIgg",
      } }),
      unsubscribe: async () => {
        if (localStorage.getItem("fixture-fail-removal")) {
          localStorage.removeItem("fixture-fail-removal");
          return false;
        }
        localStorage.removeItem("fixture-active");
        return true;
      },
    };
    PushManager.prototype.getSubscription = async () => localStorage.getItem("fixture-active") ? subscription as PushSubscription : null;
    PushManager.prototype.subscribe = async () => {
      localStorage.setItem("fixture-active", "true");
      return subscription as PushSubscription;
    };
  });
  await page.goto("/notifications");
  await expect(page.getByText("ブラウザ", { exact: true })).toBeVisible();
  await expect(page.getByText("この端末の購読は未登録です。", { exact: true })).toBeVisible();
  // A lost registration response must leave the screen unsaved and retryable.
  await page.route("**/notifications/subscription", (route) => route.fulfill({ status: 401, body: "Sign in" }));
  await page.getByRole("button", { name: "通知を有効にする", exact: true }).click();
  await expect(page.getByRole("link", { name: "ログインし直す", exact: true })).toHaveAttribute("href", "/login");
  await expect(page.getByText("未保存", { exact: true })).toBeVisible();
  await page.unroute("**/notifications/subscription");
  await page.route("**/notifications/subscription", (route) => route.fulfill({ status: 503, body: "保存できませんでした" }));
  await page.getByRole("button", { name: "通知を有効にする", exact: true }).click();
  await expect(page.getByText("未保存", { exact: true })).toBeVisible();
  await expect(page.getByText("保存できませんでした", { exact: true })).toBeVisible();
  await page.unroute("**/notifications/subscription");
  await page.getByRole("button", { name: "通知を有効にする", exact: true }).click();
  await expect(page.getByText("保存済み", { exact: true })).toBeVisible();
  await page.reload();
  await expect(page.getByText("保存済み", { exact: true })).toBeVisible();
  // A disabled D1 row with a browser subscription must differ from a
  // missing row, and re-registering must repair it. HTTP Push tests cover
  // the 404/410 -> disabled-row path separately.
  const disabled = await page.request.post("/notifications/unsubscribe", {
    headers: { Accept: "text/event-stream", "Sec-Fetch-Site": "same-origin", "Content-Type": "application/json" },
    data: JSON.stringify("https://fcm.googleapis.com/fcm/send/pwa-removal"),
  });
  expect(disabled.status()).toBe(200);
  await page.reload();
  const expired = page.getByText("この購読は失効または解除済みです。通知を有効にして再登録してください。", { exact: true });
  await expect(expired).toBeVisible();
  await expect(page.getByText("未保存", { exact: true })).toBeVisible();
  await page.getByRole("button", { name: "通知を有効にする", exact: true }).click();
  await expect(page.getByText("保存済み", { exact: true })).toBeVisible();
  await expect(expired).toBeHidden();
  const reservation = page.waitForResponse("**/notifications/probe");
  await page.getByRole("button", { name: "テスト通知を予約", exact: true }).click();
  expect((await reservation).status()).toBe(200);
  await expect(page.getByText("テスト通知の予約を受け付けました。端末への表示は端末で確認してください。", { exact: true })).toBeVisible();
  await page.route("**/notifications/unsubscribe", (route) => route.abort());
  await page.evaluate(() => localStorage.setItem("fixture-fail-removal", "true"));
  await page.getByRole("button", { name: "通知を解除する" }).click();
  await expect(page.getByText("未保存", { exact: true })).toBeVisible();
  await page.unroute("**/notifications/unsubscribe");
  const removed = page.waitForResponse("**/notifications/unsubscribe");
  await page.reload();
  expect((await removed).status()).toBe(200);
  await expect.poll(() => page.evaluate(() => localStorage.getItem("push-pending-removal"))).toBeNull();
  const response = await page.request.post("/notifications/subscription/status", {
    headers: { Accept: "text/event-stream", "Sec-Fetch-Site": "same-origin", "Content-Type": "application/json" },
    data: JSON.stringify("https://fcm.googleapis.com/fcm/send/pwa-removal"),
  });
  expect(await response.text()).toContain('"saved":false');
  // Missing custom-event detail cannot register an empty subscription.
  const invalid = page.waitForResponse("**/notifications/subscription");
  await page.locator("push-subscribe").evaluate((element) => element.dispatchEvent(new CustomEvent("push-subscription")));
  expect((await invalid).status()).toBe(400);
  await expect(page.getByText("未保存", { exact: true })).toBeVisible();
});

test("a synthetic push creates a notification; clicking selects the correct destination", async ({ context, page }) => {
  await context.grantPermissions(["notifications"]);
  await page.goto("/notifications");
  const cdp = await context.newCDPSession(page);
  // CDP can report registrations from other browser contexts. Identify
  // this worker's version by a unique script URL rather than scope alone.
  const scriptURL = new URL("/sw.js", page.url());
  scriptURL.searchParams.set("test", `${test.info().testId}-${Date.now()}`);
  let registrationId: string | undefined;
  cdp.on("ServiceWorker.workerVersionUpdated", ({ versions }) => {
    registrationId = versions.find((version) => version.scriptURL === scriptURL.href)?.registrationId ?? registrationId;
  });
  await cdp.send("ServiceWorker.enable");
  const workerPromise = context.waitForEvent("serviceworker");
  await page.evaluate(async (script) => {
    await navigator.serviceWorker.register(script, { scope: "/" });
    await navigator.serviceWorker.ready;
  }, scriptURL.href);
  let worker = await workerPromise;
  await expect.poll(() => worker.evaluate(() => (self as unknown as ServiceWorkerGlobalScope).registration.active?.state)).toBe("activated");
  await worker.evaluate(() => {
    const scope = self as unknown as ServiceWorkerGlobalScope & { pushEvents: number };
    scope.pushEvents = 0;
    scope.addEventListener("push", () => { scope.pushEvents += 1; });
  });
  await expect.poll(() => registrationId).toBeTruthy();
  await cdp.send("ServiceWorker.deliverPushMessage", {
    origin: "http://localhost:8791", registrationId: registrationId!, data: "probe",
  });
  await expect.poll(() => worker.evaluate(() => (self as unknown as { pushEvents: number }).pushEvents), { message: "CDP must deliver a push event to the selected SW before notification assertions" }).toBe(1);
  const notifications = () => worker.evaluate(async () => {
    const notifications = await (self as unknown as ServiceWorkerGlobalScope).registration.getNotifications();
    return notifications.map((n) => ({ title: n.title, body: n.body, tag: n.tag, data: n.data }));
  });
  await expect.poll(notifications).toEqual([{
    title: "服薬の時間です", body: "アプリで今日の予定を確認してください",
    tag: "medication-test", data: { url: "http://localhost:8791/medications" },
  }]);
  await worker.evaluate(async () => {
    for (const notification of await (self as unknown as ServiceWorkerGlobalScope).registration.getNotifications()) notification.close();
  });
  if (!process.env.PUSH_FIXTURE_PATH) throw new Error("Run npm run test:worker to generate the actual Worker payload");
  const payload = JSON.parse(readFileSync(process.env.PUSH_FIXTURE_PATH, "utf8"));
  await cdp.send("ServiceWorker.deliverPushMessage", {
    origin: "http://localhost:8791", registrationId: registrationId!, data: JSON.stringify(payload),
  });
  await expect.poll(notifications).toEqual([{
    title: "服薬の時間です", body: "アプリで今日の予定を確認してください",
    tag: payload.tag, data: { url: new URL(payload.url, "http://localhost:8791").href },
  }]);
  await worker.evaluate(async () => {
    for (const notification of await (self as unknown as ServiceWorkerGlobalScope).registration.getNotifications()) notification.close();
  });
  await cdp.send("ServiceWorker.deliverPushMessage", {
    origin: "http://localhost:8791", registrationId: registrationId!,
    data: JSON.stringify({ url: "https://untrusted.invalid/", tag: "scheduled-fixture", body: "ignored private content" }),
  });
  await expect.poll(notifications).toEqual([{
    title: "服薬の時間です", body: "アプリで今日の予定を確認してください",
    tag: "scheduled-fixture", data: { url: "http://localhost:8791/medications" },
  }]);
  // A synthetic event has no OS user activation, so focus/openWindow would
  // throw. Record those boundary calls while running the actual SW handler.
  const destinations = await worker.evaluate(async () => {
    const scope = self as unknown as ServiceWorkerGlobalScope;
    let focusedUrl: string | undefined;
    const matchAll = scope.clients.matchAll.bind(scope.clients);
    scope.clients.matchAll = async (options) => {
      const clients = await matchAll(options);
      for (const client of clients) {
        const window = client as WindowClient;
        let destination = window.url;
        window.navigate = async (url) => { destination = url; return window; };
        window.focus = async () => {
          focusedUrl = destination;
          return window;
        };
      }
      return clients;
    };
    const [notification] = await scope.registration.getNotifications();
    const click = async () => {
      const pending: Promise<unknown>[] = [];
      const event = new NotificationEvent("notificationclick", { notification });
      Object.defineProperty(event, "waitUntil", { value: (promise: Promise<unknown>) => pending.push(promise) });
      scope.dispatchEvent(event);
      await Promise.all(pending);
    };
    await click();
    let openedUrl: string | undefined;
    scope.clients.matchAll = async () => [];
    scope.clients.openWindow = async (url) => {
      openedUrl = url;
      return null;
    };
    await click();
    return { focusedUrl, openedUrl };
  });
  expect(destinations).toEqual({
    focusedUrl: "http://localhost:8791/medications",
    openedUrl: "http://localhost:8791/medications",
  });
  await expect.poll(notifications).toEqual([]);
  await expect(page).toHaveURL("http://localhost:8791/notifications");
  // Updating the script on the same scope must activate the replacement
  // and continue handling the production-generated payload.
  const updatedURL = new URL(scriptURL);
  updatedURL.searchParams.set("revision", "2");
  const replacement = context.waitForEvent("serviceworker", (candidate) => candidate.url() === updatedURL.href);
  await page.evaluate(async (script) => { await navigator.serviceWorker.register(script, { scope: "/" }); }, updatedURL.href);
  worker = await replacement;
  await expect.poll(() => worker.evaluate(() => (self as unknown as ServiceWorkerGlobalScope).registration.active?.scriptURL)).toBe(updatedURL.href);
  await expect.poll(() => worker.evaluate(() => (self as unknown as ServiceWorkerGlobalScope).registration.active?.state)).toBe("activated");
  await worker.evaluate(() => {
    const scope = self as unknown as ServiceWorkerGlobalScope & { pushEvents: number };
    scope.pushEvents = 0;
    scope.addEventListener("push", () => { scope.pushEvents += 1; });
  });
  await cdp.send("ServiceWorker.deliverPushMessage", {
    origin: "http://localhost:8791", registrationId: registrationId!, data: JSON.stringify(payload),
  });
  await expect.poll(() => worker.evaluate(() => (self as unknown as { pushEvents: number }).pushEvents)).toBe(1);
  await expect.poll(notifications).toEqual([{
    title: "服薬の時間です", body: "アプリで今日の予定を確認してください",
    tag: payload.tag, data: { url: new URL(payload.url, "http://localhost:8791").href },
  }]);
  await worker.evaluate(async () => {
    for (const notification of await (self as unknown as ServiceWorkerGlobalScope).registration.getNotifications()) notification.close();
  });
  await expect.poll(notifications).toEqual([]);
  await cdp.detach();
});
