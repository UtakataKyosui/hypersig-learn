// Service Worker for Web Push. It only receives a push, shows the
// notification, and opens the app when the notification is clicked.
self.addEventListener("install", () => self.skipWaiting());
self.addEventListener("activate", (event) => event.waitUntil(self.clients.claim()));

self.addEventListener("push", (event) => {
  let payload = {};
  try { payload = event.data?.json() ?? {}; } catch { /* Old probe payload. */ }
  event.waitUntil(
    self.registration.showNotification("服薬の時間です", {
      body: "アプリで今日の予定を確認してください",
      tag: typeof payload.tag === "string" ? payload.tag : "medication-test",
      data: { url: safeDestination(payload.url) },
    }),
  );
});

function safeDestination(value) {
  const allowed = new URL(appRoutes.today, self.location.origin);
  try {
    const candidate = new URL(value, self.location.origin);
    if (candidate.href === allowed.href) return candidate.href;
  } catch { /* Missing or untrusted destination. */ }
  return allowed.href;
}

self.addEventListener("notificationclick", (event) => {
  event.notification.close();
  const url = safeDestination(event.notification.data?.url);
  event.waitUntil(
    self.clients.matchAll({ type: "window", includeUncontrolled: true }).then((windows) => {
      for (const client of windows) {
        if (new URL(client.url).origin === self.location.origin && "focus" in client) {
          return client.navigate(url).then((window) => (window ?? client).focus());
        }
      }
      return self.clients.openWindow(url);
    }),
  );
});
