# YOLO target proof operator guide

`postfiat-yolo-target` verifies Nitro certificates and COSE signatures, the
Ed25519 statement chain, complete collection commitments, and the enclave's
commitment to the entire normalized target input. It independently calculates
the existing portfolio methodology and emits the locked 408-byte target ABI.
Normalization is attested by measured enclave code; target calculation is proved.
The guest never accepts a host assertion that attestation passed.

The governing plan and runbook are NAVStrategies
`docs/pre_production/yolo_indices_build_plan.md` and
`docs/pre_production/yolo_e2e_tee_proof_runbook.md`. Protocol details are in its
`yolo_phase3_witness_and_policy_v1.md`. This implementation alone does not qualify
an EIF, authorize AWS resources, or register a PFTL receipt operation.

## Build

From this directory, use the repository Rust pin and a working `protoc`:

```bash
cargo build --release --locked -p postfiat-yolo-target --features sp1
```

That profile uses Docker for the final Gnark stage. The private Nitro prover
uses the native profile instead, built with the recorded Rust, Go, C/C++ and
protobuf toolchains in a runtime-compatible Linux image:

```bash
cargo build --release --locked -p postfiat-yolo-target --features sp1-native
```

The qualified native build uses source `9b74999e`, Rust 1.95.0 and Go 1.27.1;
binary SHA-256 is
`f9f22381fce0b2cd188429acb72aed4f584013e93d6f630831113121dfa50950`.
NAV's `docs/pre_production/yolo_phase6_private_prover_v1.md` records its exact
build image, circuits, runtime and proof qualification. Do not infer binary
identity from a later node revision or a different host libc/toolchain.

Install SP1 6.3.1 and build `programs/yolo-target-guest` using
`cargo prove build --docker --tag v6.3.1 --locked`, with `--workspace-directory`
set to the repository root and a new `--output-directory`. Record the image
digest, source revision, guest lock hash, ELF SHA-256 and `identity` output.
Use `SP1_DOCKER_IMAGE` with the recorded image digest for subsequent builds.
All SP1/support crates and official SHA-2/Curve25519 guest patches are locked.
The host Rust pin and SP1's guest compiler version are recorded separately.

## Independent acceptance inputs

Every witness/proof acceptance command requires a separate JSON file containing
exactly `programSha256` and `collectionManifestSha256`, as lowercase 32-byte hex.
These are approved public run values established before collection. Do not
derive acceptance pins from the witness or an untrusted proof sidecar.

The manifest commits the complete epoch, normalization-source and parameter
commitments, root DER digest, approved PCR combinations, explicit age/skew
allowances, and named statement/time rules. There are no production policy
defaults. Certificate/attestation time derives from the committed epoch window
end, not the proving host's clock. Production uses an independently approved
AWS Nitro root and non-debug measurements; test roots prove only synthetic runs.

## Local commands

Here `RUN` is a new private run directory, `ELF` is the approved guest file,
`PINS` is the independent acceptance JSON, and `CLI` is the release binary.
Use explicit paths appropriate to the authorized run. Witness JSON/CBOR contains
private inputs and attestation evidence; retain it only under the approved
private-data policy. The CLI never exports keys or source rows in public values.

```bash
"$CLI" identity --elf "$ELF"
"$CLI" create-witness --input "$RUN/witness.json" --acceptance "$PINS" \
  --output "$RUN/witness.cbor"
"$CLI" native-execute --witness "$RUN/witness.cbor" --acceptance "$PINS" \
  --output "$RUN/native.bin"
"$CLI" guest-execute --witness "$RUN/witness.cbor" --acceptance "$PINS" \
  --elf "$ELF" --output "$RUN/guest.bin"
"$CLI" decode --public-values "$RUN/guest.bin"
```

For local Groth16 proving, run the repository's bounded CPU launcher from the
repository root. It checks Docker availability and sets the reviewed worker
profile; both target CLI and launcher select local CPU proving explicitly.

```bash
scripts/nav-reserve-proof-cpu-bounded --prover "$CLI" \
  --witness "$RUN/witness.cbor" --acceptance "$PINS" --elf "$ELF" \
  --output-dir "$RUN/proof"
"$CLI" verify --proof "$RUN/proof/proof.bin" --acceptance "$PINS" --elf "$ELF"
```

