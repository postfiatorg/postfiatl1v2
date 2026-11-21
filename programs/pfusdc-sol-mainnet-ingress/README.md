# Mainnet Solana deposit guests

The local relay reads `program-info.json` and verifies the ELF digest before loading a job. The deposit uses the aggregate guest. It binds the shard proof key in the aggregate program. The keys and handoff guests serve epoch transitions. The governed handoff pin can differ from the candidate handoff key in this release.

The source, Cargo files and manifest come from the service checkout at `a60d93baa`. Its last guest source change was `72bb66ac8`. The manifest retains the original build record, including its older source-commit field and its negative byte-replay result. Use the image digest and command in the manifest for an independent build. Compare every output against the recorded SHA-256 and independently derive its SP1 vkey before use. The sibling `pfusdc-sol-ingress` directory holds the earlier ledger release.

The ELF files are external release artifacts. Place them under `elf/`. The host crate and its client and server are under `tools/pfusdc-sol-ingress-host`. See [the service cutover](../../ops/sol-live/CUTOVER.md) for local artifact installation.
