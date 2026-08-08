#!/usr/bin/env python3
import importlib.util
import hashlib
import json
import os
import pathlib
import sys
import tempfile
import unittest

HERE = pathlib.Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
_driver_path = pathlib.Path(os.environ.get("NATIVE_DRIVER_PATH", str(HERE / "native_campaign_driver.py")))
_spec = importlib.util.spec_from_file_location("native_campaign_driver", _driver_path)
d = importlib.util.module_from_spec(_spec)
assert _spec.loader is not None
_spec.loader.exec_module(d)


class DriverContractTests(unittest.TestCase):
    def test_wrong_state_root(self):
        with self.assertRaises(ValueError):
            d._validate_finality({"accepted": True, "height": 4, "state_root": "root-b"}, {"height": 4, "state_root": "root-a"})

    def test_unknown_receipt_id(self):
        with self.assertRaises(ValueError):
            d._validate_receipt_identity({"receipt_id": "r-2"}, {"receipt_id": "r-1"})

    def test_duplicate_nonce_packet_burn(self):
        with self.assertRaises(ValueError):
            d._validate_unique(["n-1", "n-1"])

    def test_wrong_recipient(self):
        with self.assertRaises(ValueError):
            d._validate_recipient({"recipient": "pf-other"}, "pf-owner")

    def test_amount_mismatch(self):
        with self.assertRaises(ValueError):
            d._validate_delta("10", "19", "10")

    def test_stale_nav(self):
        with self.assertRaises(ValueError):
            d._validate_nav({"epoch": 4, "packet_hash": "nav-a"}, {"epoch": 5, "packet_hash": "nav-b"})

    def test_replayed_campaign(self):
        with self.assertRaises(ValueError):
            d._validate_replay({"receipt_ids": ["r-1"]}, {"receipt_ids": ["r-1"]})

    def test_wallet_total_substitution(self):
        with self.assertRaises(ValueError):
            d._validate_account_assets({"wallet_total": "100"}, "asset-a")

    def test_missing_eth_finality(self):
        with self.assertRaises(ValueError):
            d._validate_eth_finality({"status": "0x0"})

    def test_wrong_signer(self):
        with self.assertRaises(ValueError):
            d._validate_signer({"signer": "pf-bad"}, "pf-good")

    def test_swap_output_mismatch(self):
        with self.assertRaises(ValueError):
            d._validate_swap({"output_atoms": "9", "min_output_atoms": "10"})

    def test_stakehub_import_detector(self):
        with self.assertRaises(ValueError):
            d._validate_state_forbidden({"state": "external-control-plane marker"})

    def test_resume_after_interruption(self):
        with tempfile.TemporaryDirectory() as td:
            p = pathlib.Path(td) / "journal.json"
            p.write_text(json.dumps({"schema": "postfiat.native-campaign-journal-v1", "legs": []}))
            with self.assertRaises(ValueError):
                d.resume_after_interruption(p, "leg-1")



class FlowTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = pathlib.Path(self.tmp.name)
        self.node = self.root / "node"
        self.node.write_text("synthetic")

    def tearDown(self):
        self.tmp.cleanup()

    def packet(self, leg="0", operation=None):
        packet = {
            "packet_id": "packet-" + str(leg), "leg": leg,
            "route": "route-synthetic", "chain_id": "chain-synthetic",
            "genesis_hash": "genesis-synthetic",
            "source_rpc_url": "https://source.example",
            "budget_guard": {"cap_usdc": "530", "prior_spend_usdc": "1", "leg_ceiling_usdc": "0", "eth_usd": "3000"},
        }
        if operation is not None:
            packet.update({"operation": operation, "source": "pf-source", "key_file": "/tmp/key-ref"})
        return packet

    def write_packet_binding(self, packet, match=True):
        path = self.root / ("packet-" + str(packet["leg"]) + ".json")
        path.write_text(json.dumps(packet, sort_keys=True))
        digest = __import__("hashlib").sha256(path.read_bytes()).hexdigest()
        binding = self.root / "binding.json"
        binding.write_text(json.dumps({"packets": [{"packet_id": packet["packet_id"], "sha256": digest if match else "bad"}]}))
        return path, binding

    def runner(self, bad_endpoint=None, height_spread=False, accepted=True, root="root-a", receipts=None):
        receipts = receipts or {}
        def call(argv):
            text = " ".join(argv)
            if " --method status " in (" " + text + " "):
                endpoint = argv[argv.index("--endpoint") + 1]
                h = 10
                if height_spread and endpoint.endswith("39655"):
                    h = 12
                sr = root
                if bad_endpoint and endpoint == bad_endpoint:
                    sr = "root-b"
                return {"genesis_hash": "genesis-synthetic", "state_root": sr, "block_height": h}
            if "receipts" in argv and "--method" in argv:
                tx = argv[argv.index("--tx-id") + 1]
                endpoint = argv[argv.index("--endpoint") + 1]
                receipt_root = "root-b" if bad_endpoint and endpoint == bad_endpoint else root
                value = receipts.get(tx, {"accepted": accepted, "receipt_id": "receipt-" + tx, "height": 10, "state_root": receipt_root})
                return value
            if "--method tx" in argv or "--method batch_archive" in argv:
                return {"ok": True}
            if argv and argv[0] == "cast":
                if "--rpc-url" not in argv:
                    raise AssertionError("cast receipt missing packet-bound --rpc-url")
                tx_hash = argv[2] if len(argv) > 2 else ""
                return {"status": "0x1", "blockNumber": "0x10", "blockHash": "0xblock", "transactionHash": tx_hash}
            return {"accepted": accepted, "tx_ids": ["tx-1"]}
        return call

    def test_t14_binding_gate_and_match(self):
        packet = self.packet("0")
        packet_path, binding = self.write_packet_binding(packet, match=False)
        with self.assertRaises(d.StopError):
            d.run_leg(packet_path, binding, self.root / "bad", node=self.node, runner=self.runner())
        packet_path, binding = self.write_packet_binding(packet, match=True)
        result = d.run_leg(packet_path, binding, self.root / "good", node=self.node, runner=self.runner())
        self.assertEqual(result["status"], "FINALIZED")

    def operation(self, tag):
        common = {"route_id": "route-synthetic"}
        values = {
            "pftl_uniswap_order_reserve": {"subscriber":"pf-subscriber", "reservation_id":"r", "ethereum_recipient":"0x1234", "route_epoch":6, "policy_epoch":5, "policy_hash":"p", "mint_amount_atoms":11, "max_settlement_value_atoms":10, "expires_at_height":100},
            "pftl_uniswap_primary_subscribe_v2": {"subscriber":"pf-subscriber", "reservation_id":"r", "subscription_nonce":"n", "settlement_asset_id":"asset", "settlement_value_atoms":10, "pricing_nav_epoch":5, "pricing_reserve_packet_hash":"nav"},
            "pftl_uniswap_export_debit": {"owner":"pf-owner", "packet_hash":"packet", "export_nonce":"n", "ethereum_recipient":"0x1234", "amount_atoms":11, "destination_deadline_seconds":10, "refund_delay_blocks":2},
            "pftl_uniswap_return_import": {"operator":"pf-operator", "burn_event_hash":"burn", "ethereum_chain_id":1, "bridge_controller":"0x1234", "wrapped_navcoin_token":"0x2345", "native_nav_asset_id":"asset", "ethereum_sender":"0x3456", "pftl_recipient":"pf-recipient", "amount_atoms":11, "return_nonce":"n", "burn_height":8, "finalized_height":9},
            "pftl_uniswap_primary_redeem": {"owner":"pf-owner", "settlement_recipient":"pf-recipient", "redemption_nonce":"n", "nav_amount_atoms":11, "min_settlement_value_atoms":10, "route_epoch":6, "policy_epoch":5, "policy_hash":"p", "pricing_nav_epoch":5, "pricing_reserve_packet_hash":"nav", "expires_at_height":100},
        }
        return {"operation": tag, **common, **values[tag]}

    def test_t15_render_all_certified_schemas(self):
        expected = {
            "2a": {"subscriber","route_id","reservation_id","ethereum_recipient","route_epoch","policy_epoch","policy_hash","mint_amount_atoms","max_settlement_value_atoms","expires_at_height"},
            "2b": {"subscriber","route_id","reservation_id","subscription_nonce","settlement_asset_id","settlement_value_atoms","pricing_nav_epoch","pricing_reserve_packet_hash"},
            "3a": {"owner","route_id","packet_hash","export_nonce","ethereum_recipient","amount_atoms","destination_deadline_seconds","refund_delay_blocks"},
            "4-import": {"operator","route_id","burn_event_hash","ethereum_chain_id","bridge_controller","wrapped_navcoin_token","native_nav_asset_id","ethereum_sender","pftl_recipient","amount_atoms","return_nonce","burn_height","finalized_height"},
            "5a": {"owner","settlement_recipient","route_id","redemption_nonce","nav_amount_atoms","min_settlement_value_atoms","route_epoch","policy_epoch","policy_hash","pricing_nav_epoch","pricing_reserve_packet_hash","expires_at_height"},
        }
        for leg, tag in d.CERTIFIED.items():
            packet = self.packet(leg, self.operation(tag))
            path, rendered_tag, _ = d._render_ops(packet, self.root / leg)
            body = json.loads(path.read_text())["operations"][0]["operation"]
            self.assertEqual(rendered_tag, tag)
            self.assertEqual(set(body) - {"operation"}, expected[leg])

    def test_t16_preflight_quorum(self):
        packet = self.packet("0")
        with self.assertRaises(d.StopError):
            d._preflight(packet, {}, self.node, self.runner(bad_endpoint="127.0.0.1:39654"))
        with self.assertRaises(d.StopError):
            d._preflight(packet, {}, self.node, self.runner(height_spread=True))
        result = d._preflight(packet, {}, self.node, self.runner())
        self.assertEqual(result["height"], 10)

    def test_t17_receipt_gate_and_journal(self):
        packet = self.packet("1")
        packet["delta_assertions"] = {}
        pre = {"height":10,"state_root":"root-a","genesis_hash":"genesis-synthetic"}
        dispatch = {"reports": []}
        with self.assertRaises(d.StopError):
            d._receipt_gate(["tx-1"], packet, self.node, self.runner(accepted=False), pre, dispatch, self.root / "leg")
        with self.assertRaises(d.StopError):
            d._receipt_gate(["tx-1"], packet, self.node, self.runner(bad_endpoint="127.0.0.1:39654"), pre, dispatch, self.root / "leg")
        final = d._receipt_gate(["tx-1"], packet, self.node, self.runner(), pre, dispatch, self.root / "leg")
        d._append_journal(self.root, packet, pre, pre, dispatch, final)
        journal = json.loads((self.root / "campaign-journal.json").read_text())
        self.assertEqual(journal["schema"], "postfiat.native-campaign-journal-v1")
        self.assertEqual(journal["legs"][0]["pre_state_root"], "root-a")
        self.assertEqual(journal["legs"][0]["post_height"], 10)

    def test_t18_ordering(self):
        packet = self.packet("2b", self.operation("pftl_uniswap_primary_subscribe_v2"))
        packet_path, binding = self.write_packet_binding(packet)
        with self.assertRaises(d.StopError):
            d.run_leg(packet_path, binding, self.root / "empty", node=self.node, runner=self.runner())
        journal_dir = self.root / "ordered"
        journal_dir.mkdir()
        journal = {"schema": d.SCHEMA, "legs": [{"leg":"2a", "status":"FINALIZED", "finality":[{"tx_id":"old","receipt_id":"receipt-old","accepted":True,"height":10,"state_root":"root-a"}]}]}
        (journal_dir / "campaign-journal.json").write_text(json.dumps(journal))
        result = d.run_leg(packet_path, binding, journal_dir, resume=True, node=self.node, runner=self.runner())
        self.assertEqual(result["status"], "FINALIZED")

    def test_t19_crash_resume_refuses_duplicate_submit(self):
        packet = self.packet("2a", self.operation("pftl_uniswap_order_reserve"))
        packet_path, binding = self.write_packet_binding(packet)
        run_dir = self.root / "crash"
        run_dir.mkdir()
        journal = {"schema": d.SCHEMA, "legs": [{"leg":"2a", "status":"SUBMITTED", "submission":{"tx_ids":["crashed-tx"]}, "finality":[]}]}
        (run_dir / "campaign-journal.json").write_text(json.dumps(journal))
        calls = []
        base = self.runner()
        def record(argv):
            calls.append(list(argv))
            return base(argv)
        with self.assertRaises(d.StopError):
            d.run_leg(packet_path, binding, run_dir, resume=True, node=self.node, runner=record)
        self.assertFalse(any("pftl-submit-certified-asset-ops" in x for call in calls for x in call))
        fixed = json.loads((run_dir / "campaign-journal.json").read_text())
        self.assertEqual(fixed["legs"][0]["status"], "FINALIZED")
        self.assertTrue(fixed["legs"][0]["finality"])

    def test_t20_static_source_scan(self):
        source = (HERE / "native_campaign_driver.py").read_text().lower()
        for marker in ("stakehub", "wallet_demo", "live_loop", "private_swap", "spend_ledger", "run_manifest", "wallet_proxy"):
            self.assertNotIn(marker, source)

    def test_t21_verify_journal_unknown_receipt(self):
        run_dir = self.root / "verify"
        run_dir.mkdir()
        journal = {"schema": d.SCHEMA, "legs": [{"leg":"1", "finality":[{"tx_id":"missing","receipt_id":"wanted","accepted":True,"height":10,"state_root":"root-a"}], "submission": {}}]}
        (run_dir / "campaign-journal.json").write_text(json.dumps(journal))
        def unknown(argv):
            if "receipts" in argv and "--method" in argv:
                return {"accepted": False}
            return self.runner()(argv)
        with self.assertRaises(d.StopError):
            d.verify_journal(run_dir, runner=unknown, node=self.node)

    def test_t22_leaf_static_and_non_ok(self):
        leaf = (HERE / "native_agentd_leaf.py").read_text().lower()
        send_leaf = (HERE / "native_evm_leaf_send.py").read_text().lower()
        self.assertNotIn("stakehub.cli", leaf + send_leaf)
        self.assertNotIn("agentd import", leaf + send_leaf)
        original = __import__("native_agentd_leaf")
        old = original._request
        try:
            original._request = lambda *args, **kwargs: (_ for _ in ()).throw(RuntimeError("denied"))
            with self.assertRaises(RuntimeError):
                original.session_status(self.root)
        finally:
            original._request = old

    def test_t23_evm_receipt_fields(self):
        packet = self.packet("3b0")
        packet["expected_receipt"] = {"from": "0xaaaa", "to": "0xbbbb", "value_wei": 10}
        packet["budget_guard"] = {"cap_usdc":"530", "prior_spend_usdc":"1", "leg_ceiling_usdc":"1", "eth_usd":"3000"}
        dispatch = {"reports": [{"tx_hash":"tx-evm", "from":"0xaaaa", "to":"0xbbbb", "value_wei":10}]}
        good = self.runner()
        def good_runner(argv):
            if argv and argv[0] == "cast":
                if "--rpc-url" not in argv:
                    raise AssertionError("cast receipt missing packet-bound --rpc-url")
                return {"status":"0x1", "blockNumber":"0x10", "blockHash":"0xblock", "transactionHash":"tx-evm", "from":"0xaaaa", "to":"0xbbbb", "value":"0xa", "gasUsed":"0x2", "effectiveGasPrice":"0x3"}
            return good(argv)
        result = d._receipt_gate([], packet, self.node, good_runner, {"height":10,"state_root":"root-a"}, dispatch, self.root / "leg")
        self.assertEqual(len(result["evm_receipts"]), 1)
        bad = dict(dispatch, reports=[{"tx_hash":"tx-evm", "from":"0xcccc", "to":"0xbbbb", "value_wei":10}])
        with self.assertRaises(d.StopError):
            d._receipt_gate([], packet, self.node, good_runner, {"height":10,"state_root":"root-a"}, bad, self.root / "leg")

    def test_t24_budget_guard_and_actual_cost(self):
        with self.assertRaises(d.StopError):
            d._budget_guard({"leg":"3b0", "budget_guard":{"cap_usdc":"5", "prior_spend_usdc":"4", "leg_ceiling_usdc":"2"}}, {"legs":[]})
        with self.assertRaises(d.StopError):
            d._budget_guard({"leg":"3b0"}, {"legs":[]})
        packet = self.packet("3b0")
        packet["budget_guard"] = {"cap_usdc":"530", "prior_spend_usdc":"1", "leg_ceiling_usdc":"1", "eth_usd":"3000"}
        pre = {"height":10,"state_root":"root-a"}
        finality = {"finality":[], "tx_ids":[], "ethereum_tx_hashes":[], "evm_receipts":[{"gasUsed":"0x2", "effectiveGasPrice":"0x3"}]}
        d._append_journal(self.root, packet, pre, pre, {}, finality)
        journal = json.loads((self.root / "campaign-journal.json").read_text())
        from decimal import Decimal
        self.assertEqual(Decimal(journal["legs"][0]["actual_cost_usdc"]), Decimal("0.000000000000018"))

    def test_t25_real_envelope_parsing(self):
        real = json.loads((HERE / "testdata-native-driver/fleet-status-real.json").read_text())
        status = real["result"]
        packet = self.packet("0")
        packet["budget_guard"] = {"cap_usdc":"530","prior_spend_usdc":"1","leg_ceiling_usdc":"0"}
        def real_runner(argv):
            if "status" in argv:
                return status
            if "receipts" in argv:
                return {"accepted":True,"receipt_id":"receipt-synthetic","height":10,"state_root":status["state_root"]}
            return {"ok":True}
        result = d._preflight(packet, {}, self.node, real_runner)
        self.assertEqual(result["height"], int(status["block_height"]))
        self.assertEqual(result["state_root"], status["state_root"])
        supply = json.loads((HERE / "testdata-native-driver/supply-status-real.json").read_text())["result"]
        self.assertEqual((supply["route_epoch"], supply["pricing_nav_epoch"], supply["nav_per_unit"]), (6,5,90234207))
        bad = dict(status); bad.pop("block_height"); bad["height"] = 776
        with self.assertRaises(d.StopError):
            d._preflight(packet, {}, self.node, lambda argv: bad)
        dispatch={"reports":[]}
        receipt_packet=self.packet("1")
        final=d._receipt_gate(["tx-1"], receipt_packet, self.node, real_runner, {"height":10,"state_root":status["state_root"]}, dispatch, self.root / "leg")
        self.assertTrue(final["finality"][0]["accepted"])

    def test_t26_protocol_version_request(self):
        import native_campaign_driver as mod
        import socket as sockmod
        sent=[]
        class Fake:
            def __enter__(self): return self
            def __exit__(self,*a): pass
            def sendall(self,b): sent.append(b)
            def recv(self,n): return b'{"version":"postfiat-local-rpc-v1","id":"x","ok":true,"result":{}}\n'
        old=mod.socket.create_connection; mod.socket.create_connection=lambda *a,**k: Fake()
        try: mod._rpc_call("127.0.0.1:1","status",{},None,self.node)
        finally: mod.socket.create_connection=old
        self.assertIn(b'"version":"postfiat-local-rpc-v1"', sent[0]); self.assertNotIn(b'"jsonrpc"', sent[0])

    def test_t27_real_packet_templates(self):
        base=HERE.parent / "docs/evidence/a666-public-reserve-product-20260803/live-demo/native-v1"
        names=[("native-leg2a-order-reserve.json","pftl_uniswap_order_reserve"),("native-leg2b-primary-subscribe.json","pftl_uniswap_primary_subscribe_v2"),("native-leg3a-export-debit.json","pftl_uniswap_export_debit"),("native-leg4-return-burn-import.json","pftl_uniswap_return_import"),("native-leg5a-primary-redeem.json","pftl_uniswap_primary_redeem")]
        for name,tag in names:
            packet=json.loads((base/name).read_text())
            try:
                got,body=d._operation_from_packet(packet,str(packet["leg"]))
                self.assertEqual(got,tag)
            except d.ConfigError as exc:
                msg=str(exc)
                self.assertNotRegex(msg,r"route_id|ethereum_recipient|ethereum_chain_id|pftl_recipient")

    def test_t28_leg4_two_phase_gate_and_injection(self):
        packet=self.packet("4", {"operation":"pftl_uniswap_return_import","operator":"pf-op","route_id":"route-synthetic","burn_event_hash":"FROM-PHASE-1-RECEIPT","ethereum_chain_id":1,"bridge_controller":"0x1111","wrapped_navcoin_token":"0x2222","native_nav_asset_id":"asset","ethereum_sender":"0x3333","pftl_recipient":"pf-rec","amount_atoms":"PENDING-FIRE-TIME:amount","return_nonce":"nonce","burn_height":1,"finalized_height":2})
        packet["executor"]={"kind":"phases","phases":[{"kind":"evm_script","commands":[["python3","burn.py"]]},{"kind":"certified_ops"}]}
        packet["ops_file_template"]={"operations":[{"source":"pf-op","key_file":"/tmp/k","operation":packet.pop("operation") }]}
        calls=[]
        def run(argv):
            calls.append(list(argv))
            if argv and argv[0]=="cast": return {"status":"0x1","blockNumber":"0x1","blockHash":"0xblock","transactionHash":"burn-hash","from":"0x3333","to":"0x2222","value":"0x1"}
            return {"tx_hash":"burn-hash","status":"0x1"}
        with self.assertRaisesRegex(d.ConfigError,"amount_atoms"):
            d._dispatch(packet,self.node,self.root / "leg",run)
        self.assertTrue(calls)

    def test_t28b_phase_report_missing_hash(self):
        packet=self.packet("4", {"operation":"pftl_uniswap_return_import","operator":"pf-op","route_id":"route-synthetic","burn_event_hash":"FROM-PHASE-1-RECEIPT","ethereum_chain_id":1,"bridge_controller":"0x1","wrapped_navcoin_token":"0x2","native_nav_asset_id":"asset","ethereum_sender":"0x3","pftl_recipient":"pf-rec","amount_atoms":"PENDING-FIRE-TIME:x","return_nonce":"n","burn_height":1,"finalized_height":2}); packet["executor"]={"kind":"phases","phases":[{"kind":"evm_script","commands":[["burn"]]},{"kind":"certified_ops"}]}; packet["ops_file_template"]={"operations":[{"operation":packet.pop("operation")}]}
        with self.assertRaisesRegex(d.StopError,"phase 1 report omitted tx_hash"): d._dispatch(packet,self.node,self.root/"x",lambda argv:{"status":"0x1"})

    def test_t28c_phase_wrong_recipient_stops_before_phase2(self):
        packet=self.packet("4"); packet["expected_receipt"]={"from":"0xa","to":"0xb","value_wei":1}; reports=[]
        with self.assertRaises(d.StopError): d._phase_receipt_gate(packet,[{"tx_hash":"x","status":"0x1","from":"0xa","to":"0xc","value_wei":1}],lambda argv: {})

    def test_t28d_phase_amount_mismatch(self):
        packet=self.packet("3a", {"operation":"pftl_uniswap_export_debit","owner":"o","route_id":"route-synthetic","packet_hash":"p","export_nonce":"n","ethereum_recipient":"0x1","amount_atoms":9,"destination_deadline_seconds":1,"refund_delay_blocks":1}); packet["amount_atoms"]=10
        with self.assertRaisesRegex(d.ConfigError,"amount mismatch"): d._operation_from_packet(packet,"3a")

    def test_t28e_phase_replay_position(self):
        journal={"legs":[{"leg":"4","status":"FINALIZED"}]}; self.assertFalse(d._first_unjournaled(journal,"4"))

    def test_t28f_existing_burn_report_skips_broadcast(self):
        leg=self.root/"leg"; leg.mkdir(); (leg/"burn-report.json").write_text(json.dumps({"tx_hash":"burn","status":"0x1","from":"0xa","to":"0xb","value_wei":1})); packet=self.packet("4"); packet["expected_receipt"]={"from":"0xa","to":"0xb","value_wei":1}; packet["executor"]={"kind":"phases","phases":[{"kind":"evm_script","commands":[["burn"]],"report":"{artifact_dir}/burn-report.json"},{"kind":"certified_ops"}]}; calls=[]
        def run(argv): calls.append(argv); return {"status":"0x1","blockNumber":"0x1","blockHash":"0xblock","transactionHash":"burn","from":"0xa","to":"0xb","value":"0x1"}
        with self.assertRaises(d.ConfigError): d._dispatch(packet,self.node,leg,run)
        self.assertFalse(any(c and c[0] == "burn" for c in calls))

    def test_t29_deposit_gate_passes_and_resume_skips_stage1(self):
        leg=self.root/"d"; leg.mkdir(); (leg/"evm-deposit.json").write_text(json.dumps({"deposit_tx":"tx","delta_ok":True})); packet=self.packet("1"); packet["deposit_receipt_timeout_secs"]=0; packet["deposit_receipt_poll_interval_secs"]=0
        rec=lambda argv:{"status":"0x1","blockNumber":"0x1","blockHash":"0xblock","transactionHash":"tx"}
        self.assertTrue(d._gate_deposit_stage(packet,leg,rec)["delta_ok"])

    def test_t30_deposit_reverted_stops(self):
        leg=self.root/"d"; leg.mkdir(); (leg/"evm-deposit.json").write_text(json.dumps({"deposit_tx":"tx","delta_ok":True})); packet=self.packet("1"); packet["deposit_receipt_timeout_secs"]=0
        with self.assertRaises(d.StopError): d._gate_deposit_stage(packet,leg,lambda argv:{"status":"0x0","blockNumber":"0x1","blockHash":"0xblock","transactionHash":"tx"})

    def test_t31_deposit_delta_false_stops(self):
        leg=self.root/"d"; leg.mkdir(); (leg/"evm-deposit.json").write_text(json.dumps({"deposit_tx":"tx","delta_ok":False})); packet=self.packet("1")
        with self.assertRaises(d.StopError): d._gate_deposit_stage(packet,leg,lambda argv:{})

    def test_t32_deposit_timeout_stops(self):
        leg=self.root/"d"; leg.mkdir(); (leg/"evm-deposit.json").write_text(json.dumps({"deposit_tx":"tx","delta_ok":True})); packet=self.packet("1"); packet["deposit_receipt_timeout_secs"]=0
        with self.assertRaises(d.StopError): d._gate_deposit_stage(packet,leg,lambda argv:{"status":"0x0","transactionHash":"tx","blockHash":"0xblock","blockNumber":"0x1"})

    def test_t33_deposit_malformed_stops(self):
        leg=self.root/"d"; leg.mkdir(); (leg/"evm-deposit.json").write_text("not-json"); packet=self.packet("1")
        with self.assertRaises(d.StopError): d._gate_deposit_stage(packet,leg,lambda argv:{})

    def test_t34_deposit_expected_receipt_mismatch_stops(self):
        leg=self.root/"d"; leg.mkdir(); (leg/"evm-deposit.json").write_text(json.dumps({"deposit_tx":"tx","delta_ok":True})); packet=self.packet("1"); packet["deposit_receipt_timeout_secs"]=0; packet["expected_receipt"]={"to":"0xgood"}
        with self.assertRaises(d.StopError): d._gate_deposit_stage(packet,leg,lambda argv:{"status":"0x1","blockNumber":"0x1","blockHash":"0xblock","transactionHash":"tx","to":"0xbad"})

    def test_t68_cast_receipt_uses_packet_rpc_url_and_strict_json(self):
        leg = self.root / "rpc-bound"; leg.mkdir()
        (leg / "evm-deposit.json").write_text(json.dumps({"deposit_tx": "tx", "delta_ok": True}))
        packet = self.packet("1"); packet["deposit_receipt_timeout_secs"] = 0
        seen = []
        def fake_cast(argv):
            seen.append(list(argv))
            self.assertIn("--rpc-url", argv)
            self.assertIn(packet["source_rpc_url"], argv)
            return {"status": "0x1", "blockNumber": "0x1", "blockHash": "0xblock", "transactionHash": "tx"}
        self.assertTrue(d._gate_deposit_stage(packet, leg, fake_cast)["delta_ok"])
        self.assertEqual(1, len(seen))

    def test_t69_cast_receipt_requires_block_hash_and_matching_tx(self):
        packet = self.packet("1")
        with self.assertRaises(d.StopError):
            d._cast_receipt(packet, "tx", lambda argv: {"status": "0x1", "blockNumber": "0x1", "transactionHash": "tx"})
        with self.assertRaises(d.StopError):
            d._cast_receipt(packet, "tx", lambda argv: {"status": "0x1", "blockNumber": "0x1", "blockHash": "0xblock", "transactionHash": "other"})

    def test_t35_linter_rejects_current_held_packets(self):
        binding=HERE.parent/"docs/evidence/a666-public-reserve-product-20260803/live-demo/native-v1/authorization-binding-native-v1.json"
        with self.assertRaises(d.ConfigError): d.validate_executable(binding,"1")

    def test_t35b_linter_rejects_odd_length_calldata(self):
        packet = self.packet("3h")
        packet["executor"] = {"kind": "evm_script", "commands": [["leaf", "--calldata", "0x123"]]}
        path, binding = self.write_packet_binding(packet)
        bound = json.loads(binding.read_text())
        bound["packets"][0]["path"] = path.name
        binding.write_text(json.dumps(bound))
        with self.assertRaisesRegex(d.ConfigError, r"malformed --calldata hex"):
            d.validate_executable(binding, "3h")

    def test_t36_leg1_dispatch_resume_path(self):
        leg=self.root/"leg"; leg.mkdir(); (leg/"evm-deposit.json").write_text(json.dumps({"deposit_tx":"tx","delta_ok":True})); packet=self.packet("1"); packet["deposit_receipt_timeout_secs"]=0; packet["executor"]={"commands":[["burn","--artifact-dir",str(leg)],["relay","--artifact-dir",str(leg)]]}; calls=[]
        def run(argv): calls.append(argv); return {"status":"0x1","blockNumber":"0x1","blockHash":"0xblock","transactionHash":"tx"}
        result=d._dispatch(packet,self.node,leg,run); self.assertEqual(result["commands"][0][0],"relay"); self.assertEqual(len(calls),2)

    def test_t37_leg1_gate_reverted_public_dispatch(self):
        leg=self.root/"leg"; leg.mkdir(); (leg/"evm-deposit.json").write_text(json.dumps({"deposit_tx":"tx","delta_ok":True})); packet=self.packet("1"); packet["deposit_receipt_timeout_secs"]=0; packet["executor"]={"commands":[["burn"],["relay"]]}; calls=[]
        with self.assertRaises(d.StopError): d._dispatch(packet,self.node,leg,lambda argv:(calls.append(argv) or {"status":"0x0","blockNumber":"0x1","blockHash":"0xblock","transactionHash":"tx"}))
        self.assertEqual(len(calls),1)

    def test_t38_prover_hash_mismatch_stops_three_stage(self):
        leg=self.root/"leg"; leg.mkdir(); (leg/"evm-deposit.json").write_text(json.dumps({"deposit_tx":"tx","delta_ok":True})); (leg/"proof-report.json").write_text(json.dumps({"proof-calldata_sha256":"bad"})); packet=self.packet("1"); packet["deposit_receipt_timeout_secs"]=0; packet["expected_proof_hashes"]={"proof-calldata_sha256":"good"}; packet["executor"]={"commands":[["deposit"],["prove"],["relay"]]}; calls=[]
        with self.assertRaises(d.StopError): d._dispatch(packet,self.node,leg,lambda argv:(calls.append(argv) or {"status":"0x1","blockNumber":"0x1","blockHash":"0xblock","transactionHash":"tx"}))

    def test_t39_prover_leaf_resume_hashes(self):
        import native_prover_leaf as leaf
        out=self.root/"proof"; out.mkdir(); (out/"proof-calldata.bin").write_bytes(b"a"); (out/"public-values.bin").write_bytes(b"b"); import hashlib
        report={"proof-calldata_sha256":hashlib.sha256(b"a").hexdigest(),"public-values_sha256":hashlib.sha256(b"b").hexdigest(),"program_vkey":leaf.EXPECTED_PROGRAM_VKEY,"elf_sha256":leaf.EXPECTED_ELF_SHA256}; (out/"proof-report.json").write_text(json.dumps(report)); self.assertEqual(leaf.verify_existing(out)["proof-calldata_sha256"],report["proof-calldata_sha256"])

    def test_t40_staged_exemption_requires_complete_entry(self):
        packet={"packet_id":"p","leg":"5a","executor":{"kind":"evm_script","commands":[["x","PENDING-FIRE-TIME:v"]]}}; pp=self.root/"p.json"; pp.write_text(json.dumps(packet)); import hashlib; h=hashlib.sha256(pp.read_bytes()).hexdigest(); bp=self.root/"b.json"; bp.write_text(json.dumps({"packets":[{"packet_id":"p","path":"p.json","sha256":h}],"staged_fields":[{"packet":"p.json","json_pointer":"/executor/commands/0/1","source":"leg4 receipt","stage":"S4"}]})); lines=d.validate_executable(bp,"5a"); self.assertTrue(any("STAGED-EXEMPT" in x for x in lines))

    def test_t41_staged_missing_source_rejects(self):
        packet={"packet_id":"p","leg":"5a","executor":{"commands":[["x","PENDING-FIRE-TIME:v"]]}}; pp=self.root/"p.json"; pp.write_text(json.dumps(packet)); bp=self.root/"b.json"; bp.write_text(json.dumps({"packets":[{"packet_id":"p","path":"p.json"}],"staged_fields":[{"packet":"p.json","json_pointer":"/executor/commands/0/1","stage":"S4"}]}))
        with self.assertRaises(d.ConfigError): d.validate_executable(bp,"5a")

    def test_t42_staged_missing_stage_rejects(self):
        packet={"packet_id":"p","leg":"5a","executor":{"commands":[["x","PENDING-FIRE-TIME:v"]]}}; pp=self.root/"p.json"; pp.write_text(json.dumps(packet)); bp=self.root/"b.json"; bp.write_text(json.dumps({"packets":[{"packet_id":"p","path":"p.json"}],"staged_fields":[{"packet":"p.json","json_pointer":"/executor/commands/0/1","source":"receipt"}]}))
        with self.assertRaises(d.ConfigError): d.validate_executable(bp,"5a")

    def test_t43_exemption_resolved_field_rejects(self):
        packet={"packet_id":"p","leg":"5a","executor":{"commands":[["x","resolved"]]}}; pp=self.root/"p.json"; pp.write_text(json.dumps(packet)); bp=self.root/"b.json"; bp.write_text(json.dumps({"packets":[{"packet_id":"p","path":"p.json"}],"staged_fields":[{"packet":"p.json","json_pointer":"/executor/commands/0/1","source":"receipt","stage":"S4"}]}))
        with self.assertRaises(d.ConfigError): d.validate_executable(bp,"5a")

    def test_t44_dispatch_pending_still_fails(self):
        packet=self.packet("5a",self.operation("pftl_uniswap_primary_redeem")); packet["operation"]["nav_amount_atoms"]="PENDING-FIRE-TIME:x"
        with self.assertRaises(d.ConfigError): d._operation_from_packet(packet,"5a")


    def _phase_linter_fixture(self, staged_pointer=None):
        packet = {
            "packet_id": "phase-pointer",
            "leg": "4",
            "executor": {
                "kind": "phases",
                "phases": [
                    {
                        "kind": "evm_script",
                        "commands": [["burn", "PENDING-FIRE-TIME:burn_event_hash"]],
                    },
                    {"kind": "certified_ops"},
                ],
            },
        }
        packet_path = self.root / "phase-pointer.json"
        packet_path.write_text(json.dumps(packet))
        binding = {
            "packets": [{
                "packet_id": "phase-pointer",
                "path": packet_path.name,
                "sha256": hashlib.sha256(packet_path.read_bytes()).hexdigest(),
            }]
        }
        if staged_pointer is not None:
            binding["staged_fields"] = [{
                "packet": packet_path.name,
                "json_pointer": staged_pointer,
                "source": "phase-1 burn receipt",
                "stage": "S4",
            }]
        binding_path = self.root / "phase-binding.json"
        binding_path.write_text(json.dumps(binding))
        return binding_path

    def test_t45_phase_linter_uses_real_pointer(self):
        binding = self._phase_linter_fixture("/executor/phases/0/commands/0/1")
        lines = d.validate_executable(binding, "4")
        self.assertIn(
            "STAGED-EXEMPT 4 /executor/phases/0/commands/0/1",
            "\n".join(lines),
        )

    def test_t46_phase_linter_missing_entry_names_real_pointer(self):
        binding = self._phase_linter_fixture()
        with self.assertRaisesRegex(
            d.ConfigError, r"/executor/phases/0/commands/0/1"
        ):
            d.validate_executable(binding, "4")

    def test_t47_phase_linter_rejects_synthetic_pointer(self):
        binding = self._phase_linter_fixture("/executor/0/commands/0/1")
        with self.assertRaisesRegex(
            d.ConfigError, r"/executor/phases/0/commands/0/1"
        ):
            d.validate_executable(binding, "4")

    def test_t48_certified_linter_requires_operation_staged_entry(self):
        operation = self.operation("pftl_uniswap_primary_redeem")
        operation["nav_amount_atoms"] = "PENDING-FIRE-TIME:redeem-amount"
        packet = self.packet("5a")
        packet["executor"] = {"kind": "certified_ops"}
        packet["ops_file_template"] = {
            "operations": [{"source": "pf-owner", "key_file": "/tmp/key-ref", "operation": operation}]
        }
        path, binding = self.write_packet_binding(packet)
        bound = json.loads(binding.read_text())
        bound["packets"][0]["path"] = path.name
        binding.write_text(json.dumps(bound))
        with self.assertRaisesRegex(
            d.ConfigError,
            r"/ops_file_template/operations/0/operation/nav_amount_atoms",
        ):
            d.validate_executable(binding, "5a")

    def test_t49_prover_remote_commands_are_absolute_and_env_pinned(self):
        import argparse
        from types import SimpleNamespace
        import native_prover_leaf as leaf

        out = self.root / "proof"
        out.mkdir()
        tx = "11" * 32
        nonce = "22" * 32
        route = leaf.EXPECTED_ROUTE_BINDING
        recipient = "pfab9b9228942e5c529633a13aa271d5297bec6353"
        depositor = "1455bd7fbfbf92a171ef36025e13959e3b0ad8c0"
        def word(value):
            return f"{int(value, 16) if isinstance(value, str) else value:064x}"
        event_data = "".join([
            word(0xE0), word(10_000_000), nonce, route, word(1),
            word(leaf.EXPECTED_VAULT_ADDRESS[2:]),
            word(leaf.EXPECTED_TOKEN_ADDRESS[2:]), word(len(recipient)),
            recipient.encode().hex().ljust(64, "0"),
        ])
        receipt = {
            "status": "0x1", "blockNumber": "0x100",
            "transactionHash": "0x" + tx, "from": "0x" + depositor,
            "logs": [{"address": leaf.EXPECTED_VAULT_ADDRESS,
                       "topics": ["0x" + "00" * 32, "0x" + "33" * 32,
                                  "0x" + "0" * 24 + depositor],
                       "data": "0x" + event_data}],
        }
        deposit_report = {
            "deposit_tx": "0x" + tx, "vault_address": leaf.EXPECTED_VAULT_ADDRESS,
            "usdc_address": leaf.EXPECTED_TOKEN_ADDRESS, "stakehub_wallet": "0x" + depositor,
            "pftl_recipient": recipient, "amount_atoms": 10_000_000,
            "nonce": "0x" + nonce,
        }
        (out / "evm-deposit.json").write_text(json.dumps(deposit_report))
        deposit_id = "0x" + "33" * 32
        capture_values = {
            "deposit_id": deposit_id,
            "vault_address": leaf.EXPECTED_VAULT_ADDRESS,
            "token_address": leaf.EXPECTED_TOKEN_ADDRESS, "depositor": "0x" + depositor,
            "pftl_recipient": recipient, "amount_atoms": 10_000_000,
            "nonce": "0x" + nonce, "route_binding": "0x" + route,
        }
        calls = []

        def fake_run(argv):
            calls.append(list(argv))
            if argv and argv[0] == "scp" and len(argv) >= 3:
                destination = argv[-1]
                if not destination.startswith("prover.example:"):
                    target = pathlib.Path(destination)
                    if target.name == "proof-calldata.bin":
                        target.write_bytes(b"calldata")
                    elif target.name == "public-values.bin":
                        target.write_bytes(b"public")
                    elif target.name == "remote-proof-report.json":
                        target.write_text(json.dumps({
                            "program_vkey": leaf.EXPECTED_PROGRAM_VKEY,
                            "elf_sha256": leaf.EXPECTED_ELF_SHA256,
                        }))
                    elif target.name == "capture-public-values.json":
                        target.write_text(json.dumps(capture_values))
            return None

        def fake_run_output(argv):
            calls.append(list(argv))
            descriptor = out / "deployment-descriptor.json"
            return SimpleNamespace(stdout=f"{leaf.digest(descriptor)}  deployment.json\\n")

        old_rpc = leaf._rpc_receipt
        old_run_output = leaf.run_output
        leaf._rpc_receipt = lambda _url, _tx: receipt
        leaf.run_output = fake_run_output

        args = argparse.Namespace(
            artifact_dir=str(out),
            witness=None,
            prover_host="prover.example",
            remote_workdir="/work/test campaign",
            ssh_key=None,
            execution_rpc="https://execution.example",
            beacon_rpc="https://beacon.example",
            source_rpc_url=["https://source.example"],
            deposit_tx=None,
        )
        old_run = leaf.run
        leaf.run = fake_run
        try:
            leaf.prove(args)
        finally:
            leaf.run = old_run
            leaf.run_output = old_run_output
            leaf._rpc_receipt = old_rpc
        ssh_calls = [(idx, call[-1]) for idx, call in enumerate(calls) if call and call[0] == "ssh"]
        ssh_commands = [command for _, command in ssh_calls]
        self.assertEqual(len(ssh_commands), 3)
        binary = "/work/test campaign/tools/eth-l1-mainnet-fast-lane-p0/target/release/eth-l1-mainnet-fast-lane-p0"
        descriptor_scp = next(i for i, call in enumerate(calls)
                              if call and call[0] == "scp" and call[-1].endswith("/deployment.json"))
        remote_hash = next(i for i, command in ssh_calls if command.startswith("sha256sum "))
        capture_idx = next(i for i, command in ssh_calls if " capture " in command)
        prove_idx = next(i for i, command in ssh_calls if " prove " in command)
        self.assertLess(descriptor_scp, remote_hash)
        self.assertLess(remote_hash, capture_idx)
        self.assertLess(capture_idx, prove_idx)
        for _, command in ssh_calls[1:]:
            self.assertTrue(command.startswith("SP1_PROVER=cuda "))
            self.assertIn(binary, command)
            self.assertNotIn("cargo run", command)
            self.assertNotIn("&&", command)
            self.assertNotIn("cd ", command)

    def test_t50_prover_wrong_vkey_rejected(self):
        import native_prover_leaf as leaf

        out = self.root / "bad-proof"
        out.mkdir()
        (out / "proof-calldata.bin").write_bytes(b"calldata")
        (out / "public-values.bin").write_bytes(b"public")
        (out / "proof-report.json").write_text(json.dumps({
            "program_vkey": "0xwrong",
            "elf_sha256": leaf.EXPECTED_ELF_SHA256,
            "proof-calldata_sha256": hashlib.sha256(b"calldata").hexdigest(),
            "public-values_sha256": hashlib.sha256(b"public").hexdigest(),
        }))
        with self.assertRaisesRegex(RuntimeError, "program_vkey"):
            leaf.verify_existing(out)


if __name__ == "__main__":
    unittest.main(verbosity=2)
