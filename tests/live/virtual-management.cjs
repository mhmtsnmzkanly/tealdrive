// Public SDK -> root helper -> distinct locked UID/GID; explicit fixture only.
const { chromium } = require("playwright");
const fs = require("node:fs");
const { execFileSync } = require("node:child_process");
let checkpoint = "startup";
(async () => {
  if (
    process.env.TEALDRIVE_ALLOW_ADMIN_ACCEPTANCE !== "1" ||
    process.env.TEALDRIVE_VIRTUAL_ACCEPTANCE !== "1"
  )
    throw Error("Explicit virtual fixture required");
  const { users } = JSON.parse(
    fs.readFileSync(process.env.TEALDRIVE_ADMIN_FIXTURE_MANIFEST, "utf8"),
  );
  const native = process.env.TEALDRIVE_NATIVE_QUOTA_ACCEPTANCE === "1";
  const browser = await chromium.launch({ headless: true });
  const url = "http://127.0.0.1:13102";
  async function page(user) {
    const page = await browser.newPage();
    await page.goto(url);
    await page.evaluate(async (user) => {
      const { createClient } = await import("/sdk/v1/tealdrive.js");
      globalThis.api = createClient({
        stateMode: "client",
        storage: null,
        quotaPollMs: 0,
      });
      await api.handshake();
      await api.auth.login({
        username: user.username,
        password: user.password,
      });
    }, user);
    return page;
  }
  try {
    const admin = await page(users[0]);
    const regular = await page(users[1]);
    const names = [
      "virtual-a-" + users[0].username.slice(-10),
      "virtual-b-" + users[0].username.slice(-10),
    ];
    const password = users[1].password;
    checkpoint = "create";
    const created = await admin.evaluate(
      async ({ names, password }) => {
        const idempotencyKey = crypto.randomUUID();
        const first = await api.admin.virtualUsers.create({
          name: names[0],
          password,
          quotaBytes: 8192,
          idempotencyKey,
        });
        const replay = await api.admin.virtualUsers.create({
          name: names[0],
          password: "different-password-ignored-on-replay",
          quotaBytes: 8192,
          idempotencyKey,
        });
        if (
          first.id !== replay.id ||
          !first.passwordEnrolled ||
          first.uid === 0
        )
          throw Error("Create/idempotency failed");
        const second = await api.admin.virtualUsers.create({
          name: names[1],
          password,
          quotaBytes: 8192,
          idempotencyKey: crypto.randomUUID(),
        });
        if (first.uid === second.uid || first.gid === second.gid)
          throw Error("Virtual identities share OS identity");
        const budget = await api.admin.quota.getBudget();
        if (budget.reservedBytes !== 16384 || budget.remainingBytes !== 49152)
          throw Error("Budget reservation mismatch");
        try {
          await api.admin.virtualUsers.create({
            name: "virtual-too-large",
            password,
            quotaBytes: 65536,
            idempotencyKey: crypto.randomUUID(),
          });
          throw Error("Budget overbooking");
        } catch (error) {
          if (error.code !== "QUOTA_EXCEEDED") throw error;
        }
        return [first, second];
      },
      { names, password },
    );
    await regular.evaluate(async (password) => {
      try {
        await api.admin.virtualUsers.create({
          name: "virtual-unprivileged",
          password,
          quotaBytes: 8192,
          idempotencyKey: crypto.randomUUID(),
        });
        throw Error("Normal user provisioned identity");
      } catch (error) {
        if (error.status !== 403) throw error;
      }
    }, password);
    const first = await page({ username: names[0], password });
    const second = await page({ username: names[1], password });
    checkpoint = "UID files and quota";
    await first.evaluate(async () => {
      await api.files.createText({
        rootId: "home",
        path: "/a.txt",
        content: "virtual file bytes",
      });
      const content = await api.files.readText({
        rootId: "home",
        path: "/a.txt",
      });
      if (content.content !== "virtual file bytes")
        throw Error("Real UID helper read failed");
      try {
        await api.auth.revokeSelf();
        throw Error("Virtual user revoked itself");
      } catch (error) {
        if (error.status !== 403) throw error;
      }
    });
    await second.evaluate(() =>
      api.files.createText({
        rootId: "home",
        path: "/b.txt",
        content: "other virtual file",
      }),
    );
    for (const account of created) {
      const passwd = execFileSync(
        "/usr/bin/getent",
        ["passwd", "tdv_" + account.uid],
        { encoding: "utf8" },
      )
        .trim()
        .split(":");
      if (
        Number(passwd[2]) !== account.uid ||
        Number(passwd[3]) !== account.gid ||
        passwd[5] !== account.home ||
        passwd[6] !== "/usr/sbin/nologin"
      )
        throw Error("NSS identity mismatch");
      const shadow = execFileSync(
        "/usr/bin/getent",
        ["shadow", "tdv_" + account.uid],
        { encoding: "utf8" },
      ).split(":")[1];
      if (!/^[!*]/.test(shadow)) throw Error("OS password unlocked");
    }
    const noRead = require("node:child_process").spawnSync(
      "/usr/sbin/runuser",
      [
        "-u",
        "tdv_" + created[0].uid,
        "--",
        "/usr/bin/cat",
        created[1].home + "/b.txt",
      ],
      { stdio: "ignore" },
    );
    if (noRead.status === 0) throw Error("Unix isolation failed");
    await first.evaluate(async () => {
      try {
        await api.files.createText({
          rootId: "home",
          path: "/too-large.txt",
          content: "x".repeat(16384),
        });
        throw Error("Quota enforcement failed");
      } catch (error) {
        if (error.code !== "DISK_FULL") throw error;
      }
      const quota = await api.files.quota.get();
      if (quota.limitBytes !== 8192 || quota.source !== "helper" || quota.stale)
        throw Error("Quota source mismatch");
    });
    if (native) {
      const beyond = require("node:child_process").spawnSync(
        "/usr/sbin/runuser",
        [
          "-u",
          "tdv_" + created[0].uid,
          "--",
          "/usr/bin/python3",
          "-c",
          'import pathlib,sys; pathlib.Path(sys.argv[1]).write_bytes(b"x"*65536)',
          created[0].home + "/native-overflow",
        ],
        { stdio: "ignore" },
      );
      if (beyond.status === 0)
        throw Error("Kernel quota bypassed without helper");
    }
    checkpoint = "quota mutation";
    await admin.evaluate(
      async ({ account, shrink }) => {
        try {
          await api.admin.virtualUsers.setQuota({
            accountId: account.id,
            quotaBytes: shrink,
            expectedGeneration: account.generation,
          });
          throw Error("Quota shrank below usage");
        } catch (error) {
          if (error.code !== "QUOTA_EXCEEDED") throw error;
        }
        await api.admin.virtualUsers.setQuota({
          accountId: account.id,
          quotaBytes: 16384,
          expectedGeneration: account.generation,
        });
        try {
          await api.admin.virtualUsers.setQuota({
            accountId: account.id,
            quotaBytes: 8192,
            expectedGeneration: account.generation,
          });
          throw Error("Quota stale generation accepted");
        } catch (error) {
          if (error.status !== 409) throw error;
        }
      },
      { account: created[0], shrink: native ? 4096 : 1 },
    );
    await first.waitForFunction(
      () => api.getState().connection === "disconnected",
      {},
      { timeout: 5000 },
    );
    await first.evaluate(
      async (user) => {
        await api.handshake();
        await api.auth.login(user);
        const quota = await api.files.quota.get();
        if (quota.limitBytes !== 16384) throw Error("Updated quota missing");
      },
      { username: names[0], password },
    );
    checkpoint = "delete retention";
    await admin.evaluate(async (account) => {
      const list = await api.admin.virtualUsers.list();
      const fresh = list.find((a) => a.id === account.id);
      await api.admin.virtualUsers.delete({
        accountId: fresh.id,
        expectedGeneration: fresh.generation,
      });
    }, created[0]);
    await first.waitForFunction(
      () => api.getState().connection === "disconnected",
      {},
      { timeout: 5000 },
    );
    if (!fs.existsSync(created[0].home + "/a.txt"))
      throw Error("Deletion removed data before retention");
    checkpoint = "virtual UI";
    await admin.reload();
    await admin.waitForSelector("#manageAccountsBtn:not(.hidden)");
    await admin.click("#manageAccountsBtn");
    await admin.click("#accountsVirtualTab");
    await admin.waitForSelector("#accountsVirtualArea:not(.hidden)");
    await admin.click("#accountsCreateVirtualBtn");
    await admin
      .locator("dialog")
      .getByRole("button", { name: "Cancel", exact: true })
      .click();
    await admin.click("#accountsCreateVirtualBtn");
    await admin
      .locator("dialog input[name=name]")
      .fill("virtual-ui-" + users[0].username.slice(-10));
    await admin.locator("dialog input[name=password]").fill(password);
    await admin.locator("dialog input[name=quotaBytes]").fill("8192");
    await admin
      .locator("dialog")
      .getByRole("button", { name: "Create user", exact: true })
      .click();
    await admin.waitForSelector("dialog", { state: "detached" });
    const uiRow = admin
      .locator("#accountsTableBody tr")
      .filter({ hasText: "virtual-ui-" + users[0].username.slice(-10) });
    await uiRow.getByRole("button", { name: "Quota", exact: true }).click();
    await admin.locator("dialog input[name=quotaBytes]").fill("16384");
    await admin
      .locator("dialog")
      .getByRole("button", { name: "Save quota", exact: true })
      .click();
    await admin.waitForSelector("dialog", { state: "detached" });
    await uiRow.getByRole("button", { name: "Delete", exact: true }).waitFor();
    admin.once("dialog", (dialog) => dialog.accept());
    await uiRow.getByRole("button", { name: "Delete", exact: true }).click();
    await uiRow.locator('[data-label="State"]').filter({hasText:"Deleted"}).waitFor();
    await admin.click("#accountsCloseBtn");
    await admin.click("#logoutBtn");

    await second.evaluate(() => api.auth.logout());
    await regular.evaluate(() => api.auth.logout());
    console.log(
      JSON.stringify({
        virtualProvisioning: true,
        idempotentCreate: true,
        separateUidGid: true,
        lockedNologin: true,
        unixIsolation: true,
        quotaBudget: true,
        helperQuota: true,
        nativeQuota: native,
        virtualUi: true,
        quotaCasAndSessions: true,
        virtualSelfRevokeDenied: true,
        deletedDataRetained: true,
      }),
    );
  } finally {
    await browser.close();
  }
})().catch((error) => {
  console.error(checkpoint, error.name, error.code ?? error.message);
  process.exitCode = 1;
});
