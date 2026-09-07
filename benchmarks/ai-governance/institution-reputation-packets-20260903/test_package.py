from __future__ import annotations

import hashlib
import json
import pathlib
import unittest

ROOT = pathlib.Path(__file__).resolve().parent
CORPUS = ROOT.parent / "validator-identity-packets-20260901"


def sha(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


class FrozenPacketPackageTests(unittest.TestCase):
    def test_counts_and_boundaries_are_frozen(self) -> None:
        manifest = json.loads((ROOT / "manifest.json").read_text())
        counts = manifest["counts"]
        self.assertEqual(
            {k: counts[k] for k in ("xrpl", "postfiat", "scoring", "padding", "slots")},
            {"xrpl": 35, "postfiat": 20, "scoring": 55, "padding": 9, "slots": 64},
        )
        self.assertTrue(manifest["shadow_only"])
        self.assertFalse(manifest["openrouter_used"])
        self.assertFalse(manifest["live_search_used"])
        self.assertFalse(manifest["corbanu_rerun"])

    def test_package_binds_the_frozen_corpus_hashes(self) -> None:
        manifest = json.loads((ROOT / "manifest.json").read_text())
        corpus_manifest = json.loads((CORPUS / "manifest.json").read_text())
        self.assertEqual(
            manifest["identity_corpus"]["packet_set_sha256"],
            corpus_manifest["hashes"]["packet_set_sha256"],
        )
        self.assertEqual(
            manifest["identity_corpus"]["index_json_sha256"],
            sha((CORPUS / "index.json").read_bytes()),
        )

    def test_every_request_carries_exact_packet_bytes_and_hash(self) -> None:
        requests = json.loads((ROOT / "inputs" / "requests.json").read_text())
        index = {row["validator_id"]: row for row in json.loads((CORPUS / "index.json").read_text())}
        self.assertEqual(len(requests), 64)
        scoring = [row for row in requests if not row["padding"]]
        self.assertEqual(len(scoring), 55)
        for row in scoring:
            entry = index[row["validator_id"]]
            packet = (CORPUS / entry["packet_path"]).read_bytes()
            self.assertEqual(row["packet_sha256"], entry["packet_sha256"])
            self.assertEqual(sha(packet), row["packet_sha256"])
            content = row["body"]["messages"][1]["content"]
            self.assertTrue(content.endswith(packet.decode("utf-8")))
            self.assertIn(row["packet_sha256"], content)
            self.assertEqual(row["body"]["model"], "Qwen/Qwen3.8-27B-FP8")
            self.assertEqual(row["body"]["temperature"], 0)
            self.assertFalse(row["body"]["chat_template_kwargs"]["enable_thinking"])
            self.assertNotIn("url", row["body"])
            self.assertNotIn("api_key", row["body"])

    def test_prompt_defines_every_five_point_band_and_zero_rule(self) -> None:
        prompt = (ROOT / "inputs" / "prompt.txt").read_text()
        for lower in range(0, 100, 5):
            self.assertIn(f"B{lower:02d}", prompt)
        self.assertIn("score=0", prompt)
        self.assertIn("sanctions", prompt.lower())
        self.assertIn("prestige", prompt.lower())
        self.assertIn("Layer-1", prompt)
        self.assertIn("must not raise the score", prompt)

    def test_input_files_match_manifest_hashes(self) -> None:
        manifest = json.loads((ROOT / "manifest.json").read_text())
        for name, key in (
            ("prompt.txt", "prompt_sha256"),
            ("requests.json", "requests_sha256"),
            ("batch_schedule.json", "batch_schedule_sha256"),
            ("packets.json", "packets_sha256"),
        ):
            self.assertEqual(sha((ROOT / "inputs" / name).read_bytes()), manifest[key], name)


if __name__ == "__main__":
    unittest.main()
