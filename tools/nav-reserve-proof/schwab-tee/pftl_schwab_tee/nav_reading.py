"""Schwab reported NLV: read-only collector, signed statement and release-4 ABI.

The native verifier proves the measured reader's claim, not broker solvency.
PFNAV001 carries one already-netted broker-equity source: gross_assets is that
source's value, liabilities are zero *additional* adjustments, not broker debt.
An SP1 guest must verify this evidence before these bytes can back a NAV epoch.
"""

from __future__ import annotations

import base64
from datetime import datetime, timezone
from decimal import Decimal, InvalidOperation, localcontext
import hashlib
import json
import re
import struct

from .canonical import canonical_json_bytes, domain_sha256
from .nitro_attestation import NitroNativeAttestationVerifier
from .nitro_runtime import Ed25519HexSignatureVerifier

MANIFEST_SCHEMA = "postfiat.b23.nitro_nav_manifest.v1"
STATEMENT_SCHEMA = "postfiat.b23.nitro_nav_reading.v1"
ENVELOPE_SCHEMA = "postfiat.b23.attested_nav_reading.v1"
SOURCE_FIELD = "securitiesAccount.currentBalances.liquidationValue"
MAX_RESPONSE = 4 * 1024 * 1024
SCALE = 1_000_000
U64_MAX = (1 << 64) - 1
HASH_FIELDS = {"pftl_genesis_hash": 48, "nav_asset_id": 48,
               "proof_profile_id": 48, "valuation_policy_hash": 32,
               "source_manifest_hash": 48, "valuation_unit_id": 48}
MANIFEST_FIELDS = {"schema", "account_commitment_sha256", "cutoff_unix_ms",
                   "observation_epoch", "observation_height", "challenge_sha256",
                   "operator_public_key_der_base64", *HASH_FIELDS}
READING_FIELDS = {"account_commitment_sha256", "cutoff_unix_ms", "started_unix_ms",
                  "observed_unix_ms", "liquidation_value_atoms", "source_field",
                  "account_list_response_sha256", "account_response_sha256"}


def exact(value, fields, label):
    if not isinstance(value, dict) or set(value) != set(fields):
        raise ValueError(label + " fields differ")
    return value


def hex_bytes(value, size):
    if not isinstance(value, str) or re.fullmatch(r"[0-9a-f]{%d}" % (size * 2), value) is None:
        raise ValueError("noncanonical digest")
    return bytes.fromhex(value)


def uint(value, bits=64):
    if type(value) is not int or not 0 <= value < (1 << bits):
        raise ValueError("unsigned integer out of bounds")
    return value


def account_commitment(account_hash):
    # Exact same domain as the existing MINT reader's schwab_deposits.py.
    if not isinstance(account_hash, str) or re.fullmatch(r"[A-Za-z0-9_-]{1,128}", account_hash) is None:
        raise ValueError("invalid linked account identifier")
    return hashlib.sha256(b"s8-broker-account\0" + account_hash.encode()).hexdigest()


