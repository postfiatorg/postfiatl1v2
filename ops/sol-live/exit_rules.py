"""The Solana exit's rules as the chain reports them in its route report.

A chain on the first release (71dbc9b3) reports none of the second release's fields. Then
only the revision-8 bucket exits, every decision needs every validator, and no minimum
applies. From the second release's height the report carries `covered_bucket_ids`,
`decision_signatures` and `min_withdrawal_atoms`, and the tools follow those values. The
same tools therefore work before and after the second release.
"""

import re

BUCKET = "4c045e289eaaa2c7ae7592b96ec0eeabe9fa0cfdd032783d359409dcc08d9dd0dfdd7cc8d483d6a88c12dc713522d2cc"
ACTIONS = ("open", "close", "paid", "min_withdrawal", "retire")
# Reported from the build that carries the refund decision; older second-release builds omit it.
OPTIONAL_ACTIONS = ("refund",)
# The first release's canary amount, 0.02 USDC; it has no governed minimum.
FIRST_RELEASE_CANARY_ATOMS = 20_000


def require(ok, message):
    if not ok:
        raise ValueError(message)


def first_release(n=6):
    # The first release has no refund decision.
    return dict(release=1, covered=(BUCKET,), signatures={a: n for a in ACTIONS}, minimum=None)


def of_route(route, n=6):
    """The rules one validator's route report states."""
    fields = ("covered_bucket_ids", "decision_signatures", "min_withdrawal_atoms")
    present = [k for k in fields if route.get(k) is not None]
    if not present:
        return first_release(n)
    require(len(present) == len(fields), "Route report states only part of the exit rules.")
    covered = route["covered_bucket_ids"]
    signatures = route["decision_signatures"]
    minimum = route["min_withdrawal_atoms"]
    require(
        isinstance(covered, list)
        and BUCKET in covered
        and all(isinstance(b, str) and re.fullmatch(r"[0-9a-f]{96}", b) for b in covered),
        "Route report has invalid covered buckets.",
    )
    require(
        isinstance(signatures, dict)
        and set(ACTIONS) <= set(signatures) <= set(ACTIONS + OPTIONAL_ACTIONS)
        and all(type(v) is int and 0 < v <= n for v in signatures.values()),
        "Route report has invalid decision signatures.",
    )
    require(type(minimum) is int and minimum > 0, "Route report has an invalid minimum.")
    return dict(
        release=2, covered=tuple(sorted(covered)), signatures=dict(signatures), minimum=minimum
    )


def agreed(routes, n=6):
    """The rules every given validator reports; the caller passes its agreeing group."""
    rules = [of_route(r, n) for r in routes]
    require(rules and all(r == rules[0] for r in rules), "Validators disagree on the exit rules.")
    return rules[0]


def signatures(rules, kind):
    """Signatures a wrapper kind (`exit-open`, `exit-close`, `exit-paid`, ...) needs now."""
    action = kind.removeprefix("exit-").replace("-", "_")
    require(action in rules["signatures"], f"The chain reports no {action} decision.")
    return rules["signatures"][action]


def canary_atoms(rules):
    """The canary burns the governed minimum, or the first release's 0.02 USDC."""
    return rules["minimum"] or FIRST_RELEASE_CANARY_ATOMS
