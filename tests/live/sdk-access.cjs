// Opt-in real-browser acceptance. Run as root with a dedicated test account/root.
// Requires Playwright on NODE_PATH; password is read from a private file, never argv/logs.
const { chromium } = require('playwright');
const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');
(async () => {
  if (process.getuid() !== 0) throw Error('This fixture requires root');
  for (const key of ['TEALDRIVE_TEST_URL', 'TEALDRIVE_TEST_ROOT', 'TEALDRIVE_TEST_USERNAME', 'TEALDRIVE_TEST_UID', 'TEALDRIVE_TEST_GID', 'TEALDRIVE_TEST_PASSWORD_FILE']) {
    if (!process.env[key]) throw Error('Missing ' + key);
  }
  const root = fs.realpathSync(process.env.TEALDRIVE_TEST_ROOT);
  if (root === '/') throw Error('A dedicated test root is required');
  const uid = Number(process.env.TEALDRIVE_TEST_UID), gid = Number(process.env.TEALDRIVE_TEST_GID);
  if (!Number.isInteger(uid) || uid <= 0 || !Number.isInteger(gid) || gid <= 0) throw Error('Nonroot test identity required');
  const passwordPath = process.env.TEALDRIVE_TEST_PASSWORD_FILE;
  const secretMetadata = fs.statSync(passwordPath);
  if ((secretMetadata.mode & 0o077) !== 0) throw Error('Password file must be private');
  const directory = 'policy-test-' + crypto.randomUUID();
  const fixture = path.join(root, directory), moved = fixture + '-moved';
  fs.mkdirSync(fixture, { mode: 0o700 });
  fs.chownSync(fixture, uid, gid);
  fs.mkdirSync(path.join(fixture, '.ssh'), { mode: 0o700 });
  fs.chownSync(path.join(fixture, '.ssh'), uid, gid);
  fs.writeFileSync(path.join(fixture, '.ssh/secret'), 'hidden-fixture', {mode:0o600});
  fs.chownSync(path.join(fixture, '.ssh/secret'), uid, gid);
  fs.linkSync(path.join(fixture, '.ssh/secret'), path.join(fixture, 'alias.txt'));
  fs.writeFileSync(path.join(fixture, 'public.txt'), 'public-fixture', {mode:0o600});
  fs.chownSync(path.join(fixture, 'public.txt'), uid, gid);
  let browser;
  try {
    browser = await chromium.launch({headless:true});
    const page = await browser.newPage();
    await page.goto(process.env.TEALDRIVE_TEST_URL);
    const result = await page.evaluate(async ({directory, username, password}) => {
      const {createClient} = await import('/sdk/v1/tealdrive.js');
      const api = createClient({storage:null,quotaPollMs:0});
      await api.handshake();
      await api.auth.login({username,password});
      async function absent(operation) {
        try {await operation();} catch(e) {if (e.code === 'NOT_FOUND') return; throw Error('Unexpected rejection: ' + e.code);}
        throw Error('Hidden object was accessible');
      }
      try {
        const list = await api.files.list({path:directory,includeHidden:true});
        if (list.entries.some(e=>e.name==='.ssh'||e.name==='alias.txt')) throw Error('Discovery leak');
        for (const name of ['.ssh/secret','alias.txt']) {
          await absent(()=>api.files.stat({path:directory+'/'+name}));
          await absent(()=>api.files.readText({path:directory+'/'+name}));
          await absent(()=>api.files.download({path:directory+'/'+name}));
        }
        await absent(()=>api.files.move({source:{path:directory},destination:{path:directory+'-moved'}}));
        const visible = await api.files.readText({path:directory+'/public.txt'});
        if (visible.content !== 'public-fixture') throw Error('Allowed file stopped working');
        return {discovery:true,directRead:true,download:true,hardlink:true,parentMove:true,allowedRead:true};
      } finally {await api.auth.logout();api.disconnect();}
    }, {directory,username:process.env.TEALDRIVE_TEST_USERNAME,password:fs.readFileSync(passwordPath,'utf8').trim()});
    console.log('PASS live SDK path policy:',JSON.stringify(result));
  } finally {
    if (browser) await browser.close();
    fs.rmSync(fixture,{recursive:true,force:true});
    fs.rmSync(moved,{recursive:true,force:true});
  }
})().catch(e=>{console.error(e.message);process.exitCode=1;});
