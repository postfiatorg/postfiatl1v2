"""Use the upgraded recorder writer with the installed cluster's capture and retention code."""

import runpy
import sys
from pathlib import Path

from recorder_release import IMAGE, RELEASE
import bridge
import recorder  # Keep the upgraded writer loaded when local-setup imports it.
from hashlib import sha256

# Keep the installed vault and egress checks. Deploy only the recorder from the
# pinned release, even when the other images have different local verifier keys.
original_deploy = bridge.deploy


def deploy_checked(cluster, name, key):
    if name != "recorder":
        return original_deploy(cluster, name, key)
    if sha256((RELEASE / "pfusdc_recorder.so").read_bytes()).hexdigest() != IMAGE:
        raise RuntimeError("unexpected recorder image")
    original = bridge.IMAGES
    try:
        bridge.IMAGES = RELEASE
        return original_deploy(cluster, name, key)
    finally:
        bridge.IMAGES = original


bridge.deploy = deploy_checked
installed = Path.cwd() / "ops/sol-live"
sys.path.insert(0, str(installed))
runpy.run_path(str(installed / "local-setup.py"), run_name="__main__")
