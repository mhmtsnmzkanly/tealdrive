// Actual Chromium -> WS -> root SQLite/PAM/helper. Dedicated account and private secret files only.
const { chromium } = require("playwright");
const fs = require("node:fs");
const crypto = require("node:crypto");
(async () => {
  for (const key of [
    "TEALDRIVE_TEST_URL",
    "TEALDRIVE_TEST_USERNAME",
    "TEALDRIVE_TEST_PASSWORD_FILE",
    "TEALDRIVE_TEST_WEB_PASSWORD_FILE",
  ])
    if (!process.env[key]) throw Error("Missing " + key);
  function secret(file) {
    if (fs.statSync(file).mode & 0o077)
      throw Error("Secret file is not private");
    return fs.readFileSync(file, "utf8").trim();
  }
  const browser = await chromium.launch({ headless: true });
  try {
    const page = await browser.newPage();
    await page.goto(process.env.TEALDRIVE_TEST_URL);
    const result = await page.evaluate(
      async ({ username, linuxPassword, webPassword, directory }) => {
        const { createClient } = await import("/sdk/v1/tealdrive.js");
        const first = createClient({
          storage: null,
          quotaPollMs: 0,
          stateMode: "client",
        });
        await first.handshake();
        try {
          await first.auth.login({ username, password: linuxPassword });
          throw Error("Linux password accepted as web password");
        } catch (e) {
          if (e.code !== "INVALID_CREDENTIALS") throw e;
        }
        let initialized = false;
        try {
          await first.auth.login({ username, password: webPassword });
          initialized = true;
        } catch (e) {
          if (e.code !== "INVALID_CREDENTIALS")
            throw Error("Initial login: " + e.code);
        }
        if (!initialized) {
          const enrollment = await first.auth.beginEnrollment({
            username,
            linuxPassword,
          });
          if (!enrollment.enrollmentRequired || "id" in enrollment)
            throw Error("Enrollment proof leaked");
          await first.auth.completeEnrollment({ webPassword });
        }
        const second = createClient({
          storage: null,
          quotaPollMs: 0,
          stateMode: "client",
        });
        // Separate transport can log in independently without copying its cookie into this client.
        await second.handshake();
        if (!second.getState().session) {
          try {
            await second.auth.login({ username, password: linuxPassword });
            throw Error("Linux password accepted as web password");
          } catch (e) {
            if (
              e.code !== "INVALID_CREDENTIALS" &&
              e.code !== "ALREADY_AUTHENTICATED"
            )
              throw e;
          }
        }
        // Existing HttpOnly cookie resumption authenticates this second connection.
        await second.auth.getSession();
        await first.files.createFolder({ path: directory });
        try {
          await first.files.createText({
            path: directory + "/note.txt",
            content: "persisted-test-content",
          });
          const original = await first.files.readText({
            path: directory + "/note.txt",
          });
          if (
            original.content !== "persisted-test-content" ||
            !original.revision
          )
            throw Error("Read/revision mismatch");
          await first.files.chmod({ path: directory + "/note.txt", mode: 0 });
          await first.files.chmod({
            path: directory + "/note.txt",
            mode: 0o600,
          });
          const restored = await first.files.readText({
            path: directory + "/note.txt",
          });
          if (restored.content !== original.content)
            throw Error("chmod changed content");
          const quota = await first.files.quota.get();
          if (quota.usedBytes < 10) throw Error("Quota grant missing");
          const downloaded = await first.files.download({
            path: directory + "/note.txt",
          });
          if ((await downloaded.text()) !== original.content)
            throw Error("Download grant missing");
          const session = await first.auth.getSession();
          if (!session.authenticated || session.isAdmin)
            throw Error("Root role boundary");
          await first.files.trash.move({ path: directory });
        } catch (e) {
          try {
            await first.files.chmod({
              path: directory + "/note.txt",
              mode: 0o600,
            });
            await first.files.trash.move({ path: directory });
          } catch {}
          throw e;
        }
        await first.auth.logout();
        // A resumed socket uses the same session grant and must lose file authority after logout.
        let denied = false;
        try {
          await second.files.list({ path: "/" });
        } catch (e) {
          denied = [
            "AUTH_REQUIRED",
            "PERMISSION_DENIED",
            "DISCONNECTED",
            "CONNECTION_CLOSED",
            "HANDSHAKE_REQUIRED",
            "TIMEOUT",
          ].includes(e.code);
          if (!denied) throw Error("Unexpected logout rejection: " + e.code);
        }
        second.disconnect();
        if (!denied) throw Error("Revoked socket retained authority");
        return {
          enrollment: !initialized,
          webLogin: true,
          fileGrant: true,
          chmod000: true,
          download: true,
          quota: true,
          logout: true,
        };
      },
      {
        username: process.env.TEALDRIVE_TEST_USERNAME,
        linuxPassword: secret(process.env.TEALDRIVE_TEST_PASSWORD_FILE),
        webPassword: secret(process.env.TEALDRIVE_TEST_WEB_PASSWORD_FILE),
        directory: "enrollment-test-" + crypto.randomUUID(),
      },
    );
    console.log("PASS real account enrollment:", JSON.stringify(result));
  } finally {
    await browser.close();
  }
})().catch((e) => {
  console.error(e.message);
  process.exitCode = 1;
});
