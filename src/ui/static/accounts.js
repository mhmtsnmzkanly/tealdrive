/** Account-management presentation. Identity, authorization and persistence stay on the server/helper. */
export function createAccountManagement({ getClient, toast, formatBytes }) {
  const $ = (id) => document.getElementById(id);
  const modal = $("accountsModal");
  const body = $("accountsTableBody");
  const systemTab = $("accountsSystemTab");
  const virtualTab = $("accountsVirtualTab");
  const select = $("accountsCandidate");
  let session = null,
    kind = "linux",
    busy = false,
    epoch = 0,
    candidates = [],
    capabilities = null;
  const visible = () => !modal.classList.contains("hidden");
  const globalAdmin = () =>
    session?.accountKind === "linux" && session?.isAdmin;
  const message = (error) =>
    ({
      PERMISSION_DENIED:
        "Root policy or your account permissions prevent this action.",
      CONFLICT:
        "The account changed or is already registered. Refresh the list before retrying.",
      AUTH_REQUIRED: "Your session has ended. Sign in again.",
      FEATURE_DISABLED: "Account management is unavailable on this server.",
      SERVICE_UNAVAILABLE:
        "Account management is temporarily unavailable. Try again shortly.",
      INVALID_ARGUMENT: "Select a valid account before continuing.",
    })[error?.code] ?? "Unable to complete the request. Refresh and try again.";
  function note(text) {
    const row = document.createElement("tr"),
      cell = document.createElement("td");
    cell.colSpan = 5;
    cell.textContent = text;
    row.append(cell);
    body.replaceChildren(row);
  }
  function setBusy(value) {
    busy = value;
    modal.setAttribute("aria-busy", String(value));
    for (const button of modal.querySelectorAll("button"))
      if (button.id !== "accountsCloseBtn")
        button.disabled = value || button.dataset.blocked === "true";
    select.disabled = value || !candidates.length;
    $("accountsApproveBtn").disabled = value || !select.value;
  }
  function configure(info) {
    capabilities = info.authentication;
    systemTab.classList[globalAdmin() ? "remove" : "add"]("hidden");
    if (!globalAdmin()) kind = "virtual";
    systemTab.setAttribute("aria-pressed", String(kind === "linux"));
    virtualTab.setAttribute("aria-pressed", String(kind === "virtual"));
    $("accountsApproveArea").classList[
      kind === "linux" && info.authentication.linuxApproval ? "remove" : "add"
    ]("hidden");
    $("accountsPolicyNote").textContent = info.authentication.selfRegistration
      ? "Eligible system users can register themselves. Approval also prepares an account for its first Linux-password verification. Root restrictions always apply."
      : "System users need approval or root eligibility before their first Linux-password verification. Root restrictions always apply.";
    $("accountsScope").textContent =
      kind === "linux"
        ? "Registered system users"
        : globalAdmin()
          ? "Registered virtual users"
          : "Your virtual users";
  }
  async function virtualDialog(account) {
    try {
      const client = await getClient();
      const { openVirtualDialog } = await import("/static/virtual.js");
      await openVirtualDialog({ client, account, onSaved: refresh });
    } catch (error) {
      $("accountsError").textContent = message(error);
    }
  }
  function status(account) {
    if (account.status === "active" && !account.passwordEnrolled)
      return "Awaiting registration";
    return (
      {
        active: "Active",
        suspended: "Suspended",
        revoked: "Access revoked",
        deleted: "Deleted",
      }[account.status] ?? "Unavailable"
    );
  }
  function render(accounts) {
    body.replaceChildren();
    if (!accounts.length) {
      note("No accounts in this group.");
      return;
    }
    for (const account of accounts) {
      const row = document.createElement("tr");
      row.dataset.accountId = account.id;
      const accountCell = document.createElement("td"),
        name = document.createElement("strong"),
        details = document.createElement("small");
      name.textContent = account.username;
      details.textContent = `UID ${account.uid} · ${account.home}`;
      details.className = "account-details";
      accountCell.append(name, details);
      row.append(accountCell);
      for (const [label, value] of [
        ["Role", account.role === "admin" ? "Admin" : "User"],
        ["State", status(account)],
        [
          "Allocation",
          account.quotaBytes == null ? "—" : formatBytes(account.quotaBytes),
        ],
      ]) {
        const cell = document.createElement("td");
        cell.dataset.label = label;
        cell.textContent = value;
        row.append(cell);
      }
      const actions = document.createElement("td");
      actions.dataset.label = "Actions";
      if (account.role === "admin" || account.id === session.accountId)
        actions.textContent = "Root managed";
      else if (account.status === "deleted") actions.textContent = "Deleted";
      else {
        const enable = account.status !== "active" || !account.loginEnabled;
        const button = document.createElement("button");
        button.type = "button";
        button.className = `btn btn-sm ${enable ? "btn-secondary" : "btn-danger"}`;
        button.textContent = enable ? "Enable" : "Suspend";
        button.onclick = () =>
          mutate(
            async (client) => {
              const api =
                kind === "linux"
                  ? client.admin.systemUsers
                  : client.admin.virtualUsers;
              return enable
                ? kind === "linux"
                  ? api.allow({ accountId: account.id })
                  : api.enable({ accountId: account.id })
                : kind === "linux"
                  ? api.deny({ accountId: account.id })
                  : api.disable({ accountId: account.id });
            },
            enable
              ? `Enable web access for ${account.username}? Root restrictions still apply; old sessions remain invalid.`
              : `Suspend web access for ${account.username}? Active web sessions will close. Existing files and the Linux account are kept.`,
            enable ? "Web access enabled." : "Web access suspended.",
          );
        actions.append(button);
        const policy = document.createElement("button");
        policy.type = "button";
        policy.className = "btn btn-secondary btn-sm";
        policy.textContent = "File policy";
        policy.onclick = async () => {
          try {
            const client = await getClient();
            const { openPolicyEditor } = await import("/static/policy.js");
            await openPolicyEditor({
              client,
              target: { kind, accountId: account.id },
              onSaved: refresh,
            });
          } catch (error) {
            $("accountsError").textContent = message(error);
          }
        };
        actions.append(policy);
        if (kind === "virtual") {
          const quota = document.createElement("button");
          quota.type = "button";
          quota.className = "btn btn-secondary btn-sm";
          quota.textContent = "Quota";
          quota.onclick = () => virtualDialog(account);
          actions.append(quota);
          const remove = document.createElement("button");
          remove.type = "button";
          remove.className = "btn btn-danger btn-sm";
          remove.textContent = "Delete";
          remove.onclick = () =>
            mutate(
              async (client) =>
                client.admin.virtualUsers.delete({
                  accountId: account.id,
                  expectedGeneration: account.generation,
                }),
              `Delete web access for ${account.username}? Data is retained for the Root-configured recovery period and then purged.`,
              "Account deleted; data retained for recovery.",
            );
          actions.append(remove);
          remove.dataset.blocked = String(!capabilities?.virtualDelete);
          quota.dataset.blocked = String(!capabilities?.virtualAccounts);
          remove.disabled = remove.dataset.blocked === "true";
          quota.disabled = quota.dataset.blocked === "true";
        }
      }
      row.append(actions);
      body.append(row);
    }
  }
  async function refresh() {
    const ticket = ++epoch;
    setBusy(true);
    $("accountsError").textContent = "";
    $("accountsStatus").textContent = "Loading accounts…";
    candidates = [];
    select.replaceChildren();
    note("Loading accounts…");
    try {
      const client = await getClient();
      session = await client.auth.getSession();
      if (session?.role !== "admin") throw { code: "AUTH_REQUIRED" };
      const info = client.getInfo();
      configure(info);
      $("accountsVirtualArea").classList[
        kind === "virtual" && info.authentication.virtualAccounts
          ? "remove"
          : "add"
      ]("hidden");
      if (kind === "virtual" && info.authentication.virtualAccounts) {
        const budget = await client.admin.quota.getBudget();
        if (ticket !== epoch || !visible()) return;
        $("accountsBudget").textContent =
          `Assigned ${formatBytes(budget.limitBytes)} · Reserved ${formatBytes(budget.reservedBytes)} · Available ${formatBytes(budget.remainingBytes)}`;
      }
      const [accounts, eligible] =
        kind === "linux"
          ? await Promise.all([
              client.admin.systemUsers.accounts(),
              client.admin.systemUsers.list(),
            ])
          : [await client.admin.virtualUsers.list(), null];
      if (ticket !== epoch || !visible()) return;
      render(accounts);
      const registered = new Set(accounts.map((account) => account.username));
      candidates = (eligible?.users ?? []).filter(
        (user) => !registered.has(user.username),
      );
      const placeholder = document.createElement("option");
      placeholder.value = "";
      placeholder.textContent = "Select an eligible system user";
      select.append(placeholder);
      for (const user of candidates) {
        const option = document.createElement("option");
        option.value = user.username;
        option.textContent = `${user.username} (UID ${user.uid})`;
        select.append(option);
      }
      $("accountsStatus").textContent =
        `${accounts.length} registered account(s).${eligible?.truncated || accounts.length >= 1025 ? " The list is limited; additional accounts may exist." : ""}`;
      $("accountsCandidatesNote").textContent = eligible?.truncated
        ? "Local user discovery is limited to the first 1024 eligible identities."
        : candidates.length
          ? "System and service accounts are filtered. The user completes PAM verification and chooses their web password."
          : "No unregistered eligible local users found.";
    } catch (error) {
      if (ticket === epoch && visible()) {
        note("Accounts could not be loaded.");
        $("accountsError").textContent = message(error);
        $("accountsStatus").textContent = "";
      }
    } finally {
      if (ticket === epoch) setBusy(false);
    }
  }
  async function mutate(operation, confirmation, success) {
    if (busy || !visible() || !window.confirm(confirmation)) return;
    const ticket = epoch;
    setBusy(true);
    $("accountsError").textContent = "";
    try {
      const client = await getClient();
      await operation(client);
      if (ticket !== epoch || !visible()) return;
      await refresh();
      toast(success, "success");
    } catch (error) {
      if (ticket === epoch && visible()) {
        $("accountsError").textContent = message(error);
        toast(message(error), "error");
      }
    } finally {
      if (ticket === epoch) setBusy(false);
    }
  }
  function close() {
    if (!visible()) return;
    epoch++;
    busy = false;
    modal.classList.add("hidden");
    body.replaceChildren();
    select.replaceChildren();
    candidates = [];
    session = null;
    $("manageAccountsBtn").focus();
  }
  modal.addEventListener("keydown", (event) => {
    if (event.key !== "Tab" || !visible()) return;
    const controls = [
      ...modal.querySelectorAll("button:not(:disabled), select:not(:disabled)"),
    ].filter((element) => element.getClientRects().length);
    const first = controls[0],
      last = controls.at(-1);
    if (event.shiftKey && document.activeElement === first) {
      event.preventDefault();
      last?.focus();
    } else if (!event.shiftKey && document.activeElement === last) {
      event.preventDefault();
      first?.focus();
    }
  });
  $("accountsCreateVirtualBtn").onclick = () => virtualDialog(null);
  $("accountsCloseBtn").onclick = close;
  $("accountsRefreshBtn").onclick = () => {
    if (!busy) refresh();
  };
  systemTab.onclick = () => {
    if (!busy && globalAdmin()) {
      kind = "linux";
      refresh();
    }
  };
  virtualTab.onclick = () => {
    if (!busy) {
      kind = "virtual";
      refresh();
    }
  };
  select.onchange = () => {
    $("accountsApproveBtn").disabled = busy || !select.value;
  };
  $("accountsApproveBtn").onclick = () => {
    const username = select.value;
    if (!candidates.some((user) => user.username === username)) return;
    mutate(
      (client) => client.admin.systemUsers.approve({ username }),
      `Approve first web registration for ${username}? The user must verify their Linux password and choose a web password. Root restrictions still apply.`,
      "Web registration approved. The user can complete their first sign-in.",
    );
  };
  return {
    close,
    async open() {
      if (busy && visible()) return;
      const client = await getClient();
      session = await client.auth.getSession();
      if (
        session?.role !== "admin" ||
        !client.getInfo().authentication.accountManagement
      ) {
        toast("Account management is unavailable for this account.", "error");
        return;
      }
      kind = globalAdmin() ? "linux" : "virtual";
      modal.classList.remove("hidden");
      const info = client.getInfo();
      configure(info);
      $("accountsVirtualArea").classList[
        kind === "virtual" && info.authentication.virtualAccounts
          ? "remove"
          : "add"
      ]("hidden");
      if (kind === "virtual" && info.authentication.virtualAccounts) {
        const budget = await client.admin.quota.getBudget();
        if (ticket !== epoch || !visible()) return;
        $("accountsBudget").textContent =
          `Assigned ${formatBytes(budget.limitBytes)} · Reserved ${formatBytes(budget.reservedBytes)} · Available ${formatBytes(budget.remainingBytes)}`;
      }
      $("accountsCloseBtn").focus();
      await refresh();
    },
  };
}
