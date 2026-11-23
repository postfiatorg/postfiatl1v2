import json
import unittest
from unittest.mock import patch
import exit_govern as gov


class RotationTest(unittest.TestCase):
    def test_all_resident_signers_clear_pending_rounds_at_registered_parent(self):
        head = dict(block_height=2000, block_tip_hash="tip", state_root="root", build_git_revision="build")
        parent = gov.parent_of(head)
        route = dict(active=True, burns_open=False, profile_hash=gov.EXIT, profile=gov.read(gov.PROFILE))
        rows = [dict(validator=f"validator-{i}", parent=parent, revision="build", profile_hash=gov.EXIT,
                     hold_through_open=True, outstanding_signed_rotation_rounds=[]) for i in range(6)]
        with patch.object(gov, "fleet", return_value=head), patch.object(gov, "rpc", return_value=route), patch(
                "govern.remote", side_effect=lambda i, args: json.dumps(rows[i])):
            rows[5]["outstanding_signed_rotation_rounds"] = ["signed but not submitted"]
            with self.assertRaisesRegex(ValueError, "rotation"):
                gov.rotation_clearance(head)
            rows[5]["outstanding_signed_rotation_rounds"] = []
            self.assertEqual(gov.rotation_clearance(head), rows)
            rows[1]["parent"] = dict(parent, block_height=1999)
            with self.assertRaises(ValueError):
                gov.rotation_clearance(head)
