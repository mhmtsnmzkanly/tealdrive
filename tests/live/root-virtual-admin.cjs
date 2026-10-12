const { chromium } = require('playwright');
const fs = require('node:fs');
const { execFileSync, spawnSync } = require('node:child_process');
(async () => {
  const user = JSON.parse(fs.readFileSync(process.env.TEALDRIVE_ROOT_VIRTUAL_MANIFEST, 'utf8'));
  const browser = await chromium.launch({headless:true});
  try {
    const page = await browser.newPage();
    await page.goto('http://127.0.0.1:13102');
    const child = await page.evaluate(async user => {
      const {createClient} = await import('/sdk/v1/tealdrive.js');
      globalThis.api = createClient({storage:null,quotaPollMs:0});
      await api.handshake();
      await api.auth.login({username:user.username,password:user.password});
      const denied = await fetch('/api/admin/users');
      if (denied.status !== 403) throw Error('Virtual administrator escaped tenant authority');
      await api.files.createText({rootId:'home',path:'/own-usage.txt',content:'admin own file bytes'});
      return api.admin.virtualUsers.create({name:user.username+'-child',password:user.password,quotaBytes:8192});
    }, user);
    const result = spawnSync('/home/tealdrive/bin/tealdrive-wizard',['budget','set',user.account.id,'8192'],{encoding:'utf8'});
    if (result.status === 0) throw Error('Budget reduction ignored own usage');
    const limited = await page.evaluate(async () => {
      const budget = await api.admin.quota.getBudget();
      await api.auth.logout();
      return budget;
    });
    if (limited.limitBytes !== 1048576) throw Error('Rejected budget change persisted');
    console.log(JSON.stringify({rootVirtualAdmin:true,tenantBoundary:true,ownUsageBudgetGuard:true}));
  } finally { await browser.close(); }
})().catch(error => { console.error(error.name,error.code ?? error.message); process.exitCode=1; });
