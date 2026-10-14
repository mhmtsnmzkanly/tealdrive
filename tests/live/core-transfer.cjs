// Candidate-only private namespace acceptance; credentials never appear in output.
const {chromium}=require('playwright');
const fs=require('node:fs');
(async()=>{
 const browser=await chromium.launch({headless:true,args:['--host-resolver-rules=MAP tealdrive-origin.local 127.0.0.1','--no-proxy-server']});
 try {
  const context=await browser.newContext({ignoreHTTPSErrors:true});
  const page=await context.newPage();
  page.on('console',message=>{if(message.text().startsWith('core-step:'))console.log(message.text());});
  await page.exposeFunction('tamperUpload',kind=>{
   const directory='/home/tealdrive/data/runtime/tmp';
   const files=fs.readdirSync(directory).filter(n=>n.startsWith('.tdrv-upload-'));
   if(files.length!==1)throw Error('Unexpected staging count');
   const file=directory+'/'+files[0];
   if(kind==='permissions')fs.chmodSync(file,0o666);
   else if(kind==='truncate')fs.truncateSync(file,1);
   else throw Error('Unknown fixture manipulation');
  });
  await page.goto('https://tealdrive-origin.local:13106');
  await page.fill('#usernameInput',process.env.TEALDRIVE_TEST_USERNAME);
  await page.fill('#passwordInput',fs.readFileSync(process.env.TEALDRIVE_CORE_BOOTSTRAP==='1'?process.env.TEALDRIVE_BOOTSTRAP_PASSWORD_FILE:process.env.TEALDRIVE_TEST_WEB_PASSWORD_FILE,'utf8').trim());
  await page.click('#loginForm button[type=submit]');
  await page.waitForSelector('#loginModal.hidden',{state:'attached'});
  if(process.env.TEALDRIVE_CORE_BOOTSTRAP==='1'){
   await page.waitForSelector('#accountSecurityModal:not(.hidden)');
   const restricted=await page.evaluate(async()=>{
    const session=await(await fetch('/api/session')).json();
    const status=(await fetch('/api/admin/config')).status;
    const {createClient}=await import('/sdk/v1/tealdrive.js');const client=createClient();await client.handshake();
    let denied=false;try{await client.files.list({rootId:'home',path:'/'});}catch(error){denied=error.code==='PASSWORD_CHANGE_REQUIRED';}
    // A custom/hostile client that removes the presentation flag still hits server enforcement.
    client._session.passwordChangeRequired=false;client._session.roots=[{rootId:'home'}];
    let serverDenied=false;try{await client.files.list({rootId:'home',path:'/'});}catch(error){serverDenied=error.code==='PERMISSION_DENIED';}
    client.disconnect();return {restricted:session.password_change_required,noRoots:session.roots.length===0,adminDisabled:!session.is_admin,status,denied,serverDenied};
   });
   if(!restricted.restricted||!restricted.noRoots||!restricted.adminDisabled||restricted.status!==403||!restricted.denied||!restricted.serverDenied)throw Error('First-login restriction bypassed');
   await page.fill('#accountCurrentPassword',fs.readFileSync(process.env.TEALDRIVE_BOOTSTRAP_PASSWORD_FILE,'utf8').trim());
   const replacement=fs.readFileSync(process.env.TEALDRIVE_TEST_WEB_PASSWORD_FILE,'utf8').trim();
   await page.fill('#accountNewPassword',replacement);await page.fill('#accountConfirmPassword',replacement);
   await page.click('#accountPasswordForm button[type=submit]');
   await page.waitForSelector('#loginModal:not(.hidden)');
   await page.fill('#usernameInput',process.env.TEALDRIVE_TEST_USERNAME);await page.fill('#passwordInput',replacement);
   await page.click('#loginForm button[type=submit]');await page.waitForSelector('#loginModal.hidden',{state:'attached'});
   console.log('core-step: random bootstrap, restricted UI/SDK/server, first password change and re-login passed');
  }

  if(process.env.TEALDRIVE_EXPECT_GLOBAL==='True'){
   if(await page.locator('#settingsBtn').isDisabled())throw Error('Virtual global settings disabled');
   const statuses=await page.evaluate(async()=>[(await fetch('/api/admin/config')).status,(await fetch('/api/admin/accounts/system')).status]);
   if(statuses.some(status=>status!==200))throw Error('Virtual global management denied');
  }

  const result=await page.evaluate(async({channelKey})=>{
   const {createClient}=await import('/sdk/v1/tealdrive.js');
   if(channelKey){
    await new Promise((resolve,reject)=>{
     const socket=new WebSocket('wss://'+location.host+'/ws/auth');const timer=setTimeout(()=>reject(Error('Plaintext client not rejected')),3000);
     socket.onopen=()=>socket.send(new Uint8Array(64));
     socket.onclose=()=>{clearTimeout(timer);resolve();};socket.onerror=()=>{};
    });
    const wrong=createClient({encryption:'required',trustedServerKeys:['00'.repeat(32)]});
    let rejected=false;try{await wrong.handshake();}catch(error){rejected=error.code==='SERVER_KEY_MISMATCH';}finally{wrong.disconnect();}
    if(!rejected)throw Error('Wrong server pin accepted');
   }
   const client=createClient({timeoutMs:120000,...(channelKey?{encryption:'required',trustedServerKeys:[channelKey]}:{})});await client.handshake();
   if(channelKey&&(!client.getInfo().transport.encryptedWebSocket||client.getInfo().transport.keyTrust!=='pinned'))throw Error('Encrypted pinned channel unavailable');
   const bytes=new Uint8Array(17*1024*1024);bytes.fill(0x5a);
   const file=new File([bytes],'core-large.bin');
   const progress=[];
   const upload=await client.files.upload({directory:'/',file,onProgress:p=>progress.push(p.transferredBytes)});
   if(upload.bytesWritten!==bytes.length)throw Error('Wrong commit byte count');
   if(progress.at(-1)!==bytes.length||progress.some((n,i)=>i&&n<=progress[i-1]))throw Error('Progress not monotonic');
   const response=await client.files.download({path:'/core-large.bin'});
   const reader=response.body.getReader();let total=0;
   while(true){const {done,value}=await reader.read();if(done)break;for(const byte of value)if(byte!==0x5a)throw Error('Download content changed');total+=value.length;}
   if(total!==bytes.length)throw Error('Incomplete streaming download');
   const range=await fetch(client.files.getDownloadUrl({path:'/core-large.bin'}),{headers:{Range:'bytes=1048576-1049599'}});
   if(range.status!==206||(await range.arrayBuffer()).byteLength!==1024)throw Error('Range failed');
   console.log('core-step: large upload/download/range passed');
   let interrupted;
   const resumeFile=new File([bytes.slice(0,2*1024*1024)],'core-resume.bin');
   try{await client.files.upload({directory:'/',file:resumeFile,onProgress:()=>client.disconnect()});}
   catch(error){interrupted=error.params?.resumeTransferId;}
   if(!interrupted)throw Error('Interrupted upload did not expose resume transfer');
   await new Promise(resolve=>setTimeout(resolve,100));await client.handshake();
   let resumedBytes=0;
   const resumed=await client.files.upload({directory:'/',file:resumeFile,resumeTransferId:interrupted,onTransfer:t=>{resumedBytes=t.resumedBytes;}});
   if(resumed.bytesWritten!==resumeFile.size||resumedBytes<=0)throw Error('Resume did not preserve received bytes');
   const resumedDownload=await client.files.download({path:'/core-resume.bin'});
   const resumedContent=new Uint8Array(await resumedDownload.arrayBuffer());
   if(resumedContent.length!==resumeFile.size||resumedContent.some(b=>b!==0x5a))throw Error('Resume changed file content');
   console.log('core-step: new WS same-session resume, prefix hash and complete bytes passed');
   let mismatchTransfer;
   try{await client.files.upload({directory:'/',file:new File([bytes.slice(0,1048576)],'core-resume-mismatch.bin'),onProgress:()=>client.disconnect()});}
   catch(error){mismatchTransfer=error.params?.resumeTransferId;}
   if(!mismatchTransfer)throw Error('Mismatch fixture did not retain transfer');
   await new Promise(resolve=>setTimeout(resolve,100));await client.handshake();
   let mismatch=false;
   try{await client.files.upload({directory:'/',file:new File([new Uint8Array(1048576)],'core-resume-mismatch.bin'),resumeTransferId:mismatchTransfer});}
   catch(error){mismatch=error.code==='RESUME_FILE_MISMATCH';}
   if(!mismatch)throw Error('Wrong file prefix accepted for resume');


   const controller=new AbortController();
   let cancelled=false;
   try{await client.files.upload({directory:'/',file:new File([bytes.slice(0,1048576)],'core-abort.bin'),signal:controller.signal,onProgress:()=>controller.abort()});}
   catch(error){cancelled=error.code==='ABORTED';}
   if(!cancelled)throw Error('Cancellation did not reject upload');
   console.log('core-step: cancellation passed');
   const original=client._request.bind(client);
   const rejected=[];
   for(const kind of ['permissions','truncate']){
    client._request=async(type,payload,options)=>{
     if(type==='UploadEnd')await window.tamperUpload(kind);
     return original(type,payload,options);
    };
    console.log('core-step: testing '+kind);
    let failed=false;
    try{await client.files.upload({directory:'/',file:new File([bytes.slice(0,65536)],'core-bad-'+kind+'.bin')});}
    catch(error){failed=true;console.log('core-step: rejected '+kind+' '+error.code);}
    if(!failed)throw Error('Tampered staging was published');
    rejected.push(kind);
    client.disconnect();
    await client.handshake();
   }
   client._request=original;
   console.log('core-step: rejected staging checks passed');
   let listing;try{listing=await client.files.list({path:'/'});}catch(error){console.log('core-step: list failed '+error.code);throw error;}
   console.log('core-step: listed '+listing.entries?.length);
   if(listing.entries.some(entry=>entry.name==='core-abort.bin'||entry.name==='core-resume-mismatch.bin'||entry.name.startsWith('core-bad-')))throw Error('Failed upload published a target');
   if(channelKey){
    const send=client._socket.send.bind(client._socket);let captured;
    client._socket.send=bytes=>{captured=new Uint8Array(bytes).slice();send(bytes);};
    await client.files.list({path:'/'});
    client._socket.send=send;
    const closed=new Promise((resolve,reject)=>{const timer=setTimeout(()=>reject(Error('Replay did not close channel')),3000);client._socket.addEventListener('close',()=>{clearTimeout(timer);resolve();},{once:true});});
    send(captured);await closed;
    client.disconnect();await client.handshake();
    client._socket.send=bytes=>{const cipher=new Uint8Array(bytes).slice();cipher[cipher.length-1]^=1;sendTampered(cipher);};
    const sendTampered=WebSocket.prototype.send.bind(client._socket);
    let tampered=false;try{await client.files.list({path:'/'});}catch{tampered=true;}
    if(!tampered)throw Error('Tampered ciphertext accepted');
    client.disconnect();await client.handshake();
    const malformedClosed=new Promise((resolve,reject)=>{const timer=setTimeout(()=>reject(Error('Malformed fragments accepted')),3000);client._socket.addEventListener('close',()=>{clearTimeout(timer);resolve();},{once:true});});
    client._socket.send(client._channel.seal(new Uint8Array([0,1])));await malformedClosed;
    console.log('core-step: pinned Noise, plaintext/wrong pin/replay/tamper/malformed fragment rejection passed');
   }
   client.disconnect();
   return {encryptedChannel:Boolean(channelKey),uploadBytes:upload.bytesWritten,downloadBytes:total,progressEvents:progress.length,range:true,cancelled,rejected};
  },{channelKey:process.env.TEALDRIVE_CORE_CHANNEL==='1'?fs.readFileSync(process.env.TEALDRIVE_CHANNEL_KEY_FILE,'utf8').trim():null});

  if(process.env.TEALDRIVE_EXPECT_GLOBAL==='True'){
   const {execFileSync}=require('node:child_process');
   const id=fs.readFileSync(process.env.TEALDRIVE_PANEL_ACCOUNT_ID_FILE,'utf8').trim();
   execFileSync('/home/tealdrive/bin/tealdrive-wizard',['account','scope',id,'tenant'],{stdio:'ignore'});
   const revoked=await page.evaluate(async()=>(await fetch('/api/admin/config')).status);
   if(revoked!==401)throw Error('Old global session survived scope revocation');
   await context.clearCookies();await page.reload();
   await page.fill('#usernameInput',process.env.TEALDRIVE_TEST_USERNAME);
   await page.fill('#passwordInput',fs.readFileSync(process.env.TEALDRIVE_TEST_WEB_PASSWORD_FILE,'utf8').trim());
   await page.click('#loginForm button[type=submit]');
   await page.waitForSelector('#loginModal.hidden',{state:'attached'});
   if(!(await page.locator('#settingsBtn').isDisabled()))throw Error('Tenant retained global settings');
   const status=await page.evaluate(async()=>(await fetch('/api/admin/config')).status);
   if(status!==403)throw Error('Tenant accessed global configuration');
   result.scopeRevocation=true;
  }
  await page.click('#logoutBtn');
  console.log('PASS large transfer '+JSON.stringify(result));
 }finally{await browser.close();}
})().catch(error=>{const known=['Wrong commit byte count','Progress not monotonic','Download content changed','Incomplete streaming download','Range failed','Cancellation did not reject upload','Tampered staging was published','Failed upload published a target','Unexpected staging count','Unknown fixture manipulation'];const reason=known.find(v=>error.message?.includes(v));console.error('Core transfer acceptance failed: '+(reason||error.code||error.name));process.exitCode=1;});
