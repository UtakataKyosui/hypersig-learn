import { execFileSync } from "node:child_process";
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

test("a synthetic push creates a notification; clicking selects the correct destination", async ({ context, page }) => {
  await context.grantPermissions(["notifications"]);
  await page.goto("/notifications");
  const cdp = await context.newCDPSession(page);
  let registrationId: string | undefined;
  cdp.on("ServiceWorker.workerRegistrationUpdated", ({ registrations }) => {
    registrationId = registrations.find((r) => r.scopeURL === "http://localhost:8791/")?.registrationId ?? registrationId;
  });
  await cdp.send("ServiceWorker.enable");
  const workerPromise = context.waitForEvent("serviceworker");
  await page.evaluate(async () => {
    await navigator.serviceWorker.register("/sw.js", { scope: "/" });
    await navigator.serviceWorker.ready;
  });
  const worker = await workerPromise;
  await expect.poll(() => registrationId).toBeTruthy();
  await cdp.send("ServiceWorker.deliverPushMessage", {
    origin: "http://localhost:8791", registrationId: registrationId!, data: "probe",
  });
  const notifications = () => worker.evaluate(async () => {
    const notifications = await (self as unknown as ServiceWorkerGlobalScope).registration.getNotifications();
    return notifications.map((n) => ({ title: n.title, body: n.body, tag: n.tag, data: n.data }));
  });
  await expect.poll(notifications).toEqual([{
    title: "服薬の時間です", body: "アプリで今日の予定を確認してください",
    tag: "medication-probe", data: { url: "/notifications" },
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
        window.focus = async () => {
          focusedUrl = window.url;
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
    focusedUrl: "http://localhost:8791/notifications",
    openedUrl: "http://localhost:8791/notifications",
  });
  await expect.poll(notifications).toEqual([]);
  await expect(page).toHaveURL("http://localhost:8791/notifications");
  await cdp.detach();
});
