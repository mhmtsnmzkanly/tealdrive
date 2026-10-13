import { t, text } from "/static/i18n.js";

/** Own history presentation. Authority, inverse paths and state checks belong to the helper. */
export async function openHistory({ client, onChanged = () => {}, restoreFocus }) {
  const prior = restoreFocus ?? document.activeElement;
  const dialog = document.createElement("dialog");
  dialog.id = "historyDialog";
  dialog.className = "policy-editor history-dialog";
  dialog.setAttribute("aria-label", t("Your operation history"));
  const make = (tag, caption) => {
    const node = document.createElement(tag);
    if (caption !== undefined) {
      const span = document.createElement("span");
      text(span, caption);
      node.append(span);
    }
    return node;
  };
  const button = (caption, id) => {
    const node = make("button", caption);
    node.type = "button";
    node.className = "btn btn-secondary";
    if (id) node.id = id;
    return node;
  };
  const title = make("h3", "Your operation history");
  const notice = make(
    "p",
    "Only your operations are shown. Undo checks current permissions and file state again; changed files and occupied targets are preserved.",
  );
  const controls = make("fieldset");
  const mode = make("select");
  mode.id = "historyMode";
  for (const [value, caption] of [
    ["history", "All operations"],
    ["undo", "Undo candidates"],
  ]) {
    const option = make("option");
    option.value = value;
    text(option, caption);
    mode.append(option);
  }
  mode.disabled = !client.getInfo().authentication.undo;
  const path = make("input");
  path.id = "historyPath";
  path.type = "search";
  path.maxLength = 256;
  path.setAttribute("aria-label", t("Filter by path"));
  path.dataset.i18nAriaLabel = "Filter by path";
  const pathLabel = make("label", "Filter by path");
  pathLabel.append(path);
  const action = make("select");
  action.id = "historyAction";
  for (const value of [
    "",
    "chmod",
    "rename",
    "move",
    "trash",
    "restore",
    "write",
    "upload",
    "delete",
    "copy",
    "readText",
    "download",
    "archive",
    "extract",
    "createFolder",
  ]) {
    const option = make("option");
    option.value = value;
    text(option, value || "All actions");
    action.append(option);
  }
  action.setAttribute("aria-label", t("Action"));
  action.dataset.i18nAriaLabel = "Action";
  const status = make("select");
  status.id = "historyStatus";
  for (const value of ["", "success", "failed", "pending", "unknown"]) {
    const option = make("option");
    option.value = value;
    text(option, value || "All statuses");
    status.append(option);
  }
  status.setAttribute("aria-label", t("Status"));
  status.dataset.i18nAriaLabel = "Status";
  const refresh = button("Refresh", "historyRefresh");
  controls.append(mode, pathLabel, action, status, refresh);
  const preferences = make("fieldset");
  preferences.append(make("legend", "Undo duration"));
  const inherit = make("input");
  inherit.id = "historyInherit";
  inherit.type = "checkbox";
  const inheritLabel = make("label", "Use the server maximum");
  inheritLabel.prepend(inherit);
  const hours = make("input");
  hours.id = "historyHours";
  hours.type = "number";
  hours.min = "1";
  const hoursLabel = make("label", "Hours");
  hoursLabel.append(hours);
  const maximum = make("p");
  const save = button("Save preference", "historySavePreference");
  preferences.append(inheritLabel, hoursLabel, maximum, save);
  const error = make("p");
  error.id = "historyError";
  error.setAttribute("role", "alert");
  const state = make("p");
  state.id = "historyState";
  state.setAttribute("role", "status");
  const confirmation = make("section");
  confirmation.hidden = true;
  const confirmationText = make("p");
  const confirm = button("Confirm undo", "historyConfirmUndo");
  const cancel = button("Cancel");
  confirmation.append(confirmationText, confirm, cancel);
  const list = make("div");
  list.id = "historyItems";
  const first = button("First page", "historyFirst");
  const next = button("Next page", "historyNext");
  next.disabled = true;
  const close = button("Close", "historyClose");
  dialog.append(
    title,
    notice,
    controls,
    preferences,
    error,
    state,
    confirmation,
    list,
    first,
    next,
    close,
  );
  let epoch = 0,
    busy = false,
    cursor = null,
    nextCursor = null,
    selected = null,
    preference = null;
  const visible = () => dialog.isConnected && dialog.open;
  const tracked = [
    mode,
    path,
    action,
    status,
    refresh,
    first,
    next,
    save,
    inherit,
    hours,
    confirm,
    cancel,
  ];
  const setBusy = (value) => {
    busy = value;
    for (const node of tracked) node.disabled = value;
    mode.disabled = value || !client.getInfo().authentication.undo;
    next.disabled = value || !nextCursor;
    save.disabled = value || !preference;
    hours.disabled = value || inherit.checked;
    for (const node of list.querySelectorAll("button"))
      node.disabled = value || node.dataset.blocked === "true";
  };
  const finish = () => {
    epoch++;
    dialog.close();
    dialog.remove();
    prior?.focus();
  };
  close.onclick = finish;
  dialog.addEventListener("cancel", (event) => {
    event.preventDefault();
    finish();
  });
  dialog.addEventListener("keydown", (event) => event.stopPropagation());
  const fail = (exception) => {
    error.textContent = t("Request failed: {code}", {
      code: exception.code || "UNAVAILABLE",
    });
  };
  function render(items) {
    list.replaceChildren();
    for (const item of items) {
      const candidate = mode.value === "undo" ? item : null;
      const record = candidate ? item.operation : item;
      const row = make("article");
      row.className = "history-row";
      row.dataset.operationId = record.id;
      const detail = make("p");
      detail.textContent = `${new Date(record.createdAt * 1000).toLocaleString()} · ${t(record.action)} · ${t(record.status)} · ${record.sourcePath || "/"}`;
      if (record.targetPath && record.action !== "trash")
        detail.textContent += ` → ${record.targetPath}`;
      row.append(detail);
      if (record.oldMode !== null && record.oldMode !== undefined) {
        const modes = make("p");
        modes.textContent = `${t("Permissions")}: ${record.oldMode.toString(8).padStart(3, "0")} → ${(record.newMode ?? record.requestedMode ?? 0).toString(8).padStart(3, "0")}`;
        row.append(modes);
      }
      if (record.undoStatus) {
        const result = make("p");
        result.textContent = `${t("Undo")}: ${t(record.undoStatus)}${record.undoErrorCode ? ` · ${record.undoErrorCode}` : ""}`;
        row.append(result);
      }
      if (candidate) {
        const note = make("p");
        note.textContent = candidate.available
          ? t("Available until {time}", {
              time: new Date(candidate.expiresAt * 1000).toLocaleString(),
            })
          : t(candidate.unavailableReason || "Unavailable");
        row.append(note);
        const undo = button("Undo");
        undo.dataset.blocked = String(!candidate.available);
        undo.disabled = !candidate.available;
        undo.onclick = () => {
          if (busy || !candidate.available) return;
          selected = candidate;
          confirmationText.textContent = t(
            "Undo {action} for {path}? The helper will verify the recorded state before changing files.",
            { action: t(record.action), path: record.sourcePath },
          );
          confirmation.hidden = false;
          confirm.focus();
        };
        row.append(undo);
      }
      list.append(row);
    }
  }
  async function load(continuation = false) {
    if (busy || !visible()) return;
    cursor = continuation ? nextCursor : null;
    nextCursor = null;
    const ticket = ++epoch;
    selected = null;
    confirmation.hidden = true;
    error.textContent = "";
    text(state, "Loading history…");
    list.replaceChildren();
    setBusy(true);
    try {
      const query = {
        limit: 50,
        ...(cursor ? { cursor } : {}),
        ...(path.value ? { path: path.value } : {}),
        ...(action.value ? { action: action.value } : {}),
        ...(status.value ? { status: status.value } : {}),
      };
      const page =
        mode.value === "undo"
          ? await client.files.undo.list(query)
          : await client.files.audit.list(query);
      if (ticket !== epoch || !visible()) return;
      nextCursor = page.nextCursor;
      render(page.items);
      text(
        state,
        page.items.length
          ? "Showing {count} operations"
          : "No operations found",
        { count: page.items.length },
      );
    } catch (exception) {
      if (ticket === epoch && visible()) {
        fail(exception);
        text(state, "History unavailable");
      }
    } finally {
      if (ticket === epoch && visible()) setBusy(false);
    }
  }
  refresh.onclick = () => load();
  first.onclick = () => load();
  next.onclick = () => load(true);
  for (const node of [mode, action, status]) node.onchange = () => load();
  path.onkeydown = (event) => {
    if (event.key === "Enter") {
      event.preventDefault();
      load();
    }
  };
  inherit.onchange = () => {
    hours.disabled = inherit.checked;
  };
  cancel.onclick = () => {
    selected = null;
    confirmation.hidden = true;
  };
  confirm.onclick = async () => {
    if (busy || !selected) return;
    const candidate = selected,
      ticket = epoch;
    setBusy(true);
    error.textContent = "";
    try {
      await client.files.undo.apply({
        operationId: candidate.operation.id,
        expectedState: candidate.expectedState,
      });
      onChanged();
      if (ticket !== epoch || !visible()) return;
      selected = null;
      confirmation.hidden = true;
      setBusy(false);
      await load();
    } catch (exception) {
      if (ticket === epoch && visible()) {
        selected = null;
        confirmation.hidden = true;
        setBusy(false);
        await load();
        if (visible()) fail(exception);
      }
    } finally {
      if (ticket === epoch && visible()) setBusy(false);
    }
  };
  save.onclick = async () => {
    if (busy || !preference) return;
    const value = inherit.checked ? null : Number(hours.value);
    if (
      value !== null &&
      (!Number.isSafeInteger(value) ||
        value < 1 ||
        value > preference.maximumHours)
    ) {
      fail({ code: "INVALID_ARGUMENT" });
      return;
    }
    const ticket = epoch;
    setBusy(true);
    error.textContent = "";
    try {
      preference = await client.files.undo.setPreferences({
        hours: value,
        expectedRevision: preference.revision,
      });
      if (ticket !== epoch || !visible()) return;
      setBusy(false);
      await load();
    } catch (exception) {
      if (ticket === epoch && visible()) {
        fail(exception);
        try {
          preference = await client.files.undo.getPreferences();
          syncPreference();
        } catch {}
      }
    } finally {
      if (ticket === epoch && visible()) setBusy(false);
    }
  };
  function syncPreference() {
    if (!preference || !visible()) return;
    inherit.checked = preference.hours === null;
    hours.value = String(preference.effectiveHours);
    hours.max = String(preference.maximumHours);
    text(maximum, "Server maximum: {hours} hours", {
      hours: preference.maximumHours,
    });
  }
  document.body.append(dialog);
  dialog.showModal();
  close.focus();
  setBusy(true);
  try {
    preference = await client.files.undo.getPreferences();
    syncPreference();
  } catch (exception) {
    if (visible()) text(maximum, "Undo preference unavailable");
  }
  if (visible()) {
    setBusy(false);
    await load();
  }
}
