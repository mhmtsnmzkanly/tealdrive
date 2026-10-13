// Explicit own-account history development acceptance; capability stays gated until log quota/undo integration.
const { chromium } = require("playwright");
const fs = require("node:fs");
(async () => {
  if (process.env.TEALDRIVE_ALLOW_ADMIN_ACCEPTANCE !== "1")
    throw Error("Explicit fixture required");
  const { users } = JSON.parse(
    fs.readFileSync(process.env.TEALDRIVE_ADMIN_FIXTURE_MANIFEST, "utf8"),
  );
  const browser = await chromium.launch({ headless: true });
  async function login(user) {
    const page = await browser.newPage();
    await page.goto("http://127.0.0.1:13102");
    await page.evaluate(async (user) => {
      const { createClient } = await import("/sdk/v1/tealdrive.js");
      window.historyApi = createClient({
        storage: null,
        quotaPollMs: 0,
        stateMode: "client",
      });
      await historyApi.handshake();
      await historyApi.auth.login({
        username: user.username,
        password: user.password,
      });
    }, user);
    return page;
  }
  try {
    const admin = await login(users[0]);
    const user = await login(users[1]);
    const suffix = users[1].username.slice(-10);
    const privateName = `history-private-${suffix}.txt`;
    await admin.evaluate(
      async (suffix) =>
        historyApi.files.createText({
          path: `history-admin-${suffix}.txt`,
          content: "admin content",
        }),
      suffix,
    );
    const result = await user.evaluate(
      async ({ privateName, suffix }) => {
        await historyApi.files.createText({
          path: privateName,
          content: "private content must not enter journal",
        });
        await historyApi.files.createText({
          path: `history-visible-${suffix}.txt`,
          content: "visible content must not enter journal",
        });
        const own = (await historyApi.auth.getSession()).accountId;
        const first = await historyApi.files.audit.list({
          limit: 1,
          action: "write",
        });
        if (
          first.items.length !== 1 ||
          !first.hasMore ||
          !first.nextCursor ||
          first.items[0].accountId !== own
        )
          throw Error("Own history first page");
        const next = await fetch(
          "/api/audit?limit=1&action=write&cursor=" +
            encodeURIComponent(first.nextCursor),
        ).then((r) => r.json());
        if (
          next.items.length !== 1 ||
          next.items[0].id === first.items[0].id ||
          next.items[0].accountId !== own
        )
          throw Error("Own history continuation");
        const page = await fetch("/api/audit?limit=200").then((r) => r.json());
        if (
          page.items.some((r) => r.accountId !== own) ||
          JSON.stringify(page).includes("must not enter journal")
        )
          throw Error("History scope/content leak");
        if (!page.items.some((r) => r.sourcePath === privateName))
          throw Error("Private fixture absent before policy");
        const narrowed = await fetch(
          "/api/audit?limit=1&status=success&action=write&rootId=home&path=" +
            encodeURIComponent(privateName) +
            "&from=0&to=" +
            Math.floor(Date.now() / 1000),
        ).then((r) => r.json());
        if (
          narrowed.items.length !== 1 ||
          narrowed.items[0].sourcePath !== privateName
        )
          throw Error("Combined filters before pagination");
        const invalid = await fetch("/api/audit?from=100&to=1");
        if (invalid.status !== 400) throw Error("Invalid time range accepted");
        const unknown = await fetch("/api/audit?accountId=foreign");
        if (unknown.status !== 400)
          throw Error("Unknown scope option accepted");
        return { own, cursor: first.nextCursor };
      },
      { privateName, suffix },
    );
    const foreign = await admin.evaluate(
      async (cursor) =>
        fetch("/api/audit?cursor=" + encodeURIComponent(cursor)).then((r) =>
          r.json(),
        ),
      result.cursor,
    );
    if (foreign.items.some((r) => r.accountId === result.own))
      throw Error("Admin read another user through cursor");
    await user.evaluate(async (privateName) => {
      const view = await historyApi.files.policy.get();
      await historyApi.files.policy.set({
        expectedGeneration: view.account.generation,
        rules: [{ path: privateName, recursive: false, deny: ["discover"] }],
      });
    }, privateName);
    const fresh = await login(users[1]);
    await fresh.evaluate(async (privateName) => {
      const page = await fetch("/api/audit?limit=200").then((r) => r.json());
      if (
        page.items.some(
          (r) => r.sourcePath === privateName || r.targetPath === privateName,
        )
      )
        throw Error("Current policy did not hide history");
    }, privateName);
    console.log(
      JSON.stringify({
        ownHistory: true,
        historyPagination: true,
        actionFilter: true,
        foreignCursorScope: true,
        unknownScopeRejected: true,
        currentPolicyHidesHistory: true,
        noContentHistory: true,
      }),
    );
  } finally {
    await browser.close();
  }
})().catch((e) => {
  console.error(e.name, e.message);
  process.exitCode = 1;
});
