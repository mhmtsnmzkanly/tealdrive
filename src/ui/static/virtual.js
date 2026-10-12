/** Virtual-account dialogs. Root decides OS identities, pool and lifecycle policy. */
export async function openVirtualDialog({ client, account, onSaved }) {
  const budget = await client.admin.quota.getBudget();
  const native = budget.enforcement === "user_quota",
    minimum = native ? 4096 : 1,
    step = native ? 1024 : 1;
  const prior = document.activeElement,
    dialog = document.createElement("dialog");
  dialog.className = "policy-editor";
  dialog.setAttribute(
    "aria-label",
    account ? "Change virtual quota" : "Create virtual user",
  );
  dialog.addEventListener("keydown", (event) => event.stopPropagation());
  const node = (tag, text) => {
    const n = document.createElement(tag);
    if (text !== undefined) n.textContent = text;
    return n;
  };
  const title = node(
      "h3",
      account ? `Quota · ${account.username}` : "Create virtual user",
    ),
    form = node("form"),
    error = node("p"),
    save = node("button", account ? "Save quota" : "Create user"),
    cancel = node("button", "Cancel");
  error.setAttribute("role", "alert");
  const fields = {};
  function input(key, label, type, value) {
    const wrapper = node("label", label),
      field = node("input");
    field.name = key;
    field.type = type;
    field.required = true;
    field.value = value ?? "";
    field.setAttribute("aria-label", label);
    wrapper.append(field);
    form.append(wrapper);
    fields[key] = field;
    return field;
  }
  if (!account) {
    const name = input("name", "Login name", "text");
    name.maxLength = 64;
    name.pattern = "[a-z][a-z0-9_.-]{0,63}";
    const password = input(
      "password",
      "Initial web password (12+ characters)",
      "password",
    );
    password.autocomplete = "new-password";
    password.minLength = 12;
    password.maxLength = 1024;
  }
  const allocation =
    account?.quotaBytes ??
    Math.max(minimum, Math.floor(budget.remainingBytes / 4 / step) * step);
  const quota = input(
    "quotaBytes",
    "Quota in bytes",
    "number",
    String(allocation),
  );
  quota.min = String(minimum);
  quota.step = String(step);
  form.append(
    node(
      "p",
      `Assigned ${budget.limitBytes} bytes; reserved ${budget.reservedBytes}; available ${budget.remainingBytes}. ${native ? "Kernel quota counts allocated blocks and metadata. New quotas must be multiples of 1024 bytes." : "Usage is measured by the helper."}`,
    ),
  );
  form.append(
    node(
      "p",
      account
        ? "Quota changes sign out this account. A lower quota is rejected if existing data or reservations would exceed it."
        : "The server creates a separate locked/nologin OS identity. Share only the initial web password; the user can change it after signing in. Repeating this create request does not reset credentials.",
    ),
  );
  let busy = false;
  const key = crypto.randomUUID();
  const close = () => {
    if (fields.password) fields.password.value = "";
    dialog.close();
    dialog.remove();
    prior?.focus();
  };
  cancel.type = "button";
  cancel.className = "btn btn-secondary";
  cancel.onclick = () => {
    if (!busy) close();
  };
  save.type = "submit";
  save.className = "btn btn-primary";
  dialog.addEventListener("cancel", (event) => {
    event.preventDefault();
    if (!busy) close();
  });
  form.onsubmit = async (event) => {
    event.preventDefault();
    if (busy) return;
    const name = fields.name?.value,
      password = fields.password?.value;
    if (fields.password) fields.password.value = "";
    busy = true;
    for (const field of form.querySelectorAll("input,button"))
      field.disabled = true;
    try {
      const bytes = Number(quota.value);
      const result = account
        ? await client.admin.virtualUsers.setQuota({
            accountId: account.id,
            expectedGeneration: account.generation,
            quotaBytes: bytes,
          })
        : await client.admin.virtualUsers.create({
            name,
            password,
            quotaBytes: bytes,
            idempotencyKey: key,
          });
      close();
      await onSaved(result);
    } catch (failure) {
      error.textContent =
        failure.code === "QUOTA_EXCEEDED"
          ? "The allocation exceeds available budget or existing usage."
          : failure.code === "CONFLICT"
            ? "The account changed. Close and refresh before retrying."
            : "Request failed: " + failure.message;
    } finally {
      busy = false;
      for (const field of form.querySelectorAll("input,button"))
        field.disabled = false;
    }
  };
  form.append(error, save, cancel);
  dialog.append(title, form);
  document.body.append(dialog);
  dialog.showModal();
  (fields.name ?? quota).focus();
}
