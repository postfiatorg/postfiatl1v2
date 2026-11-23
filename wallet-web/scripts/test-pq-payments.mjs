// Run from the worktree after npm ci in wallet-web. The Rust fixture creates
// the initial Orchard note. Both browser sessions use the shipping binaries.
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import fs from 'node:fs';
import net from 'node:net';
import http from 'node:http';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawn, spawnSync } from 'node:child_process';
import { once } from 'node:events';
import { setTimeout as delay } from 'node:timers/promises';
import { chromium } from 'playwright';

const root = fileURLToPath(new URL('../../', import.meta.url));
const env = { ...process.env, PYTHONPATH:path.join(root,'python'), RAYON_NUM_THREADS:process.env.RAYON_NUM_THREADS || '4' };
const target = path.resolve(env.CARGO_TARGET_DIR || path.join(root,'target'));
const node = path.join(target,'release/postfiat-node');
const daemon = path.join(target,'release/pftl-pq-walletd');
const evidence = path.join(root,'docs/evidence/pq-private-payments-m7');

function run(exe, args, options = {}) {
  const r = spawnSync(exe, args, { cwd:root, env, encoding:'utf8', maxBuffer:8*1024*1024, ...options });
  if (r.status !== 0) throw Error(`${path.basename(exe)} ${args[0]} failed (${r.status})`);
  return r.stdout;
}
if (!process.argv.includes('--exercise')) {
  run('npm', ['run','build'], { cwd:path.join(root,'wallet-web'), stdio:'inherit' });
  run('cargo', ['build','--release','--locked','-p','postfiat-node','--features','pq-wallet','--bin','postfiat-node','--bin','pftl-pq-walletd'], {stdio:'inherit'});
  run('cargo', ['test','--release','--locked','-p','postfiat-node','--lib','--features','pq-wallet-browser-test','--','two_browser_wallets_and_orchard_migration','--nocapture'], {stdio:'inherit'});
} else {
  await exercise();
}

