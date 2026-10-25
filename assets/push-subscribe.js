// <push-subscribe vapid-key="..."> registers the Service Worker and
// subscribes to Web Push when its button is clicked. It reports through two
// events: "push-status" with a string, and "push-subscription" with
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
  connectedCallback() {
    this.button = document.createElement("button");
    this.button.type = "button";
    this.button.textContent = this.getAttribute("label") ?? "通知を有効にする";
    this.button.addEventListener("click", () => this.subscribe());
    this.append(this.button);
    this.report(this.supported() ? Notification.permission : "unsupported");
  }

  supported() {
    return "serviceWorker" in navigator && "PushManager" in window && "Notification" in window;
  }

  report(status) {
    this.dispatchEvent(new CustomEvent("push-status", { detail: status }));
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
    } catch (error) {
      this.report("error");
      console.error(error);
    }
  }
}

customElements.define("push-subscribe", PushSubscribe);
