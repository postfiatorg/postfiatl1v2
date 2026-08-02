import runpy
from pathlib import Path

import pytest


ROOT = Path(__file__).resolve().parents[2]
CHECKER = runpy.run_path(str(ROOT / "scripts/check-a666-public-adapter-readiness"))
validate_deprecation_state = CHECKER["validate_deprecation_state"]


def passing_plan() -> str:
    return "\n".join(f"| `G{gate}` gate | condition | PASS |" for gate in range(8))


def test_qualified_adapters_do_not_force_complete_deprecation() -> None:
    validate_deprecation_state(
        deprecated=False,
        all_qualified=True,
        plan=passing_plan(),
    )


def test_deprecation_requires_every_adapter() -> None:
    with pytest.raises(SystemExit, match="all six adapters are qualified"):
        validate_deprecation_state(
            deprecated=True,
            all_qualified=False,
            plan=passing_plan(),
        )


def test_deprecation_requires_every_canonical_acceptance_gate() -> None:
    incomplete_plan = "\n".join(
        f"| `G{gate}` gate | condition | {'OPEN' if gate == 7 else 'PASS'} |"
        for gate in range(8)
    )
    with pytest.raises(SystemExit, match="records a passing G7"):
        validate_deprecation_state(
            deprecated=True,
            all_qualified=True,
            plan=incomplete_plan,
        )


def test_deprecation_accepts_all_adapters_and_all_gates() -> None:
    validate_deprecation_state(
        deprecated=True,
        all_qualified=True,
        plan=passing_plan(),
    )
