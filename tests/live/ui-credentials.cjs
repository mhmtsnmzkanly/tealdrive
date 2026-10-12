// Opt-in real browser test: rotates the dedicated account's credential generation,
// preserving its password value. Never run against a real user's account.
const { chromium } = require("playwright");
const fs = require("node:fs");
(async () => {
  if (process.env.TEALDRIVE_ALLOW_CREDENTIAL_MUTATION !== "1")
    throw Error("Explicit dedicated-account mutation opt-in required");
  for (const key of [
    "TEALDRIVE_TEST_URL",
    "TEALDRIVE_TEST_USERNAME",
    "TEALDRIVE_TEST_WEB_PASSWORD_FILE",
  ])
    if (!process.env[key]) throw Error("Missing " + key);
  const file = process.env.TEALDRIVE_TEST_WEB_PASSWORD_FILE;
  if (fs.statSync(file).mode & 0o077)
    throw Error("Private credential file required");
  const password = fs.readFileSync(file, "utf8").trim();
  const browser = await chromium.launch({ headless: true });
  try {
    const page = await browser.newPage();
    await page.goto(process.env.TEALDRIVE_TEST_URL);
    await page.fill("#usernameInput", process.env.TEALDRIVE_TEST_USERNAME);
    await page.fill("#passwordInput", password);
    await page.click("#loginForm button[type=submit]");
    await page.waitForSelector("#loginModal.hidden", { state: "attached" });
    const before = await page.evaluate(async () => {
      const s = await (await fetch("/api/session")).json();
      const missingCsrf = await fetch("/api/auth/password", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          currentPassword: "invalid-value",
          newPassword: "replacement-test-password",
        }),
      });
      if (missingCsrf.status !== 403) throw Error("Missing CSRF accepted");
      const wrong = await fetch("/api/auth/password", {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
          "x-csrf-token": s.csrf_token,
        },
        body: JSON.stringify({
          currentPassword: "invalid-value",
          newPassword: "replacement-test-password",
        }),
      });
      if (wrong.status !== 401) throw Error("Wrong current password accepted");
      return s.account_generation;
    });
    await page.click("#accountSecurityBtn");
    await page.waitForSelector("#accountSecurityModal:not(.hidden)");
    await page.fill("#accountCurrentPassword", password);
    await page.fill("#accountNewPassword", password);
    await page.fill("#accountConfirmPassword", password);
    await page.click("#accountPasswordForm button[type=submit]");
    await page.waitForSelector("#loginModal:not(.hidden)");
    await page.fill("#usernameInput", process.env.TEALDRIVE_TEST_USERNAME);
    await page.fill("#passwordInput", password);
    await page.click("#loginForm button[type=submit]");
    await page.waitForSelector("#loginModal.hidden", { state: "attached" });
    const after = await page.evaluate(
      async () =>
        (await (await fetch("/api/session")).json()).account_generation,
    );
    if (after <= before) throw Error("Credential generation did not advance");
    await page.click("#logoutBtn");
    console.log(
      JSON.stringify({
        csrf: true,
        wrongPasswordRejected: true,
        uiPasswordChange: true,
        generationAdvanced: true,
        passwordPreserved: true,
      }),
    );
  } finally {
    await browser.close();
  }
})().catch((e) => {
  console.error(e.name, e.message);
  process.exitCode = 1;
});