def validate_manifest(manifest):
    exact(manifest, MANIFEST_FIELDS | ({"schedule"} if "schedule" in manifest else set()), "NAV manifest")
    if manifest["schema"] != MANIFEST_SCHEMA:
        raise ValueError("NAV manifest schema differs")
    for name, size in HASH_FIELDS.items():
        hex_bytes(manifest[name], size)
    for name in ("account_commitment_sha256", "challenge_sha256"):
        if hex_bytes(manifest[name], 32) == bytes(32):
            raise ValueError("empty manifest commitment")
    for name in ("cutoff_unix_ms", "observation_epoch", "observation_height"):
        if uint(manifest[name]) == 0:
            raise ValueError("zero NAV manifest epoch/time/height")
    # Retain the measured opening policy, but permit subsequent on-demand fixes.
    # The job binds the next finalized epoch and a strictly increasing cutoff.
    if "schedule" in manifest:
        schedule = exact(manifest["schedule"], {"opening_epoch", "opening_unix_ms", "first_daily_epoch", "first_daily_unix_ms"}, "schedule")
        for value in schedule.values():
            if not uint(value): raise ValueError("zero schedule field")
        if schedule["first_daily_epoch"] != schedule["opening_epoch"] + 1:
            raise ValueError("invalid opening epoch policy")
        if manifest["observation_epoch"] < schedule["opening_epoch"]:
            raise ValueError("epoch precedes approved opening")
        if manifest["cutoff_unix_ms"] < schedule["opening_unix_ms"]:
            raise ValueError("cutoff precedes approved opening")
    from cryptography.hazmat.primitives import serialization
    from cryptography.hazmat.primitives.asymmetric import rsa
    encoded = manifest["operator_public_key_der_base64"]
    if not isinstance(encoded, str) or len(encoded) > 2048:
        raise ValueError("operator recovery key bound")
    raw = base64.b64decode(encoded, validate=True)
    key = serialization.load_der_public_key(raw)
    if (base64.b64encode(raw).decode() != encoded or
            not isinstance(key, rsa.RSAPublicKey) or key.key_size not in (2048, 3072, 4096)):
        raise ValueError("operator recovery key differs")
    return domain_sha256(MANIFEST_SCHEMA, manifest)


def strict_json(raw):
    def pairs(items):
        result = {}
        for key, value in items:
            if key in result:
                raise ValueError("duplicate JSON field")
            result[key] = value
        return result
    def bad_constant(_):
        raise ValueError("non-finite JSON number")
    try:
        return json.loads(raw, parse_float=Decimal, parse_constant=bad_constant, object_pairs_hook=pairs)
    except (ValueError, TypeError, UnicodeDecodeError, RecursionError):
        raise ValueError("invalid broker JSON") from None


class SchwabAccountClient:
    """Only the account mapping and one mapped account GET; no generic signer."""

    def __init__(self, transport, access_token):
        if transport.host != "api.schwabapi.com" or not 0 < transport.max_response_bytes <= MAX_RESPONSE:
            raise ValueError("account client requires bounded Schwab TLS")
        self.transport, self.access_token = transport, access_token
        self.account_path = None

    def get(self, path):
        if path != "/trader/v1/accounts/accountNumbers" and path != self.account_path:
            raise ValueError("account endpoint is not allowed")
        token = self.access_token()
        if not isinstance(token, str) or not token or any(ord(c) < 33 or ord(c) > 126 for c in token):
            raise ValueError("invalid access token")
        response = self.transport.request("GET", path,
            params={"fields": "positions"} if path == self.account_path else None,
            headers={"Authorization": "Bearer " + token})
        if response.status_code != 200:
            raise ValueError("Schwab account HTTP status is not 200")
        headers = {k.lower(): v for k, v in response.headers.items()}
        if headers.get("content-type", "").split(";", 1)[0].strip().lower() != "application/json":
            raise ValueError("Schwab account response is not JSON")
        if not 0 < len(response.content) <= MAX_RESPONSE:
            raise ValueError("account response exceeds bounds")
        return strict_json(response.content), response.content


