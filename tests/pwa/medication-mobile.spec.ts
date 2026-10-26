import { execFileSync } from "node:child_process";
import { expect, test } from "@playwright/test";

test.use({ viewport: { width: 390, height: 844 }, timezoneId: "UTC", isMobile: true, hasTouch: true });

test("mobile forms retain rejected input and keyed slots display JST order in a UTC browser", async ({ context, page }) => {
  if (!process.env.PERSIST_DIR) throw new Error("Run npm run test:worker");
  const { cookie, owner } = JSON.parse(execFileSync("python3", ["scripts/pwa-fixture.py"], { encoding: "utf8" }));
  const [name, value] = cookie.split("=");
  await context.addCookies([{ name, value, domain: "localhost", path: "/", secure: true, httpOnly: true, sameSite: "Lax" }]);
  const id = `${owner}-med`;
  const headers = { Accept: "text/event-stream", "Sec-Fetch-Site": "same-origin", "Content-Type": "application/json" };
  const seed = await context.request.post("http://localhost:8791/medications/save", { headers, data: {
    id, revision: 0, enabled: true,
    medication: { name: "モバイル試験", start: "2026-01-01", end: "", repeat: "daily", interval: 1, weekdays: 0 },
    slots: [], pauses: [],
  } });
  expect(seed.status()).toBe(200);
  await page.goto(`/medications/${id}`);
  expect(await page.evaluate(() => Intl.DateTimeFormat().resolvedOptions().timeZone)).toBe("UTC");

  const start = page.getByLabel("開始日", { exact: false }).first();
  await start.focus();
  await start.fill("");
  // Move focus as a visitor would; Chromium's date-picker shadow control
  // can remain focused after calling blur() on its host input.
  await page.getByLabel("お薬の名前", { exact: false }).click();
  await expect(start).toHaveAttribute("aria-invalid", "true");
  await start.fill("2026-01-01");
  const end = page.getByLabel("終了日（任意）", { exact: true });
  await end.fill("2025-12-31");
  const rejected = page.waitForResponse("**/medications/edit/save");
  await page.getByRole("button", { name: "この内容で保存", exact: true }).click();
  expect((await rejected).status()).toBe(200);
  await expect(end).toHaveAttribute("aria-invalid", "true");
  await expect(end).toHaveValue("2025-12-31");
  await expect(page.getByLabel("お薬の名前", { exact: false })).toHaveValue("モバイル試験");
  await end.fill("");
  await page.getByLabel("お薬の名前", { exact: false }).fill("モバイル保存済み");
  await expect(page.getByText("未保存の変更があります。", { exact: true })).toBeVisible();
  let releaseSave!: () => void;
  let saveRequests = 0;
  const saveGate = new Promise<void>((resolve) => { releaseSave = resolve; });
  await page.route("**/medications/edit/save", async (route) => {
    saveRequests += 1;
    await saveGate;
    await route.continue();
  });
  const saveButton = page.getByRole("button", { name: /^(この内容で保存|保存中…)$/ });
  await saveButton.click();
  await expect(saveButton).toBeDisabled();
  await expect.poll(() => saveRequests).toBe(1);
  // Retry by keyboard and by the disabled button while the first response
  // is held. Neither may create another mutation request.
  await page.getByLabel("お薬の名前", { exact: false }).press("Enter");
  await saveButton.evaluate((button) => (button as HTMLButtonElement).click());
  releaseSave();
  await expect(page.getByText("保存しました。", { exact: true })).toBeVisible();
  expect(saveRequests).toBe(1);
  await page.unroute("**/medications/edit/save");
  await expect(page.getByText("未保存の変更があります。", { exact: true })).toBeHidden();

  const time = page.getByLabel("お薬の通知時刻", { exact: false });
  await time.focus();
  await time.fill("");
  await page.getByLabel("1回の量", { exact: false }).click();
  await expect(time).toHaveAttribute("aria-invalid", "true");
  for (const clock of ["20:00", "08:00"]) {
    await time.fill(clock);
    await page.getByLabel("1回の量", { exact: false }).fill("0.5");
    const saved = page.waitForResponse("**/medications/edit/slot/save");
    await page.getByRole("button", { name: "枠を追加", exact: true }).click();
    expect((await saved).status()).toBe(200);
    await expect(page.getByText("服薬枠を保存しました。", { exact: true })).toBeVisible();
    await expect(time).toHaveValue("08:00");
  }
  await expect(page.locator(".med-item").filter({ has: page.getByRole("button", { name: "編集", exact: true }) }).first()).toContainText("08:00");
  await page.goto("/medications");
  await expect(page.locator(".med-dose-time")).toHaveText(["08:00", "20:00"]);
  await expect(page.getByText(/Asia\/Tokyo/).first()).toBeVisible();
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);
  await page.goto(`/medications/${id}`);
  await page.locator(".med-item").filter({ has: page.getByRole("button", { name: "編集", exact: true }) }).first().getByRole("button", { name: "編集", exact: true }).click();
  await page.getByRole("button", { name: "この枠を削除", exact: true }).click();
  await page.getByRole("alertdialog").getByRole("button", { name: "削除する", exact: true }).click();
  await expect(page.getByText("服薬枠を削除しました。", { exact: true })).toBeVisible();
  await page.goto("/medications");
  await expect(page.locator(".med-dose-time")).toHaveText(["20:00"]);
});
