// Real embedded UI signs in through the SDK WebSocket transport; dedicated test account only.
const { chromium } = require("playwright");
const fs = require("node:fs");
(async () => {
  for (const key of [
    "TEALDRIVE_TEST_URL",
    "TEALDRIVE_TEST_USERNAME",
    "TEALDRIVE_TEST_WEB_PASSWORD_FILE",
  ])
    if (!process.env[key]) throw Error("Missing " + key);
  const file = process.env.TEALDRIVE_TEST_WEB_PASSWORD_FILE;
  if (fs.statSync(file).mode & 0o077)
    throw Error("Private secret file required");
  const browser = await chromium.launch({ headless: true });
  try {
    const page = await browser.newPage();
    const errors = [];
    page.on("pageerror", (e) => errors.push(e.name));
    await page.goto(process.env.TEALDRIVE_TEST_URL);
    await page.waitForSelector("#registerWebOption:not(.hidden)");
    if ((await page.locator("#passwordLabel").innerText()) !== "Web password")
      throw Error("Password mode is unclear");
    await page.fill("#usernameInput", process.env.TEALDRIVE_TEST_USERNAME);
    await page.fill("#passwordInput", fs.readFileSync(file, "utf8").trim());
    await page.click("#loginForm button[type=submit]");
    await page.waitForSelector("#loginModal.hidden", { state: "attached" });
    await page.waitForSelector("#wsStatusDot.connected");
    if ((await page.locator("#passwordInput").inputValue()) !== "")
      throw Error("Password field retained secret");
    if (!(await page.locator("#settingsBtn").isDisabled()))
      throw Error("Unprivileged enrollment acquired administration");
    const managementDenied = await page.evaluate(async () => {
      const system = await fetch("/api/admin/accounts/system");
      const virtual = await fetch("/api/admin/virtual/list");
      return system.status === 403 && virtual.status === 403;
    });
    if (!managementDenied)
      throw Error("Unprivileged account read management metadata");
    await page.click("#logoutBtn");
    await page.waitForSelector("#loginModal:not(.hidden)");
    if (errors.length) throw Error("Browser errors: " + errors.join(","));
    console.log(
      "PASS UI SDK auth: mode label, login, file socket, cleared input, role boundary, logout",
    );
  } finally {
    await browser.close();
  }
})().catch((e) => {
  console.error(e.message);
  process.exitCode = 1;
});
