#!/usr/bin/env python3
"""KEY-HOLDER ONLY: sign the reviewed release inputs using the existing publisher."""
import argparse
import json
from pathlib import Path
import subprocess
import sys

packet = Path(__file__).resolve().parent
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--stage", type=Path, required=True)
parser.add_argument("--publisher-key-file", type=Path, required=True)
parser.add_argument("--valid-from-unix", type=int, required=True)
parser.add_argument("--valid-until-unix", type=int, required=True)
args = parser.parse_args()
stage = args.stage.resolve()
data = json.loads((packet / "manifest-input.unsigned.json").read_text())
release = data["deployment_id"]
config = stage / "rootfs/etc/postfiat/releases" / release
binary = stage / "rootfs/opt/postfiat/releases" / release / "postfiat-node"
output = config / "deployment-manifest.json"
subprocess.run([sys.executable, str(packet / "local-preflight.py"), "--stage", str(stage)], check=True)
command = [
    str(binary), "deployment-manifest-create",
    "--deployment-id", release,
    "--valid-from-unix", str(args.valid_from_unix),
    "--valid-until-unix", str(args.valid_until_unix),
    "--chain-id", data["chain_id"], "--genesis-hash", data["genesis_hash"],
    "--git-revision", data["git_revision"], "--binary-file", str(binary),
    "--build-profile", data["build_profile"],
    "--build-features", ",".join(data["build_features"]),
    "--protocol-version", str(data["protocol_version"]), "--rpc-schema", data["rpc_schema"],
    "--service-unit-file", str(stage / "operator/postfiat-release-operator.service"),
    "--environment-file", str(stage / "operator/operator.env"),
    "--validator-bindings-file", str(stage / "validator-bindings.signing.json"),
    "--topology-file", str(config / "topology.json"),
    "--swap-circuit-metadata-file", str(config / "swap.metadata.json"),
    "--private-egress-circuit-metadata-file", str(config / "private-egress.metadata.json"),
    "--publisher-key-file", str(args.publisher_key_file.resolve()),
    "--manifest-file", str(output),
]
subprocess.run(command, check=True, stdout=subprocess.DEVNULL)
subprocess.run([sys.executable, str(packet / "local-preflight.py"), "--stage", str(stage),
                "--require-signed"], check=True)
print(f"Signed manifest: {output}")
