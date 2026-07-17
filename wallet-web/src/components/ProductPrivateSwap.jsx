import React, { useState } from 'react';
import { Check, Loader2, ShieldCheck } from 'lucide-react';

import { executeProductPrivateSwap } from '../lib/product-private-swap.js';


const LABELS = {
  preflight: 'Preflight',
  public_funding: 'Public funding',
  shield_ingress: 'Shield ingress',
  private_swap: 'Private swap',
  private_egress: 'Private egress',
  public_settlement: 'Public settlement',
  final_verify: 'Final verification',
};


export default function ProductPrivateSwap({ address, swapServer, onToast }) {
  const [phase, setPhase] = useState('idle');
  const [result, setResult] = useState(null);
  const [error, setError] = useState('');

  const execute = async () => {
    setPhase('running');
    setError('');
    setResult(null);
    try {
      const completed = await executeProductPrivateSwap({ swapServer, walletAddress: address });
      setResult(completed);
      setPhase('done');
      onToast?.('Certified private swap verified');
    } catch (cause) {
      setError(cause?.data?.error || cause?.message || 'Certified private swap failed');
      setPhase('failed');
    }
  };

  return (
    <section className="pfs-card pfs-route-card" aria-label="Certified private swap workflow">
      <div className="pfs-route-head">
        <span>Certified private swap</span>
        <div className="pfs-pill-row">
          <span className={`pf-pill${phase === 'done' ? ' good' : phase === 'failed' ? ' warn' : ''}`}>
            <ShieldCheck size={11} /> {phase === 'done' ? 'verified' : phase === 'running' ? 'running' : 'no-money'}
          </span>
        </div>
      </div>
      <p>
        Runs the same resumable five-leg product state machine as the CLI with a new manifest-bound run wallet.
        Your FastPay seed, backup, and private keys never leave this wallet.
      </p>
      <button
        type="button"
        className="pf-primary"
        onClick={execute}
        disabled={!address || !swapServer || phase === 'running'}
      >
        {phase === 'running' ? <><Loader2 size={15} className="pfs-spin" /> Running private swap…</> : 'Run verified private swap'}
      </button>
      {error && <div className="pf-warning" role="alert">{error}</div>}
      {result && (
        <>
          <div className="pf-swap-rail">
            {result.steps.map((step, index) => (
              <div className={`pf-swap-step ${step.state === 'verified' ? 'done' : step.state}`} key={step.name}>
                <span className="pf-step-num">{index + 1}</span>
                <strong>{LABELS[step.name]}</strong>
                <span className="pf-step-status">{step.state === 'verified' ? <Check size={14} /> : step.state}</span>
              </div>
            ))}
          </div>
          <div className="pfs-detail-list">
            <div><span>Run</span><strong>{result.runId}</strong></div>
            <div><span>Final height</span><strong>{result.finalHeight}</strong></div>
            <div><span>State root</span><strong>{result.finalStateRoot}</strong></div>
          </div>
        </>
      )}
    </section>
  );
}
