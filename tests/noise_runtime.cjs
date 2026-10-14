// Public generated runtime: concurrent initialization must preserve opaque Rust values.
const assert=require('node:assert/strict');
const fs=require('node:fs');
(async()=>{
 const source=fs.readFileSync('src/ui/sdk/noise/noise.js','utf8');
 const module=await import('data:text/javascript;base64,'+Buffer.from(source).toString('base64'));
 const bytes=fs.readFileSync('src/ui/sdk/noise/noise_bg.wasm');
 const first=module.default({module_or_path:bytes});
 const second=module.default({module_or_path:bytes});
 assert.equal(first,second,'parallel callers must share the pending initialization');
 assert.throws(()=>module.initSync({module:bytes}),/async initialization/);
 const one=await first;
 const client=new module.NoiseClient(new Uint8Array(32).fill(1));
 assert.equal(client.start().length,48);
 const two=await second;assert.equal(one,two);
 assert.equal((await module.default()).memory,one.memory);
 client.free();
 const hash=new module.Sha256Stream();hash.update(new TextEncoder().encode('abc'));
 assert.equal(Buffer.from(hash.finish()).toString('hex'),'ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad');
 console.log('PASS: single WASM instance across pending loaders, sync-race rejection, stable Noise handle, streaming SHA256 vector');
})().catch(error=>{console.error(error.message);process.exitCode=1;});
