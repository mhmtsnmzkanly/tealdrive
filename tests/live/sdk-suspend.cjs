// Root-only test operator suspends the dedicated candidate account; Linux account/data stay intact.
const { chromium } = require("playwright");
const fs = require("node:fs");
const { execFileSync } = require("node:child_process");
(async () => {
  if (
    process.getuid() !== 0 ||
    process.env.TEALDRIVE_ALLOW_TEST_ACCOUNT_MUTATION !== "1"
  )
    throw Error("Explicit root test-account mutation flag required");
  for (const key of [
    "TEALDRIVE_TEST_URL",
    "TEALDRIVE_TEST_USERNAME",
    "TEALDRIVE_TEST_WEB_PASSWORD_FILE",
  ])
    if (!process.env[key]) throw Error("Missing " + key);
  const username = process.env.TEALDRIVE_TEST_USERNAME;
  const db = "/home/tealdrive/auth/accounts.db";
  const python = `import sqlite3,sys,time,os,uuid
os.umask(0o077)
c=sqlite3.connect(sys.argv[1])
a=c.execute('SELECT status,login_enabled,kind,uid FROM accounts WHERE username=?',(sys.argv[2],)).fetchone()
if not a or a[2]!='linux' or a[3]==0: raise RuntimeError('Dedicated nonroot Linux account required')
if sys.argv[3]=='suspend':
 if a[:2]!=('active',1): raise RuntimeError('Account must initially be active')
 backup=sqlite3.connect(sys.argv[1]+'.suspend-test-'+str(uuid.uuid4())+'.backup');c.backup(backup);backup.close()
 c.execute("UPDATE accounts SET status='suspended',login_enabled=0,deny_enrollment=1,generation=generation+1,updated_at=? WHERE username=?",(int(time.time()),sys.argv[2]))
else:
 c.execute("UPDATE accounts SET status='active',login_enabled=1,deny_enrollment=0,generation=generation+1,updated_at=? WHERE username=?",(int(time.time()),sys.argv[2]))
c.commit();c.close()
`;
  const mutate = (action) =>
    execFileSync("python3", ["-c", python, db, username, action], {
      stdio: ["ignore", "ignore", "pipe"],
    });
  const secretFile = process.env.TEALDRIVE_TEST_WEB_PASSWORD_FILE;
  if (fs.statSync(secretFile).mode & 0o077)
    throw Error("Private secret file required");
  const browser = await chromium.launch({ headless: true });
  let suspended = false;
  try {
    const page = await browser.newPage();
    await page.goto(process.env.TEALDRIVE_TEST_URL);
    await page.evaluate(
      async ({ username, password }) => {
        const { createClient } = await import("/sdk/v1/tealdrive.js");
        globalThis.testApi = createClient({
          storage: null,
          stateMode: "client",
          quotaPollMs: 0,
        });
        await testApi.handshake();
        await testApi.auth.login({ username, password });
        await testApi.files.list({ path: "/" });
      },
      { username, password: fs.readFileSync(secretFile, "utf8").trim() },
    );
    mutate("suspend");
    suspended = true;
    await page.waitForFunction(
      () => testApi.getState().connection === "disconnected",
      {},
      { timeout: 5000 },
    );
    await page.evaluate(async () => {
      try {
        await testApi.files.list({ path: "/" });
      } catch (e) {
        if (
          [
            "HANDSHAKE_REQUIRED",
            "AUTH_REQUIRED",
            "CONNECTION_CLOSED",
            "PERMISSION_DENIED",
          ].includes(e.code)
        )
          return;
        throw Error("Unexpected rejection: " + e.code);
      }
      throw Error("Suspended session retained access");
    });
    console.log(
      "PASS native suspend: persistent generation invalidates the active WS/lease",
    );
  } finally {
    if (suspended) mutate("restore");
    await browser.close();
  }
})().catch((e) => {
  console.error(e.message);
  process.exitCode = 1;
});
