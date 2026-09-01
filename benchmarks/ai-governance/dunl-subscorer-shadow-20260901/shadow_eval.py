#!/usr/bin/env python3
"""Shadow evaluation of the deterministic sub-scorers against rounds 12-19.

For every fetched frozen testnet round this script:

1. computes deterministic sub-scores for every validator from the round's
   frozen inputs (``subscorer.py`` v1);
2. runs the fork's own ``compute_final_score`` and ``select_unl``
   (the pinned module files of the read-only ``dynamic-unl-scoring``
   clone, loaded directly so no third-party dependency is needed) over
   those sub-scores, using the round's frozen selector parameters and
   frozen previous UNL from its own manifest/inputs; and
3. compares against the round's published outputs: per-dimension mean/max
   absolute delta vs the model's sub-scores, final-score deltas, the number
   of validators crossing the score-40 cutoff in either direction (the
   headline number), UNL overlap and seats changed vs the published
   selection, and cutoff-boundary cases (final within 5 of 40 under either
   scorer) listed individually.

The per-era baseline is the round's published authoritative score: the
score formula over the model's sub-scores where the round's manifest pins
``code.score_formula`` (rounds 16+), the model's overall score before that
(rounds 12-15). As an internal control the baseline scores are also fed
through the selector to confirm the published UNL reproduces exactly under
the frozen parameters before the deterministic candidate is compared.

Writes ``results.json`` (machine-readable) and ``results-tables.md``
(per-round tables). No network, no model, no third-party dependencies:
the two fork modules whose logic runs (``score_formula``,
``unl_selector``) are loaded from their pinned files with local plain
data carriers standing in for the parser's ``ScoringResult`` and
``ValidatorScore`` (field-identical, no validation logic to replicate)
and a fail-loud stand-in for the fork settings, which the evaluation
must never read because every selector parameter comes from the round's
manifest. This keeps the evaluation and its guard tests runnable under
a bare interpreter.

Fail-loud guards (regression-tested in ``test_shadow_eval.py``): the
pinned fork module files must match the round's manifest content-hash
pins,
the round's pinned ``score_cutoff`` must equal the flip line, and the
frozen-entry and published-model validator sets must match exactly —
any mismatch raises instead of narrowing the comparison silently.

``--scorer-version 2`` evaluates ``subscorer_v2.py`` instead (era-aware
consensus: the round's frozen prompt version is passed to the scorer),
writes ``results-v2.json`` / ``results-tables-v2.md``, and embeds a
per-round v1-vs-v2 comparison read from the immutable ``results.json``.

Usage: ``.venv/bin/python shadow_eval.py [--scorer-version {1,2}]``
"""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import statistics
import sys
import types
from dataclasses import dataclass, field
from pathlib import Path

HERE = Path(__file__).resolve().parent
FORK_ROOT = Path.home() / "repos" / "dynamic-unl-scoring"
FORK_SERVICES = FORK_ROOT / "scoring_service" / "services"
sys.path.insert(0, str(HERE))

import subscorer  # noqa: E402
import subscorer_v2  # noqa: E402


@dataclass
class ValidatorScore:
    """Field-identical stand-in for the pinned parser's ``ValidatorScore``.

    The pinned class declares these fields with no validators or coercion,
    so a plain carrier is behaviorally identical for the values this
    evaluation constructs.
    """

    master_key: str
    score: int
    consensus: int
    reliability: int
    software: int
    diversity: int
    identity: int
    reasoning: str


@dataclass
class ScoringResult:
    """Field-identical stand-in for the pinned parser's ``ScoringResult``."""

    validator_scores: list[ValidatorScore]
    raw_response: str
    complete: bool
    errors: list[str] = field(default_factory=list)


