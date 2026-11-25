"""Create a clean, pinned build context and image for the YOLO enclave."""

import argparse
import hashlib
import json
from pathlib import Path
import subprocess


SOURCE_ROOT = "tools/nav-reserve-proof/schwab-tee/"


MODULES = (
    "attestation", "canonical", "methodology", "models", "nitro_archive_transport",
    "nitro_attestation", "nitro_cms", "nitro_entrypoint", "nitro_kms",
    "nitro_kms_transport", "nitro_oauth", "nitro_run", "nitro_runtime", "nitro_transport",
    "private_archive", "schwab_chain", "schwab_methodology", "tee_policy",
)


def git(repo, *args):
    return subprocess.check_output(["git", "-C", str(repo), *args])


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repository", type=Path, required=True)
    parser.add_argument("--revision", required=True, help="Committed source revision; no working-tree files are copied.")
    parser.add_argument("--nsm-library", type=Path, required=True)
    parser.add_argument("--root-der", type=Path, required=True)
    parser.add_argument("--output-directory", type=Path, required=True, help="New local output directory.")
    parser.add_argument("--image-tag", required=True, help="New local Docker image tag; never pushed.")
    parser.add_argument("--nav-manifest", type=Path, help="Build the account-value collector with its stable measured policy")
    parser.add_argument("--prepare-only", action="store_true", help="Write pinned build context without invoking Docker")
    args = parser.parse_args(argv)
    revision = git(args.repository, "rev-parse", "--verify", args.revision + "^{commit}").decode().strip()
    def source(path): return git(args.repository, "show", revision + ":" + path)
    recipe = SOURCE_ROOT + "enclave/"
    pins = json.loads(source(recipe + ("nav-build-inputs.json" if args.nav_manifest else "build-inputs.json")))
    # The measured image holds the reader's files and these pins only. It holds neither the
    # commit nor the commit's time, so a commit elsewhere in the repository leaves the PCRs as they are.
    source_epoch = pins["sourceDateEpoch"]
    if type(source_epoch) is not int or source_epoch <= 0:
        raise ValueError("build pin sourceDateEpoch must be a positive integer")
    binary = args.nsm_library.read_bytes()
    root_der = args.root_der.read_bytes()
    if hashlib.sha256(binary).hexdigest() != pins["nsmLibrarySha256"]:
        raise ValueError("NSM library differs from the committed build pin")
    if hashlib.sha256(root_der).hexdigest() != pins["nitroRootDerSha256"]:
        raise ValueError("Nitro root differs from the committed build pin")
    if not args.prepare_only and subprocess.run(["docker", "image", "inspect", args.image_tag], capture_output=True).returncode == 0:
        raise ValueError("image tag already exists; choose a new qualification output")
    args.output_directory.mkdir(parents=True, exist_ok=False)
    context = args.output_directory / "context"
    context.mkdir()
    for name in ("Dockerfile", "assemble.py", "requirements.lock"):
        (context / name).write_bytes(source(recipe + name))
    for parent in ("pftl_schwab_tee",):
        package = context / "source" / parent
        package.mkdir(parents=True, exist_ok=True)
        (package / "__init__.py").write_text('"""Measured YOLO enclave package; explicit module imports only."""\n')
    hashes = {}
    modules = MODULES + (("nitro_capture", "nav_reading", "nav_reading_entrypoint") if args.nav_manifest else ())
    for module in modules:
        path = "pftl_schwab_tee/" + module + ".py"
        data = source(SOURCE_ROOT + path)
        (context / "source" / path).write_bytes(data)
        hashes[path] = hashlib.sha256(data).hexdigest()
    if args.nav_manifest:
        from .nav_reading_entrypoint import measured_policy
        policy = measured_policy(json.loads(args.nav_manifest.read_bytes()))
        policy_bytes = json.dumps(policy, sort_keys=True, separators=(",", ":")).encode()
        (context / "nav-policy.json").write_bytes(policy_bytes)
        hashes["nav-policy.json"] = hashlib.sha256(policy_bytes).hexdigest()
        docker = (context / "Dockerfile").read_text().replace(
            "COPY assemble.py /build/assemble.py", "COPY nav-policy.json /build/nav-policy.json\nCOPY assemble.py /build/assemble.py").replace(
            "pftl_schwab_tee.nitro_entrypoint\"", "pftl_schwab_tee.nav_reading_entrypoint\"")
        (context / "Dockerfile").write_text(docker)
        assemble = (context / "assemble.py").read_text().replace(
            '"aws-nitro-root.sha256", "build-inputs.json")', '"aws-nitro-root.sha256", "build-inputs.json", "nav-policy.json")')
        (context / "assemble.py").write_text(assemble)
    (context / "libnsm.so").write_bytes(binary)
    (context / "aws-nitro-root.der").write_bytes(root_der)
    (context / "aws-nitro-root.sha256").write_text(pins["nitroRootDerSha256"] + "\n")
    manifest = {**pins, "modules": hashes,
                "requirementsLockSha256": hashlib.sha256((context / "requirements.lock").read_bytes()).hexdigest()}
    (context / "build-inputs.json").write_text(json.dumps(manifest, sort_keys=True, indent=2) + "\n")
    command = ["docker", "build", "--no-cache", "--progress=plain", "--build-arg",
               f"SOURCE_DATE_EPOCH={source_epoch}", "--tag", args.image_tag, str(context)]
    (args.output_directory / "command.json").write_text(json.dumps({"runId": pins["runId"], "command": command}, indent=2) + "\n")
    if args.prepare_only:
        print(json.dumps({"contextPrepared": True, "sourceRevision": revision, "dockerExecuted": False}))
        return 0
    log = args.output_directory / "docker-build.log"
    print(f"Building {args.image_tag}; full build output: {log}", flush=True)
    with log.open("xb") as output:
        result = subprocess.run(command, stdout=output, stderr=subprocess.STDOUT)
    if result.returncode:
        raise RuntimeError(f"Docker build failed with exit {result.returncode}; see {log}")
    identity = json.loads(subprocess.check_output(["docker", "image", "inspect", args.image_tag]))[0]
    report = {"runId": pins["runId"], "navSourceRevision": revision, "imageTag": args.image_tag,
              "imageId": identity["Id"], "imageBytes": identity["Size"],
              "buildManifestSha256": hashlib.sha256((context / "build-inputs.json").read_bytes()).hexdigest()}
    (args.output_directory / "image.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