async function exercise() {
  process.umask(0o077);
  const scratch = fs.mkdtempSync(path.join(os.tmpdir(),'pq-browser-'));
  const frames=path.join(scratch,'screenshots');
  const dist=path.join(scratch,'wallet-web');
  fs.cpSync(path.join(root,'wallet-web/dist'),dist,{recursive:true});
  const dirs = [env.PQ_CHAIN_DIR, path.join(scratch,'peer')];
  fs.cpSync(dirs[0], dirs[1], {recursive:true,filter:p=>!['alice.key','bob.key','initial.note','recovered.note','exit.batch','exit.json'].includes(path.basename(p))});
  const children = new Set();
  const addresses = {alice:env.PQ_ALICE, bob:env.PQ_BOB};
  const native = '00'.repeat(48);
  const issue = env.PQ_ISSUE;
  const pages = {};
  const wallets = {};
  const bridges = {};
  const shots = [];
  const tokenFile = {};
  const nodePorts = [];
  const taps = [];
  const validatorReads = {alice:[], bob:[]};
  const port = async () => {
    const s = net.createServer(); s.listen(0,'127.0.0.1'); await once(s,'listening');
    const p = s.address().port; await new Promise(r=>s.close(r)); return p;
  };
  const gwPort = await port();
  let audit, gatewayOrigin;
  const relayCounts = {snapshot:0,page:0,submit:0,receipt:0};
  const boundaryErrors = [];
  let withholdBobPage = false;
  let refuseNextSpend = false;
  const ports = {alice:await port(), bob:await port()};
  let height = 1;
  let browser;
  function start(exe, args, extra = {}) {
    const p = spawn(exe,args,{cwd:root,env:{...env,...extra},stdio:'ignore'});
    children.add(p); return p;
  }
  async function stop(p) {
    if (p && p.exitCode === null && p.signalCode === null) {
      const done = once(p,'exit'); p.kill(); await done;
    }
    children.delete(p);
  }
  async function until(check, name, timeout=30000) {
    const end = Date.now()+timeout;
    while (Date.now()<end) { if (await check()) return; await delay(100); }
    throw Error('Timed out waiting for '+name);
  }
  function cliArgs(w) {
    return ['-m','postfiat_rpc','pq-wallet','--wallet',path.join(scratch,w),'--endpoint','127.0.0.1:'+nodePorts[0]];
  }
  async function startDaemon(w) {
    const home = path.join(scratch,w), key = path.join(scratch,w+'.wrap');
    wallets[w] = start(daemon,[home,key]);
    await until(()=>{try {run('python3',[...cliArgs(w),'status']); return true;} catch {return fs.existsSync(path.join(home,'wallet.sock')) && !fs.existsSync(path.join(home,'wallet.enc'));}},'wallet daemon');
  }
  async function startBridge(w) {
    const pair = path.join(scratch,w+'.pair');
    fs.rmSync(pair,{force:true});
    const trusted = ['--validator-endpoint','127.0.0.1:'+taps[w === 'alice' ? 0 : 1].address().port];
    bridges[w] = start('python3',[...cliArgs(w),...trusted,'browser','--dist',dist,
      '--port',String(ports[w]),'--pair-file',pair,'--gateway',gatewayOrigin,
      '--gateway-token-file',tokenFile[w],'--payer',addresses[w],'--account-key',env['PQ_'+w.toUpperCase()+'_KEY'],
      '--issue',issue,...(w==='alice'?['--orchard-exit-batch',env.PQ_EXIT_BATCH]:[])]);
    await until(()=>fs.existsSync(pair) && fs.statSync(pair).size>0,'owner-only pairing file');
    assert.equal(fs.statSync(pair).mode & 0o077,0);
    // Avoid printing a capability-bearing URL in any Playwright error.
    const connected = action(pages[w], 'status');
    try {
      await pages[w].goto(fs.readFileSync(pair,'utf8').trim());
      assert.ok((await connected).ok());
    } catch {throw Error('Could not open local wallet.');}
    await until(() => !new URL(pages[w].url()).hash, 'local pairing');
    await pages[w].getByRole('button',{name:'Sync',exact:true}).waitFor();
  }
  const click = (p,name) => p.getByRole('button',{name,exact:true}).click();
  const action = (p,cmd) => p.waitForResponse(async r=>r.url().endsWith('/local/pq/v1') && r.request().postDataJSON().cmd===cmd,{timeout:180000});
  async function walletCall(p,cmd,button) {
    const response = action(p,cmd); await click(p,button); const r = await response;
    const body = await r.json(); assert.ok(r.ok(),cmd+' failed: '+(body.error || r.status())); return body;
  }
  async function sync(w) {
    const s = await walletCall(pages[w],'sync','Sync'); assert.equal(s.synced,true); return s;
  }
  async function capture(p,name) {
    if (!name.startsWith('migration-')) {
      await p.getByText(/matching (PFT|pfUSDC) .* finalized pool actions/).waitFor();
    }
    if (await p.getByRole('alert').count() || new URL(p.url()).hash) throw Error('Unexpected content in evidence frame.');
    assert.equal(await p.locator('textarea').count(),0);
    const visible=await p.locator('body').innerText();
    assert.ok(!/private_key_hex|spend_secret|leaf_index|witness|Bearer |seed phrase/i.test(visible));
    for (const w of Object.keys(tokenFile)) {
      const pairing=path.join(scratch,w+'.pair');
      if (fs.existsSync(pairing)) assert.ok(!visible.includes(fs.readFileSync(pairing,'utf8').trim().split('#')[1]));
      assert.ok(!visible.includes(fs.readFileSync(tokenFile[w],'utf8').trim()));
    }
    const file = path.join(frames,name+'.png');
    await p.screenshot({path:file,fullPage:true,animations:'disabled'}); shots.push(name);
  }
  async function rpc(port,method,params) {
    return new Promise((resolve,reject)=>{
      const s=net.connect(port,'127.0.0.1'); let data='';
      s.once('error',reject); s.setEncoding('utf8');
      s.on('connect',()=>s.end(JSON.stringify({version:'postfiat-local-rpc-v1',id:'browser-merge',method,params})+'\n'));
      s.on('data',d=>{data+=d;}); s.on('end',()=>{
        try {const v=JSON.parse(data); assert.ok(v.ok,method+' refused'); resolve(v.result);} catch(e){reject(e);}
      });
    });
  }
  async function migrationSupply(label, expectedOrchard, expectedPQ) {
    const view=async (pool, section) => {
      const snap=await rpc(nodePorts[0],'private_pool_snapshot',{pool_id:pool});
      const page=await rpc(nodePorts[0],'private_pool_page',{snapshot_token:snap.snapshot_token,section,limit:16});
      assert.equal(page.status,'ok'); assert.equal(page.next_cursor,null);
      return page.items;
    };
    const orchard=(await view('asset-orchard-v1','assets')).find(row=>row.asset_id===issue).live_total;
    const poolState=(await view('PFT-PQ-SHIELDED-POOL-V1','pq_assets')).find(([id])=>id===issue)[1];
    const pq=poolState.reserve;
    const {asset}=await rpc(nodePorts[0],'asset_info',{asset_id:issue});
    const collected=asset.collected_private_fees;
    assert.equal(collected,6);
    let publicAtoms=0;
    for(const account of Object.values(addresses)) {
      const {lines}=await rpc(nodePorts[0],'account_lines',{account,asset_id:issue});
      for(const line of lines) publicAtoms+=line.balance;
    }
    assert.equal(orchard,expectedOrchard); assert.equal(pq,expectedPQ); assert.equal(publicAtoms+pq+orchard+collected,100);
    console.log(label+': public '+publicAtoms+', PQ '+pq+', Orchard '+orchard+', collector '+collected+' = 100 source-series atoms');
  }
  async function finalize(intent) {
    assert.match(intent,/^[a-f0-9]{96}$/);
    const a=(await rpc(nodePorts[0],'private_action_receipt',{intent_id:intent})).record;
    const b=(await rpc(nodePorts[1],'private_action_receipt',{intent_id:intent})).record;
    if (!a || !b) {
      const gatewayRecord=JSON.parse(fs.readFileSync(path.join(scratch,'relay','intents',intent+'.json'),'utf8'));
      throw Error('Missing validator admission: '+gatewayRecord.last_dispatch.sources.map(s=>s.code || 'admitted').join(', '));
    }
    assert.ok(a.batch_json===b.batch_json && a.action_hash===b.action_hash);
    const batch = path.join(scratch,'round.batch'), blockFile = path.join(scratch,'block.json'), cert = path.join(scratch,'certificate.json');
    fs.writeFileSync(batch,a.batch_json);
    const apply=(dir,more=[])=>JSON.parse(run(node,['apply-shield-batch','--data-dir',dir,'--batch-file',batch,...more]));
    const [receipt]=apply(dirs[0]); assert.ok(receipt.accepted,receipt.code);
    height++;
    const [block] = await rpc(nodePorts[0],'blocks',{from_height:height,limit:1});
    fs.writeFileSync(blockFile,JSON.stringify(block));
    run(node,['block-certificate-from-archive','--data-dir',dirs[0],'--block-file',blockFile,'--batch-file',batch,'--certificate-file',cert]);
    assert.ok(apply(dirs[1],['--certificate-file',cert])[0].accepted);
    const [peer] = await rpc(nodePorts[1],'blocks',{from_height:height,limit:1});
    assert.equal(block.header.block_hash,peer.header.block_hash);
    assert.deepEqual(block.receipt_ids,peer.receipt_ids);
    return block.receipt_ids[0];
  }
  async function check(w,intent,name,alreadyFinal=false) {
    const p=pages[w];
    const expected=alreadyFinal || await finalize(intent);
    const r=await walletCall(p,'receipt','Check finality');
    assert.equal(r.state,'final'); assert.equal(r.intent,intent); assert.equal(r.receipt,expected);
    await p.getByTestId('receipt-state').filter({hasText:'final'}).waitFor();
    await p.getByText(expected,{exact:true}).waitFor();
    await until(async()=>!(await p.getByRole('button',{name:'Sync',exact:true}).isDisabled()),'post-finality sync');
    await capture(p,name);
    console.log(name+' block '+r.height+' receipt '+r.receipt);
  }
  async function prove(w,confirmButton='Confirm and prove locally') {
    const p=pages[w], prepared=action(p,'prove');
    await click(p,confirmButton); const pr=await prepared;
    if(!pr.ok()) throw Error('Local proving failed: '+(await pr.json()).error);
    await until(async()=>await p.getByTestId('receipt-state').textContent().catch(()=>'')==='queued','admitted payment');
    return (await pr.json()).intent;
  }
  async function transfer(w,asset,mode,amount,to,name) {
    const p=pages[w]; await sync(w);
    await p.getByLabel('Asset',{exact:true}).selectOption(asset);
    await click(p,mode);
    await p.getByLabel(/^Amount \(/).fill(String(amount));
    if(to) await p.getByLabel(mode==='Pay'?'Recipient receive address':'Public exit account',{exact:true}).fill(to);
    if (asset === issue && mode !== 'Shield') {
      const fee = p.getByLabel('Fee (pfUSDC atoms)',{exact:true});
      assert.equal(await fee.inputValue(), '2');
      assert.equal(await fee.isEditable(), false);
    }
    const start = validatorReads[w].length;
    await walletCall(p,'review','Review payment');
    if (refuseNextSpend) {
      const reply = action(p,'submit');
      await click(p,'Confirm and prove locally');
      assert.equal((await reply).status(), 409);
      await p.getByRole('alert').filter({hasText:'pq_fee_mismatch'}).waitFor();
      // Re-review the same notes without a restart or manual cancellation.
      await walletCall(p,'review','Review payment');
    }
    await check(w,await prove(w),name);
    if (mode !== 'Shield') {
      const reads = validatorReads[w].slice(start);
      for (const method of ['private_pool_snapshot','receipts','blocks']) assert.ok(reads.includes(method), method);
      assert.ok(!reads.includes('public-account'), 'private spend disclosed an account');
    }
  }
  try {
    fs.mkdirSync(frames);
    for(const [i,dir] of dirs.entries()) {
      const ready=path.join(scratch,'node'+i+'.ready'), topology=path.join(scratch,'topology.json');
      fs.writeFileSync(topology,'{}'); const tcp=await port(); nodePorts.push(tcp);
      start(node,['rpc-serve','--unsafe-devnet-json-storage','--data-dir',dir,'--bind-host','127.0.0.1','--port',String(tcp),
        '--ready-file',ready,'--max-requests','3000','--allow-private-action-admission','--finality-topology',topology,'--finality-key-file',path.join(dir,'validator_keys.json')]);
      await until(()=>fs.existsSync(ready),'validator RPC');
      // Observe only bridge -> trusted validator traffic. The fixture also has
      // independent supply-check queries, which use the original node ports.
      const reads = validatorReads[i === 0 ? 'alice' : 'bob'];
      const tap = net.createServer({allowHalfOpen:true}, client => {
        const upstream = net.connect({port:tcp, host:'127.0.0.1', allowHalfOpen:true});
        const chunks = [];
        client.on('data', chunk => {
          chunks.push(chunk);
          if (!chunk.includes(10)) return;
          const request = JSON.parse(Buffer.concat(chunks));
          reads.push(request.method);
          if (['account','account_lines','private_account_lines'].includes(request.method)
              || Object.values(addresses).some(address => JSON.stringify(request.params).includes(address))) {
            reads.push('public-account');
          }
        });
        client.pipe(upstream).pipe(client);
      });
      tap.listen(0,'127.0.0.1'); await once(tap,'listening'); taps.push(tap);
    }
    const domain=await rpc(nodePorts[0],'private_pool_snapshot',{pool_id:'PFT-PQ-SHIELDED-POOL-V1'});
    const birthday = await rpc(nodePorts[0],'private_pool_snapshot',{pool_id:'asset-orchard-v1'});
    const legacyPage = () => rpc(nodePorts[0],'private_pool_page',{snapshot_token:birthday.snapshot_token,section:'outputs',limit:1});
    const historic = await legacyPage();
    assert.ok(historic.status==='ok' && historic.item_count===1 && historic.items[0].ciphertext);
    const tokens={};
    for(const w of Object.keys(addresses)){
      tokens[w]=crypto.randomBytes(32).toString('hex'); tokenFile[w]=path.join(scratch,w+'.token');
      fs.writeFileSync(tokenFile[w],tokens[w]);
    }
    const principals=path.join(scratch,'principals.json'); fs.writeFileSync(principals,JSON.stringify(tokens));
    const origins=Object.values(ports).map(p=>'http://127.0.0.1:'+p);
    start(process.execPath,[path.join(root,'wallet-proxy/server.js')],{
      HOME:scratch,LISTEN_HOST:'127.0.0.1',LISTEN_PORT:String(gwPort),ALLOWED_ORIGINS:origins.join(','),
      WALLET_PROXY_API_TOKENS_FILE:principals,WALLET_PROXY_PRIVATE_ACTIONS_CHAIN_ID:domain.chain_id,
      WALLET_PROXY_PRIVATE_ACTIONS_GENESIS_HASH:domain.genesis_hash,WALLET_PROXY_PQ_ISSUE_ID:issue,
      WALLET_PROXY_PRIVATE_ACTIONS_RPC_SOURCES:nodePorts.map((p,i)=>'v'+i+'=127.0.0.1:'+p).join(','),
      WALLET_PROXY_PRIVATE_ACTIONS_STORE:path.join(scratch,'relay'),WALLET_PROXY_PRIVATE_ACTIONS_RPC_TIMEOUT_MS:'90000',
      NAVSWAP_IDEMPOTENCY_STORE_PATH:'off',NAVSWAP_RUN_STORE_PATH:'off',ENABLE_UPSTREAM_KEEPALIVE:'false',
    });
    await until(async()=>{try{return !!(await fetch('http://127.0.0.1:'+gwPort+'/')).status;}catch{return false;}},'gateway');
    // Inspect the actual relay boundary before forwarding to the shipping proxy.
    // Keep only counters. A rejected frame never reaches the gateway or a log.
    audit = http.createServer(async (req,res) => {
      try {
        const chunks=[]; for await(const chunk of req) chunks.push(chunk);
        const raw=Buffer.concat(chunks), body=JSON.parse(raw);
        const match=/^\/api\/private\/v1\/(pq\/)?(snapshot|page|submit|receipt)$/.exec(req.url);
        assert.ok(match && req.method==='POST');
        const route=match[2];
        const fields=route==='snapshot'?[]:route==='page'?['cursor','limit','section','snapshot_token']:['action_json','address','intent_id'];
        assert.deepEqual(Object.keys(body).sort(), fields.sort());
        if(body.action_json){
          const action=JSON.parse(body.action_json);
          if(match[1]) {
            assert.deepEqual(Object.keys(action),['kind','body']);
            assert.equal(action.kind,'pq_payment_v1');
            if (Buffer.from(action.body, 'base64')[71] === 0) assert.equal(body.address, null);
          } else assert.equal(action.kind,'asset_orchard_private_egress_v1');
        }
        assert.ok(!/"(?:seed|witness|spend_secret|scan_key|account_key|private_key_hex|leaf_index)"/.test(raw.toString()));
        if (refuseNextSpend && route === 'submit') {
          refuseNextSpend = false;
          res.writeHead(409, {'Content-Type':'application/json'});
          return res.end(JSON.stringify({ok:false, schema:'postfiat-private-actions-v1',
            reserved:false, code:'pq_fee_mismatch'}));
        }
        relayCounts[route]++;
        if (withholdBobPage && route === 'page' && body.section === 'pq_pairs'
            && req.headers.authorization === 'Bearer '+tokens.bob) {
          withholdBobPage = false;
          res.writeHead(200,{'Content-Type':'application/json'});
          return res.end(JSON.stringify({status:'ok',section:body.section,snapshot_token:body.snapshot_token,
            offset:0,total:1,item_count:0,items:[],next_cursor:null}));
        }
        const upstream=http.request({host:'127.0.0.1',port:gwPort,path:req.url,method:'POST',headers:req.headers},reply=>{
          res.writeHead(reply.statusCode,reply.headers); reply.pipe(res);
        });
        upstream.on('error',()=>res.destroy()); upstream.end(raw);
      } catch {
        boundaryErrors.push('Unexpected relay request');
        res.writeHead(400); res.end('{}');
      }
    });
    audit.listen(0,'127.0.0.1'); await once(audit,'listening');
    gatewayOrigin='http://127.0.0.1:'+audit.address().port;
    browser=await chromium.launch({headless:true,args:['--disable-dev-shm-usage']});
    const receive={};
    for(const w of Object.keys(addresses)){
      const context=await browser.newContext({viewport:{width:1010,height:750}});
      pages[w]=await context.newPage();
      const home=path.join(scratch,w), key=path.join(scratch,w+'.wrap');
      fs.mkdirSync(home,{mode:0o700}); run('python3',[...cliArgs(w),'keygen','--key-file',key]);
      await startDaemon(w); run('python3',[...cliArgs(w),'init']);
      await startBridge(w);
      const s=await sync(w); receive[w]=s.address;
    }
    const a=pages.alice,b=pages.bob;
    // Pairing requires both exact origin and per-wallet capability, even on loopback.
    for(const [origin,bearer] of [[origins[1],fs.readFileSync(path.join(scratch,'alice.pair'),'utf8').trim().split('#')[1]],[origins[0],'invalid']]){
      const r=await fetch(origins[0]+'/local/pq/v1',{method:'POST',headers:{Origin:origin,Authorization:'Bearer '+bearer,'Content-Type':'application/json'},body:'{"cmd":"status"}'});
      assert.equal(r.status,403);
    }
    await a.getByLabel(/^Amount \(/).fill('100');
    const original=await walletCall(a,'review','Review payment');
    const changed=await a.evaluate(async review=>{
      review.terms.amount='101';
      return (await fetch('/local/pq/v1',{method:'POST',headers:{'Content-Type':'application/json',Authorization:'Bearer '+sessionStorage.getItem('postfiat.pq.local-session.v1')},body:JSON.stringify({cmd:'prove',body:review})})).status;
    },original);
    assert.equal(changed,409);
    await b.getByText('hidden, small pool',{exact:true}).waitFor();
    await capture(b,'thin-pool');
    await transfer('alice',native,'Shield',100,null,'pft-01-alice-shield');
    await b.reload(); await b.getByText(/Sync required/).waitFor();
    await b.getByLabel(/^Amount \(/).fill('1'); assert.ok(await b.getByRole('button',{name:'Review payment'}).isDisabled());
    withholdBobPage = true;
    const partial = action(b,'sync');
    await click(b,'Sync');
    assert.equal((await partial).status(),409);
    await b.getByRole('alert').filter({hasText:'invalid finalized page coverage'}).waitFor();
    assert.equal(withholdBobPage,false);
    assert.ok(await b.getByRole('button',{name:'Review payment'}).isDisabled());
    await transfer('alice',native,'Pay',30,receive.bob,'pft-02-alice-pay');
    assert.equal((await sync('bob')).private[native],'30');
    await transfer('bob',native,'Pay',10,receive.alice,'pft-03-bob-second-spend');
    await transfer('bob',native,'Unshield',18,addresses.bob,'pft-04-bob-exit');
    await sync('alice');
    await a.getByLabel('Asset',{exact:true}).selectOption(issue);
    await click(a,'Shield');
    await a.getByText('0 matching pfUSDC shield actions in the latest 4 finalized pool actions.').waitFor();
    await transfer('alice',issue,'Shield',80,null,'pfusdc-01-alice-shield');
    refuseNextSpend = true;
    await transfer('alice',issue,'Pay',25,receive.bob,'pfusdc-02-alice-pay');
    assert.equal((await sync('bob')).private[issue],'25');
    await transfer('bob',issue,'Pay',5,receive.alice,'pfusdc-03-bob-second-spend');
    await transfer('bob',issue,'Unshield',16,addresses.bob,'pfusdc-04-bob-exit');
    await click(a,'Orchard migration');
    const before=await sync('alice'); assert.deepEqual(before.private,{[native]:'78',[issue]:'58'});
    assert.deepEqual((await sync('bob')).private,{});
    await migrationSupply('Before migration',9,58);
    await click(a,'Orchard migration'); await capture(a,'migration-01-review-exit');
    const exit=await walletCall(a,'exit','Authorize Orchard exit');
    await a.getByText('Awaiting Orchard finality',{exact:true}).waitFor();
    await capture(a,'migration-02-await-finality');
    assert.equal((await walletCall(a,'receipt','Check finality')).state,'queued');
    assert.equal(await a.getByRole('button',{name:'Review new PQ shield'}).count(),0);
    await stop(bridges.alice); await stop(wallets.alice);
    const exitReceipt=await finalize(exit.intent);
    await startDaemon('alice'); await startBridge('alice'); await click(a,'Orchard migration');
    await check('alice',exit.intent,'migration-03-final-exit-after-restart',exitReceipt);
    await migrationSupply('After Orchard exit',0,58);
    const afterExit=await sync('alice'); assert.equal(BigInt(afterExit.public[issue]),BigInt(before.public[issue])+9n);
    await a.getByLabel('New shield fee (PFT atoms)').fill('3');
    await walletCall(a,'review','Review new PQ shield');
    await capture(a,'migration-04-review-shield');
    const shieldIntent = await prove('alice');
    await stop(bridges.alice); await stop(wallets.alice);
    await startDaemon('alice'); await startBridge('alice'); await click(a,'Orchard migration');
    await a.getByText('PQ shield awaiting finality',{exact:true}).waitFor();
    assert.equal(await a.getByRole('button',{name:'Review new PQ shield'}).count(),0);
    assert.equal((await walletCall(a,'receipt','Check finality')).state,'queued');
    await capture(a,'migration-04b-await-shield-finality');
    await check('alice',shieldIntent,'migration-05-final-shield');
    await migrationSupply('After fresh PQ shield',0,67);
    const after=await sync('alice'); assert.equal(after.migration.state,'Migration final');
    assert.equal(after.public[issue],before.public[issue]); assert.equal(after.private[issue],'67');
    await stop(wallets.bob); await click(b,'Sync'); await b.getByRole('alert').filter({hasText:'Disconnected'}).waitFor();
    assert.ok(await b.getByRole('button',{name:'Review payment'}).isDisabled());
    assert.deepEqual(await legacyPage(),historic);
    console.log('Historical Orchard-only snapshot returned the original ciphertext after migration.');
    assert.deepEqual(boundaryErrors,[]);
    assert.equal(relayCounts.submit,10);
    assert.ok(relayCounts.page>0 && relayCounts.receipt>=10 && relayCounts.snapshot>0);
    console.log('Public gateway frames verified: '+JSON.stringify(relayCounts));
    fs.mkdirSync(evidence,{recursive:true});
    for(const shot of shots) {
      const filename=shot+'.png';
      fs.copyFileSync(path.join(frames,filename),path.join(evidence,filename));
      fs.chmodSync(path.join(evidence,filename),0o644);
    }
    console.log('Two isolated browser sessions, changed signer terms, partial sync, disconnected daemon and restart passed. '+shots.length+' PNGs.');
  } finally {
    if(browser) await browser.close();
    for (const tap of taps) await new Promise(done => tap.close(done));
    if(audit) await new Promise(r=>audit.close(r));
    for(const p of [...children].reverse()) await stop(p);
    fs.rmSync(scratch,{recursive:true,force:true});
  }
}
