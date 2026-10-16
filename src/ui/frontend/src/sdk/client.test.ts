import test from 'node:test';
import assert from 'node:assert/strict';
import { createClient } from './client.ts';

/** A fake SDK client: records calls and answers from `handlers`. */
function fakeSdk(handlers: Record<string, (args: any) => any> = {}) {
  const calls: Array<{ method: string; args: any }> = [];
  const call = (method: string) => async (args: any) => {
    calls.push({ method, args });
    const handler = handlers[method];
    if (!handler) throw Object.assign(new Error(`unexpected ${method}`), { code: 'UNEXPECTED' });
    return handler(args);
  };
  const sdk = {
    handshake: async () => ({}),
    on: () => () => {},
    files: {
      list: call('files.list'),
      stat: call('files.stat'),
      search: call('files.search'),
      readText: call('files.readText'),
      writeText: call('files.writeText'),
      move: call('files.move'),
      audit: { list: call('files.audit.list') },
      undo: {
        list: call('files.undo.list'),
        apply: call('files.undo.apply'),
        getPreferences: call('files.undo.getPreferences'),
        setPreferences: call('files.undo.setPreferences'),
      },
      trash: { list: call('files.trash.list'), deletePermanently: call('files.trash.deletePermanently') },
      archives: { create: call('files.archives.create'), extract: call('files.archives.extract') },
      policy: { get: call('files.policy.get'), set: call('files.policy.set') },
    },
    admin: {
      virtualUsers: {
        list: call('admin.virtualUsers.list'),
        create: call('admin.virtualUsers.create'),
        setQuota: call('admin.virtualUsers.setQuota'),
        enable: call('admin.virtualUsers.enable'),
        disable: call('admin.virtualUsers.disable'),
        delete: call('admin.virtualUsers.delete'),
        getPolicy: call('admin.virtualUsers.getPolicy'),
        setPolicy: call('admin.virtualUsers.setPolicy'),
      },
      systemUsers: {
        accounts: call('admin.systemUsers.accounts'),
        allow: call('admin.systemUsers.allow'),
        deny: call('admin.systemUsers.deny'),
        getPolicy: call('admin.systemUsers.getPolicy'),
      },
      quota: { getBudget: call('admin.quota.getBudget') },
    },
  };
  return { sdk, calls };
}

const REV = 'a'.repeat(64);
const user = {
  id: 'acc-1',
  username: 'alex',
  uid: 1,
  generation: 4,
  quotaBytes: 10,
  enabled: true,
  status: 'active',
  passwordChangeRequired: false,
  createdAt: '',
};

test('search maps nested matches to their real folder and searches the whole root', async () => {
  const { sdk, calls } = fakeSdk({
    'files.search': () => ({
      matches: [{ relativePath: 'docs/notes/plan.md', entry: { name: 'plan.md', fileType: 'File', size: 3 } }],
    }),
  });
  const client = createClient({ sdk });
  const [hit] = await client.files.search({ query: 'plan' });
  assert.equal(hit.path, '/docs/notes/plan.md');
  assert.equal(hit.name, 'plan.md');
  assert.equal(calls[0].args.path, '');
});

test('search failures surface instead of looking like "no results"', async () => {
  const { sdk } = fakeSdk({ 'files.search': () => Promise.reject(new Error('boom')) });
  await assert.rejects(createClient({ sdk }).files.search({ query: 'x' }), /boom/);
});

test('saving keeps the new revision only when the server still holds our content', async () => {
  let stored = 'mine';
  const { sdk } = fakeSdk({
    'files.writeText': () => ({}),
    'files.readText': () => ({ content: stored, revision: 'b'.repeat(64) }),
  });
  const client = createClient({ sdk });
  const saved = await client.files.writeText({ path: '/a.txt', content: 'mine', expectedRevision: REV });
  assert.equal(saved.revision, 'b'.repeat(64));

  stored = 'someone else';
  const raced = await client.files.writeText({ path: '/a.txt', content: 'mine', expectedRevision: REV });
  assert.equal(raced.revision, null);
});

test('saving without a known revision is refused rather than overwriting', async () => {
  const { sdk, calls } = fakeSdk({});
  await assert.rejects(
    createClient({ sdk }).files.writeText({ path: '/a.txt', content: 'x', expectedRevision: 1 as any }),
    (err: any) => err.code === 'REVISION_CONFLICT'
  );
  assert.equal(calls.length, 0);
});

test('history joins undo candidates and passes the cursor through', async () => {
  const record = { id: 'op-1', action: 'rename', sourcePath: 'a', targetPath: 'b', createdAt: 5, status: 'success' };
  const { sdk, calls } = fakeSdk({
    'files.audit.list': () => ({ items: [record], nextCursor: 'next-1' }),
    'files.undo.list': () => ({ items: [{ operation: record, available: true, expectedState: REV }] }),
  });
  const page = await createClient({ sdk }).files.undo.list({ limit: 10, cursor: 'c-0' });
  assert.equal(page.nextCursor, 'next-1');
  assert.equal(page.entries[0].undoable, true);
  assert.equal(page.entries[0].expectedState, REV);
  assert.equal(calls.find((c) => c.method === 'files.audit.list')!.args.cursor, 'c-0');
});

