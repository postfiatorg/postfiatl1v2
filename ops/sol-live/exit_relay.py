#!/usr/bin/python3
"""Operate the reviewed relay using the saved release and unit paths."""

import argparse
import json
import os
from pathlib import Path
import re
import secrets
import subprocess
import tempfile
import time
from urllib.parse import urlsplit

from exit_cutover import relay

CONFIG = Path.home() / ".config/postfiat/sol-exit-relay.json"
ROUTE = "/api/l1/bridge/solana/withdrawals"


def atomic_write(path, data):
    with tempfile.NamedTemporaryFile(mode="w", dir=path.parent, delete=False) as f:
        f.write(data)
        f.flush()
        os.fsync(f.fileno())
    os.replace(f.name, path)


def http(cfg, path, *, body=None):
    cmd = [
        "curl",
        "--disable",
        "-sS",
        "--max-time",
        "60",
        "--cookie",
        cfg["cookie_file"],
        "--write-out",
        "\n%{http_code}",
    ]
    if body is not None:
        cmd += [
            "--header",
            "Origin: " + cfg["gateway"],
            "--header",
            "Sec-Fetch-Site: same-origin",
            "--header",
            "Content-Type: application/json",
            "--data-binary",
            json.dumps(body),
        ]
    data, status = subprocess.check_output(
        cmd + [cfg["gateway"] + path], text=True
    ).rsplit("\n", 1)
    return int(status), json.loads(data)


def relay_request_seen(unit, probe, since):
    message = "sol-exit-disabled-probe " + probe
    for _ in range(10):
        result = subprocess.run(
            [
                "journalctl",
                "--user",
                "-u",
                unit,
                "--no-pager",
                "-o",
                "cat",
                "--since",
                f"@{since}",
                "--grep",
                "^" + message + "$",
            ],
            capture_output=True,
            text=True,
        )
        if result.returncode == 0 and message in result.stdout.splitlines():
            return True
        time.sleep(0.5)
    return False


def verify_disabled(cfg):
    code, health = http(cfg, "/health")
    if (
        code != 200
        or health.get("ok") is not True
        or health.get("buildId") != cfg["build"]
    ):
        raise ValueError("Gateway build check failed.")
    status, ready = http(cfg, ROUTE + "/readiness")
    if (
        status != 200
        or ready.get("ready") is not False
        or ready.get("ok") is not True
        or ready.get("message") != "Solana withdrawals are not available yet"
    ):
        raise ValueError("Unexpected public readiness response.")
    probe = "0" * 16 + secrets.token_hex(40)
    since = int(time.time())
    status, job = http(cfg, ROUTE + "/" + probe)
    if (
        status != 503
        or job.get("ok") is not False
        or job.get("code") != "solana_withdrawals_unavailable"
    ):
        raise ValueError("Unexpected disabled job response.")
    if not relay_request_seen(cfg["unit"], probe, since):
        raise ValueError("No matching disabled request on the relay.")
    print("Disabled probe confirmed on gateway and relay", probe)


def verify_paid(cfg, burn, signature):
    if not re.fullmatch("[0-9a-f]{96}", burn) or not signature:
        raise ValueError("Missing paid canary identity.")
    status, ready = http(cfg, ROUTE + "/readiness")
    if status != 200 or ready.get("ok") is not True or ready.get("ready") is not True:
        raise ValueError("Public readiness failed.")

    def check(response):
        code, job = response
        if (
            code != 200
            or job.get("stage") != "paid"
            or job.get("burn_tx_id") != burn
            or job.get("solana_tx") != signature
        ):
            raise ValueError("Paid canary or original signature differs.")

    check(http(cfg, ROUTE + "/" + burn))
    check(http(cfg, ROUTE, body={"burn_tx_id": burn}))
    print("Paid GET and same-origin POST matched", burn, signature)


def set_flag(cfg, enabled):
    value = "true" if enabled else "false"
    atomic_write(
        Path(cfg["flag_file"]),
        f"L1_SOLANA_WITHDRAWALS_ENABLED={value}\n"
        "L1_SOLANA_WITHDRAW_SERVICE_URL=http://127.0.0.1:18089\n",
    )


def require_supported_node(node):
    version = subprocess.check_output([node, "--version"], text=True).strip()
    match = re.fullmatch(r"v24\.(\d+)\.\d+", version)
    if not match or int(match[1]) < 21:
        raise ValueError(f"Unsupported relay Node version {version}.")


def verify_process(cfg, enabled):
    require_supported_node(cfg["node"])
    relay(
        cfg["unit"],
        True,
        release=Path(cfg["release"]),
        enabled=enabled,
        node=Path(cfg["node"]),
        probe_module=Path(cfg["probe_module"]),
    )


def disable(cfg):
    set_flag(cfg, False)
    subprocess.run(["systemctl", "--user", "restart", cfg["unit"]], check=True)
    verify_process(cfg, False)
    verify_disabled(cfg)


def main():
    p = argparse.ArgumentParser(description=__doc__)
    sub = p.add_subparsers(dest="action")
    setup = sub.add_parser("configure")
    for opt in (
        "unit",
        "release",
        "node",
        "probe_module",
        "flag_file",
        "gateway",
        "build",
        "cookie_file",
    ):
        setup.add_argument("--" + opt.replace("_", "-"), required=True)
    for action in ("disable", "verify-disabled"):
        sub.add_parser(action)
    sub.add_parser("enable").add_argument("paid", type=Path)
    a = p.parse_args()
    if a.action == "configure":
        cfg = vars(a).copy()
        cfg.pop("action")
        url = urlsplit(cfg["gateway"])
        if (
            url.scheme != "https"
            or not url.hostname
            or url.username
            or cfg["gateway"] != "https://" + url.netloc
        ):
            raise ValueError("Gateway must be the browser's HTTPS origin.")
        for k in ("release", "node", "probe_module", "flag_file", "cookie_file"):
            cfg[k] = str(Path(cfg[k]).resolve())
        require_supported_node(cfg["node"])
        CONFIG.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
        set_flag(cfg, False)
        atomic_write(CONFIG, json.dumps(cfg))
        return
    cfg = json.loads(CONFIG.read_text())
    if a.action == "verify-disabled":
        verify_process(cfg, False)
        verify_disabled(cfg)
    elif a.action == "enable":
        paid = json.loads(a.paid.read_text())
        verify_process(cfg, False)
        verify_disabled(cfg)
        try:
            set_flag(cfg, True)
            subprocess.run(["systemctl", "--user", "restart", cfg["unit"]], check=True)
            verify_process(cfg, True)
            verify_paid(cfg, paid["burn_tx_id"], paid["solana_tx"])
        except BaseException:
            disable(cfg)
            raise
    else:
        disable(cfg)


if __name__ == "__main__":
    main()
