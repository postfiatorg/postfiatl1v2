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
            native = Path(node)
            with native.open("rb") as stream:
                header = bytearray(stream.read(64))
            arches = {"linux-x86_64": 62, "linux-aarch64": 183}
            native_arch = next(arch for arch, machine in arches.items() if machine == int.from_bytes(header[18:20], "little"))
            foreign_arch = next(arch for arch in arches if arch != native_arch)
            shutil.copy2(native, release / f"postfiat-node-{native_arch}")
            foreign = release / f"postfiat-node-{foreign_arch}"
            foreign.write_bytes(header)
            assemble = [str(REPO / "scripts/release-assemble"), str(release)]
            refused = subprocess.run(assemble, capture_output=True)
            self.assertNotEqual(refused.returncode, 0)
            self.assertIn(b"wrong ELF architecture", refused.stderr)
            # Synthetic foreign ELF header; native execution is tested by each architecture's CI job.
            header[18:20] = arches[foreign_arch].to_bytes(2, "little")
            foreign.write_bytes(header)
            subprocess.run(assemble, check=True, capture_output=True)
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
            self.assertEqual(manifest["artifacts"][native_arch]["binary_sha256"], info["binary_sha256"])
            self.assertNotEqual(run(*create, ok=False).returncode, 0, "a manifest is never overwritten")

            verify = ["release-manifest-verify", "--manifest-file", str(manifest_file), "--keyset-file", str(keyset_file),
                      "--runtime-binary-file", str(release / f"postfiat-node-{native_arch}"), "--arch", native_arch]
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

            # 6. Another binary than the one the manifest names is refused at start.
            other = tmp / "other-binary"
            other.write_bytes(b"not the release binary")
            refused = run(*verify[:6], "--runtime-binary-file", str(other), "--arch", native_arch, ok=False)
            self.assertNotEqual(refused.returncode, 0)
            # A changed manifest is refused too.
            manifest = json.loads(manifest_file.read_text())
            manifest["release_sequence"] = 2
            manifest_file.write_text(json.dumps(manifest))
            self.assertNotEqual(run(*verify, ok=False).returncode, 0)


if __name__ == "__main__":
    unittest.main()
