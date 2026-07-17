# External evidence archive

Raw browser captures, validator responses, proof artifacts, screenshots, and
live/devnet run directories are deliberately not distributed in the public
source tree. They can contain operator topology, machine paths, wallet
addresses, and shielded note openings even when the underlying test funds are
not valuable.

The source tree retains only redaction-safe summaries and cryptographic archive
manifests. Maintainers with authorized evidence-store access can retrieve an
archive by its identifier and verify it before use. Never copy a raw archive
back into a publication branch.

See `ARCHIVE-MANIFEST-20260716.md` for the pre-publication archive removed from
this tree.
