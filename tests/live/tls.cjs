const {chromium}=require('playwright');
const fs=require('node:fs');
(async()=>{
  if(process.env.TEALDRIVE_TLS_ACCEPTANCE!=='1')throw Error('Explicit TLS fixture required');
  const {users}=JSON.parse(fs.readFileSync(process.env.TEALDRIVE_ADMIN_FIXTURE_MANIFEST,'utf8'));
  const browser=await chromium.launch({headless:true});
  try{
    const page=await browser.newPage({ignoreHTTPSErrors:true}); // self-signed TEST certificate only.
    const endpoints=[];page.on('websocket',socket=>endpoints.push(socket.url()));
    await page.goto(process.env.TEALDRIVE_TLS_TEST_URL);
    await page.fill('#usernameInput',users[1].username);await page.fill('#passwordInput',users[1].password);
    await page.locator('#loginForm button[type=submit]').click();
    await page.waitForSelector('#loginModal.hidden',{state:'attached'});
    await page.waitForFunction(()=>document.querySelector('#wsStatusText').textContent==='Connected');
    if(endpoints.length!==1||!endpoints[0].startsWith('wss://'))throw Error('TLS UI did not use WSS');
    const cookies=await page.context().cookies();
    if(!cookies.some(cookie=>cookie.httpOnly&&cookie.secure))throw Error('Secure HttpOnly session missing');
    await page.evaluate(async()=>{
      const {createClient}=await import('/sdk/v1/tealdrive.js');const api=createClient({storage:null,quotaPollMs:0});await api.handshake();
      const files=await api.files.list({rootId:'home',path:'/'});if(!files.entries.some(entry=>entry.name==='fixture.txt'))throw Error('TLS helper operation failed');
      api.disconnect();
    });
    const row=page.locator('#fileTableBody tr').filter({has:page.locator('.item-name').filter({hasText:'fixture.txt'})});
    await row.getByRole('button',{name:'Edit',exact:true}).click();
    await page.waitForSelector('#editorModal:not(.hidden)');
    await page.fill('#editorTextarea','unsaved TLS shutdown content');
    const pid=Number(process.env.TEALDRIVE_TLS_STOP_PID);
    if(!Number.isSafeInteger(pid)||pid<=1)throw Error('Missing isolated fixture PID');
    process.kill(-pid,'SIGTERM');
    await page.waitForSelector('#shutdownModal:not(.hidden)');
    if(await page.inputValue('#editorTextarea')!=='unsaved TLS shutdown content')throw Error('TLS shutdown lost editor input');
    console.log(JSON.stringify({httpsUi:true,wssHandshake:true,secureCookie:true,sdkHelperFiles:true,tlsShutdownPopup:true,unsavedInputPreserved:true}));
  }finally{await browser.close();}
})().catch(error=>{console.error(error.name,error.code??error.message);process.exitCode=1;});
