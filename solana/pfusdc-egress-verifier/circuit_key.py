"""Reproduce the BN254 key in syscall (EIP-197) order from the pinned SP1 Solidity verifier."""
from pathlib import Path
import argparse
import re

here = Path(__file__).resolve().parent
source = here.parents[1] / "ops/new-navcoin/receipt-qualification/src/vendor/Groth16Verifier.sol"
words = dict(re.findall(r"uint256 constant (\w+) = (\d+);", source.read_text()))
def point(name, g2=False):
    axes = ["X_1", "X_0", "Y_1", "Y_0"] if g2 else ["X", "Y"]
    return b"".join(int(words[f"{name}_{axis}"]).to_bytes(32, "big") for axis in axes)

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("output", type=Path, nargs="?", default=here / "src/circuit-key.bin")
parser.parse_args().output.write_bytes(b"".join([
    point("CONSTANT"), *(point(f"PUB_{i}") for i in range(5)), point("DELTA_NEG", True),
    point("ALPHA"), point("BETA_NEG", True), point("GAMMA_NEG", True),
]))
