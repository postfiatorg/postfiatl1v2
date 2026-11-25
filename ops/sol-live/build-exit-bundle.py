"""As the orchestrator, package the committed runtime and reviewed public release inputs."""
import argparse
import hashlib
import io
import os
from pathlib import Path
import re
import subprocess
import tarfile
import tempfile

OPS = Path(__file__).resolve().parent
REPO = OPS.parents[1]
RUNTIME = """
ops/sol-live/exit_setup.py ops/sol-live/exit_payout.py ops/sol-live/exit_precheck.py
ops/sol-live/exit_proof.py ops/sol-live/exit_segments.py ops/sol-live/exit_service.py ops/sol-live/exit_qualification_socket.py
ops/sol-live/exit_worker_policy.py ops/sol-live/exit_sandbox_check.py ops/sol-live/exit_keys.py ops/sol-live/exit_rebuild.py
ops/sol-live/live.py ops/sol-live/exit_rules.py ops/sol-live/profile.py ops/sol-live/check-prover-host.sh
ops/sol-live/check-prover-identity.sh ops/sol-live/exit-prover.fingerprint
ops/sol-live/fixtures/exit-setup.json ops/sol-live/fixtures/exit-checkpoint.json
ops/sol-live/pft-sol-mainnet-withdraw.service ops/sol-live/pft-sol-exit-self-test.service
ops/sol-live/exit-requirements.lock solana/deploy/bridge.py solana/deploy/keys.py
solana/deploy/route_binding.py solana/pfusdc-egress-verifier/bootstrap.py
""".split()


def git(*args):
    return subprocess.check_output(["git", "-C", REPO, *args])


def add(tar, name, data, mode=0o644):
    info = tarfile.TarInfo(name)
    info.size = len(data)
    info.mode = mode
    tar.addfile(info, io.BytesIO(data))


def write_bundle(args, stream):
    revision = git("rev-parse", "HEAD").decode().strip()
    def committed(path):
        return git("show", f"{revision}:{path}")
    with tarfile.open(fileobj=stream, mode="w") as tar:
        add(tar, "revision", (revision + "\n").encode())
        for name in ("install-exit-system.sh", "exit_install_credential.py"):
            add(tar, name, committed("ops/sol-live/" + name))
        for path in RUNTIME:
            add(tar, "runtime/" + path, committed(path))
        for name in ("anchor-witness.json", "roles.json"):
            add(tar, "runtime/" + name, (args.kit / "evidence/solana" / name).read_bytes())
        add(tar, "known_hosts", args.trust.read_bytes())
        for name, digest in [
            ("bin/proof-host", "1fa81b51967ccf72e91ce2edde135968abd519395047fd1f304b9d5aec25a600"),
            ("historical/egress.elf", "4d5f84493c9b02b0d2a082c446229e30ce6645210a00c271dfb125b2761c67e0"),
        ]:
            data = (args.proof / name).read_bytes()
            if hashlib.sha256(data).hexdigest() != digest:
                raise ValueError(f"Wrong frozen artifact {name}")
            add(tar, "runtime/proof/" + name, data, 0o755 if name.startswith("bin/") else 0o644)
        add(tar, "runtime/proof/bin/exit-verify-proof", args.verifier.read_bytes(), 0o755)
        lock = committed("ops/sol-live/exit-requirements.lock").decode().splitlines()
        for line in lock:
            package, version, digest = re.fullmatch(r"(\w+)==([\w.]+) --hash=sha256:([0-9a-f]{64})", line).groups()
            matches = list(args.wheels.glob(f"{package}-{version}-*.whl"))
            if len(matches) != 1:
                raise ValueError(f"Expected exactly one wheel for {package}")
            data = matches[0].read_bytes()
            if hashlib.sha256(data).hexdigest() != digest:
                raise ValueError(f"Wrong wheel digest for {package}")
            add(tar, "wheels/" + matches[0].name, data)


def build(args):
    # Publish only a complete tar, without replacing an earlier release bundle.
    with tempfile.NamedTemporaryFile(dir=args.output.parent) as stream:
        write_bundle(args, stream)
        stream.flush()
        os.fsync(stream.fileno())
        os.link(stream.name, args.output)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("kit", "wheels", "proof", "verifier", "trust", "output"):
        parser.add_argument("--" + name, type=Path, required=True)
    args = parser.parse_args()
    os.umask(0o077)
    build(args)