The proof command verifies its result before writing `proof.bin`,
`proof-calldata.bin`, `public-values.bin`, and `proof-report.json`. Outputs are
create-only. `verify` requires Groth16, exact ABI, independent public pins, actual
ELF hash and the verification key derived from that ELF. No network prover,
broker order, reserve submission, or chain transaction is invoked.
Execution, identity and verification use SP1's local light client with full
cryptographic verification. They do not initialize CPU proving workers.
SDK progress goes to stderr; stdout remains JSON. The CLI rejects SDK witness/
trace-dump switches and circuit/verification overrides before starting SP1,
so inherited debug settings cannot export private inputs or bypass proving.

For private production inputs, these witness/prove commands run only through
NAV's measured `nitro_prover_entrypoint` inside Nitro. Its parent accepts an
encrypted replay and pinned public circuits; the parent receives only public
proof bytes and encrypted diagnostics. Do not use the Docker-checking CPU
launcher inside the enclave. The fixed native environment supplies its bounded
workers and `TMPDIR=/private-tmp` because Nitro mounts `/tmp` non-executable.
The offline container qualification also requires explicit shared memory
(`--shm-size 16g`); Docker's default 64 MiB is insufficient for SP1's executor.

## Qualification

```bash
cargo test --locked --no-default-features -p reserve-proof-types
YOLO_TARGET_GUEST_ELF="$ELF" YOLO_EXPECTED_ELF_SHA256="$APPROVED_ELF_SHA256" \
  cargo test --release --locked -p postfiat-yolo-target --features sp1 \
  --test guest_qualification -- --ignored --nocapture
```

Run SP1 qualification under the same bounded worker environment and record
`/usr/bin/time -v` output. The ignored guest test checks all 14 Python/native/
guest public-value vectors and directly executes 13 malformed or tampered
witnesses, bypassing host prechecks to test guest rejection.

The durable fixtures in `crates/reserve-proof-types/tests/fixtures` contain only
synthetic public certificates/documents and synthetic portfolio inputs. They
preserve cross-language protocol regressions; ephemeral signing keys are never
exported. The shared 38-case corpus plus six actual-AWS-document cases must
match Python and native Rust, including warmed-cache rejection tests.
Retain full logs locally; promote only
disclosure-safe measurements and artifact hashes to qualification reports.

`programs/yolo-nitro-test-guest` is a separate qualification harness that executes
the combined 44-case Nitro corpus using the target's exact crypto dependency locks
and patches. Its output is only a vector of test results; it is not a target
receipt program and must never be registered as one. Build it with the same
pinned Docker image, then run the ignored `nitro_guest_qualification` test with
`YOLO_NITRO_TEST_GUEST_ELF` and `YOLO_NITRO_TEST_ELF_SHA256` set explicitly.

After a proof exists, run the ignored `proof_qualification` test with
`YOLO_TARGET_PROOF`, `YOLO_TARGET_GUEST_ELF`, `YOLO_OTHER_GUEST_ELF` (a different
built guest) and independently supplied `YOLO_TARGET_ACCEPTANCE`. It checks real
Groth16 verification and rejection of corrupted proofs, changed/truncated public
values and the wrong program verification key.

## Independent guest and node revisions

The accepted AWS-compatible guest is built from source `459a6d3d` with the pinned SP1 Docker
toolchain. Its ELF SHA-256 is
`9aa40d86cc478a01fd7c6b2a63308188fb7e671079bd3321b405e7ac1a275cff` and
verification key is
`0x0008ed5f9307fc00d9e60fbcd0e463e10d1bb3143ce3f73bcb2c807aa0670968`.
The earlier `38909717` identity remains historical synthetic evidence.

Later node receipt changes also change shared `postfiat-types` source. Do not
assume rebuilding the guest from a later node HEAD preserves its identity. Use
the qualified ELF or a clean checkout of `459a6d3d` and compare the exact hash.
The target receipt verifier accepts the explicitly registered key independently
of the node build revision. See `docs/yolo/target-receipt-v1.md`.
