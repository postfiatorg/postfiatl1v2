"""Retain public/encrypted demo outputs before completion-triggered cleanup.

Run periodically on the operator host. SSH identity and paths come from the
private run directory. Neither raw replay data nor AWS credentials are fetched.
"""
import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import shlex
import subprocess

from qualification_infra import write_json


def allowed_output(name):
    parts = PurePosixPath(name).parts
    if not parts or str(PurePosixPath(name)) != name or any(p in (".", "..") for p in parts):
        return False
    if len(parts) == 1:
        return name in {"recipient.json", "recipient-accepted.json", "launch.json", "launch.stderr",
            "result.json", "timing.json", "terminate.log"} or (
            name.startswith("event-") and name.endswith(".json") and name[6:-5].isdigit())
    if len(parts) != 2:
        return False
    if parts[0] in {"MU", "NVDA"}:
        return parts[1] in {"target-ready.json", "result.json", "proof.bin", "proof-calldata.bin", "public-values.bin"}
    return (parts[0] == "archive" and parts[1].endswith(".ciphertext") and
        len(parts[1]) == 75 and all(c in "0123456789abcdef" for c in parts[1][:-11]))


def terminal(report):
    service = report["service"]
    if service["ActiveState"] not in {"inactive", "failed"}:
        return False
    if service["LoadState"] == "loaded":
        return True
    if service["LoadState"] != "not-found":
        return False
    # systemd can collect a completed transient unit between polling intervals.
    # Distinguish that from a misspelled/unstarted unit. A short service can also
    # finish before our first poll; its complete output and teardown record then
    # supply the terminal evidence. All listed files are retained before use.
    return report.get("servicePreviouslyObserved") is True or (
        report.get("lastEvent") == "postfiat.yolo.demo_complete.v1"
        and report.get("status") in {"VERIFIED", "FAILED"}
        and report.get("lastStatus") == report["status"]
        and {"result.json", "timing.json", "terminate.log"} <= report.get("files", {}).keys())


def deliver(directory):
    if (directory / "completion.json").exists():
        return
    config_path = directory / "delivery.json"
    if not config_path.exists():
        return
    config = json.loads(config_path.read_bytes())
    local_output = config.get("localOutput", "output")
    if (not isinstance(local_output, str) or not local_output or local_output in {".", ".."}
            or any(c not in "abcdefghijklmnopqrstuvwxyz0123456789-_" for c in local_output)):
        raise ValueError("local output must be a distinct run-directory child")
    plan = json.loads((directory / "plan.json").read_bytes())
    ssh = json.loads((directory / "ssh-command.json").read_bytes())
    remote = '''
import hashlib,json,subprocess
from pathlib import Path
root=Path(ROOT)
service=dict(line.split('=',1) for line in subprocess.check_output(
 ['systemctl','show',SERVICE,'-p','LoadState','-p','ActiveState','-p','SubState','-p','Result'],text=True).splitlines())
files={}
for p in root.rglob('*'):
 if p.is_file() and not p.is_symlink():
  h=hashlib.sha256()
  with p.open('rb') as f:
   while block:=f.read(1024*1024):h.update(block)
  files[str(p.relative_to(root))]={'bytes':p.stat().st_size,'sha256':h.hexdigest()}
events=sorted(root.glob('event-*.json'),key=lambda p:int(p.stem[6:]))
last=json.loads(events[-1].read_bytes()) if events else {}
complete=json.loads((root/'result.json').read_bytes()) if (root/'result.json').exists() else {}
print(json.dumps({'service':service,'files':files,'lastEvent':last.get('schema'),
 'lastStatus':last.get('status'),'lastSymbol':last.get('symbol'),'status':complete.get('status')}))
'''.replace("ROOT", repr(config["remoteRoot"])).replace("SERVICE", repr(config["service"]))
    result = subprocess.run(ssh + ["python3 -c " + shlex.quote(remote)], stdout=subprocess.PIPE, check=True, timeout=60)
    report = json.loads(result.stdout)
    output = directory / local_output
    output.mkdir(mode=0o700, exist_ok=True)
    for name, pin in report["files"].items():
        if not allowed_output(name) or not 0 <= pin["bytes"] <= 256 * 1024**2:
            raise ValueError("unexpected output path or size")
        path = output / name
        if path.exists() and path.stat().st_size == pin["bytes"] and hashlib.sha256(path.read_bytes()).hexdigest() == pin["sha256"]:
            continue
        path.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
        temp = path.with_name(path.name + ".download")
        subprocess.run(["scp", "-q"] + ssh[1:-1] + [ssh[-1] + ":" + config["remoteRoot"] + "/" + name, str(temp)], check=True, timeout=120)
        if temp.stat().st_size != pin["bytes"] or hashlib.sha256(temp.read_bytes()).hexdigest() != pin["sha256"]:
            raise ValueError("output changed during transfer; retry delivery")
        temp.replace(path)
    identity = {"service": config["service"], "remoteRoot": config["remoteRoot"],
        "localOutput": local_output, "parent": ssh[-1]}
    try:
        previous = json.loads((directory / "delivery-status.json").read_bytes())
    except (FileNotFoundError, json.JSONDecodeError):
        previous = {}
    previously_observed = (isinstance(previous, dict) and previous.get("serviceIdentity") == identity
        and previous.get("servicePreviouslyObserved") is True)
    report.update(at=datetime.now(timezone.utc).isoformat(), runId=plan["runId"],
        serviceIdentity=identity, servicePreviouslyObserved=(previously_observed
            or report["service"]["LoadState"] == "loaded"))
    write_json(directory / "delivery-status.json", report)
    if terminal(report) and report["files"]:
        write_json(directory / "completion.json", {"runId": plan["runId"], "outputsRetained": True,
            "status": report["status"] or "FAILED", "at": report["at"], "files": report["files"]})
    print(json.dumps({k: report[k] for k in ("at", "service", "lastEvent", "lastStatus", "lastSymbol", "status")}), flush=True)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--directory", type=Path, required=True)
    args = parser.parse_args()
    os.umask(0o077)
    deliver(args.directory)