class _UnusedForkSettings:
    """The fork settings must never be read by this evaluation.

    ``select_unl`` falls back to settings only for parameters not passed
    explicitly; the evaluation always passes the round's frozen manifest
    parameters, so any settings read is a bug and fails loudly.
    """

    def __getattr__(self, name: str):
        raise AttributeError(
            f"fork settings.{name} was read during the shadow evaluation; "
            "every selector parameter must come from the round's manifest"
        )


def _register_stub_module(name: str) -> types.ModuleType:
    module = types.ModuleType(name)
    sys.modules[name] = module
    parent_name, _, child = name.rpartition(".")
    if parent_name:
        setattr(sys.modules[parent_name], child, module)
    return module


def _load_pinned_module(name: str, path: Path) -> types.ModuleType:
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    parent_name, _, child = name.rpartition(".")
    if parent_name:
        setattr(sys.modules[parent_name], child, module)
    spec.loader.exec_module(module)
    return module


# The real files behind every module name a round manifest may pin. Pin
# verification hashes these exact files; the loader below executes the
# score-formula and selector files directly, so the logic that runs is
# the pinned fork code without the fork package's optional dependencies.
_PINNED_MODULE_FILES = {
    "scoring_service.services.response_parser": FORK_SERVICES / "response_parser.py",
    "scoring_service.services.score_formula": FORK_SERVICES / "score_formula.py",
    "scoring_service.services.unl_selector": FORK_SERVICES / "unl_selector.py",
}

_register_stub_module("scoring_service")
_register_stub_module("scoring_service.services")
_register_stub_module("scoring_service.config").settings = _UnusedForkSettings()
_parser_stub = _register_stub_module("scoring_service.services.response_parser")
_parser_stub.ValidatorScore = ValidatorScore
_parser_stub.ScoringResult = ScoringResult

compute_final_score = _load_pinned_module(
    "scoring_service.services.score_formula",
    _PINNED_MODULE_FILES["scoring_service.services.score_formula"],
).compute_final_score
select_unl = _load_pinned_module(
    "scoring_service.services.unl_selector",
    _PINNED_MODULE_FILES["scoring_service.services.unl_selector"],
).select_unl

ROUNDS = tuple(range(12, 20))
CUTOFF_LINE = 40
BOUNDARY_MARGIN = 5
DIMENSIONS = subscorer.DIMENSIONS


def _load(round_dir: Path, rel_path: str) -> dict:
    return json.loads((round_dir / rel_path).read_text())


def _verify_code_pins(code: dict) -> None:
    """Fail loudly if a pinned fork module drifted from the round's pins.

    Every round's execution manifest content-hash-pins the parser, selector,
    and (formula-era) score-formula modules. The evaluation loads today's
    fork clone; if that clone has moved past the pinned content, the numbers
    it computes would no longer describe the frozen round.
    """
    for key in ("parser", "selector", "score_formula"):
        pin = code.get(key)
        if pin is None:
            continue
        module_file = _PINNED_MODULE_FILES.get(pin["module"])
        if module_file is None:
            raise ValueError(
                f"round pins unrecognized fork module {pin['module']!r}"
            )
        digest = hashlib.sha256(module_file.read_bytes()).hexdigest()
        if digest != pin["content_sha256"]:
            raise ValueError(
                f"fork module {pin['module']} (sha256 {digest}) does not match "
                f"the round's pinned {key} hash {pin['content_sha256']}; the "
                "fork clone has drifted from the frozen round"
            )


def _select(finals: dict[str, int], previous_unl: list[str], params: dict) -> list[str]:
    result = ScoringResult(
        validator_scores=[
            ValidatorScore(
                master_key=mk, score=score, consensus=0, reliability=0,
                software=0, diversity=0, identity=0, reasoning="",
            )
            for mk, score in finals.items()
        ],
        raw_response="", complete=True, errors=[],
    )
    return select_unl(
        result,
        previous_unl=previous_unl,
        cutoff=params["score_cutoff"],
        max_size=params["max_size"],
        min_gap=params["min_score_gap"],
    ).unl


