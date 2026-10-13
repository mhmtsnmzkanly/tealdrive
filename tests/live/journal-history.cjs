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
    const context = await browser.newContext();
    const page = await context.newPage();
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
        const prefs = await historyApi.files.undo.getPreferences();
        if (prefs.hours !== null || prefs.effectiveHours !== prefs.maximumHours)
          throw Error("Undo default must inherit Root maximum");
        const changed = await historyApi.files.undo.setPreferences({
          hours: 1,
          expectedRevision: prefs.revision,
        });
        if (
          changed.effectiveHours !== 1 ||
          changed.revision !== prefs.revision + 1
        )
          throw Error("Undo preference update");
        let staleRejected = false;
        try {
          await historyApi.files.undo.setPreferences({
            hours: 1,
            expectedRevision: prefs.revision,
          });
        } catch (error) {
          staleRejected = error.code === "CONFLICT";
        }
        if (!staleRejected) throw Error("Stale undo preference accepted");
        await historyApi.files.undo.setPreferences({
          hours: null,
          expectedRevision: changed.revision,
        });
        const undoPath = `undo-fixture-${suffix}.txt`;
        await historyApi.files.createText({
          path: undoPath,
          content: "undo content",
        });
        async function undoCandidate(action, sourcePath) {
          const page = await historyApi.files.undo.list({ limit: 200 });
          const item = page.items.find(
            (item) =>
              item.operation.action === action &&
              item.operation.sourcePath === sourcePath &&
              item.operation.undoStatus === null,
          );
          if (!item?.available || !item.expectedState)
            throw Error(
              `Missing undo candidate ${action}: ${JSON.stringify(item)}`,
            );
          return item;
        }
        async function apply(item) {
          await historyApi.files.undo.apply({
            operationId: item.operation.id,
            expectedState: item.expectedState,
          });
        }
        await historyApi.files.chmod({ path: undoPath, mode: 0 });
        const chmod = await undoCandidate("chmod", undoPath);
        await apply(chmod);
        if (
          (await historyApi.files.readText({ path: undoPath })).content !==
          "undo content"
        )
          throw Error("Mode000 undo did not restore readability");
        let duplicate = false;
        try {
          await apply(chmod);
        } catch (error) {
          duplicate = error.code === "CONFLICT";
        }
        if (!duplicate) throw Error("Repeated undo accepted");
        const dir = `undo-tree-${suffix}`;
        await historyApi.files.createFolder({ path: dir });
        await historyApi.files.createText({
          path: `${dir}/nested.txt`,
          content: "nested",
        });
        await historyApi.files.rename({ path: dir, newName: `${dir}-renamed` });
        await apply(await undoCandidate("rename", dir));
        if (
          (await historyApi.files.readText({ path: `${dir}/nested.txt` }))
            .content !== "nested"
        )
          throw Error("Directory undo");
        const renamed = `undo-renamed-${suffix}.txt`;
        await historyApi.files.rename({ path: undoPath, newName: renamed });
        await apply(await undoCandidate("rename", undoPath));
        await historyApi.files.createFolder({ path: `undo-dir-${suffix}` });
        await historyApi.files.move({
          source: { path: undoPath },
          destination: { path: `undo-dir-${suffix}/moved.txt` },
        });
        await apply(await undoCandidate("move", undoPath));
        await historyApi.files.trash.move({ path: undoPath });
        await apply(await undoCandidate("trash", undoPath));
        if (
          (await historyApi.files.readText({ path: undoPath })).content !==
          "undo content"
        )
          throw Error("Trash undo content");
        await historyApi.files.rename({ path: undoPath, newName: renamed });
        const changedCandidate = await undoCandidate("rename", undoPath);
        const text = await historyApi.files.readText({ path: renamed });
        await historyApi.files.writeText({
          path: renamed,
          content: "changed content",
          expectedRevision: text.revision,
        });
        let mismatch = false;
        try {
          await apply(changedCandidate);
        } catch (error) {
          mismatch = error.code === "REVISION_CONFLICT";
        }
        if (
          !mismatch ||
          (await historyApi.files.readText({ path: renamed })).content !==
            "changed content"
        )
          throw Error("Changed state undo was not rejected");
        const conflictPath = `undo-conflict-${suffix}.txt`;
        await historyApi.files.createText({
          path: conflictPath,
          content: "original",
        });
        await historyApi.files.rename({
          path: conflictPath,
          newName: `${conflictPath}-moved`,
        });
        const conflictCandidate = await undoCandidate("rename", conflictPath);
        await historyApi.files.createText({
          path: conflictPath,
          content: "new occupant",
        });
        let conflict = false;
        try {
          await apply(conflictCandidate);
        } catch (error) {
          conflict = error.code === "ALREADY_EXISTS";
        }
        if (
          !conflict ||
          (await historyApi.files.readText({ path: conflictPath })).content !==
            "new occupant" ||
          (await historyApi.files.readText({ path: `${conflictPath}-moved` }))
            .content !== "original"
        )
          throw Error("Undo overwrote conflict target");
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
        const uiPath = `ui-undo-${suffix}.txt`;
        await historyApi.files.createText({ path: uiPath, content: "UI undo" });
        await historyApi.files.rename({
          path: uiPath,
          newName: `${uiPath}-renamed`,
        });
        return {
          uiPath,
          own,
          cursor: first.nextCursor,
          undoId: chmod.operation.id,
          expectedState: chmod.expectedState,
        };
      },
      { privateName, suffix },
    );
    const ui = await user.context().newPage();
    await ui.goto("http://127.0.0.1:13102");
    await ui.waitForSelector("#loginModal.hidden", { state: "attached" });
    await ui.click("#historyBtn");
    await ui.waitForSelector("#historyDialog[open]");
    await ui.waitForFunction(
      () => !document.querySelector("#historyRefresh").disabled,
    );
    await ui.selectOption("#historyMode", "undo");
    await ui.waitForFunction(
      () => !document.querySelector("#historyRefresh").disabled,
    );
    await ui.fill("#historyPath", result.uiPath);
    await ui.click("#historyRefresh");
    const undoRow = ui
      .locator("#historyItems .history-row")
      .filter({ hasText: result.uiPath });
    await undoRow.getByRole("button", { name: "Undo", exact: true }).click();
    await ui.getByRole("button", { name: "Cancel", exact: true }).click();
    if (
      (await user.evaluate(
        async (path) => (await historyApi.files.readText({ path })).content,
        result.uiPath + "-renamed",
      )) !== "UI undo"
    )
      throw Error("UI undo cancel mutated files");
    await undoRow.getByRole("button", { name: "Undo", exact: true }).click();
    await ui.click("#historyConfirmUndo");
    await ui.waitForFunction(
      () => !document.querySelector("#historyRefresh").disabled,
    );
    if (
      (await user.evaluate(
        async (path) => (await historyApi.files.readText({ path })).content,
        result.uiPath,
      )) !== "UI undo"
    )
      throw Error("UI undo did not restore");
    await ui.uncheck("#historyInherit");
    await ui.fill("#historyHours", "1");
    await ui.click("#historySavePreference");
    await ui.waitForFunction(
      () => !document.querySelector("#historyRefresh").disabled,
    );
    if (
      (await user.evaluate(() => historyApi.files.undo.getPreferences()))
        .effectiveHours !== 1
    )
      throw Error("UI undo preference not persisted");
    await ui.check("#historyInherit");
    await ui.click("#historySavePreference");
    await ui.waitForFunction(
      () => !document.querySelector("#historyRefresh").disabled,
    );
    await ui.press("#historyClose", "Escape");
    await ui.waitForSelector("#historyDialog", { state: "detached" });
    if (
      !(await ui
        .locator("#historyBtn")
        .evaluate((node) => node === document.activeElement))
    )
      throw Error("History focus not restored");
    const foreign = await admin.evaluate(
      async (cursor) =>
        fetch("/api/audit?cursor=" + encodeURIComponent(cursor)).then((r) =>
          r.json(),
        ),
      result.cursor,
    );
    if (foreign.items.some((r) => r.accountId === result.own))
      throw Error("Admin read another user through cursor");
    await admin.evaluate(async (result) => {
      let denied = false;
      try {
        await historyApi.files.undo.apply({
          operationId: result.undoId,
          expectedState: result.expectedState,
        });
      } catch (error) {
        denied =
          error.status === 403 ||
          error.code === "FORBIDDEN" ||
          error.code === "HTTP_403";
      }
      if (!denied) throw Error("Admin could apply foreign own undo");
    }, result);
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
        undoPreferencesCas: true,
        undoChmodMode000: true,
        undoRenameMoveTrashDirectory: true,
        undoChangedStateRejected: true,
        undoTargetPreserved: true,
        ownHistoryUi: true,
        undoUiCancelConfirm: true,
        undoUiPreferences: true,
      }),
    );
  } finally {
    await browser.close();
  }
})().catch((e) => {
  console.error(e.name, e.message);
  process.exitCode = 1;
});
