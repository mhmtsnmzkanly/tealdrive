import { tr } from './tr.ts';

// English strings are the keys; a missing translation falls back to English.
export type Language = 'en' | 'tr';

const STORAGE_KEY = 'tdrv_language';
const dictionaries: Record<Language, Record<string, string>> = { en: {}, tr };

function detect(): Language {
  try {
    const saved = globalThis.localStorage?.getItem(STORAGE_KEY);
    if (saved === 'en' || saved === 'tr') return saved;
  } catch {
    // Storage can be blocked; fall back to the browser language.
  }
  const nav = (globalThis as any).navigator?.language;
  return typeof nav === 'string' && nav.toLowerCase().startsWith('tr') ? 'tr' : 'en';
}

let current: Language = detect();

export function language(): Language {
  return current;
}

/** Saves the choice and reloads, so every component renders in the new language. */
export function setLanguage(next: Language): void {
  try {
    globalThis.localStorage?.setItem(STORAGE_KEY, next);
  } catch {
    // Without storage the choice lasts until reload only.
  }
  current = next;
  globalThis.location?.reload();
}

/** For tests: switch without touching storage or reloading. */
export function useLanguageForTests(next: Language): void {
  current = next;
}

/** Translates `text` and fills `{name}` placeholders from `params`. */
export function t(text: string, params: Record<string, string | number> = {}): string {
  const template = dictionaries[current][text] ?? text;
  return template.replace(/\{(\w+)\}/g, (match, key) => (key in params ? String(params[key]) : match));
}

/** Locale for dates and numbers. */
export function locale(): string {
  return current === 'tr' ? 'tr-TR' : 'en-US';
}
