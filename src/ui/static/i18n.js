/** Presentation only. No credentials, filesystem contents or authorization state are persisted. */
let language = "en";
let account = null;
let translations = {};
let epoch = 0;
const known = new Set(["en", "tr"]);
const key = () =>
  account ? `tealdrive:ui-language:v1:${location.origin}:${account}` : null;
export function t(text, values = {}) {
  const template = language === "tr" ? (translations[text] ?? text) : text;
  return template.replace(/\{(\w+)\}/g, (match, name) =>
    String(values[name] ?? match),
  );
}
export function text(node, source, values = {}) {
  node.textContent = t(source, values);
  node.dataset.i18n = source;
  node.dataset.i18nValues = JSON.stringify(values);
}
function apply(root = document) {
  for (const node of root.querySelectorAll("[data-i18n]")) {
    let values = {};
    try {
      values = JSON.parse(node.dataset.i18nValues ?? "{}");
    } catch {}
    const translated = t(node.dataset.i18n, values);
    if (node.textContent !== translated) node.textContent = translated;
  }
  for (const attr of ["title", "placeholder", "aria-label"]) {
    for (const node of root.querySelectorAll(`[data-i18n-${attr}]`)) {
      const value = t(node.getAttribute("data-i18n-" + attr));
      if (node.getAttribute(attr) !== value) node.setAttribute(attr, value);
    }
  }
}
export async function setLanguage(next, persist = false) {
  if (!known.has(next)) throw Error("Unsupported language");
  const revision = ++epoch;
  const dictionary =
    next === "tr"
      ? await fetch("/static/locales/tr.json", { cache: "no-cache" }).then(
          (response) => {
            if (!response.ok) throw Error("Translation unavailable");
            return response.json();
          },
        )
      : {};
  if (revision !== epoch) return;
  language = next;
  translations = dictionary;
  document.documentElement.lang = next;
  document.getElementById("uiLanguage").value = next;
  apply();
  if (persist && key())
    try {
      localStorage.setItem(key(), next);
    } catch {}
  document.dispatchEvent(
    new CustomEvent("tealdrive.language", { detail: { language: next } }),
  );
}
export async function bindAccount(id) {
  account = id === null ? null : String(id);
  let preferred = navigator.language?.startsWith("tr") ? "tr" : "en";
  if (key())
    try {
      const stored = localStorage.getItem(key());
      if (known.has(stored)) preferred = stored;
    } catch {}
  await setLanguage(preferred);
}
window.TealDriveI18n = {
  t,
  text,
  setLanguage,
  bindAccount,
  get language() {
    return language;
  },
};
const select = document.getElementById("uiLanguage");
select.addEventListener("change", () =>
  setLanguage(select.value, true).catch(() => {
    select.value = language;
  }),
);
// Only marked presentation nodes are translated. A filename/content has no marker.
new MutationObserver(() => apply()).observe(document.body, {
  childList: true,
  subtree: true,
});
bindAccount(document.documentElement.dataset.uiAccount ?? null).catch(() => {});
