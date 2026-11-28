"""The release commands, end to end, the way the release process uses them.

Three signers make their keys, the release lead builds the manifest from the
assembled release directory, two signers sign, and an operator's pre-start
check accepts the binary. One signature is not a release, and a binary the
manifest does not name is refused.
"""

from __future__ import annotations

import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

REPO = Path(__file__).resolve().parents[2]


def node_binary() -> str:
    configured = os.environ.get("POSTFIAT_NODE_BIN")
    if configured:
        return configured
    target = Path(os.environ.get("CARGO_TARGET_DIR", REPO / "target"))
    subprocess.run(
        ["cargo", "build", "--offline", "--locked", "--release", "-p", "postfiat-node", "--bin", "postfiat-node"],
        cwd=REPO,
        check=True,
    )
    return str(target / "release/postfiat-node")


class ReleaseFlow(unittest.TestCase):
    def test_keys_manifest_two_signatures_and_the_pre_start_check(self) -> None:
        node = node_binary()

        def run(*args: str, ok: bool = True) -> subprocess.CompletedProcess[str]:
            result = subprocess.run([node, *args], capture_output=True, text=True, check=False)
            if ok:
                self.assertEqual(result.returncode, 0, result.stderr[:400])
            return result

        with tempfile.TemporaryDirectory() as tmp_name:
            tmp = Path(tmp_name)
            # 1. Each signer makes a key on its own machine and hands over the public entry.
            signers = []
            for signer_id, role in (("release-lead-1", "release_lead"), ("independent-builder-2", "independent_builder"),
                                    ("security-custodian-3", "security_custodian")):
                key_file = tmp / f"{signer_id}.key.json"
                entry = json.loads(run("release-key-create", "--signer-id", signer_id, "--role", role,
                                       "--out-file", str(key_file)).stdout)
                self.assertEqual(oct(key_file.stat().st_mode & 0o777), "0o600")
                self.assertNotIn("private_key_hex", entry)
                signers.append(entry)
                # A key is never overwritten.
                self.assertNotEqual(run("release-key-create", "--signer-id", signer_id, "--role", role,
                                        "--out-file", str(key_file), ok=False).returncode, 0)
            keyset = {"schema": "postfiat.release_keyset.v1", "keyset_id": "postfiat-release-keyset-test-v1",
                      "threshold": 2, "total_signers": 3, "algorithm_id": "ML-DSA-65",
                      "signing_context": "postfiat-l1-v2/release-manifest/v3", "signers": signers}
            keyset_file = tmp / "release-keyset.json"
            keyset_file.write_text(json.dumps(keyset))

            # 2. The release directory: the two binaries as CI built them, then the rest.
            release = tmp / "release"
            release.mkdir()
            # One of them is the node built here. The other stands in for the
            # other runner's build: the same bytes with the ELF machine of its name.
            built = Path(node).read_bytes()
            machines = {"linux-x86_64": 62, "linux-aarch64": 183}
            for arch, machine in machines.items():
                target = release / f"postfiat-node-{arch}"
                target.write_bytes(built[:18] + machine.to_bytes(2, "little") + built[20:])
                target.chmod(0o755)
            # A binary under the wrong name stops the assembly.
            swapped = tmp / "swapped"
            swapped.mkdir()
            for arch in machines:
                shutil.copy2(node, swapped / f"postfiat-node-{arch}")
            refused = subprocess.run([str(REPO / "scripts/release-assemble"), str(swapped)], capture_output=True, text=True)
            self.assertEqual(refused.returncode, 1, refused.stderr)
            self.assertIn("built for ELF machine", refused.stderr)
            self.assertFalse((swapped / "SHA256SUMS").exists())
            subprocess.run([str(REPO / "scripts/release-assemble"), str(release)], check=True, capture_output=True)
            sums = (release / "SHA256SUMS").read_text()
            for name in ("postfiat-units.tar", "postfiat-cargo-sbom.cdx.json", "postfiat-node-linux-aarch64"):
                self.assertIn(name, sums)
            info = json.loads(run("release-build-info").stdout)
            self.assertIn(info["binary_sha256"], sums, "the binary that ran is the binary in SHA256SUMS")

            # 3. The release lead builds the unsigned manifest.
            metadata = tmp / "circuit.metadata.json"
            metadata.write_text("{}")
            manifest_file = tmp / "release-manifest.json"
            create = ["release-manifest-create", "--keyset-file", str(keyset_file), "--release-id", "v0.0.1-test",
                      "--release-sequence", "1", "--chain-id", "postfiat-release-flow-test", "--genesis-hash", "aa" * 48,
                      "--git-revision", "11" * 20, "--build-features", "privacy,rpc,transport", "--protocol-version", "1",
                      "--min-compatible-protocol-version", "1", "--checkpoint-height", "10",
                      "--checkpoint-block-hash", "bb" * 48, "--checkpoint-state-root", "cc" * 48,
                      "--checkpoint-certificate-id", "dd" * 48, "--checkpoint-committee-epoch", "1",
                      "--url-base", "https://releases.example.org",
                      "--binary-x86-64", str(release / "postfiat-node-linux-x86_64"),
                      "--binary-aarch64", str(release / "postfiat-node-linux-aarch64"),
                      "--systemd-bundle", str(release / "postfiat-units.tar"),
                      "--docker-profiles", str(release / "postfiat-docker-profiles.tar"),
                      "--sbom", str(release / "postfiat-cargo-sbom.cdx.json"), "--checksums", str(release / "SHA256SUMS"),
                      "--swap-circuit-metadata", str(metadata), "--private-egress-circuit-metadata", str(metadata),
                      "--output", str(manifest_file)]
            run(*create)
            manifest = json.loads(manifest_file.read_text())
            self.assertEqual((manifest["release_sequence"], manifest["signatures"]), (1, []))
            self.assertEqual(manifest["artifacts"]["linux-x86_64"]["binary_sha256"], info["binary_sha256"])
            self.assertNotEqual(run(*create, ok=False).returncode, 0, "a manifest is never overwritten")

            verify = ["release-manifest-verify", "--manifest-file", str(manifest_file), "--keyset-file", str(keyset_file),
                      "--runtime-binary-file", str(release / "postfiat-node-linux-x86_64"), "--arch", "linux-x86_64"]
            # 4. Unsigned, and with one signature, it is not a release.
            self.assertNotEqual(run(*verify, ok=False).returncode, 0)
            run("release-manifest-sign", "--manifest-file", str(manifest_file), "--key-file", str(tmp / "release-lead-1.key.json"))
            self.assertNotEqual(run(*verify, ok=False).returncode, 0)
            self.assertNotEqual(run("release-manifest-sign", "--manifest-file", str(manifest_file),
                                    "--key-file", str(tmp / "release-lead-1.key.json"), ok=False).returncode, 0,
                                "one signer cannot sign twice")
            # 5. With the second signature it is.
            run("release-manifest-sign", "--manifest-file", str(manifest_file),
                "--key-file", str(tmp / "security-custodian-3.key.json"))
            verified = json.loads(run(*verify).stdout)
            self.assertEqual(verified["verified_signers"], ["release-lead-1", "security-custodian-3"])
            self.assertEqual((verified["release_id"], verified["release_sequence"]), ("v0.0.1-test", 1))

            # The check for a first install, which runs nothing from the release:
            # an independent implementation reads the same signatures the same way.
            first_install = [sys.executable, str(REPO / "scripts/verify-release"), "--manifest-file", str(manifest_file),
                             "--keyset-file", str(keyset_file)]
            for arch in ("linux-x86_64", "linux-aarch64"):
                independent = subprocess.run([*first_install, "--binary-file", str(release / f"postfiat-node-{arch}"),
                                              "--arch", arch], capture_output=True, text=True)
                self.assertEqual(independent.returncode, 0, independent.stderr)
                self.assertEqual(json.loads(independent.stdout)["verified_signers"], verified["verified_signers"])
            wrong = subprocess.run([*first_install, "--binary-file", str(release / "postfiat-node-linux-aarch64"),
                                    "--arch", "linux-x86_64"], capture_output=True, text=True)
            self.assertEqual(wrong.returncode, 1)
            one_signature = tmp / "one-signature.json"
            signed_manifest = json.loads(manifest_file.read_text())
            one_signature.write_text(json.dumps({**signed_manifest, "signatures": signed_manifest["signatures"][:1]}))
            too_few = subprocess.run([sys.executable, str(REPO / "scripts/verify-release"), "--manifest-file", str(one_signature),
                                      "--keyset-file", str(keyset_file), "--binary-file",
                                      str(release / "postfiat-node-linux-x86_64"), "--arch", "linux-x86_64"],
                                     capture_output=True, text=True)
            self.assertEqual(too_few.returncode, 1)
            self.assertIn("the keyset needs 2", too_few.stderr)
            changed = tmp / "changed.json"
            changed.write_text(json.dumps({**signed_manifest, "release_sequence": 2}))
            tampered = subprocess.run([sys.executable, str(REPO / "scripts/verify-release"), "--manifest-file", str(changed),
                                       "--keyset-file", str(keyset_file), "--binary-file",
                                       str(release / "postfiat-node-linux-x86_64"), "--arch", "linux-x86_64"],
                                      capture_output=True, text=True)
            self.assertEqual(tampered.returncode, 1)
            self.assertIn("does not verify", tampered.stderr)

            # 6. Another binary than the one the manifest names is refused at start.
            other = tmp / "other-binary"
            other.write_bytes(b"not the release binary")
            refused = run(*verify[:6], "--runtime-binary-file", str(other), "--arch", "linux-x86_64", ok=False)
            self.assertNotEqual(refused.returncode, 0)
            # A changed manifest is refused too.
            manifest = json.loads(manifest_file.read_text())
            manifest["release_sequence"] = 2
            manifest_file.write_text(json.dumps(manifest))
            self.assertNotEqual(run(*verify, ok=False).returncode, 0)


if __name__ == "__main__":
    unittest.main()
