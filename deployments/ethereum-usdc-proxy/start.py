#!/usr/bin/env python3
"""Launch the proxy from this release with the operator's runtime JSON."""
import json
import os
from pathlib import Path
import stat
import sys

def read_settings(path):
    config = Path(path)
    info = config.lstat()
    if not stat.S_ISREG(info.st_mode) or info.st_uid != os.getuid() or info.st_mode & 0o077:
        sys.exit("settings must be a private regular file owned by the service user")
    settings = json.loads(config.read_text())
    if not isinstance(settings, dict) or not all(isinstance(v, str) for v in settings.values()):
        sys.exit("settings must be a JSON object with string values")
    return settings


if len(sys.argv) != 4:
    sys.exit("usage: start.py BASE_RUNTIME_JSON RELEASE_JSON ABSOLUTE_NODE_EXECUTABLE")
node = Path(sys.argv[3])
if not node.is_absolute():
    sys.exit("supply an absolute Node executable")
release = read_settings(sys.argv[2])
if release.keys() != {
    "TRUSTLESS_BRIDGE_ROUTES_JSON_FILE", "B23_INGRESS_NODE_VERIFY_BIN",
    "B23_INGRESS_NODE_VERIFY_SHA256", "PFTL_NODE_BIN", "PFTL_TOPOLOGY",
    "B23_PFTL_NODE_SHA256",
}:
    sys.exit("release JSON must specify the routes, local verifier and remote node release")
env = {key: os.environ[key] for key in
       ("HOME", "USER", "LANG", "PATH", "SSL_CERT_FILE", "SSL_CERT_DIR") if key in os.environ}
env.update(read_settings(sys.argv[1]))
env.update(release)
env.pop("NODE_PATH", None)
os.chdir(Path(__file__).resolve().parents[2])
os.execve(node, [str(node), "wallet-proxy/server.js"], env)
