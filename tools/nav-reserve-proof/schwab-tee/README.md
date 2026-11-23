# Schwab TEE reader

Reads a Schwab brokerage account inside an AWS Nitro enclave and produces the signed account
value that the B24 NAVCoin's SP1 proof and on-chain reading are built from. It moved here from
the `navstrategies` repository on 2026-11-23. Operating it: see
[B24 Schwab NAV Reading](../../../docs/runbooks/b24-schwab-nav-reading.md).

| Directory | What it is |
| --- | --- |
| `pftl_schwab_tee/` | The package measured into the enclave: Schwab OAuth over a vsock TLS tunnel, Nitro attestation and KMS, the NAV reading and its entrypoint (`nav_reading_entrypoint.py`), canonical encoding, the private archive. `nitro_build.py` builds the image context from a committed revision only |
| `enclave/` | Dockerfile, `assemble.py`, `requirements.lock`, and the pinned build inputs (`nav-build-inputs.json`: Python image digest, NSM library and Nitro root hashes) |
| `operator/` | Runs on the operator machine. `nav_reading_run.py` launches one EC2 parent, builds the image twice and compares PCRs, seals the Schwab credentials to KMS for the measured enclave, collects the reading and tears everything down. `verify_nav_archive.py` checks the encrypted archive. IAM bootstrap in `README.md` |
| `parent/` | Scripts the runner uploads to the EC2 parent (`yolo_capture_parent.py` and the modules it uses) |
| `mint_reader/` | Read-only Schwab deposit observer used for minting, run on the credential host. Standard library only (`schwab_client.py`) |
| `tests/` | Unit and replay tests, with fixtures |

## Tests

```bash
cd tools/nav-reserve-proof/schwab-tee
~/repos/postfiatl1v2-live/.venv-ops/bin/python -m pytest -q tests
```

## Rules that keep a reading trustworthy

- The image is built only from committed source at the revision in the run plan, and the
  checkout must be clean at that revision.
- Two builds must give identical PCRs; credentials are released only to an enclave whose
  measured profile equals the asset's registered proof profile.
- Schwab credentials stay on the credential host. The runner reads them over SSH into memory
  only to encrypt them with the run's KMS key; only ciphertext is written to disk or sent to EC2.
- Any change under `pftl_schwab_tee/` or `enclave/` changes the PCRs and therefore needs a new
  proof profile before the next capture (runbook, "Changing the enclave").