def evaluate_round(round_dir: Path, scorer_version: int = 1) -> dict:
    manifest = _load(round_dir, "runtime/execution_manifest.json")
    code = manifest["code"]
    _verify_code_pins(code)
    params = code["selector"]["parameters"]
    if params["score_cutoff"] != CUTOFF_LINE:
        raise ValueError(
            f"{round_dir.name} pins score_cutoff {params['score_cutoff']} but "
            f"this evaluation computes cutoff flips against {CUTOFF_LINE}"
        )
    formula_era = "score_formula" in code
    previous_unl = _load(round_dir, "inputs/previous_unl.json")["previous_unl"]
    published_unl = _load(round_dir, "outputs/selected_unl.json")["unl"]
    validator_map = _load(round_dir, "inputs/validator_map.json")
    model_scores = {
        v["master_key"]: v
        for v in _load(round_dir, "outputs/validator_scores.json")["validator_scores"]
    }

    entries = subscorer.load_round_entries(round_dir)
    if scorer_version == 2:
        det_by_vid = subscorer_v2.score_round(entries, code["prompt"]["version"])
    else:
        det_by_vid = subscorer.score_round(entries)
    if len(det_by_vid) != len(entries):
        raise ValueError(
            f"{round_dir.name}: duplicate validator_id in the frozen entries "
            f"collapsed {len(entries)} entries to {len(det_by_vid)} scores"
        )
    det = {
        validator_map[vid]["master_key"]: dims for vid, dims in det_by_vid.items()
    }
    if len(det) != len(det_by_vid):
        raise ValueError(
            f"{round_dir.name}: duplicate master_key in validator_map "
            f"collapsed {len(det_by_vid)} scores to {len(det)}"
        )
    frozen_only = sorted(set(det) - set(model_scores))
    model_only = sorted(set(model_scores) - set(det))
    if frozen_only or model_only:
        raise ValueError(
            f"{round_dir.name}: validator sets differ between the frozen "
            f"entries and the published model scores (frozen-only "
            f"{frozen_only}, model-only {model_only})"
        )

    common = sorted(set(det) & set(model_scores))
    report: dict = {
        "round": round_dir.name,
        "prompt_version": code["prompt"]["version"],
        "selector_parameters": params,
        "baseline": "formula(model sub-scores)" if formula_era else "model overall score",
        "validators": len(entries),
        "validators_compared": len(common),
    }

    # Per-dimension deltas: deterministic vs model sub-scores.
    report["dimension_deltas"] = {
        dim: {
            "mean_abs": round(
                statistics.mean(
                    abs(det[mk][dim] - model_scores[mk][dim]) for mk in common
                ),
                2,
            ),
            "max_abs": max(abs(det[mk][dim] - model_scores[mk][dim]) for mk in common),
        }
        for dim in DIMENSIONS
    }

    # Final scores: deterministic candidate vs the round's published
    # authoritative baseline.
    det_final = {
        mk: compute_final_score(*(det[mk][dim] for dim in DIMENSIONS)) for mk in common
    }
    if formula_era:
        base_final = {
            mk: compute_final_score(
                *(model_scores[mk][dim] for dim in DIMENSIONS)
            )
            for mk in common
        }
    else:
        base_final = {mk: model_scores[mk]["score"] for mk in common}

    final_deltas = sorted(abs(det_final[mk] - base_final[mk]) for mk in common)
    report["final_score_deltas"] = {
        "mean_abs": round(statistics.mean(final_deltas), 2),
        "max_abs": final_deltas[-1],
    }

    # Cutoff flips (the headline number).
    flips_out = sorted(
        mk for mk in common
        if base_final[mk] >= CUTOFF_LINE > det_final[mk]
    )
    flips_in = sorted(
        mk for mk in common
        if det_final[mk] >= CUTOFF_LINE > base_final[mk]
    )
    report["cutoff_flips"] = {
        "eligible_to_ineligible": len(flips_out),
        "ineligible_to_eligible": len(flips_in),
        "total": len(flips_out) + len(flips_in),
        "flipped_validators": [
            {
                "master_key": mk,
                "direction": direction,
                "baseline_final": base_final[mk],
                "deterministic_final": det_final[mk],
            }
            for direction, keys in (
                ("eligible->ineligible", flips_out),
                ("ineligible->eligible", flips_in),
            )
            for mk in keys
        ],
    }

    # Internal control: the baseline scores must reproduce the published UNL
    # under the round's own frozen parameters and previous UNL.
    control_unl = _select(base_final, previous_unl, params)
    report["baseline_reproduces_published_unl"] = set(control_unl) == set(published_unl)

    # Deterministic candidate selection vs the published UNL.
    det_unl = _select(det_final, previous_unl, params)
    report["published_unl_size"] = len(published_unl)
    report["deterministic_unl_size"] = len(det_unl)
    report["unl_overlap"] = len(set(det_unl) & set(published_unl))
    report["unl_seats_changed"] = len(set(det_unl) ^ set(published_unl)) // 2
    report["seats_gained"] = sorted(set(det_unl) - set(published_unl))
    report["seats_lost"] = sorted(set(published_unl) - set(det_unl))

    # Cutoff-boundary cases: final within BOUNDARY_MARGIN of the cutoff
    # under either scorer, listed individually.
    report["cutoff_boundary_cases"] = [
        {
            "master_key": mk,
            "baseline_final": base_final[mk],
            "deterministic_final": det_final[mk],
            "model_subscores": {dim: model_scores[mk][dim] for dim in DIMENSIONS},
            "deterministic_subscores": {dim: det[mk][dim] for dim in DIMENSIONS},
            "flipped": base_final[mk] >= CUTOFF_LINE > det_final[mk]
            or det_final[mk] >= CUTOFF_LINE > base_final[mk],
        }
        for mk in common
        if abs(base_final[mk] - CUTOFF_LINE) <= BOUNDARY_MARGIN
        or abs(det_final[mk] - CUTOFF_LINE) <= BOUNDARY_MARGIN
    ]
    return report


