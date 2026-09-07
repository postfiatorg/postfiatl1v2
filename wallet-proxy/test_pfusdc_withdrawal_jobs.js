'use strict';

const assert = require('assert');
const fs = require('fs');
const os = require('os');
const path = require('path');
const { create, jobId, normalizeRequest } = require('./pfusdc-withdrawal-jobs');

const assetId = '11'.repeat(48);
const request = normalizeRequest({
    burn_tx_id: '22'.repeat(48),
    owner: `pf${'33'.repeat(20)}`,
    ethereum_recipient: `0x${'44'.repeat(20)}`,
    amount_atoms: '1000000',
    asset_id: assetId,
}, assetId);
assert(/^[0-9a-f]{64}$/.test(jobId(request)));
assert.throws(() => normalizeRequest({ ...request, burn_tx_id: '22'.repeat(32) }, assetId), /Invalid pfUSDC/);
assert.throws(() => normalizeRequest({ ...request, seed: 'must never cross the relay boundary' }, assetId), /Invalid pfUSDC/);

const root = fs.mkdtempSync(path.join(os.tmpdir(), 'pfusdc-withdrawal-jobs-'));
const script = path.join(root, 'worker.sh');
const config = path.join(root, 'config.json');
const prover = path.join(root, 'mock-prover-bin');
const elf = path.join(root, 'egress.elf');
const qualification = path.join(root, 'proof-report.json');
const vkey = `0x${'55'.repeat(32)}`;
fs.writeFileSync(script, '#!/bin/sh\nexit 0\n', { mode: 0o700 });
fs.writeFileSync(prover, '#!/bin/sh\nexit 0\n', { mode: 0o700 });
fs.writeFileSync(config, `${JSON.stringify({ pfusdc_egress_program_vkey: vkey })}\n`, { mode: 0o600 });
fs.writeFileSync(elf, 'elf', { mode: 0o600 });
fs.writeFileSync(qualification, `${JSON.stringify({ schema: 'postfiat.pfusdc.egress_proof_report.v1', program_vkey: vkey, proof_mode: 'groth16', proof_bytes: 356, public_values_bytes: 1486 })}\n`, { mode: 0o600 });

