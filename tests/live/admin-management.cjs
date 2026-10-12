// Real Chromium -> public SDK/HTTP -> installed setuid helper -> unique Linux accounts.
// Run through tools/dev/admin-acceptance.py; never changes roles/policy from the browser.
const { chromium } = require("playwright");
const fs = require("node:fs");
(async () => {
  if (process.env.TEALDRIVE_ALLOW_ADMIN_ACCEPTANCE !== "1")
    throw Error("Explicit fixture opt-in required");
  const manifest = process.env.TEALDRIVE_ADMIN_FIXTURE_MANIFEST;
  if (!manifest || fs.statSync(manifest).mode & 0o077)
    throw Error("Private fixture manifest required");
  const { users } = JSON.parse(fs.readFileSync(manifest, "utf8"));
  const phase = process.env.TEALDRIVE_ADMIN_ACCEPTANCE_PHASE ?? "access";
  const browser = await chromium.launch({ headless: true });
  const url = "http://127.0.0.1:13102";
  try {
    const admin = await browser.newPage();
    const target = await browser.newPage();
    for (const [page, user] of [
      [admin, users[0]],
      [target, users[1]],
    ]) {
      await page.goto(url);
      await page.evaluate(async ({ username, password }) => {
        const { createClient } = await import("/sdk/v1/tealdrive.js");
        globalThis.testApi = createClient({
          stateMode: "client",
          storage: null,
          quotaPollMs: 0,
        });
        await testApi.handshake();
        await testApi.auth.login({ username, password });
      }, user);
    }
    const targetSession = await target.evaluate(async () => {
      const session = await testApi.auth.getSession();
      const content = await testApi.files.readText({
        rootId: session.roots[0].rootId,
        path: "/fixture.txt",
      });
      if (content.content !== "fixture bytes")
        throw Error("Real helper content mismatch");
      try {
        await testApi.admin.systemUsers.accounts();
        throw Error("Normal user managed accounts");
      } catch (error) {
        if (error.status !== 403) throw error;
      }
      return session;
    });
    const oldCookies = await target.context().cookies();
    await admin.evaluate(
      async ({ id, username }) => {
        const accounts = await testApi.admin.systemUsers.accounts();
        if (
          !accounts.some(
            (account) => account.id === id && account.username === username,
          )
        )
          throw Error("Registered target missing");
        const session = await testApi.auth.getSession();
        if (session.role !== "admin" || !session.isAdmin)
          throw Error("Root admin fixture missing authority");
        const response = await fetch("/api/admin/system/deny", {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({ accountId: id }),
        });
        if (response.status !== 403) throw Error("Missing CSRF accepted");
        try {
          await testApi.admin.virtualUsers.disable({ accountId: id });
          throw Error("Account kind confusion");
        } catch (error) {
          if (error.status !== 403) throw error;
        }
        try {
          await testApi.admin.systemUsers.deny({
            accountId: session.accountId,
          });
          throw Error("Admin self suspension accepted");
        } catch (error) {
          if (error.status !== 403) throw error;
        }
        const updated = await testApi.admin.systemUsers.deny({ accountId: id });
        if (updated.status !== "suspended" || updated.loginEnabled)
          throw Error("Suspend failed");
      },
      { id: targetSession.accountId, username: users[1].username },
    );
    await target.waitForFunction(
      () => testApi.getState().connection === "disconnected",
      {},
      { timeout: 5000 },
    );
    await target.evaluate(async (user) => {
      try {
        await testApi.files.list({ rootId: "accept_1", path: "/" });
        throw Error("Suspended files accessible");
      } catch (error) {
        if (
          ![
            "HANDSHAKE_REQUIRED",
            "AUTH_REQUIRED",
            "CONNECTION_CLOSED",
            "PERMISSION_DENIED",
          ].includes(error.code)
        )
          throw error;
      }
      await testApi.handshake();
      try {
        await testApi.auth.login({
          username: user.username,
          password: user.password,
        });
        throw Error("Suspended login accepted");
      } catch (error) {
        if (error.code !== "INVALID_CREDENTIALS") throw error;
      }
    }, users[1]);
    await admin.evaluate(async (id) => {
      const updated = await testApi.admin.systemUsers.allow({ accountId: id });
      if (updated.status !== "active" || !updated.loginEnabled)
        throw Error("Enable failed");
    }, targetSession.accountId);
    const stale = await browser.newPage();
    await stale.context().addCookies(oldCookies);
    await stale.goto(url);
    const oldStatus = await stale.evaluate(
      async () => (await fetch("/api/session")).status,
    );
    if (oldStatus !== 401) throw Error("Re-enable resurrected old cookie");
    await stale.close();
    await target.evaluate(
      async ({ user, generation }) => {
        const session = await testApi.auth.login({
          username: user.username,
          password: user.password,
        });
        if (session.accountGeneration <= generation)
          throw Error("Generation did not advance");
        await testApi.files.list({
          rootId: session.roots[0].rootId,
          path: "/",
        });
      },
      { user: users[1], generation: targetSession.accountGeneration },
    );
    if (phase !== "access") throw Error("Phase not implemented yet");
    await target.evaluate(() => testApi.auth.logout());
    await admin.evaluate(() => testApi.auth.logout());
    console.log(
      JSON.stringify({
        publicAdminSuspend: true,
        liveSocketRevoked: true,
        suspendedLoginRejected: true,
        publicAdminEnable: true,
        oldCookieRejected: true,
        realHelperFilesRestored: true,
        kindAndRoleAndCsrfBoundaries: true,
      }),
    );
  } finally {
    await browser.close();
  }
})().catch((error) => {
  console.error(error.name, error.code ?? error.message);
  process.exitCode = 1;
});