def _round_table(report: dict) -> str:
    dims = report["dimension_deltas"]
    lines = [
        f"### {report['round']} (prompt {report['prompt_version']}, "
        f"baseline: {report['baseline']}, gap {report['selector_parameters']['min_score_gap']})",
        "",
        "| Metric | Value |",
        "|---|---|",
        f"| Validators compared | {report['validators_compared']} |",
        *(
            f"| {dim} mean/max abs delta | {dims[dim]['mean_abs']} / {dims[dim]['max_abs']} |"
            for dim in DIMENSIONS
        ),
        f"| Final score mean/max abs delta | {report['final_score_deltas']['mean_abs']} "
        f"/ {report['final_score_deltas']['max_abs']} |",
        f"| Cutoff flips (out / in) | {report['cutoff_flips']['eligible_to_ineligible']} "
        f"/ {report['cutoff_flips']['ineligible_to_eligible']} |",
        f"| Baseline reproduces published UNL | {report['baseline_reproduces_published_unl']} |",
        f"| UNL overlap / seats changed | {report['unl_overlap']}"
        f"/{report['published_unl_size']} / {report['unl_seats_changed']} |",
        "",
    ]
    if report["cutoff_boundary_cases"]:
        lines += [
            "Cutoff-boundary cases (final within 5 of 40 under either scorer):",
            "",
            "| Validator | Baseline final | Deterministic final | Flipped |",
            "|---|---|---|---|",
            *(
                f"| `{c['master_key'][:12]}...` | {c['baseline_final']} "
                f"| {c['deterministic_final']} | {'YES' if c['flipped'] else 'no'} |"
                for c in report["cutoff_boundary_cases"]
            ),
            "",
        ]
    else:
        lines += ["No cutoff-boundary cases.", ""]
    return "\n".join(lines)


