const {chromium}=require('playwright');const fs=require('node:fs');
(async()=>{
  if(process.env.TEALDRIVE_UI_PACKAGE_ACCEPTANCE!=='1')throw Error('Explicit package fixture required');
  const {users}=JSON.parse(fs.readFileSync(process.env.TEALDRIVE_ADMIN_FIXTURE_MANIFEST,'utf8'));
  const packagePath=process.env.TEALDRIVE_UI_PACKAGE_DIR;
  const browser=await chromium.launch({headless:true});
  try {
    const page=await browser.newPage();await page.goto('http://127.0.0.1:13106');
    await page.locator('#custom-theme').waitFor();
    await page.evaluate(async user=>{await themeReady;await themeApi.auth.login({username:user.username,password:user.password});const files=await themeApi.files.list({rootId:'home',path:'/'});if(!files.entries.some(file=>file.name==='fixture.txt'))throw Error('Custom SDK file operation failed');},users[1]);
    const missing=await page.request.get('http://127.0.0.1:13106/static/not-declared.js');if(missing.status()!==404)throw Error('Undeclared asset served');
    await page.goto('http://127.0.0.1:13106/ui/default');
    await page.waitForSelector('#loginModal.hidden',{state:'attached'});
    await page.waitForFunction(()=>document.querySelector('#wsStatusText').textContent==='Connected');
    await page.locator('#fileTableBody .item-name').filter({hasText:'fixture.txt'}).waitFor();
    const saved=JSON.parse(fs.readFileSync(packagePath+'/manifest.json','utf8'));
    saved.sdkMajor=2;fs.writeFileSync(packagePath+'/manifest.json',JSON.stringify(saved));
    await page.goto('http://127.0.0.1:13106');
    await page.locator('#fileTableBody .item-name').filter({hasText:'fixture.txt'}).waitFor();
    if(await page.locator('#custom-theme').count())throw Error('Incompatible package did not fall back');
    saved.sdkMajor=1;saved.assets.push('sdk/v1/tealdrive.js');fs.writeFileSync(packagePath+'/manifest.json',JSON.stringify(saved));
    await page.reload();await page.locator('#fileTableBody .item-name').filter({hasText:'fixture.txt'}).waitFor();
    const sdk=await page.request.get('http://127.0.0.1:13106/sdk/v1/tealdrive.js');if(sdk.status()!==200||!(await sdk.text()).includes('SDK_VERSION'))throw Error('Package replaced fixed SDK');
    console.log(JSON.stringify({rootApprovedPackage:true,publicSdkReuse:true,allowlistedAssets:true,defaultUiEscape:true,wholePackageFallback:true,sdkOverrideRejected:true}));
  }finally{await browser.close();}
})().catch(error=>{console.error(error.name,error.code??error.message);process.exitCode=1;});
