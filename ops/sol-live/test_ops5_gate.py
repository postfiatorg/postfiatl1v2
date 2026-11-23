from pathlib import Path
import tempfile
import unittest


class FifthReview(unittest.TestCase):
    def test_release_gate_counts_every_discovered_review_case(self):
        from exit_review_gate import run_gate

        names = [
            f"tests::solana_exit::exit_review{v}_case{i}"
            for v, n in [("", 9), ("4", 8), ("5", 1)]
            for i in range(n)
        ]
        expected = [n.split("::")[-1] for n in names]
        listing = "".join(name + ": test\n" for name in names)
        with tempfile.TemporaryDirectory() as d:

            def execute(args):
                return (
                    listing
                    if "--list" in args
                    else "test result: ok. 1 passed; 0 failed; 0 ignored;"
                )

            self.assertEqual(run_gate(execute, Path(d), expected), len(names))
            with self.assertRaises(ValueError):
                run_gate(
                    lambda args: (
                        listing
                        if "--list" in args
                        else "test result: ok. 0 passed; 0 failed;"
                    ),
                    Path(d),
                    expected,
                )
            with self.assertRaises(ValueError):
                run_gate(
                    lambda args: (
                        listing.replace("exit_review4_", "removed_")
                        if "--list" in args
                        else execute(args)
                    ),
                    Path(d),
                    expected,
                )

    def test_release_finds_cases_outside_the_node_library(self):
        from exit_review_gate import source_cases, run_gate

        with tempfile.TemporaryDirectory() as d:
            root = Path(d)
            for pkg, name in [
                ("node", "exit_review4_one"),
                ("execution", "exit_review7_two"),
            ]:
                crate = root / "crates" / pkg
                (crate / "src").mkdir(parents=True)
                (crate / "Cargo.toml").write_text(f'[package]\nname = "postfiat-{pkg}"')
                (crate / "src" / "tests.rs").write_text(f"#[test]\nfn {name}() {{}}")
            cases = source_cases(root)
            self.assertEqual(
                cases,
                {
                    "postfiat-node": ["exit_review4_one"],
                    "postfiat-execution": ["exit_review7_two"],
                },
            )
            with self.assertRaisesRegex(ValueError, "source"):
                run_gate(lambda _: "", root, cases["postfiat-execution"])
