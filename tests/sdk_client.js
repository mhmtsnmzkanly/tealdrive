const assert = require("node:assert/strict");
const { webcrypto } = require("node:crypto");
globalThis.crypto = webcrypto;
(async () => {
  const module = await import(
    "data:text/javascript;base64," +
      Buffer.from(contractSource + "\n" + sdkSource).toString("base64")
  );
  const { createClient, TealDriveError } = module;
  const W = TealDriveWire,
    M = TdrvContract.messages;
  const frames = [];
  let session = null;
  const removed = new Set();
  let delayed = null;
  const location = {
    protocol: "https:",
    host: "drive.test",
    origin: "https://drive.test",
  };
  const hello = {
    protocol_version: 1,
    max_frame_size: 1048576,
    max_control_payload_size: 262144,
    max_chunk_size: 262144,
    feature_flags: {
      save_text_file: true,
      chmod_file: true,
      extract_archive: true,
    },
  };
  const limits = {
    max_chunk_size: 4,
    max_in_flight_upload: 100,
    default_directory_page_size: 200,
    max_directory_page_size: 500,
  };
  class Socket {
    constructor(url) {
      Socket.current = this;
      assert.equal(url, "wss://drive.test/ws/auth");
      queueMicrotask(() => this.reply(M.ServerHello, hello));
    }
    reply(type, payload, id = crypto.randomUUID()) {
      this.onmessage({ data: W.encodeFrame(type, payload, id) });
    }
    send(bytes) {
      const f = W.decodeFrame(bytes);
      frames.push(f);
      const p = f.payload;
      if (f.messageType === M.ClientHello) {
        this.reply(M.ProtocolReady, { limits }, f.requestId);
        return;
      }
      if (f.messageType === M.AuthLogin) {
        if (p.action === "enrollmentBegin") {
          this.enrollmentUsername = p.username;
          this.reply(
            M.AuthResult,
            {
              enrollment_required: true,
              expires_at: Math.floor(Date.now() / 1000) + 300,
            },
            f.requestId,
          );
          return;
        }
        if (p.action === "enrollmentComplete") {
          if (!this.enrollmentUsername) {
            this.reply(
              M.AuthResult,
              { authenticated: false, code: "ENROLLMENT_REQUIRED" },
              f.requestId,
            );
            return;
          }
          p.username = this.enrollmentUsername;
          this.enrollmentUsername = null;
        }
        if (p.password === "wrong") {
          this.reply(
            M.AuthResult,
            { authenticated: false, code: "INVALID_CREDENTIALS" },
            f.requestId,
          );
          return;
        }
        session = {
          authenticated: true,
          is_admin: true,
          username: p.username,
          uid: p.username === "bob" ? 1001 : 1000,
          gid: 1000,
          csrf_token: "csrf",
          roots: [{ root_id: "home" }],
        };
        this.reply(
          M.AuthResult,
          { authenticated: true, bind_token: "bridge" },
          f.requestId,
        );
        return;
      }
      if (f.messageType === M.ListDirectory) {
        this.reply(
          M.DirectoryList,
          {
            entries: [],
            current_path: p.relative_path,
            next_cursor: null,
            has_more: false,
          },
          f.requestId,
        );
        return;
      }
      if (f.messageType === M.FileMetadataRequest) {
        if (p.relative_path === "slow") {
          delayed = f;
          return;
        }
        if (p.relative_path === "gone" || removed.has(p.relative_path)) {
          this.reply(
            M.OperationFailed,
            { error: { code: "NOTFOUND" } },
            f.requestId,
          );
          return;
        }
        this.reply(
          M.FileMetadata,
          {
            root_id: p.root_id,
            relative_path: p.relative_path,
            entry: { file_type: "Directory" },
          },
          f.requestId,
        );
        return;
      }
      if (f.messageType === M.UploadBegin) {
        this.reply(
          M.OperationStarted,
          {
            transfer_id: "11111111-1111-4111-8111-111111111111",
            chunk_size: 4,
          },
          f.requestId,
        );
        return;
      }
      if (f.messageType === M.UploadChunk) {
        this.reply(M.OperationProgress, { bytes_received: 4 }, f.requestId);
        return;
      }
      if (f.messageType === M.UploadEnd) {
        this.reply(M.OperationDone, { bytes_written: 7 }, f.requestId);
        return;
      }
      if (f.messageType === M.ReadTextFile) {
        return;
      } // cancellation/timeouts
      if (f.messageType === M.MoveToTrash) removed.add(p.relative_path);
      this.reply(M.OperationDone, { operation: "ok" }, f.requestId);
    }
    close() {
      this.onclose?.({});
    }
  }
  let quota = { limit_bytes: 100, used_bytes: 80 };
  let downloadStatus = 200;
  let policyGeneration = 1,
    personalRules = [];
  const settingsRequests = [];
  const fetch = async (url, options) => {
    if (
      url.startsWith("/api/admin/virtual/page") ||
      url.startsWith("/api/admin/accounts/system/page")
    ) {
      const query = new URLSearchParams(url.split("?")[1]);
      assert.equal(query.get("filter"), "tenant");
      assert.equal(query.get("limit"), "1");
      return {
        ok: true,
        status: 200,
        json: async () => ({
          items: [{ id: "tenant-id", username: "tenant" }],
          nextCursor: "tenant",
          hasMore: true,
        }),
      };
    }
    if (url.startsWith("/api/admin/config")) {
      settingsRequests.push({ url, options });
      if (options.method !== "GET")
        assert.equal(options.headers["x-csrf-token"], "csrf");
      return {
        ok: true,
        status: 200,
        headers: { get: () => "split" },
        json: async () => ({ config: { features: { save_text_file: true } } }),
      };
    }
    if (url === "/api/session")
      return {
        status: session ? 200 : 401,
        ok: Boolean(session),
        json: async () => session,
      };
    if (url === "/api/session/bind") {
      assert.equal(JSON.parse(options.body).token, "bridge");
      return { ok: true, status: 204 };
    }
    if (url === "/api/auth/password" || url === "/api/auth/revoke") {
      assert.ok(options.headers["x-csrf-token"]);
      if (url.endsWith("/password"))
        assert.equal(
          JSON.parse(options.body).newPassword,
          "replacement-web-password",
        );
      session = null;
      return { ok: true, status: 204 };
    }
    if (url === "/api/admin/system/approve") {
      assert.equal(JSON.parse(options.body).username, "approved-user");
      assert.ok(options.headers["x-csrf-token"]);
      return {
        ok: true,
        status: 200,
        json: async () => ({
          id: "approved-id",
          role: "user",
          password_enrolled: false,
        }),
      };
    }
    if (url === "/api/account/policy") {
      if (options.method === "POST") {
        assert.ok(options.headers["x-csrf-token"]);
        const body = JSON.parse(options.body);
        assert.equal(body.expectedGeneration, policyGeneration);
        personalRules = body.rules;
        policyGeneration++;
        return {
          ok: true,
          status: 200,
          json: async () => ({
            id: "policy-subject",
            generation: policyGeneration,
          }),
        };
      }
      return {
        ok: true,
        status: 200,
        json: async () => ({
          account: { id: "policy-subject", generation: policyGeneration },
          deny_system_components: [".ssh"],
          rules: {
            root_rules: [{ path: ".ssh", recursive: true, deny: ["discover"] }],
            admin_rules: [],
            personal_rules: personalRules,
          },
        }),
      };
    }
    if (url === "/api/admin/virtual/policy?accountId=child") {
      return {
        ok: true,
        status: 200,
        json: async () => ({
          account: { id: "child", generation: 1 },
          deny_system_components: [".ssh"],
          rules: { root_rules: [], admin_rules: [], personal_rules: [] },
        }),
      };
    }
    if (url === "/api/logout") {
      session = null;
      return { ok: true, status: 200, json: async () => ({}) };
    }
    if (url.startsWith("/api/download")) {
      const query = new URLSearchParams(url.split("?")[1]);
      assert.equal(query.get("root_id"), "home");
      assert.equal(query.get("path"), "download.txt");
      assert.equal(query.has("relative_path"), false);
      return new Response(downloadStatus === 200 ? "downloaded" : "", {
        status: downloadStatus,
      });
    }
    if (url.startsWith("/api/quota"))
      return { ok: true, status: 200, json: async () => quota };
    return { ok: true, status: 200, json: async () => ({}) };
  };
  const data = new Map();
  const storage = {
    getItem: (key) => data.get(key),
    setItem: (key, val) => data.set(key, val),
  };
  const api = createClient({
    WebSocket: Socket,
    fetch,
    location,
    storage,
    timeoutMs: 100,
  });
  assert.deepEqual(
    Object.keys(api.auth).sort(),
    [
      "beginEnrollment",
      "changePassword",
      "completeEnrollment",
      "getSession",
      "login",
      "logout",
      "revokeSelf",
    ].sort(),
  );
  await assert.rejects(
    api.files.list(),
    (e) => e.code === "HANDSHAKE_REQUIRED",
  );
  await api.handshake();
  await assert.rejects(api.files.list(), (e) => e.code === "AUTH_REQUIRED");
  await assert.rejects(
    api.auth.login({ username: "alice", password: "wrong" }),
    (e) => e.code === "INVALID_CREDENTIALS",
  );
  await api.auth.login({ username: "alice", password: "right" });
  assert.equal(
    api.files.getDownloadUrl({ rootId: "home", path: "/name with spaces.txt" }),
    "/api/download?root_id=home&path=name+with+spaces.txt",
  );
  assert.equal(
    api.files.getPreviewUrl({ rootId: "home", path: "/name.png" }),
    "/api/preview?root_id=home&path=name.png",
  );
  assert.throws(
    () => api.files.getDownloadUrl({ rootId: "other", path: "/file" }),
    (e) => e.code === "INVALID_ROOT",
  );
  assert.throws(
    () => api.files.getPreviewUrl({ rootId: "home", path: "../private" }),
    (e) => e.code === "INVALID_PATH",
  );
  assert.deepEqual(await api.admin.settings.inspect(), {
    mode: "split",
    config: { features: { saveTextFile: true } },
  });
  await api.admin.settings.patch({
    changes: { features: { save_text_file: false } },
    persist: false,
  });
  assert.equal(settingsRequests.at(-1).url, "/api/admin/config");
  await api.admin.settings.patch({
    changes: { features: { save_text_file: true } },
  });
  assert.equal(settingsRequests.at(-1).url, "/api/admin/config?save=true");
  await api.admin.settings.reload();
  assert.equal(settingsRequests.at(-1).url, "/api/admin/config/reload");
  await assert.rejects(
    api.admin.virtualUsers.listPage({ limit: 1, filter: "tenant" }),
    (e) => e.code === "FEATURE_DISABLED",
  );
  api._hello.authentication ??= {};
  api._hello.authentication.account_pagination = true;
  const paged = await api.admin.virtualUsers.listPage({
    limit: 1,
    filter: "tenant",
  });
  assert.equal(paged.nextCursor, "tenant");
  assert.equal(paged.items[0].id, "tenant-id");
  const compatible = await api.admin.virtualUsers.list({
    limit: 1,
    filter: "tenant",
  });
  assert.ok(Array.isArray(compatible));
  assert.equal(compatible[0].username, "tenant");
  await assert.rejects(
    api.admin.virtualUsers.listPage({ limit: 201 }),
    (e) => e.code === "INVALID_ARGUMENT",
  );
  await assert.rejects(
    api.admin.virtualUsers.listPage({ ownerAdminId: "foreign" }),
    (e) => e.code === "INVALID_ARGUMENT",
  );
  await api.files.goTo("/musics");
  assert.equal(api.files.getCurrentDirectory(), "/musics");
  await api.files.list({
    filter: { name: "report", type: "file", extensions: ["pdf", "txt"] },
    sort: { by: "size", direction: "desc" },
    limit: 12,
  });
  const list = frames
    .filter((f) => f.messageType === M.ListDirectory)
    .at(-1).payload;
  assert.equal(list.relative_path, "musics");
  assert.equal(list.filter.query, "report");
  assert.equal(list.filter.file_kind, "File");
  assert.equal(list.sort, "SizeDesc");
  assert.deepEqual(list.filter.extensions, ["pdf", "txt"]);
  await assert.rejects(
    api.files.list({ filter: { regex: "unsafe" } }),
    (e) => e.code === "INVALID_ARGUMENT",
  );
  await assert.rejects(
    api.files.list({ path: "../escape" }),
    (e) => e.code === "INVALID_PATH",
  );
  let snapshots = 0;
  const stop = api.files.observeState(() => snapshots++);
  assert.equal(snapshots, 1);
  stop();
  let entered = 0,
    conditions = [];
  api.files.on("quota.warning.entered", () => entered++);
  api.files.observeQuotaWarning((q) => conditions.push(q.active));
  await api.files.quota.get();
  await api.files.quota.get();
  assert.equal(entered, 1);
  quota = { limit_bytes: 100, used_bytes: 60 };
  await api.files.quota.get();
  assert.equal(conditions.at(-1), false);
  const controller = new AbortController();
  const reading = api.files.readText({
    path: "waiting.txt",
    signal: controller.signal,
  });
  controller.abort();
  await assert.rejects(reading, (e) => e.code === "ABORTED");
  await assert.rejects(
    api.files.readText({ path: "timeout.txt" }),
    (e) => e.code === "TIMEOUT",
  );
  assert.equal(api._pending.size, 0);
  const downloaded = await api.files.download({ path: "download.txt" });
  assert.equal(await downloaded.text(), "downloaded");
  for (const [status, code] of [
    [401, "AUTH_REQUIRED"],
    [403, "PERMISSION_DENIED"],
    [404, "NOT_FOUND"],
    [429, "RATE_LIMITED"],
    [503, "SERVICE_UNAVAILABLE"],
  ]) {
    downloadStatus = status;
    await assert.rejects(
      api.files.download({ path: "download.txt" }),
      (e) => e.code === code && e.status === status,
    );
  }
  downloadStatus = 200;
  const progress = [];
  const file = new Blob(["example"]);
  Object.defineProperty(file, "name", { value: "example.txt" });
  await api.files.upload({
    directory: "uploads",
    file,
    onProgress: (p) => progress.push(p.transferredBytes),
  });
  assert.deepEqual(progress, [4, 7]);
  await assert.rejects(api.files.goTo("/gone"), (e) => e.code === "NOT_FOUND");
  assert.equal(api.files.getCurrentDirectory(), "/musics");
  assert.ok(
    [...data.values()].every(
      (v) => !v.includes("csrf") && !v.includes("right"),
    ),
  );
  await api.auth.logout();
  await api.handshake();
  await api.auth.login({ username: "bob", password: "right" });
  assert.equal(api.files.getCurrentDirectory(), "/");
  assert.ok([...data.keys()].some((k) => k.includes("alice")));
  assert.ok([...data.keys()].some((k) => k.includes("bob")));
  await api.files.goTo("/musics");
  const slow = api.files.goTo("/slow");
  const slowError = assert.rejects(slow, (e) => e.code === "STATE_CONFLICT");
  await api.files.goTo("/");
  Socket.current.reply(
    M.FileMetadata,
    {
      root_id: "home",
      relative_path: "slow",
      entry: { file_type: "Directory" },
    },
    delayed.requestId,
  );
  await slowError;
  await api.files.goTo("/musics");
  await api.files.trash.move({ path: "musics" });
  assert.equal(api.files.getCurrentDirectory(), "/");
  const first = api.getState();
  assert.ok(Object.isFrozen(first));
  api.disconnect();
  assert.equal(api._pending.size, 0);
  session = null; // New browser with no session cookie.
  const enrolling = createClient({
    WebSocket: Socket,
    fetch,
    location,
    storage,
    stateMode: "client",
    timeoutMs: 100,
  });
  await enrolling.handshake();
  await assert.rejects(
    enrolling.auth.completeEnrollment({ webPassword: "short" }),
    (e) => e.code === "INVALID_ARGUMENT",
  );
  await assert.rejects(
    enrolling.auth.completeEnrollment({ webPassword: "long-web-password" }),
    (e) => e.code === "ENROLLMENT_REQUIRED",
  );
  const begun = await enrolling.auth.beginEnrollment({
    username: "new-user",
    linuxPassword: "linux-password",
  });
  assert.equal(begun.enrollmentRequired, true);
  assert.equal(typeof begun.expiresAt, "number");
  assert.ok(!("id" in begun));
  const enrollmentFrame = frames.at(-1).payload;
  assert.equal(enrollmentFrame.action, "enrollmentBegin");
  const enrolled = await enrolling.auth.completeEnrollment({
    webPassword: "separate-web-password",
  });
  assert.equal(enrolled.username, "new-user");
  await assert.rejects(
    enrolling.auth.changePassword({
      currentPassword: "old",
      newPassword: "short",
    }),
    (e) => e.code === "INVALID_ARGUMENT",
  );
  await assert.rejects(
    enrolling.auth.revokeSelf(),
    (e) => e.code === "FEATURE_DISABLED",
  );
  hello.authentication = {
    web_password: true,
    password_change: true,
    self_revoke: true,
    linux_approval: true,
  };
  await assert.rejects(
    enrolling.admin.systemUsers.approve({ username: "eligible" }),
    (e) => e.code === "FEATURE_DISABLED",
  );
  assert.equal(enrolling.getInfo().authentication.linuxApproval, false);
  enrolling.disconnect();
  await enrolling.handshake();
  await assert.rejects(
    enrolling.admin.systemUsers.approve({ username: "bad/path" }),
    (e) => e.code === "INVALID_ARGUMENT",
  );
  await assert.rejects(
    enrolling.admin.systemUsers.approve({
      username: "approved-user",
      role: "admin",
    }),
    (e) => e.code === "INVALID_ARGUMENT",
  );
  const approved = await enrolling.admin.systemUsers.approve({
    username: "approved-user",
  });
  assert.equal(approved.passwordEnrolled, false);
  assert.equal(approved.role, "user");
  await assert.rejects(
    enrolling.files.policy.get(),
    (e) => e.code === "FEATURE_DISABLED",
  );
  hello.authentication.policy_management = true;
  enrolling.disconnect();
  await enrolling.handshake();
  assert.equal(enrolling.getInfo().authentication.policyManagement, true);
  await assert.rejects(
    enrolling.files.policy.set({
      expectedGeneration: 1,
      rules: [{ path: "../secret", deny: ["discover"] }],
    }),
    (e) => e.code === "INVALID_PATH",
  );
  const policy = await enrolling.files.policy.get();
  assert.equal(policy.rules.rootRules[0].ruleId, "root_0");
  assert.equal(policy.revision, "1");
  await assert.rejects(
    enrolling.files.policy.removeRule({
      ruleId: "root_0",
      expectedRevision: "1",
    }),
    (e) => e.code === "NOT_FOUND",
  );
  await enrolling.files.policy.addRule({
    path: "private",
    recursive: true,
    deny: ["discover"],
  });
  assert.equal(personalRules[0].path, "private");
  assert.equal(personalRules[0].root_id, null);
  session.account_kind = "virtual";
  await enrolling.auth.getSession();
  const childPolicy = await enrolling.admin.policy.get({ accountId: "child" });
  assert.equal(childPolicy.account.id, "child");
  session.account_kind = "linux";
  await enrolling.auth.getSession();
  await enrolling.auth.changePassword({
    currentPassword: "right",
    newPassword: "replacement-web-password",
  });
  assert.equal(enrolling._session, null);
  await enrolling.handshake();
  await enrolling.auth.login({ username: "new-user", password: "right" });
  await enrolling.auth.revokeSelf();
  assert.equal(enrolling._session, null);
  assert.ok(TealDriveError);
})().catch((e) => {
  if (e.stack)
    e.stack = e.stack.replace(
      /data:text\/javascript;base64,[A-Za-z0-9+/=]+/g,
      "sdk",
    );
  console.error(e.stack, e.code, e.cause?.message);
  process.exitCode = 1;
});
