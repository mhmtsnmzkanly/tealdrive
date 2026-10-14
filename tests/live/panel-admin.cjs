// Read-only production check for the migrated virtual global panel administrator.
// Explicit opt-in; never print credentials or file contents.
const {chromium}=require('playwright');
const fs=require('node:fs');
(async()=>{
 if(process.env.TEALDRIVE_PANEL_ADMIN_ACCEPTANCE!=='1')throw Error('Explicit panel administrator acceptance required');
 const secret=process.env.TEALDRIVE_TEST_WEB_PASSWORD_FILE;
 if(!secret||(fs.statSync(secret).mode&0o077))throw Error('Private credential file required');
 const url=process.env.TEALDRIVE_TEST_URL;if(!url)throw Error('Explicit test URL required');
 const host=process.env.TEALDRIVE_TEST_ORIGIN_HOST;
 if(host&&new URL(url).hostname!==host)throw Error('Origin host mismatch');
 const browser=await chromium.launch({headless:true,args:host?[`--host-resolver-rules=MAP ${host} 127.0.0.1`]:[]});
 try{
  const page=await browser.newPage();const errors=[];page.on('pageerror',error=>errors.push(error.name));
  await page.goto(url);
  await page.fill('#usernameInput','tealdrive');await page.fill('#passwordInput',fs.readFileSync(secret,'utf8').trim());
  await page.click('#loginForm button[type=submit]');await page.waitForSelector('#loginModal.hidden',{state:'attached'});await page.waitForSelector('#wsStatusDot.connected');
  if(await page.locator('#settingsBtn').isDisabled())throw Error('Global panel controls unavailable');
  const result=await page.evaluate(async()=>{
   const {createClient}=await import('/sdk/v1/tealdrive.js');const api=createClient({storage:null,quotaPollMs:0});
   await api.handshake();try{
    const session=await(await fetch('/api/session')).json();
    if(session.account_kind!=='virtual'||!session.is_admin||session.username!=='tealdrive')throw Error('Wrong panel identity or scope');
    const budget=await api.admin.quota.getBudget();
    if(budget.ownQuotaBytes!==128*1024*1024)throw Error('Unexpected fixed own quota');
    const file=await api.files.readText({rootId:'home',path:'test/hello gpt.md'});
    if(new TextEncoder().encode(file.content).length!==93)throw Error('User file size changed');
    if((await fetch('/api/admin/config')).status!==200)throw Error('Global management denied');
    const info=api.getInfo();if(!info.transport.encryptedWebSocket)throw Error('Encrypted WebSocket required');
    return {virtualGlobalAdmin:true,preservedUserFile:true,fixedOwnQuota:true,encryptedWebSocket:true,httpTls:true};
   }finally{api.disconnect();}
  });
  await page.click('#logoutBtn');await page.waitForSelector('#loginModal:not(.hidden)');
  if(errors.length)throw Error('Browser exceptions: '+errors.join(','));console.log('PASS migrated panel admin '+JSON.stringify(result));
 }finally{await browser.close();}
})().catch(error=>{console.error(error.message);process.exitCode=1;});
