// Real Chromium -> public SDK/HTTP -> installed setuid helper -> unique Linux accounts.
// Run through tools/dev/admin-acceptance.py; never changes roles/policy from the browser.
const { chromium } = require("playwright");
const fs = require("node:fs");
let checkpoint = "startup";
(async () => {
  if (process.env.TEALDRIVE_ALLOW_ADMIN_ACCEPTANCE !== "1")
    throw Error("Explicit fixture opt-in required");
  const manifest = process.env.TEALDRIVE_ADMIN_FIXTURE_MANIFEST;
  if (!manifest || fs.statSync(manifest).mode & 0o077)
    throw Error("Private fixture manifest required");
  const { users, config_dir } = JSON.parse(fs.readFileSync(manifest, "utf8"));
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
    if (process.env.TEALDRIVE_HOME_ACCEPTANCE === "1") {
      checkpoint = "automatic home isolation";
      await target.evaluate(async (otherUsername) => {
        const session = await testApi.auth.getSession();
        if (session.roots.length !== 1 || session.roots[0].rootId !== "home")
          throw Error("Automatic home was not materialized");
        const rootId = "home";
        const environment = await testApi.files.readText({
          rootId,
          path: "/.env",
        });
        if (environment.content !== "home fixture bytes")
          throw Error("Project dotfile unavailable");
        for (const path of ["/.ssh", "/.ssh/fixture-key", "/absent-fixture"]) {
          try {
            await testApi.files.stat({ rootId, path });
            throw Error("Hidden/missing metadata accessible");
          } catch (error) {
            if (error.code !== "NOT_FOUND") throw error;
          }
        }
        for (const path of ["/.ssh/fixture-key", "/absent-fixture"]) {
          try {
            await testApi.files.readText({ rootId, path });
            throw Error("Hidden/missing content accessible");
          } catch (error) {
            if (error.code !== "NOT_FOUND") throw error;
          }
        }
        const query = new URLSearchParams({
          root_id: rootId,
          path: "fixture.txt",
        });
        const response = await fetch("/api/download?" + query);
        if (!response.ok || (await response.text()) !== "fixture bytes")
          throw Error("Automatic home HTTP download failed");
        try {
          await testApi.files.readText({
            rootId,
            path: "/../" + otherUsername + "/fixture.txt",
          });
          throw Error("Another user's home accessible");
        } catch (error) {
          if (
            ![
              "VALIDATION_ERROR",
              "INVALID_PATH",
              "PERMISSION_DENIED",
              "NOT_FOUND",
            ].includes(error.code)
          )
            throw error;
        }
      }, users[0].username);
      console.log(
        JSON.stringify({
          automaticHome: true,
          projectDotfile: true,
          hiddenMetadataAndReadDenied: true,
          httpDownload: true,
          crossHomeRejected: true,
        }),
      );
    }
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
    if (phase === "approval") {
      const pending = await browser.newPage();
      await pending.goto(url);
      await pending.evaluate(async (user) => {
        const { createClient } = await import("/sdk/v1/tealdrive.js");
        globalThis.testApi = createClient({
          stateMode: "client",
          storage: null,
          quotaPollMs: 0,
        });
        await testApi.handshake();
        if (testApi.getInfo().authentication.selfRegistration)
          throw Error("Fixture self-registration must be off");
        try {
          await testApi.auth.beginEnrollment({
            username: user.username,
            linuxPassword: user.linuxPassword,
          });
          throw Error("Unapproved enrollment accepted");
        } catch (error) {
          if (error.code !== "INVALID_CREDENTIALS") throw error;
        }
      }, users[2]);
      await target.evaluate(async (username) => {
        try {
          await testApi.admin.systemUsers.approve({ username });
          throw Error("Normal user approved account");
        } catch (error) {
          if (error.status !== 403) throw error;
        }
      }, users[2].username);
      const helperPath = "/home/tealdrive/policy/helper.toml";
      const original = fs.readFileSync(helperPath);
      const denied = fs.readFileSync(config_dir + "/helper-deny-pending.toml");
      const replace = (data) => {
        const path = helperPath + ".approval-test-new";
        fs.writeFileSync(path, data, { mode: 0o600, flag: "wx" });
        fs.renameSync(path, helperPath);
      };
      replace(denied);
      try {
        await admin.evaluate(async (username) => {
          try {
            await testApi.admin.systemUsers.approve({ username });
            throw Error("Admin overrode root deny");
          } catch (error) {
            if (error.status !== 403) throw error;
          }
        }, users[2].username);
      } finally {
        if (!fs.readFileSync(helperPath).equals(denied))
          throw Error("Helper policy modified externally");
        replace(original);
      }
      const approved = await admin.evaluate(async (username) => {
        const session = await testApi.auth.getSession();
        const extra = await fetch("/api/admin/system/approve", {
          method: "POST",
          headers: {
            "Content-Type": "application/json",
            "x-csrf-token": session.csrfToken,
          },
          body: JSON.stringify({
            username,
            role: "admin",
            uid: 0,
            home: "/root",
          }),
        });
        if (extra.status !== 422)
          throw Error("Client identity/role fields accepted");
        for (const forbidden of ["root", "www-data", "tealdrive"]) {
          try {
            await testApi.admin.systemUsers.approve({ username: forbidden });
            throw Error("Service approval accepted");
          } catch (error) {
            if (error.status !== 403) throw error;
          }
        }
        const account = await testApi.admin.systemUsers.approve({ username });
        if (
          account.role !== "user" ||
          account.passwordEnrolled ||
          account.kind !== "linux"
        )
          throw Error("Approval set credentials or elevated role");
        try {
          await testApi.admin.systemUsers.approve({ username });
          throw Error("Repeat approval silently reset account");
        } catch (error) {
          if (error.status !== 409) throw error;
        }
        return account;
      }, users[2].username);
      await pending.evaluate(async (user) => {
        try {
          await testApi.auth.login({
            username: user.username,
            password: user.password,
          });
          throw Error("Approved user skipped PAM");
        } catch (error) {
          if (error.code !== "INVALID_CREDENTIALS") throw error;
        }
        await testApi.auth.beginEnrollment({
          username: user.username,
          linuxPassword: user.linuxPassword,
        });
      }, users[2]);
      await admin.evaluate(
        (id) => testApi.admin.systemUsers.deny({ accountId: id }),
        approved.id,
      );
      await pending.evaluate(async (user) => {
        try {
          await testApi.auth.completeEnrollment({ webPassword: user.password });
          throw Error("Suspended pending proof completed");
        } catch (error) {
          if (error.code !== "INVALID_CREDENTIALS") throw error;
        }
      }, users[2]);
      try {
        await admin.evaluate(
          (username) => testApi.admin.systemUsers.approve({ username }),
          users[2].username,
        );
        throw Error("Approval cleared admin suspension");
      } catch (error) {
        if (!error.message.includes("CONFLICT")) throw error;
      }
      await admin.evaluate(
        (id) => testApi.admin.systemUsers.allow({ accountId: id }),
        approved.id,
      );
      await pending.evaluate(async (user) => {
        try {
          await testApi.auth.completeEnrollment({ webPassword: user.password });
          throw Error("Old PAM proof revived after reenable");
        } catch (error) {
          if (error.code !== "INVALID_CREDENTIALS") throw error;
        }
        // The bounded pre-auth socket permits five attempts. Restart the transport
        // after the negative cases; do not weaken that security limit for a test.
        testApi.disconnect();
        await testApi.handshake();
        await testApi.auth.beginEnrollment({
          username: user.username,
          linuxPassword: user.linuxPassword,
        });
        const session = await testApi.auth.completeEnrollment({
          webPassword: user.password,
        });
        if (session.role !== "user" || session.accountId === undefined)
          throw Error("Approved enrollment changed role");
        await testApi.files.list({
          rootId: session.roots[0].rootId,
          path: "/",
        });
        await testApi.auth.logout();
      }, users[2]);
      await pending.close();
      console.log(
        JSON.stringify({
          adminApproval: true,
          rootDenyWins: true,
          serviceAccountsDenied: true,
          noClientIdentityOrRole: true,
          approvalRequiresPam: true,
          suspendInvalidatesPendingProof: true,
          oldProofDoesNotRevive: true,
          approvedUserEnrolled: true,
        }),
      );
    }
    if (phase === "ui") {
      checkpoint = "ui_reload";
      const errors = [];
      admin.on("pageerror", (error) => errors.push(error.name));
      target.on("pageerror", (error) => errors.push(error.name));
      await admin.reload();
      await target.reload();
      await admin.waitForSelector("#manageAccountsBtn:not(.hidden)");
      await target.waitForSelector("#wsStatusDot.connected");
      if (
        !(await target
          .locator("#manageAccountsBtn")
          .evaluate((element) => element.classList.contains("hidden")))
      )
        throw Error("Normal user saw management button");
      checkpoint = "ui_open";
      await admin.click("#manageAccountsBtn");
      const row = admin
        .locator("#accountsTableBody tr")
        .filter({ hasText: users[1].username });
      await row.getByRole("button", { name: "Suspend", exact: true }).waitFor();
      const generation = await target.evaluate(
        async () =>
          (await (await fetch("/api/session")).json()).account_generation,
      );
      admin.once("dialog", (dialog) => dialog.dismiss());
      await row.getByRole("button", { name: "Suspend", exact: true }).click();
      const afterCancel = await target.evaluate(
        async () =>
          (await (await fetch("/api/session")).json()).account_generation,
      );
      if (afterCancel !== generation)
        throw Error("Cancelled confirmation mutated account");
      admin.once("dialog", (dialog) => dialog.accept());
      await row.getByRole("button", { name: "Suspend", exact: true }).click();
      await row.getByRole("button", { name: "Enable", exact: true }).waitFor();
      if (
        (await row.locator('[data-label="State"]').innerText()) !== "Suspended"
      )
        throw Error("UI did not show suspension");
      await target.waitForSelector("#wsStatusDot.disconnected", {
        timeout: 5000,
      });
      if (
        (await target.evaluate(
          async () => (await fetch("/api/session")).status,
        )) !== 401
      )
        throw Error("UI suspension left session usable");
      admin.once("dialog", (dialog) => dialog.accept());
      await row.getByRole("button", { name: "Enable", exact: true }).click();
      await row.getByRole("button", { name: "Suspend", exact: true }).waitFor();
      await target.reload();
      await target.waitForSelector("#loginModal:not(.hidden)");
      await target.fill("#usernameInput", users[1].username);
      await target.fill("#passwordInput", users[1].password);
      await target.click("#loginForm button[type=submit]");
      await target.waitForSelector("#wsStatusDot.connected");
      const values = await admin
        .locator("#accountsCandidate option")
        .evaluateAll((elements) => elements.map((element) => element.value));
      if (
        !values.includes(users[2].username) ||
        values.some((name) =>
          ["root", "caddy", "www-data", "tealdrive"].includes(name),
        )
      )
        throw Error("Approval choices not filtered");
      await admin.selectOption("#accountsCandidate", users[2].username);
      admin.once("dialog", (dialog) => dialog.dismiss());
      await admin.click("#accountsApproveBtn");
      if (
        await admin
          .locator("#accountsTableBody tr")
          .filter({ hasText: users[2].username })
          .count()
      )
        throw Error("Cancelled approval created account");
      admin.once("dialog", (dialog) => dialog.accept());
      await admin.click("#accountsApproveBtn");
      const approvedRow = admin
        .locator("#accountsTableBody tr")
        .filter({ hasText: users[2].username });
      await approvedRow.waitFor();
      if (
        (await approvedRow.locator('[data-label="State"]').innerText()) !==
        "Awaiting registration"
      )
        throw Error("UI approval skipped enrollment state");
      const pending = await browser.newPage();
      pending.on("pageerror", (error) => errors.push(error.name));
      await pending.goto(url);
      await pending.waitForSelector("#registerWebOption:not(.hidden)");
      await pending.check("#registerWeb");
      await pending.fill("#usernameInput", users[2].username);
      await pending.fill("#passwordInput", users[2].linuxPassword);
      await pending.click("#loginForm button[type=submit]");
      await pending.waitForSelector("#webPasswordGroup:not(.hidden)");
      if ((await pending.locator("#passwordInput").inputValue()) !== "")
        throw Error("UI retained Linux password");
      await pending.fill("#passwordInput", users[2].password);
      await pending.fill("#webPasswordConfirm", users[2].password);
      await pending.click("#loginForm button[type=submit]");
      await pending.waitForSelector("#loginModal.hidden", {
        state: "attached",
      });
      await pending.waitForSelector("#wsStatusDot.connected");
      if ((await pending.locator("#passwordInput").inputValue()) !== "")
        throw Error("UI retained web password");
      checkpoint = "ui_refresh_registered";
      const refreshed = admin.waitForResponse(
        (response) => response.url().endsWith("/api/admin/accounts/system"),
        { timeout: 5000 },
      );
      await admin.click("#accountsRefreshBtn");
      const response = await refreshed;
      const entry = (await response.json()).find(
        (account) => account.username === users[2].username,
      );
      if (entry?.status !== "active" || !entry.password_enrolled)
        throw Error("Registered account metadata not active");
      await admin.waitForFunction(
        (username) =>
          [...document.querySelectorAll("#accountsTableBody tr")].some(
            (row) =>
              row.textContent.includes(username) &&
              row.querySelector('[data-label="State"]')?.textContent ===
                "Active",
          ),
        users[2].username,
      );
      checkpoint = "ui_virtual_tab";
      await admin.click("#accountsVirtualTab");
      await admin.waitForSelector("#accountsApproveArea.hidden", {
        state: "attached",
      });
      await admin.waitForFunction(
        () =>
          document.getElementById("accountsModal").getAttribute("aria-busy") ===
          "false",
      );
      checkpoint = "ui_system_tab";
      await admin.click("#accountsSystemTab");
      await row.getByRole("button", { name: "Suspend", exact: true }).waitFor();
      checkpoint = "ui_failed_refresh";
      await admin.route("**/api/admin/accounts/system", (route) =>
        route.fulfill({ status: 503, body: "" }),
      );
      await admin.click("#accountsRefreshBtn");
      await admin.waitForFunction(() =>
        document
          .getElementById("accountsError")
          .textContent.includes("temporarily unavailable"),
      );
      if (await admin.locator("#accountsTableBody button").count())
        throw Error("Failed load kept stale mutation controls");
      await admin.unroute("**/api/admin/accounts/system");
      await admin.click("#accountsRefreshBtn");
      await row.getByRole("button", { name: "Suspend", exact: true }).waitFor();
      checkpoint = "ui_mobile_keyboard";
      await admin.setViewportSize({ width: 390, height: 844 });
      if (
        await admin.evaluate(
          () => document.documentElement.scrollWidth > innerWidth + 1,
        )
      )
        throw Error("Mobile management overflow");
      await admin.locator("#accountsCloseBtn").focus();
      await admin.keyboard.press("Shift+Tab");
      if (
        !(await admin.evaluate(() =>
          Boolean(document.activeElement.closest("#accountsModal")),
        ))
      )
        throw Error("Tab focus escaped account modal");
      await admin.keyboard.press("Escape");
      await admin.waitForSelector("#accountsModal.hidden", {
        state: "attached",
      });
      if (
        (await admin.evaluate(() => document.activeElement.id)) !==
        "manageAccountsBtn"
      )
        throw Error("Dialog focus not restored");
      await pending.click("#logoutBtn");
      await target.click("#logoutBtn");
      await admin.click("#logoutBtn");
      if (errors.length)
        throw Error("Browser module errors: " + errors.join(","));
      console.log(
        JSON.stringify({
          uiRoleBoundary: true,
          uiSuspendAndEnable: true,
          uiCancelNoMutation: true,
          uiFilteredApproval: true,
          uiPendingAndActiveStates: true,
          uiPamEnrollment: true,
          uiVirtualGroup: true,
          uiMobileAndEscape: true,
          uiFailedRefreshClearsControls: true,
        }),
      );
    } else {
      await target.evaluate(() => testApi.auth.logout());
      await admin.evaluate(() => testApi.auth.logout());
    }
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
  console.error(checkpoint, error.name, error.code ?? error.message);
  process.exitCode = 1;
});
