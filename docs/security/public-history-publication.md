# Public History Publication Gate

The current private Git history must not be made public. A historical provider
capture contains a Jupyter bearer token in three versions of one evidence
bundle. The value is intentionally absent from this document and from scanner
output. Treat it as compromised even if the instance is believed destroyed.

Before publication:

1. The provider owner records token revocation or instance destruction in a
   private incident record. Never paste the token into an issue, commit, or CI
   log.
2. Create a new public repository from a clean tracked-tree export of the
   reviewed release commit, or rewrite a dedicated publication clone. Do not
   push private backup refs, replace refs, notes, reflogs, pull refs, or tags.
3. Record the exact Git tree object ID of the reviewed release commit. A
   history rewrite or clean export must not silently change those source bytes.
4. Push first to a new private staging repository, fetch it into a separate
   complete, non-shallow clone, and run the fail-closed gate:

   ```text
   scripts/verify-publication-candidate \
     --repository /clean/staging/clone \
     --expected-tree <reviewed-tree-object-id> \
     --provider-revocation-record /private/provider-revocation.json \
     --allow-ref refs/heads/main \
     --allow-ref refs/remotes/origin/main
   ```

   Repeat `--allow-ref` for every expected local or remote-tracking ref in the
   clean staging clone; do not add a private backup ref merely to make the gate
   pass. The gate rejects
   a dirty or shallow checkout, any missing or unexpected branch/tag/remote/
   note/replace ref, a detached or non-public `HEAD`, tree drift, a current-tree
   secret finding, or a finding anywhere in reachable published history. The
   provider record is mandatory, must be a nonsymlink regular file outside the
   candidate with no group/other permissions, and accepts only the bounded
   `postfiat-provider-credential-revocation-v1` fields implemented by the gate.
   It records provider, private incident/evidence references, owner/verifier,
   terminal action, and UTC timestamps; it must never contain the credential.
5. Require explicit publication approval only after that staging-clone gate
   passes with the private provider record. In the clean clone, also run
   `scripts/test-productionization-closure-table --require-closed`; publication
   must fail while any P0/P1 row is open, pending or waived. Never add a
   credential/path allowlist to make the contaminated private history pass.

The scanner has narrow, path-specific exceptions only for deterministic test
vectors and negative secret-scanner fixtures. It emits rule/path/line metadata,
not candidate values or reusable hashes. The current private history's
value-redacted expected failure is exactly 27 findings: three `jupyter-token`
locations under the historical `reports/gov-inference-provider/...` bundle and
24 `private-note-opening` field occurrences across seven removed
`docs/evidence/...` ingress artifacts. The latter are six opening fields in one
legacy ingress response plus three fields in each of six private-swap ingress
batch captures. A sanitized staging history must have zero findings; the counts
are an incident baseline, never an allowlist. Any different or additional
finding in the private source history is a separate triage item.
