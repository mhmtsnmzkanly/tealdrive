// Explicit real-service acceptance. Restarts only tealdrive.service when opted in.
// Uses one unique fixture; preserves actual user files and credentials.
const { chromium } = require('playwright');
const fs = require('node:fs');
const { randomUUID } = require('node:crypto');
const { execFile, execFileSync } = require('node:child_process');
const { promisify } = require('node:util');
const path = require('node:path');
(async () => {
  const url = process.env.TEALDRIVE_TEST_URL;
  const username = process.env.TEALDRIVE_TEST_USERNAME;
  const passwordFile = process.env.TEALDRIVE_TEST_WEB_PASSWORD_FILE;
  if (!url || !username || !passwordFile || process.env.TEALDRIVE_PRODUCTION_ACCEPTANCE !== '1' || process.getuid() !== 0)
    throw Error('Explicit Root production acceptance and dedicated account required');
  if (fs.statSync(passwordFile).mode & 0o077) throw Error('Private password file required');
  const originHost = process.env.TEALDRIVE_TEST_ORIGIN_HOST;
  if (originHost && new URL(url).hostname !== originHost) throw Error('Origin host mismatch');
  const fixture = `acceptance-${randomUUID()}.txt`;
  const home = execFileSync('/usr/bin/getent', ['passwd', username], {encoding:'utf8'}).trim().split(':')[5];
  if (!home?.startsWith('/home/') || home === '/home/tealdrive') throw Error('Dedicated regular user home required');
  const browser = await chromium.launch({headless:true,args:originHost ? [`--host-resolver-rules=MAP ${originHost} 127.0.0.1`] : []});
  let restarted = false;
  try {
    const context = await browser.newContext();
    const page = await context.newPage();
    const errors = [];
    page.on('pageerror', error => errors.push(error.name));
    const sockets=[]; page.on('websocket', socket=>sockets.push(socket.url()));
    async function login(page) {
      await page.goto(url);
      await page.fill('#usernameInput', username);
      await page.fill('#passwordInput', fs.readFileSync(passwordFile,'utf8').trim());
      await page.locator('#loginForm button[type=submit]').click();
      await page.waitForSelector('#loginModal.hidden',{state:'attached'});
      await page.waitForSelector('#wsStatusDot.connected');
    }
    await login(page);
    if (!sockets.length || sockets.some(socket=>!socket.startsWith('wss://'))) throw Error('WSS required');
    if (!(await context.cookies()).some(cookie=>cookie.httpOnly && cookie.secure)) throw Error('Secure HttpOnly cookie required');
    await page.evaluate(async fixture=>{
      const {createClient}=await import('/sdk/v1/tealdrive.js');
      window.acceptanceApi=createClient({storage:null,stateMode:'client',quotaPollMs:0});
      await acceptanceApi.handshake();
      const existing=await acceptanceApi.files.readText({rootId:'home',path:'Web test.md'});
      if(existing.content.length!==60)throw Error('Original browser file not preserved');
      await acceptanceApi.files.createText({rootId:'home',path:fixture,content:'accepted'});
      const original=await acceptanceApi.files.readText({rootId:'home',path:fixture});
      await acceptanceApi.files.writeText({rootId:'home',path:fixture,content:'updated',expectedRevision:original.revision});
      let conflict=false;
      try{await acceptanceApi.files.writeText({rootId:'home',path:fixture,content:'stale',expectedRevision:original.revision});}
      catch(error){conflict=error.code==='REVISION_CONFLICT';}
      if(!conflict)throw Error('Stale save accepted');
      const response=await acceptanceApi.files.download({rootId:'home',path:fixture});
      if(await response.text()!=='updated')throw Error('HTTPS download mismatch');
      const downloadRequestId=response.headers.get('x-tealdrive-request-id');
      if(!downloadRequestId)throw Error('Missing download request ID');
      const trace=await acceptanceApi.files.audit.list({requestId:downloadRequestId});
      if(trace.items.length!==1 || trace.items[0].bytesRead!==7)throw Error('HTTP/helper audit correlation failed');
      await acceptanceApi.files.chmod({rootId:'home',path:fixture,mode:0o640});
      const candidates=await acceptanceApi.files.undo.list({action:'chmod',path:fixture});
      const candidate=candidates.items.find(item=>item.available);
      if(!candidate)throw Error('Missing protected undo');
      await acceptanceApi.files.undo.apply({operationId:candidate.operation.id,expectedState:candidate.expectedState});
      const audit=await acceptanceApi.files.audit.list({path:fixture});
      if(!audit.items.some(item=>item.action==='download' && item.bytesRead===7))throw Error('Missing download history');
      acceptanceApi.disconnect();
    },fixture);
    await page.click('#refreshBtn');
    const row=page.locator('#fileTableBody tr').filter({has:page.locator('.item-name').filter({hasText:fixture})});
    await row.getByRole('button',{name:'Edit',exact:true}).click();
    await page.waitForSelector('#editorModal:not(.hidden)');
    await page.fill('#editorTextarea','unsaved acceptance text');
    const before=execFileSync('/usr/bin/systemctl',['show','tealdrive.service','--property=MainPID','--value'],{encoding:'utf8'}).trim();
    const restart=promisify(execFile)('/usr/bin/systemctl',['restart','tealdrive.service']);
    await page.waitForSelector('#shutdownModal:not(.hidden)');
    if(await page.inputValue('#editorTextarea')!=='unsaved acceptance text')throw Error('Shutdown discarded editor input');
    await restart; restarted=true;
    const after=execFileSync('/usr/bin/systemctl',['show','tealdrive.service','--property=MainPID','--value'],{encoding:'utf8'}).trim();
    if(before===after || after==='0')throw Error('Service did not restart');
    const fresh=await context.newPage();await login(fresh);
    if(fs.readFileSync(path.join(home,fixture),'utf8')!=='updated')throw Error('Unsaved editor text persisted');
    if(errors.length)throw Error('Browser exceptions: '+errors.join(','));
    console.log(JSON.stringify({originTls:!originHost ? 'public' : 'direct-origin',httpsWss:true,preservedUserFile:true,revisionConflict:true,protectedUndo:true,downloadAudit:true,realSystemdRestart:true,shutdownPopup:true,unsavedEditorPreserved:true,relogin:true}));
  } finally {
    if(!restarted)execFileSync('/usr/bin/systemctl',['start','tealdrive.service']);
    await browser.close();
    // Cleanup runs as the real file owner, never with Root filesystem privileges.
    execFileSync('/usr/sbin/runuser',['-u',username,'--','/usr/bin/python3','-c','import os,sys; p=sys.argv[1];\ntry: os.unlink(p)\nexcept FileNotFoundError: pass',path.join(home,fixture)]);
  }
})().catch(error=>{console.error(error.name,error.message);process.exitCode=1;});
