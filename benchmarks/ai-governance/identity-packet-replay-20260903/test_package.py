from __future__ import annotations

import hashlib
import json
import pathlib
import unittest

ROOT = pathlib.Path(__file__).resolve().parent
CORPUS = ROOT.parent / "validator-identity-packets-20260901"
BEGIN_MARK = "----- BEGIN FROZEN IDENTITY PACKET (exact bytes) -----\n"
END_MARK = "\n----- END FROZEN IDENTITY PACKET -----\n"


def sha(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


class FrozenPackageTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.manifest = json.loads((ROOT / "manifest.json").read_text())
        cls.requests = json.loads((ROOT / "inputs" / "requests.json").read_text())
        cls.corpus_manifest = json.loads((CORPUS / "manifest.json").read_text())
        cls.corpus_index = {
            (row["network"], row["validator_id"]): row
            for row in json.loads((CORPUS / "index.json").read_text())
        }

    def test_counts_and_flags_are_frozen(self) -> None:
        self.assertEqual(
            self.manifest["counts"],
            {"xrpl": 35, "postfiat": 20, "scoring": 55, "padding": 9, "slots": 64, "batches": 2},
        )
        self.assertTrue(self.manifest["shadow_only"])
        self.assertFalse(self.manifest["consensus_input"])
        self.assertFalse(self.manifest["openrouter_used"])
        self.assertFalse(self.manifest["live_web_search"])
        self.assertFalse(self.manifest["agent_rerun"])
        self.assertFalse(self.manifest["profile"]["radix_cache"])
        self.assertTrue(self.manifest["profile"]["deterministic_inference"])
        self.assertTrue(self.manifest["profile"]["loopback_only"])

    def test_manifest_binds_the_frozen_identity_corpus(self) -> None:
        corpus = self.manifest["identity_corpus"]
        hashes = self.corpus_manifest["hashes"]
        self.assertEqual(corpus["artifact"], "validator-identity-packets-20260901")
        self.assertEqual(corpus["packet_set_sha256"], hashes["packet_set_sha256"])
        self.assertEqual(corpus["index_json_sha256"], hashes["index_json_sha256"])
        self.assertEqual(corpus["corpus_manifest_sha256"], sha((CORPUS / "manifest.json").read_bytes()))
        self.assertEqual(sha((CORPUS / "index.json").read_bytes()), hashes["index_json_sha256"])

    def test_input_files_match_manifest_hashes(self) -> None:
        for name, key in (
            ("prompt.txt", "prompt_sha256"),
            ("requests.json", "requests_sha256"),
            ("packet_index.json", "packet_index_sha256"),
            ("batch_schedule.json", "batch_schedule_sha256"),
        ):
            self.assertEqual(sha((ROOT / "inputs" / name).read_bytes()), self.manifest[key], name)

    def test_prompt_defines_every_band_and_the_zero_rule(self) -> None:
        prompt = (ROOT / "inputs" / "prompt.txt").read_text()
        for lower in range(0, 100, 5):
            self.assertIn(f"B{lower:02d}", prompt)
        self.assertIn("score=0", prompt)
        self.assertIn("recognized=false", prompt)
        self.assertIn("sanctions", prompt.lower())
        self.assertIn("prestige", prompt.lower())
        self.assertIn("Layer-1", prompt)
        self.assertIn("SHADOW_ONLY", prompt)
        self.assertIn("does not prove", prompt)

    def test_every_scoring_request_embeds_exact_packet_bytes_and_binds_hashes(self) -> None:
        packet_set = self.corpus_manifest["hashes"]["packet_set_sha256"]
        scoring = [row for row in self.requests if not row["padding"]]
        self.assertEqual(len(scoring), 55)
        self.assertEqual(len(self.requests), 64)
        seen = set()
        for row in scoring:
            key = (row["network"], row["validator_id"])
            self.assertNotIn(key, seen)
            seen.add(key)
            indexed = self.corpus_index[key]
            packet_bytes = (CORPUS / indexed["packet_path"]).read_bytes()
            self.assertEqual(row["packet_path"], indexed["packet_path"])
            self.assertEqual(row["packet_sha256"], indexed["packet_sha256"])
            self.assertEqual(sha(packet_bytes), row["packet_sha256"])
            self.assertEqual(row["corpus_packet_set_sha256"], packet_set)
            user = row["body"]["messages"][1]["content"]
            start = user.index(BEGIN_MARK) + len(BEGIN_MARK)
            end = user.rindex(END_MARK)
            self.assertEqual(user[start:end].encode("utf-8"), packet_bytes)
            self.assertIn(f"packet_sha256: {row['packet_sha256']}\n", user)
            self.assertIn(f"corpus_packet_set_sha256: {packet_set}\n", user)
            self.assertIn(f"validator_id: {row['validator_id']}\n", user)
            self.assertNotIn("index.md", user)
            self.assertNotIn(".jsonl", user)
        self.assertEqual(seen, set(self.corpus_index))

    def test_requests_use_only_the_pinned_local_model_contract(self) -> None:
        for row in self.requests:
            body = row["body"]
            self.assertEqual(body["model"], "Qwen/Qwen3.8-27B-FP8")
            self.assertEqual(body["temperature"], 0)
            self.assertEqual(body["top_p"], 1)
            self.assertEqual(body["chat_template_kwargs"], {"enable_thinking": False})
            self.assertEqual(body["response_format"]["type"], "json_schema")
            self.assertNotIn("url", body)
            self.assertNotIn("api_key", body)
            self.assertNotIn("tools", body)

    def test_batch_schedule_is_two_fixed_batches_of_32(self) -> None:
        batches = json.loads((ROOT / "inputs" / "batch_schedule.json").read_text())
        self.assertEqual([row["slots"] for row in batches], [list(range(32)), list(range(32, 64))])


if __name__ == "__main__":
    unittest.main()
