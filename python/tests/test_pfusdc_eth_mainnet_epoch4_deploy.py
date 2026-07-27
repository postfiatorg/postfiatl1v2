"""Offline regression tests for the active epoch-4 mainnet deployment driver."""
from __future__ import annotations

import importlib.util
import sys
import unittest
from pathlib import Path


REPO = Path(__file__).resolve().parents[2]
SCRIPT = REPO / "scripts/pfusdc-eth-mainnet-epoch4-deploy.py"
SPEC = importlib.util.spec_from_file_location("pfusdc_eth_mainnet_epoch4_deploy", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
DEPLOY = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = DEPLOY
SPEC.loader.exec_module(DEPLOY)


class MainnetEpoch4DeployTests(unittest.TestCase):
    def test_active_manifest_and_artifacts_validate_offline(self):
        manifest = DEPLOY.D._load_json(DEPLOY.MANIFEST)
        artifacts = DEPLOY.validate_offline_manifest(DEPLOY.MANIFEST, manifest)
        self.assertEqual(
            set(artifacts),
            {"PFTLFinalityVerifierV1", "ERC20BridgeVaultL1"},
        )

    def test_epoch3_manifest_is_not_accepted_by_epoch4_driver(self):
        epoch3 = REPO / "deployments/pfusdc-eth-mainnet-20260726/manifest.mainnet-epoch3.json"
        with self.assertRaises(DEPLOY.D.DeploymentError):
            DEPLOY.validate_offline_manifest(epoch3, DEPLOY.D._load_json(epoch3))


if __name__ == "__main__":
    unittest.main()
