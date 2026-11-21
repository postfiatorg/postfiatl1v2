"""Export the certified public block containing the named vault deposit."""

from pathlib import Path
import re
import subprocess
import sys

slot = int(sys.argv[1])
signature, vault, binding = sys.argv[2:5]
base = Path("/work/g")
binary = base / "sol-live/pfusdc-sol-capture"
captured = base / "main-ledger/public-capture"
out = base / "sol-live/receipts"
out.mkdir(exist_ok=True, parents=True)
if not (out / str(slot) / "certificate").exists():
    # The custody hook stores subsequent footer-only captures under carriers/.
    targets = {p.name.partition("-")[2]: p for p in captured.glob(f"{slot}-*")}
    if not targets:
        sys.exit(f"awaiting custody capture at slot {slot}")
    # Prefer a certificate for the deposit's own block. Not every block gets one: fall back to
    # the nearest later certificate, whose block commits to this one through its parent links.
    def certificates():
        for later in range(slot + 1, slot + 65):
            for directory in (captured, captured / "carriers"):
                for carrier in directory.glob(f"{later}-*"):
                    probe = subprocess.run(
                        [binary, "cert-target", carrier], capture_output=True, text=True
                    )
                    match = re.fullmatch(
                        r"carrier=(\d+) target=(\d+) block_id=(\w+)\s*", probe.stdout
                    )
                    if match:
                        yield carrier, int(match[2]), match[3]

    found = list(certificates())
    exact = [row for row in found if row[1] == slot and row[2] in targets]
    later = sorted((row for row in found if row[1] > slot), key=lambda row: row[1])
    if not exact and not later:
        sys.exit(f"awaiting certificate-bearing carrier for deposit slot {slot}")
    # Wait for the window to fill before settling for a descendant's certificate.
    if not exact and len({row[0].name.split("-")[0] for row in found}) < 8:
        sys.exit(f"awaiting certificate-bearing carrier for deposit slot {slot}")
    errors = []
    for carrier, target, block_id in exact or later[:6]:
        for candidate in [targets[block_id]] if target == slot else targets.values():
            made = subprocess.run(
                [binary, "public", candidate, carrier, out, vault, binding, signature],
                capture_output=True,
                text=True,
            )
            if made.returncode == 0:
                sys.stdout.write(made.stdout)
                break
            errors.append(made.stderr.strip().splitlines()[-1:] or ["export failed"])
        else:
            continue
        break
    else:
        sys.exit(f"no usable certificate for deposit slot {slot}: {errors[-1][0][:200]}")
block_id = (out / str(slot) / "finalized-id").read_text().strip()
subprocess.run(
    [binary, "verify-receipt-block", out, str(slot), block_id, vault, binding],
    check=True,
)