(async () => {
  const disabled = create({}, { env: {} });
  const readiness = await disabled.pfusdcWithdrawalReadiness();
    assert.strictEqual(readiness.ready, false);
  const enabled = create({}, { env: {
    PFUSDC_WITHDRAWAL_ENABLED: 'true', PFUSDC_ASSET_ID: assetId,
    PFUSDC_WITHDRAWAL_JOB_ROOT: path.join(root, 'jobs'),
    PFUSDC_WITHDRAWAL_SCRIPT: script, PFUSDC_WITHDRAWAL_CONFIG_FILE: config,
    PFUSDC_WITHDRAWAL_LOCAL_PROVER_BIN: prover, PFUSDC_WITHDRAWAL_EGRESS_ELF: elf,
    PFUSDC_WITHDRAWAL_QUALIFICATION_REPORT: qualification,
  } });
  assert.strictEqual((await enabled.pfusdcWithdrawalReadiness()).ready, true);
  enabled.closePfusdcWithdrawalJobs();

  const retryRoot = path.join(root, 'retry');
  const retryId = jobId(request);
  const retryDir = path.join(retryRoot, 'jobs', retryId);
  fs.mkdirSync(retryDir, { recursive: true, mode: 0o700 });
  const now = Math.floor(Date.now() / 1000);
  fs.writeFileSync(path.join(retryDir, 'job.json'), `${JSON.stringify({
    schema: 'postfiat-pfusdc-wallet-withdrawal-job-v1', job_id: retryId,
    status: 'failed', stage: 'Needs support', request, attempts: 3,
    created_at_unix: now, updated_at_unix: now,
    code: 'pfusdc_withdrawal_worker_failed', message: 'recoverable failure',
  }, null, 2)}\n`, { mode: 0o600 });
  const retryEnabled = create({}, { env: {
    PFUSDC_WITHDRAWAL_ENABLED: 'true', PFUSDC_ASSET_ID: assetId,
    PFUSDC_WITHDRAWAL_JOB_ROOT: retryRoot,
    PFUSDC_WITHDRAWAL_SCRIPT: script, PFUSDC_WITHDRAWAL_CONFIG_FILE: config,
    PFUSDC_WITHDRAWAL_LOCAL_PROVER_BIN: prover, PFUSDC_WITHDRAWAL_EGRESS_ELF: elf,
    PFUSDC_WITHDRAWAL_QUALIFICATION_REPORT: qualification,
  } });
  const retried = await retryEnabled.retryPfusdcWithdrawalJob(retryId);
  assert.strictEqual(retried.status, 'queued');
  assert.strictEqual(retried.idempotent_replay, false);
  assert.match(retried.stage, /Resuming safely/);
  retryEnabled.closePfusdcWithdrawalJobs();

  const orderedRoot = path.join(root, 'ordered');
  const laterRequest = normalizeRequest({
    ...request,
    burn_tx_id: '66'.repeat(48),
  }, assetId);
  const laterId = jobId(laterRequest);
  const paidDir = path.join(orderedRoot, 'jobs', retryId);
  const laterDir = path.join(orderedRoot, 'jobs', laterId);
  fs.mkdirSync(path.join(paidDir, 'pfusdc-egress'), { recursive: true, mode: 0o700 });
  fs.mkdirSync(laterDir, { recursive: true, mode: 0o700 });
  fs.writeFileSync(path.join(paidDir, 'pfusdc-egress', 'withdrawal-result.json'), '{}\n', { mode: 0o600 });
  fs.writeFileSync(path.join(paidDir, 'job.json'), `${JSON.stringify({
    schema: 'postfiat-pfusdc-wallet-withdrawal-job-v1', job_id: retryId,
    status: 'failed', stage: 'Ethereum USDC received; accounting retry required',
    request, attempts: 3, created_at_unix: now, updated_at_unix: now,
  }, null, 2)}\n`, { mode: 0o600 });
  fs.writeFileSync(path.join(laterDir, 'job.json'), `${JSON.stringify({
    schema: 'postfiat-pfusdc-wallet-withdrawal-job-v1', job_id: laterId,
    status: 'queued', stage: 'Withdrawal burn confirmed', request: laterRequest,
    attempts: 0, created_at_unix: now + 1, updated_at_unix: now + 1,
  }, null, 2)}\n`, { mode: 0o600 });
  const ordered = create({}, { env: {
    PFUSDC_WITHDRAWAL_ENABLED: 'true', PFUSDC_ASSET_ID: assetId,
    PFUSDC_WITHDRAWAL_JOB_ROOT: orderedRoot,
    PFUSDC_WITHDRAWAL_SCRIPT: script, PFUSDC_WITHDRAWAL_CONFIG_FILE: config,
    PFUSDC_WITHDRAWAL_LOCAL_PROVER_BIN: prover, PFUSDC_WITHDRAWAL_EGRESS_ELF: elf,
    PFUSDC_WITHDRAWAL_QUALIFICATION_REPORT: qualification,
  } });
  const orderedJobs = ordered.pfusdcWithdrawalJobsForOwner(request.owner);
  assert.strictEqual(orderedJobs.find(job => job.job_id === retryId).status, 'failed');
  assert.strictEqual(orderedJobs.find(job => job.job_id === laterId).status, 'queued');
  ordered.closePfusdcWithdrawalJobs();
  fs.unlinkSync(path.join(paidDir, 'pfusdc-egress', 'withdrawal-result.json'));
  const unpaidOrdered = create({}, { env: {
    PFUSDC_WITHDRAWAL_ENABLED: 'true', PFUSDC_ASSET_ID: assetId,
    PFUSDC_WITHDRAWAL_JOB_ROOT: orderedRoot,
    PFUSDC_WITHDRAWAL_SCRIPT: script, PFUSDC_WITHDRAWAL_CONFIG_FILE: config,
    PFUSDC_WITHDRAWAL_LOCAL_PROVER_BIN: prover, PFUSDC_WITHDRAWAL_EGRESS_ELF: elf,
    PFUSDC_WITHDRAWAL_QUALIFICATION_REPORT: qualification,
  } });
  assert.strictEqual(unpaidOrdered.pfusdcWithdrawalJobStatus(laterId).status, 'queued');
  unpaidOrdered.closePfusdcWithdrawalJobs();

  fs.chmodSync(elf, 0o622);
  assert.throws(() => create({}, { env: {
    PFUSDC_WITHDRAWAL_ENABLED: 'true', PFUSDC_ASSET_ID: assetId,
    PFUSDC_WITHDRAWAL_JOB_ROOT: path.join(root, 'unsafe'),
    PFUSDC_WITHDRAWAL_SCRIPT: script, PFUSDC_WITHDRAWAL_CONFIG_FILE: config,
    PFUSDC_WITHDRAWAL_LOCAL_PROVER_BIN: prover, PFUSDC_WITHDRAWAL_EGRESS_ELF: elf,
    PFUSDC_WITHDRAWAL_QUALIFICATION_REPORT: qualification,
  } }), /owner-controlled/);
  fs.rmSync(root, { recursive: true });
  console.log('pfUSDC withdrawal jobs regression passed');
})().catch(error => { console.error(error); process.exitCode = 1; });