def collect_account(manifest, client, *, clock_ms, archive):
    """clock_ms is NSM attestation time in hardware; injectable only for tests.

    Both exact bodies go to the existing ciphertext archive before signing.
    No raw body, account number/hash, token or exception text leaves here.
    """
    validate_manifest(manifest)
    started = uint(clock_ms())
    cutoff = manifest["cutoff_unix_ms"]
    if not cutoff <= started <= cutoff + 60_000:
        raise ValueError("read starts outside daily cutoff")
    numbers, numbers_raw = client.get("/trader/v1/accounts/accountNumbers")
    if not isinstance(numbers, list) or len(numbers) != 1 or not isinstance(numbers[0], dict):
        raise ValueError("exactly one linked account required")
    linked = numbers[0]
    if account_commitment(linked.get("hashValue")) != manifest["account_commitment_sha256"]:
        raise ValueError("wrong account commitment")
    number = linked.get("accountNumber")
    if not isinstance(number, str) or not number:
        raise ValueError("account identity unavailable")
    client.account_path = "/trader/v1/accounts/" + linked["hashValue"]
    detail, detail_raw = client.get(client.account_path)
    observed = uint(clock_ms())
    if not started <= observed <= cutoff + 60_000:
        raise ValueError("read finishes outside daily cutoff")
    try:
        account = detail["securitiesAccount"]
        if account["accountNumber"] != number:
            raise ValueError("detail account identity differs")
        value = account["currentBalances"]["liquidationValue"]
        if isinstance(value, bool) or not isinstance(value, (int, Decimal)):
            raise ValueError("liquidation value must be a JSON number")
        value = Decimal(value)
        if len(value.as_tuple().digits) > 128:
            raise ValueError("liquidation value precision exceeds bound")
        with localcontext() as context:
            context.prec = 140
            amount = value * SCALE
        if not amount.is_finite() or not 0 < amount <= U64_MAX or amount != amount.to_integral_value():
            raise ValueError("liquidation value is not positive exact micro-USD")
    except (KeyError, TypeError, InvalidOperation):
        raise ValueError("invalid account current liquidation value") from None
    archive("account-list", numbers_raw)
    archive("account-detail", detail_raw)
    return {"account_commitment_sha256": manifest["account_commitment_sha256"],
            "cutoff_unix_ms": cutoff, "started_unix_ms": started, "observed_unix_ms": observed,
            "liquidation_value_atoms": int(amount), "source_field": SOURCE_FIELD,
            "account_list_response_sha256": hashlib.sha256(numbers_raw).hexdigest(),
            "account_response_sha256": hashlib.sha256(detail_raw).hexdigest()}


def validate_reading(reading, manifest):
    exact(reading, READING_FIELDS, "NAV reading")
    for name in ("account_commitment_sha256", "account_list_response_sha256", "account_response_sha256"):
        hex_bytes(reading[name], 32)
    if reading["account_commitment_sha256"] != manifest["account_commitment_sha256"]:
        raise ValueError("wrong account commitment")
    if reading["cutoff_unix_ms"] != manifest["cutoff_unix_ms"] or reading["source_field"] != SOURCE_FIELD:
        raise ValueError("wrong cutoff or source field")
    for name in ("cutoff_unix_ms", "started_unix_ms", "observed_unix_ms", "liquidation_value_atoms"):
        uint(reading[name])
    if not (reading["cutoff_unix_ms"] <= reading["started_unix_ms"] <=
            reading["observed_unix_ms"] <= reading["cutoff_unix_ms"] + 60_000):
        raise ValueError("reading is outside cutoff")
    if not reading["liquidation_value_atoms"]:
        raise ValueError("nonpositive NAV reading")


def commitment48(domain, value):
    return hashlib.sha3_384(domain.encode() + b"\0" + canonical_json_bytes(value)).hexdigest()


def nav_source_policy(manifest, pcrs, root_der):
    """The chain pins this complete policy hash; the generic guest verifies it.

    Measurements are known only after building the collector. Hashes of this
    policy therefore belong to the signed runtime manifest, outside the stable
    measured image, avoiding a circular image/PCR/profile commitment.
    """
    from .nav_reading_entrypoint import measured_policy
    validate_manifest(manifest)
    if "schedule" not in manifest:
        raise ValueError("SP1 NAV policy requires an immutable epoch schedule")
    exact(pcrs, {"0", "1", "2"}, "PCR policy")
    for value in pcrs.values():
        if hex_bytes(value, 48) == bytes(48): raise ValueError("debug PCR policy")
    valuation = {"schema": "postfiat.b23.nav_valuation_policy.v1", "source_field": SOURCE_FIELD,
        "scale": SCALE, "additional_adjustments": [], "account_commitment_sha256": manifest["account_commitment_sha256"],
        "schedule": manifest["schedule"], "maximum_read_window_ms": 60_000, "evidence_class": "attested"}
    return {"schema": "postfiat.b23.nav_source_policy.v1", "measured_policy": measured_policy(manifest),
        "valuation_policy": valuation, "root_der_hex": root_der.hex(), "nitro_policy": {
        "schema": "postfiat.yolo.nitro_proof_policy.v1", "rootCertificateSha256": hashlib.sha256(root_der).hexdigest(),
        "approvedPcrSets": [[{"index": i, "sha384": pcrs[str(i)]} for i in (0,1,2)]],
        "maxAgeMs": 65_000, "maxFutureSkewMs": 0, "timeRule": "EPOCH_WINDOW_END",
        "statementRule": "NAV_READING_V1", "verifierId": "b23-nitro-nav-reading-v1"}}


