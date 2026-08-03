import subprocess
import runpy
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = runpy.run_path(str(ROOT / "scripts/nav-reserve-proof-cpu-bounded"))
BOUNDED_ENVIRONMENT = SCRIPT["BOUNDED_ENVIRONMENT"]
bounded_environment = SCRIPT["bounded_environment"]
build_command = SCRIPT["build_command"]
require_docker_access = SCRIPT["require_docker_access"]


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


def test_docker_preflight_accepts_accessible_daemon() -> None:
    observed = []

    def runner(command, **options):
        observed.append((command, options))
        return subprocess.CompletedProcess(command, 0, stdout="ok", stderr="")

    require_docker_access(runner)
    assert observed[0][0] == ["docker", "info"]
    assert observed[0][1]["timeout"] == 20


def test_docker_preflight_rejects_stale_systemd_groups() -> None:
    def runner(command, **_options):
        return subprocess.CompletedProcess(
            command,
            1,
            stdout="",
            stderr="permission denied while trying to connect to the docker API",
        )

    try:
        require_docker_access(runner)
    except RuntimeError as error:
        message = str(error)
    else:
        raise AssertionError("inaccessible Docker daemon was accepted")
    assert "before SP1 proving" in message
    assert "stale supplementary groups" in message
    assert "sg docker -c" in message
