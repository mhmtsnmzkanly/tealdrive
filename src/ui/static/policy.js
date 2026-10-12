import { t, text as uiText } from "/static/i18n.js";
/** Policy presentation only. Identity, scope, CAS and enforcement are helper responsibilities. */
export async function openPolicyEditor({ client, target, onSaved = () => {} }) {
  const api = target
    ? target.kind === "linux"
      ? client.admin.systemUsers
      : client.admin.virtualUsers
    : client.files.policy;
  const view = await (target
    ? api.getPolicy({ accountId: target.accountId })
    : api.get());
  const layer = target ? "admin" : "personal";
  const prior = document.activeElement;
  const dialog = document.createElement("dialog");
  dialog.className = "policy-editor";
  dialog.setAttribute("aria-label", "File access policy");
  dialog.addEventListener("keydown", (event) => event.stopPropagation());
  const make = (tag, text, params = {}) => {
    const node = document.createElement(tag);
    if (text !== undefined) {
      const caption = document.createElement("span");
      uiText(caption, text, params);
      node.append(caption);
    }
    return node;
  };
  const title = make("h3", "File policy · {username}", {
    username: view.account.username,
  });
  const notice = make(
    "p",
    "Rules only restrict access. Discover hides the path and blocks direct access. Saving signs out all sessions for this account; sign in again to use the new policy.",
  );
  const fixed = make("details");
  fixed.append(make("summary", "Restrictions you cannot change here"));
  fixed.append(
    make(
      "p",
      t("Protected system components: ") + view.denySystemComponents.join(", "),
    ),
  );
  for (const source of ["root", "admin", "personal"]) {
    if (source === layer) continue;
    for (const rule of view.rules[source + "Rules"])
      fixed.append(
        make(
          "p",
          `${source}: ${rule.path || "/"} · ${rule.recursive ? "subtree" : "exact"} · ${rule.deny.join(", ")}`,
        ),
      );
  }
  const rows = make("div"),
    actions = make("div"),
    error = make("p");
  error.setAttribute("role", "alert");
  const add = make("button", "Add restriction"),
    save = make("button", "Save policy"),
    close = make("button", "Cancel");
  for (const button of [add, save, close]) {
    button.type = "button";
    button.className = "btn btn-secondary";
  }
  save.className = "btn btn-primary";
  let busy = false;
  const collect = [];
  function addRow(rule = { path: "", recursive: true, deny: ["discover"] }) {
    if (collect.length >= 512) {
      error.textContent = t("At most 512 rules are allowed.");
      return;
    }
    const row = make("fieldset"),
      pathLabel = make("label", "Relative path"),
      path = make("input");
    path.type = "text";
    path.value = rule.path;
    path.maxLength = 4096;
    path.required = true;
    path.setAttribute("aria-label", t("Relative path"));
    path.dataset.i18nAriaLabel = "Relative path";
    pathLabel.append(path);
    const recursive = make("input");
    recursive.type = "checkbox";
    recursive.checked = rule.recursive ?? false;
    const recursiveLabel = make("label", "Include descendants");
    recursiveLabel.prepend(recursive);
    const checks = [];
    for (const permission of [
      "discover",
      "read",
      "write",
      "chmod",
      "delete",
      "move",
    ]) {
      const check = make("input"),
        label = make("label", permission);
      check.type = "checkbox";
      check.checked = rule.deny.includes(permission);
      label.prepend(check);
      row.append(label);
      checks.push([permission, check]);
    }
    const remove = make("button", "Remove rule");
    remove.type = "button";
    remove.className = "btn btn-danger btn-sm";
    const item = {
      row,
      value: () => ({
        path: path.value,
        recursive: recursive.checked,
        deny: checks
          .filter(([, check]) => check.checked)
          .map(([permission]) => permission),
        ...(rule.rootId == null ? {} : { rootId: rule.rootId }),
      }),
    };
    remove.onclick = () => {
      if (busy) return;
      row.remove();
      collect.splice(collect.indexOf(item), 1);
    };
    row.prepend(pathLabel, recursiveLabel);
    row.append(remove);
    rows.append(row);
    collect.push(item);
  }
  for (const rule of view.rules[layer + "Rules"]) addRow(rule);
  const finish = () => {
    dialog.close();
    dialog.remove();
    prior?.focus();
  };
  close.onclick = finish;
  dialog.addEventListener("cancel", (event) => {
    event.preventDefault();
    if (!busy) finish();
  });
  add.onclick = () => addRow();
  save.onclick = async () => {
    if (busy) return;
    if (
      !confirm(
        t(
          "Save these restrictions and sign out all sessions for this account?",
        ),
      )
    )
      return;
    busy = true;
    for (const input of dialog.querySelectorAll("input,button"))
      input.disabled = true;
    try {
      const options = {
        expectedGeneration: view.account.generation,
        rules: collect.map((item) => item.value()),
      };
      const result = await (target
        ? api.setPolicy({ ...options, accountId: target.accountId })
        : api.set(options));
      finish();
      await onSaved(result);
    } catch (failure) {
      error.textContent = t(
        failure.code === "CONFLICT"
          ? "The account changed. Close and reopen this editor before retrying."
          : t("Policy was not saved: ") + t(failure.message),
      );
    } finally {
      busy = false;
      for (const input of dialog.querySelectorAll("input,button"))
        input.disabled = false;
    }
  };
  actions.append(add, save, close);
  dialog.append(title, notice, fixed, rows, error, actions);
  document.body.append(dialog);
  dialog.showModal();
  close.focus();
}
