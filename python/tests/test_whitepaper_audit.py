"""Behavioral checks for the offline audit reader and integrity gate."""

from copy import deepcopy
from contextlib import redirect_stderr, redirect_stdout
import io
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

from postfiat_rpc import whitepaper_audit as audit


class WhitepaperAuditTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.document = json.loads((audit.REPOSITORY / audit.INVENTORY).read_text())

    def test_retained_inventory_covers_original_paper_and_evidence_register(self):
        document = audit.validate(deepcopy(self.document))
        original = audit.git_bytes(audit.REPOSITORY, document["baseline"]["commit"], "docs/whitepaper.md").decode()
        covered = {section for claim in document["claims"] for section in claim["sections"]}
        self.assertEqual(covered, set(audit.sections(original)))

    def test_missing_section_cannot_pass_as_complete(self):
        document = deepcopy(self.document)
        document["claims"] = [row for row in document["claims"] if "5.4" not in row["sections"]]
        with self.assertRaisesRegex(audit.AuditError, "uncovered.*5.4"):
            audit.validate(document)

    def test_appendix_entry_cannot_be_hidden_by_general_appendix_coverage(self):
        document = deepcopy(self.document)
        document["claims"] = [row for row in document["claims"] if not row["locator"].startswith("**[E7]")]
        with self.assertRaisesRegex(audit.AuditError, "E7"):
            audit.validate(document)

    def test_mutated_paper_identity_fails_closed(self):
        document = deepcopy(self.document)
        document["baseline"]["sha256"] = "0" * 64
        with self.assertRaisesRegex(audit.AuditError, "SHA-256 mismatch"):
            audit.validate(document)

    def test_missing_symbol_and_missing_baseline_file_fail(self):
        for field, value in (("anchor", "nonexistent_audit_symbol_ef25c1"), ("path", "crates/nonexistent_audit.rs")):
            with self.subTest(field=field):
                document = deepcopy(self.document)
                document["claims"][0]["evidence"][0][field] = value
                with self.assertRaises(audit.AuditError):
                    audit.validate(document)

    def test_unknown_schema_duplicate_ids_and_unsupported_status_fail(self):
        for mutation in ("schema", "id", "status", "plane"):
            with self.subTest(mutation=mutation):
                document = deepcopy(self.document)
                if mutation == "schema":
                    document["schema"] = "postfiat.whitepaper-audit.v999"
                elif mutation == "id":
                    document["claims"][1]["id"] = document["claims"][0]["id"]
                else:
                    document["claims"][0][mutation] = ["not-a-string"]
                with self.assertRaises(audit.AuditError):
                    audit.validate(document)

    def test_claim_locator_must_belong_to_declared_section(self):
        document = deepcopy(self.document)
        document["claims"][0]["sections"] = ["9.1"]
        with self.assertRaisesRegex(audit.AuditError, "outside declared"):
            audit.validate(document)

    def test_implemented_and_tested_requires_actual_test_reference(self):
        document = deepcopy(self.document)
        row = next(row for row in document["claims"] if row["status"] == "implemented-and-tested")
        for reference in row["evidence"]:
            reference["role"] = "code"
        with self.assertRaisesRegex(audit.AuditError, "requires a test reference"):
            audit.validate(document)

    def test_repository_paths_cannot_escape(self):
        for path in ("/tmp/secret", "../secret", "docs/../../secret", "docs/./x", "docs\\x"):
            with self.subTest(path=path), self.assertRaises(audit.AuditError):
                audit._path(path)

    def test_section_filter_json_and_unknown_section_exit_status(self):
        output = io.StringIO()
        with redirect_stdout(output):
            self.assertEqual(audit.main(["--section", "7.4", "--json"]), 0)
        result = json.loads(output.getvalue())
        self.assertTrue(result["claims"])
        self.assertTrue(all("7.4" in row["sections"] for row in result["claims"]))
        error = io.StringIO()
        with redirect_stderr(error):
            self.assertEqual(audit.main(["--section", "700"]), 1)
        self.assertIn("unknown section", error.getvalue())

    def test_malformed_file_has_clear_error_without_traceback(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "inventory.json"
            path.write_text("{broken")
            output = io.StringIO()
            with redirect_stderr(output):
                self.assertEqual(audit.main(["--inventory", str(path), "--json"]), 1)
            self.assertIn("cannot read inventory", output.getvalue())
            self.assertNotIn("Traceback", output.getvalue())

    def test_integrity_gate_detects_stale_rendering_and_download(self):
        document = audit.validate(deepcopy(self.document))
        rendered = audit.render_markdown(document)
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for relative in (audit.MATRIX, "docs/whitepaper.md", "docs/assets/raw/whitepaper.md.txt"):
                (root / relative).parent.mkdir(parents=True, exist_ok=True)
            (root / audit.MATRIX).write_text("stale")
            (root / "docs/whitepaper.md").write_text("canonical")
            (root / "docs/assets/raw/whitepaper.md.txt").write_text("stale")
            with patch.object(audit, "REPOSITORY", root), patch.object(audit, "load_inventory", return_value=document), patch.object(audit, "render_markdown", return_value=rendered):
                for expected in ("table is stale", "downloadable whitepaper differs"):
                    output = io.StringIO()
                    with redirect_stderr(output):
                        self.assertEqual(audit.main(["--check"]), 1)
                    self.assertIn(expected, output.getvalue())
                    (root / audit.MATRIX).write_text(rendered)
                (root / "docs/assets/raw/whitepaper.md.txt").write_text("canonical")
                with redirect_stdout(io.StringIO()):
                    self.assertEqual(audit.main(["--check"]), 0)


if __name__ == "__main__":
    unittest.main()
