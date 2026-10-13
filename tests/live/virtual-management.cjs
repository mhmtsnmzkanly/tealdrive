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
        const initial = await api.admin.quota.getBudget();
        const allocation = Math.max(8192, (initial.minimumQuotaBytes ?? 1) * 2);
        const idempotencyKey = crypto.randomUUID();
        const first = await api.admin.virtualUsers.create({
          name: names[0],
          password,
          quotaBytes: allocation,
          idempotencyKey,
        });
        const replay = await api.admin.virtualUsers.create({
          name: names[0],
          password: "different-password-ignored-on-replay",
          quotaBytes: allocation,
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
          quotaBytes: allocation,
          idempotencyKey: crypto.randomUUID(),
        });
        if (first.uid === second.uid || first.gid === second.gid)
          throw Error("Virtual identities share OS identity");
        const budget = await api.admin.quota.getBudget();
        if (
          budget.reservedBytes !== initial.reservedBytes + allocation * 2 ||
          budget.remainingBytes !== initial.remainingBytes - allocation * 2
        )
          throw Error("Budget reservation mismatch");
        try {
          await api.admin.virtualUsers.create({
            name: "virtual-too-large",
            password,
            quotaBytes:
              Math.ceil(budget.remainingBytes / 1024) * 1024 +
              Math.max(4096, initial.minimumQuotaBytes ?? 1),
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
    checkpoint = "scoped metadata pagination";
    await admin.evaluate(async () => {
      const firstPage = await api.admin.virtualUsers.listPage({
        limit: 1,
        filter: "virtual-",
      });
      if (
        firstPage.items.length !== 1 ||
        !firstPage.hasMore ||
        !firstPage.nextCursor
      )
        throw Error("Metadata first page failed");
      const secondPage = await api.admin.virtualUsers.listPage({
        limit: 1,
        filter: "virtual-",
        cursor: firstPage.nextCursor,
      });
      if (
        secondPage.items.length !== 1 ||
        secondPage.items[0].id === firstPage.items[0].id ||
        secondPage.hasMore
      )
        throw Error("Metadata continuation failed");
      const filtered = await api.admin.virtualUsers.list({
        limit: 1,
        filter: firstPage.items[0].username,
      });
      if (
        !Array.isArray(filtered) ||
        filtered.length !== 1 ||
        filtered[0].id !== firstPage.items[0].id
      )
        throw Error("Compatibility list filter failed");
    });
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
    await first.evaluate(async (allocation) => {
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
      if (api.getInfo().authentication.undo) {
        await api.files.rename({ path: "/a.txt", newName: "a-renamed.txt" });
        const page = await api.files.undo.list({
          action: "rename",
          path: "a.txt",
        });
        const candidate = page.items.find((candidate) => candidate.available);
        if (!candidate) throw Error("Virtual UID missing undo candidate");
        await api.files.undo.apply({
          operationId: candidate.operation.id,
          expectedState: candidate.expectedState,
        });
        if (
          (await api.files.readText({ path: "/a.txt" })).content !==
          "virtual file bytes"
        )
          throw Error("Virtual UID undo failed");
      }
      if (allocation > 8192)
        await api.files.createText({
          path: "/quota-padding.txt",
          content: "p".repeat(65536),
        });
      try {
        await api.auth.revokeSelf();
        throw Error("Virtual user revoked itself");
      } catch (error) {
        if (error.status !== 403) throw error;
      }
    }, created[0].quotaBytes);
    await second.evaluate(() =>
      api.files.createText({
        rootId: "home",
        path: "/b.txt",
        content: "other virtual file",
      }),
    );
    for (const account of created) {
      for (const path of ["/etc/subuid", "/etc/subgid"]) {
        const ranges = fs
          .readFileSync(path, "utf8")
          .trim()
          .split("\n")
          .filter(Boolean)
          .map((line) => line.split(":"));
        if (
          ranges.some(
            ([owner, start, count]) =>
              owner === "tdv_" + account.uid ||
              (!["root", "0"].includes(owner) &&
                account.uid >= Number(start) &&
                account.uid < Number(start) + Number(count)),
          )
        )
          throw Error(
            "Virtual identity can be mapped by another user namespace",
          );
        if (
          !ranges.some(
            ([owner, start, count]) =>
              ["root", "0"].includes(owner) &&
              account.uid >= Number(start) &&
              account.uid < Number(start) + Number(count),
          )
        )
          throw Error("Virtual ID range lacks Root reservation");
      }
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
    await first.evaluate(async (allocation) => {
      try {
        await api.files.createText({
          rootId: "home",
          path: "/too-large.txt",
          content: "x".repeat(Math.max(16384, allocation)),
        });
        throw Error("Quota enforcement failed");
      } catch (error) {
        if (error.code !== "DISK_FULL") throw error;
      }
      const quota = await api.files.quota.get();
      if (
        quota.limitBytes !== allocation ||
        quota.source !== "helper" ||
        quota.stale
      )
        throw Error("Quota source mismatch");
    }, created[0].quotaBytes);
    if (native) {
      const beyond = require("node:child_process").spawnSync(
        "/usr/sbin/runuser",
        [
          "-u",
          "tdv_" + created[0].uid,
          "--",
          "/usr/bin/python3",
          "-c",
          'import pathlib,sys; pathlib.Path(sys.argv[1]).write_bytes(b"x"*int(sys.argv[2]))',
          created[0].home + "/native-overflow",
          String(created[0].quotaBytes * 4),
        ],
        { stdio: "ignore" },
      );
      if (beyond.status === 0)
        throw Error("Kernel quota bypassed without helper");
      if (created[0].quotaBytes > 8192)
        await first.evaluate(async () => {
          await api.files.deletePermanently({ path: "/native-overflow" });
          const history = await fetch(
            "/api/audit?action=delete&path=native-overflow",
          ).then((r) => r.json());
          if (
            !history.items.some(
              (r) =>
                r.status === "success" &&
                r.logStorage === "systemReserve" &&
                r.logReserveBytes === 0,
            )
          )
            throw Error("Full quota cleanup audit reserve");
        });
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
          quotaBytes: account.quotaBytes * 2,
          expectedGeneration: account.generation,
        });
        try {
          await api.admin.virtualUsers.setQuota({
            accountId: account.id,
            quotaBytes: account.quotaBytes,
            expectedGeneration: account.generation,
          });
          throw Error("Quota stale generation accepted");
        } catch (error) {
          if (error.status !== 409) throw error;
        }
      },
      {
        account: created[0],
        shrink: created[0].quotaBytes > 8192 ? 65536 : native ? 4096 : 1,
      },
    );
    await first.waitForFunction(
      () => api.getState().connection === "disconnected",
      {},
      { timeout: 5000 },
    );
    await first.evaluate(
      async ({ user, allocation }) => {
        await api.handshake();
        await api.auth.login(user);
        const quota = await api.files.quota.get();
        if (quota.limitBytes !== allocation * 2)
          throw Error("Updated quota missing");
      },
      {
        user: { username: names[0], password },
        allocation: created[0].quotaBytes,
      },
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
    await admin
      .locator("dialog input[name=quotaBytes]")
      .fill(String(created[0].quotaBytes));
    await admin
      .locator("dialog")
      .getByRole("button", { name: "Create user", exact: true })
      .click();
    await admin.waitForSelector("dialog", { state: "detached" });
    const uiRow = admin
      .locator("#accountsTableBody tr")
      .filter({ hasText: "virtual-ui-" + users[0].username.slice(-10) });
    await uiRow.getByRole("button", { name: "Quota", exact: true }).click();
    await admin
      .locator("dialog input[name=quotaBytes]")
      .fill(String(created[0].quotaBytes * 2));
    await admin
      .locator("dialog")
      .getByRole("button", { name: "Save quota", exact: true })
      .click();
    await admin.waitForSelector("dialog", { state: "detached" });
    await uiRow.getByRole("button", { name: "Delete", exact: true }).waitFor();
    admin.once("dialog", (dialog) => dialog.accept());
    await uiRow.getByRole("button", { name: "Delete", exact: true }).click();
    await uiRow
      .locator('[data-label="State"]')
      .filter({ hasText: "Deleted" })
      .waitFor();
    await admin.click("#accountsCloseBtn");
    await admin.click("#logoutBtn");

    await second.evaluate(() => api.auth.logout());
    await regular.evaluate(() => api.auth.logout());
    console.log(
      JSON.stringify({
        virtualProvisioning: true,
        metadataPagination: true,
        idempotentCreate: true,
        separateUidGid: true,
        subordinateIdsExcluded: true,
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
