import hashlib
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import Mock, patch

import exit_precheck as pre
import exit_setup as setup
import govern


def fixture(height, rotation=None):
    def block(h):
        root = "aa" if rotation is None or h <= rotation else "bb"
        qc = {"domain": {"committee_root": root, "committee_epoch": 1 if root == "aa" else 2}}
        return {"header": {"height":h, "block_hash":str(h), "parent_hash":str(h-1),
                           "state_root":"state"+str(h), "consensus_v2_commit":{"precommit_qc":qc},
                           "batch_kind":"governance" if h == rotation else "asset", "batch_id":"batch"}}
    return block(height)


class QualificationTest(unittest.TestCase):
    def test_guest_executes_every_segment_and_binds_tip_root(self):
        anchor=dict(height=1,block="1",committee="aa",root="state1",chain_id="net",genesis_hash="gen",protocol_version=1)
        head=dict(block_height=132,block_tip_hash="132",state_root="state132")
        def fetch(i,method,from_height,limit):
            return [fixture(h) for h in range(from_height,from_height+limit)]
        ends=[]
        def execute(w,out):
            h=w["block"]["header"]["height"]
            ends.append(h)
            return dict(prior=w["prior_checkpoint_block_id"], result=str(h),height=h,start="aa",end="aa",execution={})
        with tempfile.TemporaryDirectory() as d, patch.object(pre,"rpc",side_effect=fetch), patch.object(pre,"execute_guest",side_effect=execute):
            result=pre.qualify_path([],anchor,head,Path(d))
            self.assertEqual(ends,[66,131,132])
            self.assertEqual(result["qualified_parent"],head)
            with self.assertRaises(ValueError):
                pre.qualify_path([],anchor,dict(head,state_root="fork"),Path(d))

    def test_forged_old_quorum_is_rejected_after_first_compatible_block(self):
        anchor=dict(height=1,block="1",committee="aa",root="state1",chain_id="net",genesis_hash="gen",protocol_version=1)
        head=dict(block_height=5,block_tip_hash="5",state_root="state5")
        def fetch(i,method,**kwargs):
            if method=="blocks":
                return [fixture(h,3) for h in range(kwargs["from_height"],kwargs["from_height"]+kwargs["limit"])]
            update=dict(activation_height=3, validators=["v"], signed_authorizations=["invalid"])
            return [dict(batch_id="batch",payload_json=json.dumps(dict(validator_registry_updates=[update])))]
        def reject(w,out):
            self.assertEqual(w["finality_ancestry"][1]["block"]["header"]["height"],3)
            raise ValueError("frozen checker refused")
        with tempfile.TemporaryDirectory() as d, patch.object(pre,"rpc",side_effect=fetch), patch.object(pre,"execute_guest",side_effect=reject):
            with self.assertRaisesRegex(ValueError,"frozen checker refused"):
                pre.qualify_path([{"node_id":"v"}],anchor,head,Path(d))

    def test_governance_refuses_parent_change_before_remote_work(self):
        head=dict(block_height=9,block_tip_hash="new",state_root="newroot",build_git_revision="rev")
        with patch.object(govern,"fleet",return_value=head),patch.object(govern,"remote") as remote:
            with self.assertRaises(govern.ParentChanged):
                govern.govern("exit-open",compatible="rev",qualified_parent=dict(head,block_tip_hash="old"))
            remote.assert_not_called()

    def test_tip_registry_update_needs_a_verified_successor(self):
        h=fixture(3,3)["header"]
        payload='{"validator_registry_updates":[{}]}'
        domain=dict(chain_id="net",genesis_hash="gen",protocol_version=1)
        encoded=json.dumps(["net","gen",1,"governance","batch",payload],separators=(",",":")).encode()
        h["consensus_v2_commit"]["precommit_qc"]["block"]={"payload_hash":hashlib.sha3_384(b"postfiat.batch_archive_payload.v1\0"+encoded).hexdigest()}
        with patch.object(pre,"rpc",return_value=[dict(batch_id="batch",payload_json=payload)]):
            with self.assertRaisesRegex(ValueError,"rotation"):
                pre.check_unproved_updates(dict(domain,block={"header":h}))
