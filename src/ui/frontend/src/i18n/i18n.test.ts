import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync, readdirSync, statSync } from 'node:fs';
import { join } from 'node:path';
import { tr } from './tr.ts';
import { t, useLanguageForTests } from './index.ts';

const SRC = join(import.meta.dirname, '..');
// Values rendered through t(variable) rather than t('literal').
const DYNAMIC = ['active', 'suspended', 'revoked', 'deleted', 'unknown', 'queued', 'uploading', 'completed', 'error', 'aborted'];

function sources(dir: string): string[] {
  return readdirSync(dir).flatMap((name) => {
    const path = join(dir, name);
    if (statSync(path).isDirectory()) return name === 'i18n' ? [] : sources(path);
    return /\.tsx?$/.test(name) && !name.endsWith('.test.ts') ? [path] : [];
  });
}

function keysInSource(): Set<string> {
  const keys = new Set<string>(DYNAMIC);
  for (const file of sources(SRC)) {
    const text = readFileSync(file, 'utf8');
    for (const m of text.matchAll(/\bt\((['"])((?:\\.|(?!\1).)*)\1/g)) {
      keys.add(m[2].replace(/\\'/g, "'").replace(/\\"/g, '"'));
    }
    // Labels translated where they are used.
    const labels = text.match(/const AUDIT_LABELS[^}]*\}/s)?.[0];
    for (const m of labels?.matchAll(/: '([^']+)'/g) ?? []) keys.add(m[1]);
  }
  return keys;
}

const placeholders = (s: string) => [...s.matchAll(/\{(\w+)\}/g)].map((m) => m[1]).sort();

test('every UI string has a Turkish translation', () => {
  const missing = [...keysInSource()].filter((k) => !(k in tr));
  assert.deepEqual(missing, []);
});

test('translations keep exactly the placeholders of their source', () => {
  const broken = Object.entries(tr).filter(([en, translated]) => placeholders(en).join() !== placeholders(translated).join());
  assert.deepEqual(broken, []);
});

test('the dictionary has no stale entries', () => {
  const used = keysInSource();
  assert.deepEqual(Object.keys(tr).filter((k) => !used.has(k)), []);
});

test('t fills placeholders and falls back to English', () => {
  useLanguageForTests('tr');
  assert.equal(t('Moved into {targetDirPath}', { targetDirPath: '/a' }), '/a içine taşındı');
  assert.equal(t('Not a key {x}', { x: 1 }), 'Not a key 1');
  useLanguageForTests('en');
  assert.equal(t('Moved into {targetDirPath}', { targetDirPath: '/a' }), 'Moved into /a');
});
