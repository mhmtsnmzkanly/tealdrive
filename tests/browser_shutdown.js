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
globalThis.WebSocket = class {
  static OPEN = 1;
  readyState = 1;
  sent = [];
  send(frame) { this.sent.push(frame); }
  close() {}
};
const init = '  // Initialize\n  setupEvents();\n  checkSession();';
assert.ok(application.includes(init));
require('node:vm').runInThisContext(application.replace(init,
  'globalThis.client = {state, el, connectWebSocket, handleServerFrame, requestWs, checkServerAvailability, setupEvents};'));
const {state, el, connectWebSocket, handleServerFrame, requestWs, checkServerAvailability, setupEvents} = client;
setupEvents();
(async () => {
  el.editorTextarea.value = 'unsaved changes';
  el.editorModal.classList.remove('hidden');
  el.editorTextarea.focus();
  connectWebSocket();
  state.wsConnected = true;
  const ws = state.ws;
  const active = requestWs(TdrvContract.messages.CreateFolder, {root_id:'home',parent_relative_path:'',folder_name:'new'});
  const requestId = [...state.pendingRequests.keys()][0];
  handleServerFrame({messageType:TdrvContract.messages.ConnectionStatus,requestId:'shutdown',payload:{shutting_down:true,grace_period_seconds:30}});
  assert.equal(el.shutdownModal.classList.contains('hidden'),false);
  assert.equal(el.wsStatusText.textContent,'Server shutting down');
  assert.match(el.shutdownMessage.textContent,/30 seconds/);
  assert.equal(document.activeElement,el.shutdownDismissBtn);
  assert.equal(el.editorTextarea.value,'unsaved changes');
  assert.equal(el.editorModal.classList.contains('hidden'),false);
  handleServerFrame({messageType:TdrvContract.messages.ProtocolReady,requestId:'late-handshake',payload:{limits:{}}});
  assert.equal(el.wsStatusText.textContent,'Server shutting down');
  assert.equal(state.wsConnected,false);
  const sent = ws.sent.length;
  await assert.rejects(requestWs(TdrvContract.messages.CreateFolder,{}),/shutting down/);
  assert.equal(ws.sent.length,sent);
  handleServerFrame({messageType:TdrvContract.messages.OperationDone,requestId,payload:{safe_message:'done'}});
  assert.equal((await active).safe_message,'done');
  assert.equal(state.pendingRequests.size,0);
  assert.equal(ws.sent.length,sent); // no refresh/new request during draining
  listeners.keydown({key:'Tab',preventDefault(){}});
  assert.equal(document.activeElement,el.shutdownRetryBtn);
  listeners.keydown({key:'Escape'});
  assert.equal(el.shutdownModal.classList.contains('hidden'),true);
  assert.equal(document.activeElement,el.editorTextarea);
  assert.equal(el.editorModal.classList.contains('hidden'),false);
  handleServerFrame({messageType:TdrvContract.messages.ConnectionStatus,requestId:'duplicate',payload:{shutting_down:true}});
  assert.equal(el.shutdownModal.classList.contains('hidden'),true); // no repeated popup
  const timerCount = timers.length;
  ws.onclose({code:1001,reason:'Server shutting down'});
  assert.equal(el.wsStatusText.textContent,'Server stopped');
  assert.equal(timers.length,timerCount); // no automatic login over shutdown notice

  connectWebSocket();
  state.wsConnected = true;
  const retry = requestWs(TdrvContract.messages.CreateFolder,{root_id:'home',parent_relative_path:'',folder_name:'retry'});
  const rejected = assert.rejects(retry,/Server stopped/);
  state.ws.onclose({code:1001,reason:'Server shutting down'}); // fallback when status was lost
  await rejected;
  assert.equal(el.shutdownModal.classList.contains('hidden'),false);
  assert.equal(state.pendingRequests.size,0);
  globalThis.fetch = async () => ({ok:false});
  await checkServerAvailability();
  assert.equal(el.shutdownRetryBtn.disabled,false);
  assert.equal(el.shutdownModal.classList.contains('hidden'),false);
  assert.match(el.shutdownMessage.textContent,/still unavailable/);
  assert.equal(el.editorTextarea.value,'unsaved changes');
  globalThis.fetch = async url => url==='/api/health'
    ? {ok:true,text:async()=> 'ok'} : {ok:true,json:async()=> ({authenticated:false})};
  await checkServerAvailability();
  assert.equal(state.serverShuttingDown,false);
  assert.equal(el.shutdownModal.classList.contains('hidden'),true);
  assert.equal(el.loginModal.classList.contains('hidden'),false);
  assert.equal(el.editorTextarea.value,'unsaved changes');
  connectWebSocket();
  const stale = state.ws;
  connectWebSocket();
  state.wsConnected = true;
  stale.onclose({code:1001,reason:'Server shutting down'});
  assert.equal(state.serverShuttingDown,false);
  assert.equal(state.wsConnected,true);
  state.ws.onclose({code:1006,reason:''});
  assert.equal(state.serverShuttingDown,false); // do not mislabel network loss as shutdown
  assert.equal(el.shutdownModal.classList.contains('hidden'),true);
  assert.equal(el.wsStatusText.textContent,'Disconnected');
})().catch(error => { console.error(error); process.exitCode=1; });