def reserve_values(manifest, reading):
    validate_manifest(manifest)
    validate_reading(reading, manifest)
    source = commitment48(STATEMENT_SCHEMA + ".source", reading)
    trust = {"source": source, "account": manifest["account_commitment_sha256"],
             "class": "attested", "basis": SOURCE_FIELD, "additional_adjustments": []}
    amount = reading["liquidation_value_atoms"]
    return {"schema": "postfiat.nav_reserve_public_values.v1", **{k: manifest[k] for k in HASH_FIELDS},
            "valuation_scale": SCALE, "observation_epoch": manifest["observation_epoch"],
            # Release-4 ABI interval is in BLOCK HEIGHTS; wall time is in the signed reading.
            "observation_not_before": manifest["observation_height"],
            "observation_not_after": manifest["observation_height"],
            "source_observation_root": source, "gross_assets": amount, "total_liabilities": 0,
            "verified_net_assets": amount, "cryptographically_verified_value": 0,
            "attested_value": amount, "controlled_value": 0, "source_count": 1,
            "quantity_trust_counts": {"cryptographic": 0, "attested": 1, "controlled": 0},
            "valuation_trust_counts": {"cryptographic": 0, "attested": 1, "controlled": 0},
            "quantity_trust_root": commitment48(STATEMENT_SCHEMA + ".quantity", trust),
            "valuation_trust_root": commitment48(STATEMENT_SCHEMA + ".valuation", trust),
            "source_disclosure_root": commitment48(STATEMENT_SCHEMA + ".disclosure", trust)}


def encode_reserve_values(values):
    """Exact release-4 NavReservePublicValuesV1 big-endian wire layout, 584 bytes."""
    out = bytearray(b"PFNAV001" + struct.pack(">I", 1))
    for key, size in HASH_FIELDS.items():
        out.extend(hex_bytes(values[key], size))
    for key in ("valuation_scale", "observation_epoch", "observation_not_before", "observation_not_after"):
        out.extend(struct.pack(">Q", uint(values[key])))
    out.extend(hex_bytes(values["source_observation_root"], 48))
    for key in ("gross_assets", "total_liabilities", "verified_net_assets",
                "cryptographically_verified_value", "attested_value", "controlled_value"):
        out.extend(struct.pack(">Q", uint(values[key])))
    out.extend(struct.pack(">I", uint(values["source_count"], 32)))
    for key in ("quantity_trust_counts", "valuation_trust_counts"):
        for kind in ("cryptographic", "attested", "controlled"):
            out.extend(struct.pack(">I", uint(values[key][kind], 32)))
    for key in ("quantity_trust_root", "valuation_trust_root", "source_disclosure_root"):
        out.extend(hex_bytes(values[key], 48))
    if len(out) != 584:
        raise ValueError("reserve ABI length differs")
    return bytes(out)


def statement(manifest, reading):
    values = reserve_values(manifest, reading)
    return {"schema": STATEMENT_SCHEMA, "manifest_sha256": validate_manifest(manifest),
            "reading": reading, "public_values_hex": encode_reserve_values(values).hex()}


def attest_reading(manifest, reading, keys, nsm):
    body = statement(manifest, reading)
    binding = domain_sha256(STATEMENT_SCHEMA, body)
    return {"schema": ENVELOPE_SCHEMA, "body": body, "binding_digest": binding,
            "signature": keys.sign_digest(binding),
            "document_base64": base64.b64encode(keys.attest_statement(nsm,
                attestation_binding_digest=binding)).decode()}


