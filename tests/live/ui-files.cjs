// Real embedded UI file manager uses the same SDK as a custom frontend.
const { chromium } = require("playwright");
const fs = require("node:fs");
(async () => {
  if (process.env.TEALDRIVE_ALLOW_ADMIN_ACCEPTANCE !== "1")
    throw Error("Explicit fixture required");
  const { users } = JSON.parse(
    fs.readFileSync(process.env.TEALDRIVE_ADMIN_FIXTURE_MANIFEST, "utf8"),
  );
  const user = users[1];
  const browser = await chromium.launch({ headless: true });
  try {
    const page = await browser.newPage();
    const sockets = [];
    page.on("websocket", (socket) => sockets.push(socket.url()));
    await page.goto("http://127.0.0.1:13102");
    await page.fill("#usernameInput", user.username);
    await page.fill("#passwordInput", user.password);
    await page.locator("#loginForm button[type=submit]").click();
    await page.waitForSelector("#loginModal.hidden", { state: "attached" });
    await page.waitForFunction(
      () => document.querySelector("#wsStatusText").textContent === "Connected",
    );
    if (
      sockets.some((url) => !url.endsWith("/ws/auth")) ||
      sockets.length !== 1
    )
      throw Error("UI created a private/duplicate socket");
    const folder = "ui-sdk-" + user.username.slice(-10);
    async function prompt(button, value) {
      await page.click(button);
      await page.fill("#promptInput", value);
      await page.locator("#promptForm button[type=submit]").click();
      await page.waitForSelector("#promptModal.hidden", { state: "attached" });
    }
    await prompt("#newFolderBtn", folder);
    await page
      .locator("#fileTableBody .item-name")
      .filter({ hasText: folder })
      .click();
    await prompt("#newFileBtn", "edit.txt");
    let row = page
      .locator("#fileTableBody tr")
      .filter({
        has: page.locator(".item-name").filter({ hasText: "edit.txt" }),
      });
    await row.getByRole("button", { name: "Edit", exact: true }).click();
    await page.waitForSelector("#editorModal:not(.hidden)");
    await page.fill("#editorTextarea", "first UI content");
    await page.click("#editorSaveBtn");
    await page.waitForSelector("#editorModal.hidden", { state: "attached" });
    await page.reload();
    await page.waitForFunction(
      () => document.querySelector("#wsStatusText").textContent === "Connected",
    );
    await page.locator("#breadcrumbs").filter({ hasText: folder }).waitFor();
    row = page
      .locator("#fileTableBody tr")
      .filter({
        has: page.locator(".item-name").filter({ hasText: "edit.txt" }),
      });
    await row.getByRole("button", { name: "Edit", exact: true }).click();
    await page.waitForSelector("#editorModal:not(.hidden)");
    if ((await page.inputValue("#editorTextarea")) !== "first UI content")
      throw Error("SDK write/read mismatch");
    await page.fill("#editorTextarea", "keep unsaved conflict");
    await page.evaluate(
      async ({ folder }) => {
        const { createClient } = await import("/sdk/v1/tealdrive.js");
        globalThis.testApi = createClient({
          stateMode: "client",
          storage: null,
          quotaPollMs: 0,
        });
        await testApi.handshake();
        const file = await testApi.files.readText({
          rootId: "home",
          path: folder + "/edit.txt",
        });
        await testApi.files.writeText({
          rootId: "home",
          path: folder + "/edit.txt",
          content: "external content",
          expectedRevision: file.revision,
        });
      },
      { folder },
    );
    await page.click("#editorSaveBtn");
    await page.waitForFunction(() =>
      document
        .querySelector("#editorMetaHint")
        .textContent.includes("REVISION_CONFLICT"),
    );
    if ((await page.inputValue("#editorTextarea")) !== "keep unsaved conflict")
      throw Error("Conflict discarded unsaved input");
    await page.click("#editorCancelBtn");
    await page.setInputFiles("#fileInput", {
      name: "upload.txt",
      mimeType: "text/plain",
      buffer: Buffer.from("SDK upload bytes"),
    });
    await page
      .locator("#fileTableBody .item-name")
      .filter({ hasText: "upload.txt" })
      .waitFor();
    await row.getByRole("button", { name: "Permissions", exact: true }).click();
    await page.waitForSelector("#chmodModal:not(.hidden)");
    await page.fill("#chmodOctalInput", "600");
    await page.click("#chmodApplyBtn");
    await page.waitForSelector("#chmodModal.hidden", { state: "attached" });
    await page.fill("#searchInput", "upload");
    await page
      .locator("#fileTableBody .item-name")
      .filter({ hasText: "upload.txt" })
      .waitFor();
    await page.fill("#searchInput", "");
    await page.evaluate(
      async ({ folder }) => {
        await testApi.files.trash.move({ rootId: "home", path: folder });
        testApi.disconnect();
      },
      { folder },
    );
    await page.click("#refreshBtn");
    await page.waitForFunction(
      () =>
        !document.querySelector("#breadcrumbs").textContent.includes("ui-sdk-"),
    );
    console.log(
      JSON.stringify({
        uiSharedSdk: true,
        singleAuthSocket: true,
        crudAndUpload: true,
        managedReloadState: true,
        unsavedConflictPreserved: true,
        chmodSelector: true,
        missingDirectoryRecovery: true,
      }),
    );
  } finally {
    await browser.close();
  }
})().catch((error) => {
  console.error(error.name, error.code ?? error.message);
  process.exitCode = 1;
});
