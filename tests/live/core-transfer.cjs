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
  await page.fill('#passwordInput',fs.readFileSync(process.env.TEALDRIVE_TEST_WEB_PASSWORD_FILE,'utf8').trim());
  await page.click('#loginForm button[type=submit]');
  await page.waitForSelector('#loginModal.hidden',{state:'attached'});
  if(process.env.TEALDRIVE_EXPECT_GLOBAL==='True'){
   if(await page.locator('#settingsBtn').isDisabled())throw Error('Virtual global settings disabled');
   const statuses=await page.evaluate(async()=>[(await fetch('/api/admin/config')).status,(await fetch('/api/admin/accounts/system')).status]);
   if(statuses.some(status=>status!==200))throw Error('Virtual global management denied');
  }

  const result=await page.evaluate(async()=>{
   const {createClient}=await import('/sdk/v1/tealdrive.js');
   const client=createClient({timeoutMs:120000});await client.handshake();
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
   if(listing.entries.some(entry=>entry.name==='core-abort.bin'||entry.name.startsWith('core-bad-')))throw Error('Failed upload published a target');
   client.disconnect();
   return {uploadBytes:upload.bytesWritten,downloadBytes:total,progressEvents:progress.length,range:true,cancelled,rejected};
  });

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
