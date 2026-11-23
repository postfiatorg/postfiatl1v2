import React, { useEffect, useState } from 'react';
import { initWasm } from '../lib/wasm-loader.js';
import { checkPublicTerms } from '../lib/pq-payments.js';
import './pq-payments.css';

const NATIVE = '00'.repeat(48);
const modes = { Shield: 'Shield', Internal: 'Pay', Exit: 'Unshield' };
const sessionKey = 'postfiat.pq.local-session.v1';

function session() {
  if (location.hostname !== '127.0.0.1') throw Error('Open the pairing link from your local wallet.');
  const token = location.hash.slice(1);
  if (token) {
    history.replaceState(null, '', location.pathname);
    sessionStorage.setItem(sessionKey, token);
  }
  return sessionStorage.getItem(sessionKey) || '';
}

// Opening a fresh pairing link in the same tab can be a fragment-only navigation.
if (location.hostname === '127.0.0.1' && location.pathname === '/private') {
  window.addEventListener('hashchange', () => {
    if (location.hash) {
      session();
      location.reload();
    }
  });
}

async function call(cmd, body = {}) {
  const response = await fetch('/local/pq/v1', { method: 'POST', cache: 'no-store', redirect: 'error',
    headers: { 'Content-Type': 'application/json', Authorization: 'Bearer ' + session() },
    body: JSON.stringify({ cmd, body }) });
  const data = await response.json();
  if (!response.ok) throw Error(data.error || 'Local connection unavailable.');
  return data;
}


function Field({ label, children }) {
  return <div className="pq-field"><span>{label}</span><strong>{children}</strong></div>;
}

