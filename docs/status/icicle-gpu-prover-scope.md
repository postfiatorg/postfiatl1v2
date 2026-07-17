# ICICLE GPU Prover Scope

Status: Phase 5 scope only
Date: 2026-06-20
Repo: `postfiatl1v2`

## Decision

Do not implement GPU proving in this CPU sprint. The CPU path now has measured
evidence and a K=15/key-cache optimization. The next acceleration path is a
separate ICICLE-Halo2 branch plus Akash/io.net prover deployment.

AKT wallet status for this sprint: awaiting funding per the sprint spec. No
Akash lease was opened and no GPU benchmark was run.

## Source Findings

Primary sources checked:

- Ingonyama ICICLE integrated provers docs:
  <https://dev.ingonyama.com/3.5.0/icicle/integrations>
- Ingonyama ICICLE getting-started docs:
  <https://dev.ingonyama.com/1.10.1/icicle/introduction>
- Ingonyama ICICLE repository:
  <https://github.com/ingonyama-zk/icicle>
- ICICLE MSM/precomputation benchmark docs:
  <https://dev.ingonyama.com/1.10.1/icicle/golang-bindings/msm-pre-computation>
- Local Halo2 dependency metadata:
  `halo2_proofs 0.3.2`, repository `https://github.com/zcash/halo2`

Relevant source facts:

- ICICLE documents a Halo2 fork integrated with GPU acceleration through an
  `icicle_gpu` feature flag.
- ICICLE setup requires NVIDIA GPU access, NVCC 12.0 or newer, CMake 3.18+,
  GCC 9+, and Linux or Windows. Docker plus NVIDIA Container Toolkit is the
  recommended container route.
- ICICLE accelerates low-level ZK prover primitives such as MSM/NTT/FFT-style
  work. The current PostFiat CPU path uses `halo2_proofs 0.3.2` with IPA.
- The current workspace is already on the Zcash/ECC Halo2 line and already has
  `multicore` enabled.

## Compatibility Assessment

Current PostFiat path:

```text
postfiat-privacy-orchard
├── halo2_gadgets v0.5.0
├── halo2_poseidon v0.1.0
├── halo2_proofs v0.3.2
└── orchard v0.14.0
```

ICICLE path:

```text
Halo2 fork + icicle_gpu feature + CUDA runtime
```

The ICICLE docs imply a backend/fork swap, not a change to the circuit
statement. That is promising because the AssetOrchard circuit code should
mostly remain the same. It still requires a compatibility branch because the
ICICLE Halo2 fork may not expose exactly the same APIs as
`halo2_proofs 0.3.2`, `halo2_gadgets 0.5.0`, and `halo2_poseidon 0.1.0`.

Do not mix the ICICLE branch into consensus until these are proven:

- the same public instance verifies under the pinned verifier semantics;
- proof bytes are accepted by the intended verifier path;
- the pinned VK metadata is regenerated and reviewed;
- the forged-nonconservation soundness regression still fails as expected;
- CPU and GPU proof outputs both verify against the same consensus verifier or
  the protocol explicitly versions the proof system.

## Code-Change Scope

Expected minimal branch shape:

```text
crates/privacy_orchard/
  Cargo.toml              add optional gpu-prover feature
  src/asset_orchard_circuit.rs
                           keep circuit statement; route proving backend
  src/prover_backend.rs    new small abstraction for CpuHalo2/IcicleHalo2

crates/node/
  src/main_parts/runtime_helpers.rs
                           prover-service/prove command help
  src/main_parts/cli_dispatch.rs
                           asset-orchard-prove-service command

StakeHub/
  stakehub/prover_service.py or rust sidecar launcher
                           lease, upload witness, collect proof, submit action
```

The first prototype should not alter consensus verification. It should create a
proof for the same `AssetOrchardSwapAction` public instance and then feed that
proof to the existing CPU verifier. If the ICICLE fork requires a different
proof encoding or verifier, that becomes a protocol-versioned proof-system
change and must go through cryptographic review.

Estimated implementation:

```text
dependency/fork integration       1-2 days if APIs align, longer if not
Docker/CUDA build                 0.5-1 day
prover service command            0.5 day
StakeHub orchestration            1 day
benchmark + soundness gate        0.5-1 day
```

## Prover Container

Base image:

```text
nvidia/cuda:12.4.1-devel-ubuntu22.04
```

Installed tooling:

```text
build-essential
cmake >= 3.18
gcc/g++ >= 9
pkg-config
curl
git
rust toolchain pinned to workspace rust-toolchain if present
postfiatl1v2 checkout
ICICLE Halo2 fork / feature branch
```

Container entrypoint:

