// Sign only in the known test browser vault. Seed/backup remain inside browser memory.
import fs from 'node:fs';
import assert from 'node:assert/strict';
const {chromium}=await import('/home/postfiat/repos/postfiatl1v2-fastswap/wallet-web/node_modules/playwright/index.mjs');
const input=JSON.parse(fs.readFileSync(process.argv[2],'utf8'));
assert.equal(input.fields.source,'pfa0c02873020b87a5dd32a5db8747c5e5d9487051');assert.ok(['asset','offer'].includes(input.kind));
const root='/home/postfiat/repos/tasknode-b23-integration/src/features/wallet/l1/wasm/';
const ctx=await chromium.launchPersistentContext('/home/postfiat/.local/state/b23-browser-journey/profile',{headless:true,args:['--no-sandbox']});
try {
  const p=await ctx.newPage();
  await p.route('**/tee-local-signer',r=>r.fulfill({contentType:'text/html',body:'<!doctype html><title>Authorized test signer</title>'}));
  await p.route('**/postfiat_wallet_wasm.js',r=>r.fulfill({contentType:'text/javascript',body:fs.readFileSync(root+'postfiat_wallet_wasm.js')}));
  await p.route('**/postfiat_wallet_wasm_bg.wasm',r=>r.fulfill({contentType:'application/wasm',body:fs.readFileSync(root+'postfiat_wallet_wasm_bg.wasm')}));
  await p.goto('https://postfiatfoundationv2.taila4ec45.ts.net:8868/tee-local-signer');
  const signed=await p.evaluate(async ({input,passphrase})=>{
    const db=await new Promise((resolve,reject)=>{const q=indexedDB.open('tasknode-l1-wallet',1);q.onsuccess=()=>resolve(q.result);q.onerror=()=>reject(new Error('vault unavailable'));});
    const rows=await new Promise((resolve,reject)=>{const q=db.transaction('vaults','readonly').objectStore('vaults').getAll();q.onsuccess=()=>resolve(q.result);q.onerror=()=>reject(new Error('vault read failed'));});db.close();
    const rowsForTest=rows.filter(r=>r.metadata?.address===input.fields.source);if(rowsForTest.length!==1)throw new Error('test wallet identity missing or ambiguous');
    const blob=rowsForTest[0].vault;const bytes=s=>Uint8Array.from(atob(s),c=>c.charCodeAt(0));
    const material=await crypto.subtle.importKey('raw',new TextEncoder().encode(passphrase),'PBKDF2',false,['deriveKey']);
    const key=await crypto.subtle.deriveKey({name:'PBKDF2',salt:bytes(blob.salt),iterations:310000,hash:'SHA-256'},material,{name:'AES-GCM',length:256},false,['decrypt']);
    const seed=new TextDecoder().decode(await crypto.subtle.decrypt({name:'AES-GCM',iv:bytes(blob.iv)},key,bytes(blob.ciphertext)));
    const wasm=await import('/postfiat_wallet_wasm.js');await wasm.default();
    const wallet=wasm.wallet_keygen(input.fields.chain_id,seed,0);
    if(wallet.address!==input.fields.source)throw new Error('test signing identity mismatch');
    const fn=input.kind==='asset'?wasm.wallet_sign_asset_transaction_fields:wasm.wallet_sign_offer_transaction_fields;
    return fn(wallet.backup_json,JSON.stringify(input.fields));
  },{input,passphrase:fs.readFileSync('/home/postfiat/.local/state/b23-browser-journey/pass','utf8').trim()});
  fs.writeFileSync(process.argv[3],JSON.stringify(signed)+'\n',{mode:0o600});
} finally {await ctx.close();}
