const assert = require('node:assert/strict');
const elements = new Map();
const listeners = {};
const timers = [];
const cleared = [];
globalThis.document = {
  activeElement: null,
  getElementById(id) {
    if (!elements.has(id)) {
      const classes = new Set(['hidden']);
      elements.set(id, {
        value: '', textContent: '', disabled: false,
        classList: {add: x => classes.add(x), remove: x => classes.delete(x), contains: x => classes.has(x)},
        focus() { document.activeElement = this; },
        addEventListener() {},
      });
    }
    return elements.get(id);
  },
  querySelectorAll() { return []; },
  addEventListener(type, listener) { listeners[type] = listener; },
};
globalThis.window = {location: {protocol: 'https:', host: 'example.test'}};
globalThis.crypto = require('node:crypto').webcrypto;
globalThis.setTimeout = callback => { timers.push(callback); return timers.length; };
globalThis.clearTimeout = id => cleared.push(id);
const callbacks = {};
const sdk = {
  on(event, callback) { callbacks[event]=callback; },
  handshake:async()=>({authentication:{},limits:{}}),
  getInfo:()=>({limits:{}}),
  files:{quota:{get:async()=>({})},refreshState:async()=>{},getState:()=>({rootId:'home',currentDirectory:'/'}),observeQuotaWarning(){}},
  auth:{getSession:async()=>null},
};
let completed;
globalThis.fileActionMock=()=>new Promise(resolve=>{completed=resolve;});
const init = '  // Initialize\n  setupEvents();\n  checkSession();';
assert.ok(application.includes(init));
const tested=application.replace("import('/sdk/v1/tealdrive.js')", 'Promise.resolve({createClient:()=>sdk})')
  .replaceAll("import('/static/files.js')", 'Promise.resolve({fileAction:fileActionMock})');
globalThis.sdk=sdk;
require('node:vm').runInThisContext(tested.replace(init,
  'loadDirectory=()=>{};globalThis.client={state,el,connectWebSocket,requestFile,checkServerAvailability,setupEvents,showShutdownNotice};'));
const {state,el,connectWebSocket,requestFile,checkServerAvailability,setupEvents}=client;
setupEvents();
(async()=>{
  el.editorTextarea.value='unsaved changes';
  el.editorModal.classList.remove('hidden');
  el.editorTextarea.focus();
  await connectWebSocket();
  const active=requestFile('CreateFolder',{root_id:'home',parent_relative_path:'',folder_name:'new'});
  await new Promise(resolve=>process.nextTick(resolve));
  callbacks['server.shutdown']({deadlineSeconds:30});
  assert.equal(el.shutdownModal.classList.contains('hidden'),false);
  assert.match(el.shutdownMessage.textContent,/30 seconds/);
  assert.equal(document.activeElement,el.shutdownDismissBtn);
  assert.equal(el.editorTextarea.value,'unsaved changes');
  assert.equal(state.wsConnected,false);
  await assert.rejects(requestFile('CreateFolder',{}),/shutting down/);
  completed({safe_message:'done'});
  assert.equal((await active).safe_message,'done');
  listeners.keydown({key:'Tab',preventDefault(){}});
  assert.equal(document.activeElement,el.shutdownRetryBtn);
  listeners.keydown({key:'Escape'});
  assert.equal(el.shutdownModal.classList.contains('hidden'),true);
  assert.equal(document.activeElement,el.editorTextarea);
  callbacks['server.shutdown']({deadlineSeconds:0});
  assert.equal(el.shutdownModal.classList.contains('hidden'),true);
  const timerCount=timers.length;
  callbacks['connection.changed']({status:'disconnected'});
  assert.equal(el.wsStatusText.textContent,'Server stopped');
  assert.equal(timers.length,timerCount);
  globalThis.fetch=async()=>({ok:false});
  await checkServerAvailability();
  assert.match(el.shutdownMessage.textContent,/still unavailable/);
  globalThis.fetch=async()=>({ok:true,text:async()=>'ok'});
  await checkServerAvailability();
  assert.equal(state.serverShuttingDown,false);
  assert.equal(el.loginModal.classList.contains('hidden'),false);
  assert.equal(el.editorTextarea.value,'unsaved changes');
  callbacks['connection.changed']({status:'disconnected'});
  assert.equal(el.wsStatusText.textContent,'Disconnected');
  assert.equal(el.shutdownModal.classList.contains('hidden'),true);
})().catch(error=>{console.error(error);process.exitCode=1;});
