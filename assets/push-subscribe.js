// <push-subscribe vapid-key="..."> registers the Service Worker and
// subscribes to Web Push when its button is clicked. It reports through two
// events: "push-status" with separate support/permission/failure fields, and "push-subscription" with
// { endpoint, p256dh, auth }. The server decides whether the subscription is
// saved; this element never claims that it was.
const toBase64Url = (buffer) =>
  btoa(String.fromCharCode(...new Uint8Array(buffer)))
    .replaceAll("+", "-")
    .replaceAll("/", "_")
    .replaceAll("=", "");

const fromBase64Url = (text) => {
  const padded = text.replaceAll("-", "+").replaceAll("_", "/").padEnd(Math.ceil(text.length / 4) * 4, "=");
  return Uint8Array.from(atob(padded), (c) => c.charCodeAt(0));
};

class PushSubscribe extends HTMLElement {
  static observedAttributes = ["removed-endpoint"];

  attributeChangedCallback() {
    if (this.isConnected && this.supported()) this.clearCompletedRemoval();
  }

  async clearCompletedRemoval() {
    try {
      const endpoint = localStorage.getItem("push-pending-removal");
      if (!endpoint || this.getAttribute("removed-endpoint") !== endpoint) return;
      const registration = await navigator.serviceWorker.getRegistration(this.getAttribute("scope"));
      if (!(await registration?.pushManager.getSubscription())) {
        localStorage.removeItem("push-pending-removal");
        this.dispatchEvent(new CustomEvent("push-missing", { detail: null }));
      }
    } catch { this.report("error"); }
  }

  connectedCallback() {
    this.button = document.createElement("button");
    this.button.type = "button";
    this.button.textContent = this.getAttribute("label") ?? "通知を有効にする";
    this.button.addEventListener("click", () => this.subscribe());
    this.append(this.button);
    const remove = document.createElement("button");
    remove.type = "button";
    remove.textContent = "通知を解除する";
    remove.addEventListener("click", () => this.unsubscribe());
    this.append(remove);
    const test = document.createElement("button");
    test.type = "button";
    test.textContent = "テスト通知を予約";
    test.addEventListener("click", () => this.sendTest());
    this.append(test);
    this.report(this.supported() ? Notification.permission : "unsupported");
    if (this.supported()) this.reconcile();
  }

  supported() {
    return "serviceWorker" in navigator && "PushManager" in window && "Notification" in window;
  }

  report(status) {
    const supported = this.supported();
    const permission = ["default", "granted", "denied"].includes(status)
      ? status : supported ? Notification.permission : "unknown";
    this.dispatchEvent(new CustomEvent("push-status", {
      detail: { supported, permission, failed: status === "error",
        standalone: matchMedia("(display-mode: standalone)").matches || navigator.standalone === true },
    }));
  }

  async reconcile() {
    try {
      const pending = localStorage.getItem("push-pending-removal");
      if (pending) {
        await this.unsubscribe();
        return;
      }
      const registration = await navigator.serviceWorker.getRegistration(this.getAttribute("scope"));
      const subscription = await registration?.pushManager.getSubscription();
      if (subscription) this.dispatchEvent(new CustomEvent("push-check", { detail: subscription.endpoint }));
      else this.dispatchEvent(new CustomEvent("push-missing", { detail: null }));
    } catch { this.report("error"); }
  }

  async unsubscribe() {
    try {
      const registration = await navigator.serviceWorker.getRegistration(this.getAttribute("scope"));
      const subscription = await registration?.pushManager.getSubscription();
      const endpoint = subscription?.endpoint ?? localStorage.getItem("push-pending-removal");
      if (!endpoint) return;
      localStorage.setItem("push-pending-removal", endpoint);
      // Keep the endpoint until both sides acknowledge removal. Replaying
      // this idempotent action repairs a lost response or expired session.
      this.report(Notification.permission);
      this.dispatchEvent(new CustomEvent("push-remove", { detail: endpoint }));
      if (subscription && !(await subscription.unsubscribe())) this.report("error");
      await this.clearCompletedRemoval();
    } catch { this.report("error"); }
  }

  async sendTest() {
    if (!this.supported()) { this.report("unsupported"); return; }
    try {
      const registration = await navigator.serviceWorker.getRegistration(this.getAttribute("scope"));
      const subscription = await registration?.pushManager.getSubscription();
      if (!subscription || localStorage.getItem("push-pending-removal")) {
        this.report("error");
        return;
      }
      this.dispatchEvent(new CustomEvent("push-test", { detail: subscription.endpoint }));
    } catch { this.report("error"); }
  }

  async subscribe() {
    if (!this.supported()) {
      this.report("unsupported");
      return;
    }
    try {
      // The permission prompt must come straight from this click.
      const permission = await Notification.requestPermission();
      this.report(permission);
      if (permission !== "granted") return;
      if (localStorage.getItem("push-pending-removal")) {
        await this.unsubscribe();
        return;
      }
      const registration = await navigator.serviceWorker.register(this.getAttribute("worker"), {
        scope: this.getAttribute("scope"),
      });
      await navigator.serviceWorker.ready;
      const subscription =
        (await registration.pushManager.getSubscription()) ??
        (await registration.pushManager.subscribe({
          userVisibleOnly: true,
          applicationServerKey: fromBase64Url(this.getAttribute("vapid-key")),
        }));
      const json = subscription.toJSON();
      this.dispatchEvent(
        new CustomEvent("push-subscription", {
          detail: { endpoint: json.endpoint, p256dh: json.keys?.p256dh, auth: json.keys?.auth },
        }),
      );
    } catch {
      this.report("error");
    }
  }
}

customElements.define("push-subscribe", PushSubscribe);