export default function PqPayments() {
  const [wallet, setWallet] = useState(null);
  const [asset, setAsset] = useState(NATIVE);
  const [mode, setMode] = useState('Shield');
  const [amount, setAmount] = useState('');
  const [fee, setFee] = useState('1');
  const [recipient, setRecipient] = useState('');
  const [error, setError] = useState('');
  const [busy, setBusy] = useState('');
  const [progress, setProgress] = useState('');
  const [review, setReview] = useState(null);
  const [result, setResult] = useState(null);
  const [activity, setActivity] = useState(null);
  const [tab, setTab] = useState('payments');
  const [showAddress, setShowAddress] = useState(false);
  const migration = wallet?.migration;
  const migrating = tab === 'migration';
  const unit = asset === NATIVE ? 'PFT' : 'pfUSDC';

  async function work(label, action) {
    setBusy(label); setError('');
    try { await action(); } catch (err) {
      setError(err.message);
      if (label === 'Syncing' || /sync|disconnected|connection|unavailable/i.test(err.message)) setWallet(w => w && ({ ...w, synced: false }));
    } finally { setBusy(''); setProgress(''); }
  }
  async function refresh(command = 'status', publicBalances = false) {
    const next = await call(command, { public: publicBalances });
    setWallet(next);
    return next;
  }
  useEffect(() => { work('Connecting', () => refresh()); }, []);
  useEffect(() => {
    let current = true;
    setActivity(null);
    if (wallet) call('activity', { asset, mode }).then(v => current && setActivity(v)).catch(() => {});
    return () => { current = false; };
  }, [asset, mode, wallet]);
  useEffect(() => {
    if (busy !== 'Proving locally') return;
    const timer = setInterval(() => call('progress').then(r => {
      if (r.active?.stage === 'prove') setProgress(Math.floor(r.active.seconds) + ' s');
    }).catch(() => {}), 700);
    return () => clearInterval(timer);
  }, [busy]);

  function change(action) { setReview(null); setResult(null); action(); }
  const governed = mode !== 'Shield' && asset !== NATIVE;
  const paymentFee = governed ? wallet?.issued_fee : fee;
  async function requestReview(isMigration = false) {
    const terms = isMigration ? { mode:'Shield', asset:migration.asset_id, amount:String(migration.amount),
      recipient:null, fee, migration:true } : { mode, asset, amount, fee:paymentFee, recipient:mode === 'Shield' ? null : recipient, migration:false };
    if (terms.mode === 'Internal') (await initWasm()).pq_check_address(wallet.network.chain_id, wallet.network.genesis_hash, recipient);
    const r = await call('review', terms);
    for (const k of Object.keys(terms)) if (r.terms[k] !== terms[k]) throw Error('Review changed.');
    setReview(r); setResult(null);
  }
  async function confirm() {
    const terms = review.terms;
    setReview(null);
    const wasm = await initWasm();
    const prepared = await call('prove', review);
    checkPublicTerms(JSON.parse(wasm.pq_review_payment(prepared.action_json)), terms, prepared.action_hash);
    setResult({ intent:prepared.intent, state:'prepared' });
    try {
      setResult(await call('submit', { intent:prepared.intent, action_hash:prepared.action_hash }));
    } catch (err) {
      setResult(null);
      throw err;
    } finally { await refresh(); }
  }
  async function checkReceipt(intent) {
    const r = await call('receipt', { intent });
    setResult(r);
    await refresh(r.state === 'final' ? 'sync' : 'status');
  }
  const selectedIntent = migrating ? migration?.intent : result?.intent || wallet?.last?.intent;
  const canReview = wallet?.synced && !wallet.pending && !busy;
  return <main className="pq-shell">
    <header><span className="pq-brand">Post Fiat <b>/ Wallet</b></span><span className="pq-status">
      {wallet ? (wallet.synced ? 'Synced' : 'Sync required') + ' · block ' + wallet.height : 'Local daemon disconnected'}</span></header>
    <h1>Private payments</h1>
    <nav aria-label="Private wallet sections">{['payments', 'migration'].map(t => <button key={t} aria-pressed={tab === t}
      disabled={!!busy} onClick={() => change(() => setTab(t))}>{t === 'payments' ? 'Payments' : 'Orchard migration'}</button>)}</nav>
    {error && <p role="alert" className="pq-warning">{error}</p>}
    {!wallet ? <section><h2>Connect your local wallet</h2><p>Start the <code>pq-wallet browser</code> command and open its owner-only pairing link.</p>
      <button onClick={() => work('Connecting', () => refresh())}>Reconnect</button></section> : <>
    <div className="pq-grid"><div>
    {!migrating ? <section><div className="pq-heading"><h2>Payment</h2><label>Asset <select aria-label="Asset" disabled={!!busy} value={asset}
      onChange={e => change(() => setAsset(e.target.value))}>
      <option value={NATIVE}>PFT</option><option value={wallet.issue}>pfUSDC</option></select></label></div>
      <div className="pq-actions">{Object.entries(modes).map(([m,name]) => <button key={m} disabled={!!busy}
        aria-pressed={mode === m} onClick={() => change(() => setMode(m))}>{name}</button>)}</div>
      <label>Amount ({unit} atoms)<input inputMode="numeric" value={amount} disabled={!!busy}
        onChange={e => change(() => setAmount(e.target.value))}/></label>
      {mode !== 'Shield' && <label>{mode === 'Exit' ? 'Public exit account' : 'Recipient receive address'}
        <input value={recipient} disabled={!!busy} onChange={e => change(() => setRecipient(e.target.value))}/></label>}
      <label>Fee ({mode === 'Shield' ? 'PFT' : unit} atoms)<input value={paymentFee ?? ''} inputMode="numeric" readOnly={governed} disabled={!!busy} onChange={e => change(() => setFee(e.target.value))}/></label>
      {mode === 'Shield' && <Field label="Public deposit account">{wallet.account || 'Configure a deposit signer to shield'}</Field>}
      <button className="pq-primary" disabled={!canReview || !amount || !paymentFee || (mode === 'Shield' && !wallet.account)} onClick={() => work('Reviewing', () => requestReview())}>Review payment</button>
    </section> : <section><h2>Orchard → Public account → PQ</h2>
      {!migration ? <p>Prepare a policy-permitted Orchard private exit with your Orchard wallet. Open its public batch using the local bridge’s Orchard exit option.</p> : <>
      <h3 data-testid="migration-state">{migration.state}</h3>
      <p>The exit and new deposit publicly link the same account, issue and amount. Each transaction needs your approval.</p>
      <Field label="Public account">{migration.to}</Field><Field label="Source-series ID">{migration.asset_id}</Field>
      <Field label="Public amount">{migration.amount} pfUSDC atoms</Field><Field label="Orchard exit fee">{migration.fee} PFT atoms</Field>
      <Field label="Exit policy">{migration.policy_id}</Field><Field label="Disclosure">{migration.disclosure_hash}</Field>
      <p>The validator enforces trustline, freeze and disclosure restrictions. A required disclosure can prevent a PQ deposit.</p>
      <label>New shield fee (PFT atoms)<input value={fee} disabled={!!busy} onChange={e => change(() => setFee(e.target.value))}/></label>
      {migration.state === 'Review Orchard exit' && <button className="pq-primary" disabled={!!busy} onClick={() => work('Submitting exit', async () => {
        setResult(await call('exit', { intent:migration.exit_intent })); await refresh();
      })}>Authorize Orchard exit</button>}
      {migration.state === 'Exit final — review new shield' && <button className="pq-primary" disabled={!canReview}
        onClick={() => work('Reviewing', () => requestReview(true))}>Review new PQ shield</button>}
      </>}
    </section>}
    {review && <section aria-label="Signer review"><h2>Signer review</h2>
      <Field label="Action">{modes[review.terms.mode]} {review.terms.amount} {review.terms.asset === NATIVE ? 'PFT' : 'pfUSDC'} atoms</Field>
      <Field label="Asset / issue">{review.terms.asset === NATIVE ? 'PFT' : review.terms.asset}</Field>
      {review.terms.recipient && <details><summary>Destination · {review.terms.recipient.slice(0,18)}…</summary><code>{review.terms.recipient}</code></details>}
      <Field label="Fee">{review.terms.fee} {review.terms.mode === 'Shield' || review.terms.asset === NATIVE ? 'PFT' : 'pfUSDC'} atoms · {review.terms.mode !== 'Shield' ? 'paid from private notes' : review.terms.asset === NATIVE ? 'deducted from deposit' : 'paid by depositor'}</Field>
      {review.terms.payer && <Field label="Depositor">{review.terms.payer}</Field>}
      {review.terms.sequence && <Field label="Account sequence">{review.terms.sequence}</Field>}
      <button className="pq-primary" disabled={!!busy} onClick={() => work('Proving locally', confirm)}>Confirm and prove locally</button>
    </section>}
    {busy && <p role="status">{busy} {progress}</p>}
    {selectedIntent && <section><div className="pq-heading"><h2>Validator receipt</h2><b data-testid="receipt-state">{result?.state || 'Check finality'}</b></div>
      <Field label="Intent">{selectedIntent}</Field>
      {result?.receipt && <><Field label="Receipt ID">{result.receipt}</Field><Field label="Certified block">{result.height}</Field></>}
      <button disabled={!!busy} onClick={() => work('Checking finality', () => checkReceipt(selectedIntent))}>Check finality</button>
    </section>}
    </div><aside><section><div className="pq-heading"><h2>Balances</h2><button disabled={!!busy}
      onClick={() => work('Syncing', () => refresh('sync', migrating || mode === 'Shield'))}>Sync</button></div>
      <table><thead><tr><th>Atoms</th><th>Public</th><th>Private</th></tr></thead><tbody>{[NATIVE,wallet.issue].map(a =>
        <tr key={a}><th>{a === NATIVE ? 'PFT' : 'pfUSDC'}</th><td>{wallet.public[a] ?? '—'}</td><td>{wallet.private[a] || '0'}</td></tr>)}</tbody></table>
      {wallet.account && <Field label="Public account">{wallet.account}</Field>}
      <Field label="pfUSDC source series">{wallet.issue}</Field>
      <Field label="Pending private change">{wallet.pending ? String(wallet.pending.delta) + (wallet.pending.asset === NATIVE ? ' PFT atoms' : ' pfUSDC atoms') : 'None'}</Field>
      <button onClick={() => setShowAddress(!showAddress)}>{showAddress ? 'Hide receive address' : 'Receive address'}</button>
      {showAddress && <textarea aria-label="Your receive address" readOnly value={wallet.address}/>}
    </section>{!migrating && <section className="pq-privacy"><h2>Privacy</h2>
      <h3>{activity?.count < 16 ? 'hidden, small pool' : 'Observer privacy unverified'}</h3>
      {activity && <p>{activity.count} matching {unit} {modes[mode].toLowerCase()} actions in the latest {activity.sampled} finalized pool actions.</p>}
      <p>Deposits and exits reveal the public account and amount. Payments reveal the asset, fee, time and tree. The payment carries no sender account.</p>
      <p>The submission service sees the caller and timing. The observer model requires 16 joint candidates. Timing, fees and disclosures can narrow the set.</p>
    </section>}</aside></div></>}
  </main>;
}