test('history falls back to undo candidates when the audit feature is off', async () => {
  const record = { id: 'op-2', action: 'chmod', sourcePath: 'x', createdAt: 5, status: 'success', oldMode: 0o644, newMode: 0o600 };
  const { sdk } = fakeSdk({
    'files.audit.list': () => Promise.reject(Object.assign(new Error('off'), { code: 'FEATURE_DISABLED' })),
    'files.undo.list': () => ({ items: [{ operation: record, available: false, unavailableReason: 'expired' }] }),
  });
  const page = await createClient({ sdk }).files.undo.list();
  assert.equal(page.entries.length, 1);
  assert.equal(page.entries[0].undoUnavailableReason, 'expired');
  assert.equal(page.nextCursor, null);
});

test('recent files skip paths that no longer exist and folders', async () => {
  const { sdk } = fakeSdk({
    'files.audit.list': () => ({
      items: [
        { action: 'write', sourcePath: 'a.txt' },
        { action: 'rename', sourcePath: 'old', targetPath: 'gone.txt' },
        { action: 'upload', sourcePath: 'a.txt' },
        { action: 'move', sourcePath: 'd', targetPath: 'sub/dir' },
      ],
    }),
    'files.stat': ({ path }: any) => {
      if (path === 'a.txt') return { entry: { name: 'a.txt', fileType: 'File' } };
      if (path === 'sub/dir') return { entry: { name: 'dir', fileType: 'Directory' } };
      throw Object.assign(new Error('missing'), { code: 'NOT_FOUND' });
    },
  });
  const recent = await createClient({ sdk }).files.getRecent(10);
  assert.deepEqual(recent.map((e) => e.path), ['/a.txt']);
});

test('emptying trash reports how many items were removed before a failure', async () => {
  const items = Array.from({ length: 10 }, (_, i) => ({ trashId: `t${i}` }));
  let deleted = 0;
  const { sdk } = fakeSdk({
    'files.trash.list': () => ({ items }),
    'files.trash.deletePermanently': ({ trashId }: any) => {
      if (trashId === 't5') return Promise.reject(new Error('locked'));
      deleted++;
      return {};
    },
  });
  await assert.rejects(createClient({ sdk }).files.trash.empty(), (err: any) => {
    assert.equal(err.message, 'locked');
    assert.equal(err.removed, deleted);
    return deleted > 0;
  });
});

test('account changes send the account id and generation, never the username', async () => {
  const { sdk, calls } = fakeSdk({
    'admin.virtualUsers.setQuota': () => ({ ...user, id: 'acc-1', quotaBytes: 20, status: 'active' }),
    'admin.virtualUsers.disable': () => ({}),
    'admin.virtualUsers.delete': () => ({}),
  });
  const client = createClient({ sdk });
  await client.admin.virtualUsers.setQuota({ user, quotaBytes: 20 });
  await client.admin.virtualUsers.toggleStatus({ user, enabled: false });
  await client.admin.virtualUsers.delete({ user });
  assert.deepEqual(calls.map((c) => c.args), [
    { accountId: 'acc-1', expectedGeneration: 4, quotaBytes: 20 },
    { accountId: 'acc-1' },
    { accountId: 'acc-1', expectedGeneration: 4 },
  ]);
});

test('virtual users are created with the admin-entered password', async () => {
  const { sdk, calls } = fakeSdk({ 'admin.virtualUsers.create': () => ({ id: 'n', username: 'new', status: 'active' }) });
  await createClient({ sdk }).admin.virtualUsers.create({ username: 'new', quotaBytes: 5, password: 'correct horse battery' });
  assert.deepEqual(calls[0].args, { name: 'new', quotaBytes: 5, password: 'correct horse battery' });
});

test('archives and policies send root-relative paths', async () => {
  const { sdk, calls } = fakeSdk({
    'files.archives.create': () => ({}),
    'files.archives.extract': () => ({}),
    'admin.virtualUsers.setPolicy': () => ({}),
  });
  const client = createClient({ sdk });
  await client.files.archives.create({ sources: ['/docs/a.txt', '/b'], target: '/docs/a.txt.zip', format: 'zip' });
  await client.files.archives.extract({ path: '/x.zip', destination: '/x' });
  await client.policy.set(
    { kind: 'virtual', accountId: 'acc-1', username: 'alex' },
    { expectedGeneration: 3, rules: [{ path: '/private/', recursive: true, deny: ['discover'] }] }
  );
  assert.deepEqual(calls[0].args, { sources: ['docs/a.txt', 'b'], path: 'docs/a.txt.zip', format: 'zip' });
  assert.deepEqual(calls[1].args, { path: 'x.zip', destination: 'x' });
  assert.deepEqual(calls[2].args, {
    accountId: 'acc-1',
    expectedGeneration: 3,
    rules: [{ path: 'private', recursive: true, deny: ['discover'] }],
  });
});

test('budget reads the camelCase reply and never invents a total', async () => {
  const { sdk } = fakeSdk({
    'admin.quota.getBudget': () => ({ limitBytes: 100, reservedBytes: 30, remainingBytes: 60, ownQuotaBytes: 10 }),
  });
  const budget = await createClient({ sdk }).admin.quota.getBudget();
  assert.deepEqual(budget, {
    totalDiskBytes: 100,
    virtualUsersAllocatedBytes: 30,
    systemReserveBytes: 10,
    freePhysicalBytes: 60,
  });
  const { sdk: empty } = fakeSdk({ 'admin.quota.getBudget': () => ({}) });
  await assert.rejects(createClient({ sdk: empty }).admin.quota.getBudget());
});