def _v1_comparison(reports: list[dict]) -> dict | None:
    """Per-round v2-vs-v1 comparison, read from the immutable results.json."""
    v1_path = HERE / "results.json"
    if not v1_path.exists():
        return None
    v1_rounds = {r["round"]: r for r in json.loads(v1_path.read_text())["rounds"]}
    rounds = []
    for r in reports:
        v1 = v1_rounds.get(r["round"])
        if v1 is None:
            continue
        rounds.append({
            "round": r["round"],
            "cutoff_flips_v1": v1["cutoff_flips"]["total"],
            "cutoff_flips_v2": r["cutoff_flips"]["total"],
            "unl_overlap_v1": v1["unl_overlap"],
            "unl_overlap_v2": r["unl_overlap"],
            "published_unl_size": r["published_unl_size"],
            "dimension_mean_abs_delta": {
                dim: {
                    "v1": v1["dimension_deltas"][dim]["mean_abs"],
                    "v2": r["dimension_deltas"][dim]["mean_abs"],
                }
                for dim in DIMENSIONS
            },
        })
    return {
        "baseline_results": "results.json (sub-scorers v1, immutable)",
        "rounds": rounds,
        "total_cutoff_flips_v1": sum(r["cutoff_flips_v1"] for r in rounds),
        "total_cutoff_flips_v2": sum(r["cutoff_flips_v2"] for r in rounds),
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--scorer-version", type=int, choices=(1, 2), default=1)
    scorer_version = parser.parse_args().scorer_version
    suffix = "" if scorer_version == 1 else f"-v{scorer_version}"

    rounds_dir = HERE / "rounds"
    reports, failures = [], []
    for round_number in ROUNDS:
        round_dir = rounds_dir / f"testnet-r{round_number}"
        if not round_dir.exists():
            failures.append(f"testnet-r{round_number}: not fetched")
            continue
        try:
            reports.append(evaluate_round(round_dir, scorer_version))
        except FileNotFoundError as exc:
            failures.append(f"testnet-r{round_number}: missing artifact {exc.filename}")

    results = {
        "evaluation": (
            f"deterministic sub-scorers v{scorer_version} shadow evaluation"
        ),
        "cutoff_line": CUTOFF_LINE,
        "boundary_margin": BOUNDARY_MARGIN,
        "rounds": reports,
        "failures": failures,
        "headline": {
            "total_cutoff_flips": sum(r["cutoff_flips"]["total"] for r in reports),
            "rounds_evaluated": len(reports),
        },
    }
    if scorer_version != 1:
        comparison = _v1_comparison(reports)
        if comparison is not None:
            results["v1_comparison"] = comparison
    (HERE / f"results{suffix}.json").write_text(json.dumps(results, indent=1) + "\n")

    tables = [
        f"# Deterministic sub-scorer v{scorer_version} shadow evaluation "
        "— per-round tables"
        if scorer_version != 1
        else "# Deterministic sub-scorer shadow evaluation — per-round tables",
        "",
        f"Rounds evaluated: {len(reports)}; fetch/evaluation failures: "
        f"{failures if failures else 'none'}.",
        "",
        *(_round_table(r) for r in reports),
    ]
    (HERE / f"results-tables{suffix}.md").write_text("\n".join(tables))

    for r in reports:
        print(
            f"{r['round']}: flips {r['cutoff_flips']['total']} "
            f"(out {r['cutoff_flips']['eligible_to_ineligible']}, "
            f"in {r['cutoff_flips']['ineligible_to_eligible']}), "
            f"UNL overlap {r['unl_overlap']}/{r['published_unl_size']}, "
            f"control {'OK' if r['baseline_reproduces_published_unl'] else 'MISMATCH'}"
        )
    if failures:
        print("failures:", failures)
    print(f"total cutoff flips: {results['headline']['total_cutoff_flips']}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