def verify_reading(envelope, *, manifest, root_pem, expected_pcrs, now_ms, max_age_ms=300_000):
    """Offline native verification using EXTERNAL root, measurements and manifest.

    Never take the trust anchor, PCR allowlist, current time or manifest from the
    evidence being verified. Historical replay needs an explicit historical time.
    """
    validate_manifest(manifest)
    uint(now_ms)
    if type(max_age_ms) is not int or not 0 < max_age_ms <= 86_400_000:
        raise ValueError("verification freshness bound differs")
    exact(expected_pcrs, {"0", "1", "2"}, "expected PCRs")
    for value in expected_pcrs.values():
        if hex_bytes(value, 48) == bytes(48):
            raise ValueError("debug PCR policy forbidden")
    exact(envelope, {"schema", "body", "binding_digest", "signature", "document_base64"}, "envelope")
    if envelope["schema"] != ENVELOPE_SCHEMA:
        raise ValueError("envelope schema differs")
    body = exact(envelope["body"], {"schema", "manifest_sha256", "reading", "public_values_hex"}, "statement")
    if body != statement(manifest, body["reading"]):
        raise ValueError("statement or public values differ")
    binding = domain_sha256(STATEMENT_SCHEMA, body)
    if envelope["binding_digest"] != binding:
        raise ValueError("altered NAV statement")
    encoded = envelope["document_base64"]
    if not isinstance(encoded, str) or len(encoded) > 24_000:
        raise ValueError("attestation bound")
    document = base64.b64decode(encoded, validate=True)
    verifier = NitroNativeAttestationVerifier(root_certificate_pem=root_pem,
        max_age_ms=max_age_ms, max_future_skew_ms=1000,
        clock=lambda: datetime.fromtimestamp(now_ms / 1000, timezone.utc))
    parsed = verifier.parse_and_verify(document, attestation_binding_digest=binding)
    for index, value in expected_pcrs.items():
        if parsed.pcrs.get(int(index)) != bytes.fromhex(value):
            raise ValueError("wrong PCR or debug enclave")
    reading = body["reading"]
    # Signed NSM time after the read must follow the read and stay near the fix.
    if not reading["observed_unix_ms"] <= parsed.timestamp_ms <= manifest["cutoff_unix_ms"] + 65_000:
        raise ValueError("attestation time differs from daily read")
    if now_ms - reading["observed_unix_ms"] > max_age_ms:
        raise ValueError("stale NAV reading")
    if not Ed25519HexSignatureVerifier().verify(statement_key=parsed.statement_key,
            signing_digest=binding, signature=envelope["signature"]):
        raise ValueError("NAV statement signature invalid")
    return reserve_values(manifest, reading)


def main(argv=None):
    import argparse
    from pathlib import Path
    parser = argparse.ArgumentParser(description="Verify native B23 NAV evidence offline; no chain writes")
    parser.add_argument("--evidence", type=Path, required=True)
    parser.add_argument("--manifest", type=Path, required=True)
    parser.add_argument("--root-pem", type=Path, required=True)
    parser.add_argument("--pcrs", type=Path, required=True)
    parser.add_argument("--now-ms", type=int, help="Explicit historical verification time; default current UTC")
    args = parser.parse_args(argv)
    values = verify_reading(json.loads(args.evidence.read_bytes()), manifest=json.loads(args.manifest.read_bytes()),
        root_pem=args.root_pem.read_bytes(), expected_pcrs=json.loads(args.pcrs.read_bytes()),
        now_ms=args.now_ms if args.now_ms is not None else int(datetime.now(timezone.utc).timestamp() * 1000))
    print(json.dumps({"native_attestation_verified": True, "evidence_class": "attested",
                      "public_values_hex": encode_reserve_values(values).hex(),
                      "sp1_proof_generated": False, "chain_submission_performed": False}, sort_keys=True))


if __name__ == "__main__":
    main()
