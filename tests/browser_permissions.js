const assert = require('node:assert/strict');
const vm = require('node:vm');
const elements = new Map();
function element() {
  const classes = new Set(['hidden']);
  return {value:'',textContent:'',disabled:false,style:{},setAttribute(){},setCustomValidity(){},
    classList:{add:x=>classes.add(x),remove:x=>classes.delete(x),contains:x=>classes.has(x)},
    addEventListener(){},appendChild(){},focus(){},remove(){}};
}
globalThis.document = {getElementById(id){if(!elements.has(id))elements.set(id,element());return elements.get(id);},
  querySelectorAll(){return [];},addEventListener(){},createElement:element};
globalThis.window = {location:{protocol:'https:',host:'test'},confirm(){return false;}};
globalThis.setTimeout = () => 1;
globalThis.WebSocket = {OPEN:1};
const init = '  // Initialize\n  setupEvents();\n  checkSession();';
assert.ok(application.includes(init));
vm.runInThisContext(application.replace(init,
  'globalThis.ui={state,el,openEditor,openChmodModal,syncChmodFromCode,performLogin,setupEvents,inject(fn){requestWs=fn;loadDirectory=()=>{};}};'));
const {state,el,openChmodModal,syncChmodFromCode,performLogin,setupEvents}=ui;
setupEvents();
(async()=>{
  openChmodModal('folder',0o40755,true);
  el.chmodOctalInput.value='0700';syncChmodFromCode();
  assert.match(el.chmodWarning.textContent,/traversal/);
  openChmodModal('file',0o100644,false);
  el.chmodOctalInput.value='0666';syncChmodFromCode();
  assert.match(el.chmodWarning.textContent,/other users/);
  const requests=[];ui.inject(async(type,payload)=>requests.push({type,payload}));
  await el.chmodForm.onsubmit({preventDefault(){}});
  assert.equal(requests.length,0,'cancel must never send chmod');
  window.confirm=()=>true;
  state.currentRoot='home';
  await el.chmodForm.onsubmit({preventDefault(){}});
  assert.equal(requests.length,1);
  assert.equal(requests[0].type,TdrvContract.messages.ChmodFile);
  assert.equal(requests[0].payload.mode,0o666);
  assert.equal(el.chmodModal.classList.contains('hidden'),true);
  openChmodModal('special',0o104755,false);
  assert.match(el.chmodWarning.textContent,/Special/);
  el.chmodOctalInput.value='888';
  await el.chmodForm.onsubmit({preventDefault(){}});
  assert.equal(requests.length,1,'invalid mode must not send');
  // New world write requires confirmation; existing world write does not.
  openChmodModal('already',0o666,false);
  window.confirm=()=>{throw Error('unnecessary confirmation');};
  await el.chmodForm.onsubmit({preventDefault(){}});
  assert.equal(requests.length,2);
  el.editorModal.classList.remove('hidden');
  ui.openEditor({root_id:'opening-root',relative_path:'note.txt',content:'original',revision:'hash',modified:null});
  el.editorTextarea.value='unsaved edits';
  state.currentRoot='different-root';
  ui.inject(async()=>{throw Error('File changed');});
  await el.editorSaveBtn.onclick();
  assert.equal(el.editorModal.classList.contains('hidden'),false);
  assert.equal(el.editorTextarea.value,'unsaved edits');
  assert.equal(el.editorSaveBtn.disabled,false);
  let saved;
  ui.inject(async(type,payload)=>{saved=payload;});
  await el.editorSaveBtn.onclick();
  assert.equal(saved.root_id,'opening-root');
  assert.equal(saved.expected_revision,'hash');
  assert.equal(el.editorModal.classList.contains('hidden'),true);
  for(const scenario of ['success','denied','network']){
    el.passwordInput.value='test-secret';
    globalThis.fetch=async(path,options)=>{
      assert.equal(el.passwordInput.value,'','field must clear before network completes');
      if(path==='/api/session')return {ok:false};
      assert.equal(JSON.parse(options.body).password,'test-secret');
      if(scenario==='network')throw Error('offline');
      return {ok:scenario==='success',json:async()=>({error:'denied'})};
    };
    await performLogin('alice','test-secret');
    assert.equal(el.passwordInput.value,'');
  }
})().catch(error=>{console.error(error);process.exitCode=1;});
