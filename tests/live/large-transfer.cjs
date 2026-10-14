// Explicit dedicated-account public-proxy transfer check; removes only its UUID fixture.
const {chromium}=require('playwright');
const fs=require('node:fs');const {randomUUID}=require('node:crypto');const {execFileSync}=require('node:child_process');
(async()=>{
 if(process.getuid()!==0||process.env.TEALDRIVE_LARGE_TRANSFER_ACCEPTANCE!=='1')throw Error('Explicit Root transfer acceptance required');
 const user=process.env.TEALDRIVE_TEST_USERNAME;if(user!=='tealdrive-test')throw Error('Dedicated test account required');
 const secret=process.env.TEALDRIVE_TEST_WEB_PASSWORD_FILE;if(!secret||(fs.statSync(secret).mode&0o077))throw Error('Private credential file required');
 const url=process.env.TEALDRIVE_TEST_URL;if(!url)throw Error('Explicit URL required');
 const home=execFileSync('/usr/bin/getent',['passwd',user],{encoding:'utf8'}).trim().split(':')[5];if(home!=='/home/tealdrive-test')throw Error('Unexpected fixture home');
 const fixture='transfer-'+randomUUID()+'.bin';const browser=await chromium.launch({headless:true});
 try{
  const page=await browser.newPage();await page.route('**/app.js*',route=>route.fulfill({contentType:'application/javascript',body:''}));await page.goto(url);
  const result=await page.evaluate(async({username,password,fixture})=>{
   const {createClient}=await import('/sdk/v1/tealdrive.js');const api=createClient({storage:null,timeoutMs:120000});await api.handshake();await api.auth.login({username,password});
   try{
    if(!api.getInfo().transport.encryptedWebSocket)throw Error('Missing encrypted channel');
    const bytes=new Uint8Array(17*1024*1024);bytes.fill(0x5a);let progress=0;
    const uploaded=await api.files.upload({rootId:'home',directory:'/',file:new File([bytes],fixture),onProgress:()=>progress++});
    if(uploaded.bytesWritten!==bytes.length)throw Error('Wrong upload size');
    if(typeof uploaded.transferId!=='string'||uploaded.operation!=='UPLOAD'||!uploaded.requestId)throw Error('Untyped upload completion result');
    const response=await api.files.download({rootId:'home',path:fixture});const reader=response.body.getReader();let received=0;
    while(true){const {done,value}=await reader.read();if(done)break;if(value.some(byte=>byte!==0x5a))throw Error('Content changed');received+=value.length;}
    if(received!==bytes.length)throw Error('Incomplete download');
    const range=await fetch(api.files.getDownloadUrl({rootId:'home',path:fixture}),{headers:{Range:'bytes=100-199'}});
    if(range.status!==206||(await range.arrayBuffer()).byteLength!==100)throw Error('Range failed');
    return {uploadBytes:uploaded.bytesWritten,downloadBytes:received,progressEvents:progress,range:true,encryptedWebSocket:true};
   }finally{await api.auth.logout();api.disconnect();}
  },{username:user,password:fs.readFileSync(secret,'utf8').trim(),fixture});
  console.log('PASS public large transfer '+JSON.stringify(result));
 }finally{
  await browser.close();execFileSync('/usr/sbin/runuser',['-u',user,'--','/usr/bin/python3','-c','import os,sys\ntry: os.unlink(sys.argv[1])\nexcept FileNotFoundError: pass',home+'/'+fixture]);
 }
})().catch(error=>{console.error(error.message);process.exitCode=1;});
