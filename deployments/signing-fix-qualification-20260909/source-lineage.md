# Source-lineage comparison

The running release evidence pins binary SHA-256
`57b0f4d1d42d66878d7dbb8c33919c7fa0f87c6cc1a4b9cc1a85d75b634eec83`
to base source `707e006fe2460048b1c7df0d4dc0a04d487f8542`, source patch SHA-256
`deaf05d46707bdb00d2d2d383fd40b18678f8378fb8b87f69898a5180066586c`,
and the 26-file source manifest formerly committed under
`deployments/a666-source-route-20260907/`.

The qualification source is
`4153376b8097670ed1af0319affdf204ed85b2f5`. The deployed base is not its
ancestor; the merge base is
`3f393b76ef0b55f52273caa7d5159660b31e888d`. Therefore
`git log 707e006f..4153376b --oneline` is not an additive release delta. The
complete command output is retained in [source-commit-range.txt](source-commit-range.txt),
while exact tree comparison supplies the runtime classification below.

## Validator-runtime classification

- **Intended signing change:** `bbb291ce` enforces one durable monotone signing
  floor across prepare, precommit, and timeout and restores it from snapshots.
- **Blocking deployed-lineage omission:** current `main` omits the deployed
  Arc/PFETH source-route types, custody, settlement execution, replay handling,
  and state commitments present in base `707e006f` plus the recorded source
  patch, including `pftl_source_settlement.rs`. This is a validator-runtime and
  replicated-state compatibility difference.
- **Additional dormant runtime:** merged YOLO target-receipt commits add proved
  receipt validation, state commitments, and RPC/query surfaces. Activation is
  default-disabled and remains unscheduled, but the binary surface differs.
- **Runtime-neutral for this question:** subsequent UNL V1/V2 Python modules,
  gate/report/docs, safety-file closeout, genesis golden-vector test repair,
  Cobalt test formatting, and the 2026-09-09 status update do not change active
  validator consensus behavior.

No attempt was made to reinterpret the divergent histories as a signing-only
upgrade. The deploy decision treats source-lineage reconciliation as a
precondition.
