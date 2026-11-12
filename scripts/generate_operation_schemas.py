#!/usr/bin/env python3
"""Generate python/postfiat_rpc/operation_schemas.json from the Rust types.

For every operation the asset, escrow, NFT and offer quotes can carry, and
every struct nested in one, it records how each serde field behaves when it is
absent, which is what the wallet's quote check may treat as equal to absence:

- "option": `Option<T>`; only `null` equals absence.
- "vec": `#[serde(default)] Vec<T>`; only `[]` equals absence.
- "bool" / "int" / "str": `#[serde(default)]` scalar; only `false` / `0` /
  `""` of that exact type equals absence.
- "required": anything else; absence is a different (or invalid) operation.

Run with `--write` after changing those types; the quote-check tests fail
while the committed file is stale.
"""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
TYPES = ROOT / "crates/types/src"
OUTPUT = ROOT / "python/postfiat_rpc/operation_schemas.json"
ENUMS = (
    "AssetTransactionOperation",
    "EscrowTransactionOperation",
    "NftTransactionOperation",
    "OfferTransactionOperation",
)
INTS = {"u8", "u16", "u32", "u64", "u128", "i8", "i16", "i32", "i64", "i128", "usize"}

STRUCT = re.compile(r"pub struct (\w+)\s*\{(.*?)\n\}", re.S)
ENUM = re.compile(r"pub enum (\w+)\s*\{(.*?)\n\}", re.S)
FIELD = re.compile(r"((?:\s*#\[[^\]]*\]\s*)*)\s*pub (\w+):\s*([^,]+?),\s*(?://[^\n]*)?$", re.M)
VARIANT = re.compile(r"#\[serde\(rename = \"(\w+)\"\)\]\s*\w+\((\w+)\)")


def _attributes(text: str) -> str:
    return " ".join(re.findall(r"#\[serde\(([^\]]*)\)\]", text))


def _source() -> str:
    return "\n".join(path.read_text(encoding="utf-8") for path in sorted(TYPES.glob("*.rs")))


def _structs(source: str) -> dict[str, list[tuple[str, str, str]]]:
    structs: dict[str, list[tuple[str, str, str]]] = {}
    for name, body in STRUCT.findall(source):
        structs[name] = [(field, kind.strip(), _attributes(attrs)) for attrs, field, kind in FIELD.findall(body)]
    return structs


def _inner(kind: str, wrapper: str) -> str | None:
    match = re.fullmatch(rf"{wrapper}<(.+)>", kind)
    return match.group(1).strip() if match else None


def build() -> dict[str, dict[str, object]]:
    source = _source()
    structs = _structs(source)
    operations: dict[str, str] = {}
    for name, body in ENUM.findall(source):
        if name in ENUMS:
            operations.update(dict(VARIANT.findall(body)))
    schemas: dict[str, dict[str, object]] = {}

    def visit(struct: str) -> None:
        if struct in schemas or struct not in structs:
            return
        fields: dict[str, object] = {}
        schemas[struct] = fields
        for field, kind, attrs in structs[struct]:
            rename = re.search(r'rename = "(\w+)"', attrs)
            key = rename.group(1) if rename else field
            option, vec = _inner(kind, "Option"), _inner(kind, "Vec")
            default = re.search(r"\bdefault\b(?!\s*=)", attrs) is not None
            if option is not None:
                rule = "option"
            elif vec is not None and default:
                rule = "vec"
            elif default and kind == "bool":
                rule = "bool"
            elif default and kind in INTS:
                rule = "int"
            elif default and kind == "String":
                rule = "str"
            else:
                rule = "required"
            nested = _inner(option or kind, "Vec") or option or kind
            if nested in structs:
                fields[key] = [rule, nested]
                visit(nested)
            else:
                fields[key] = rule

    for struct in operations.values():
        visit(struct)
    return {
        "operations": dict(sorted(operations.items())),
        "structs": {name: schemas[name] for name in sorted(schemas)},
    }


def main() -> int:
    text = json.dumps(build(), indent=1, sort_keys=False) + "\n"
    if sys.argv[1:] == ["--write"]:
        OUTPUT.write_text(text, encoding="utf-8")
        return 0
    if OUTPUT.read_text(encoding="utf-8") != text:
        print(f"{OUTPUT.relative_to(ROOT)} is stale; run {Path(__file__).name} --write", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