```bash
postfiat-node asset-orchard-prove-service \
  --listen 0.0.0.0:8788 \
  --prover-backend icicle-gpu \
  --max-concurrent-proofs 1 \
  --work-dir /proof-work
```

GPU target:

```text
minimum: NVIDIA T4 16GB only for compatibility smoke tests
preferred: RTX 3090/4090 or A5000/A6000 class, 24GB VRAM
benchmark: H100/A100 if available, but do not make H100 mandatory
```

## Minimal Akash SDL Sketch

```yaml
version: "2.0"

services:
  asset-orchard-prover:
    image: ghcr.io/postfiat/asset-orchard-icicle-prover:latest
    expose:
      - port: 8788
        as: 8788
        to:
          - global: false
    env:
      - RUST_LOG=info
      - POSTFIAT_PROVER_BACKEND=icicle-gpu
      - POSTFIAT_MAX_CONCURRENT_PROOFS=1
    params:
      storage:
        data:
          mount: /proof-work
          readOnly: false
profiles:
  compute:
    asset-orchard-prover:
      resources:
        cpu:
          units: 8
        memory:
          size: 32Gi
        storage:
          - size: 50Gi
        gpu:
          units: 1
          attributes:
            vendor:
              nvidia:
                - model: rtx4090
  placement:
    dcloud:
      pricing:
        asset-orchard-prover:
          denom: uakt
          amount: 10000
deployment:
  asset-orchard-prover:
    dcloud:
      profile: asset-orchard-prover
      count: 1
```

Akash command shape:

```bash
provider-services tx deployment create deploy.yaml --from postfiat-akash
provider-services query market lease list --owner "$AKASH_ADDRESS"
provider-services send-manifest deploy.yaml --dseq "$DSEQ" --provider "$PROVIDER"
provider-services lease-status --dseq "$DSEQ" --provider "$PROVIDER"
provider-services tx deployment close --dseq "$DSEQ" --from postfiat-akash
```

Exact CLI names may differ by installed Akash/provider-services version; pin
the container and command set in the implementation branch.

## StakeHub Orchestration

StakeHub should treat GPU proving as an ephemeral sidecar:

1. Build swap witness locally from the two private asset-typed notes.
2. Encrypt the witness bundle to a one-use prover-session public key.
3. Lease GPU capacity.
4. Start the prover container with no long-term keys.
5. Upload encrypted witness plus public instance.
6. Prover returns proof bytes, prover metadata, and timing.
7. StakeHub verifies proof locally with the CPU verifier before submission.
8. StakeHub writes the `AssetOrchardSwapAction` and submits/certifies the
   shielded batch.
9. Close the GPU lease and delete remote work directory.

Witness handling rules:

- Never send wallet private keys or spend auth master material to the GPU host.
- Send only the minimum note-opening witness required to prove one swap.
- Encrypt in transit and at rest with an ephemeral session key.
- Prefer a private network path or SSH tunnel; do not expose the prover service
  globally unless it has request authentication and strict quotas.
- Delete witness files after proof generation and close the lease.

## Benchmark Plan

First GPU benchmark gate:

```text
same K=15 circuit
same public instance layout
same forged-nonconservation regression
same proof verification API or explicit proof-system version bump
measure cold container start, cold key load/build, prove, verify, total submit
```

Report rows:

```text
CPU K=15 hot path           prove ~= 5.78s, verify ~= 0.066s
GPU ICICLE smoke            measured on T4/16GB if available
GPU ICICLE target           measured on RTX 4090/24GB or similar
```

Expected result before measurement:

```text
target: <2s proof on 24GB NVIDIA GPU
stretch: sub-second proof if ICICLE accelerates the dominant MSM/FFT work
```

This is a target, not evidence. The current measured floor remains the CPU K=15
hot proof: about `5.8s`.

## Risks

- ICICLE Halo2 fork may not be source-compatible with `halo2_proofs 0.3.2` plus
  Orchard `0.14.0`.
- GPU proof encoding may differ from the current verifier path.
- CUDA/non-deterministic scheduling does not affect proof soundness if the proof
  verifies, but it can affect reproducible benchmarking and operational
  debugging.
- Remote GPU hosts see private swap witnesses unless the witness is encrypted
  and the prover process is trusted only for computation. This is a privacy
  risk, not a consensus risk.
- Akash lease startup time may dominate one-off swaps; GPU proving only helps if
  the lease is warm or batches multiple proofs.

## Gate to Implement

Open a separate `asset-orchard-icicle-prover` branch only after funding and GPU
capacity are available. The first mergeable unit must include:

- ICICLE compatibility matrix;
- Dockerfile;
- one local GPU proof benchmark;
- CPU verifier acceptance of GPU proof or a protocol-versioned verifier update;
- forged-nonconservation rejection;
- no long-term private key or witness material in logs, images, or commits.
