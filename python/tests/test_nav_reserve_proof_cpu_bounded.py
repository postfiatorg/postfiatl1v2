import runpy
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = runpy.run_path(str(ROOT / "scripts/nav-reserve-proof-cpu-bounded"))
BOUNDED_ENVIRONMENT = SCRIPT["BOUNDED_ENVIRONMENT"]
bounded_environment = SCRIPT["bounded_environment"]
build_command = SCRIPT["build_command"]


def test_bounded_environment_replaces_unsafe_worker_overrides() -> None:
    environment = bounded_environment(
        {
            "KEEP_ME": "yes",
            "SP1_WORKER_NUM_RECURSION_PROVER_WORKERS": "99",
            "RAYON_NUM_THREADS": "99",
        }
    )
    assert environment["KEEP_ME"] == "yes"
    assert environment["SP1_WORKER_NUM_RECURSION_PROVER_WORKERS"] == "1"
    assert environment["RAYON_NUM_THREADS"] == "6"
    assert all(environment[key] == value for key, value in BOUNDED_ENVIRONMENT.items())


def test_build_command_preserves_exact_artifact_paths() -> None:
    command = build_command(
        prover=Path("/proof-kit/postfiat-reserve-proof"),
        witness=Path("/evidence/witness.cbor"),
        elf=Path("/identity/guest.elf"),
        output_dir=Path("/evidence/proof"),
    )
    assert command == [
        "/proof-kit/postfiat-reserve-proof",
        "prove",
        "--witness",
        "/evidence/witness.cbor",
        "--elf",
        "/identity/guest.elf",
        "--output-dir",
        "/evidence/proof",
    ]
