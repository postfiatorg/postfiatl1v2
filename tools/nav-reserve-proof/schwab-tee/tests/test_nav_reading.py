"""Synthetic broker and cryptographically signed synthetic Nitro tests; no AWS."""

import base64
import copy
from datetime import datetime, timezone
import json
import struct

import pytest
from cryptography.hazmat.primitives import serialization

from pftl_schwab_tee import nav_reading as nav
from pftl_schwab_tee.canonical import domain_sha256
from pftl_schwab_tee.nav_reading_entrypoint import measured_policy
from pftl_schwab_tee.nitro_runtime import NitroStatementKeys
from pftl_schwab_tee.nitro_transport import NitroHttpResponse
from test_yolo_nitro_attestation import _certificates, _document

# Synthetic only; certificates' validity is 2026-09-04, matching shared fixtures.
CUTOFF = int(datetime(2026, 9, 4, 18, 15, tzinfo=timezone.utc).timestamp() * 1000)
PCRS = {str(i): bytes([0x11 + i]).hex() * 48 for i in range(3)}


@pytest.fixture
def manifest():
    key = NitroStatementKeys.generate().recipient_public_key_der
    return {"schema": nav.MANIFEST_SCHEMA,
        **{name: ("%02x" % (i + 1)) * size for i, (name, size) in enumerate(nav.HASH_FIELDS.items())},
        "account_commitment_sha256": nav.account_commitment("synthetic-account-hash"),
        "challenge_sha256": "aa" * 32, "observation_epoch": 3, "observation_height": 1292,
        "cutoff_unix_ms": CUTOFF, "operator_public_key_der_base64": base64.b64encode(key).decode()}


class Transport:
    host = "api.schwabapi.com"
    max_response_bytes = nav.MAX_RESPONSE

    def __init__(self, detail=None, numbers=None):
        self.calls = []
        self.numbers = numbers if numbers is not None else [{"accountNumber": "SYNTHETIC-ACCOUNT-ONLY", "hashValue": "synthetic-account-hash"}]
        self.detail = detail if detail is not None else {"securitiesAccount": {
            "accountNumber": "SYNTHETIC-ACCOUNT-ONLY", "currentBalances": {"liquidationValue": 419234.35},
            "initialBalances": {"liquidationValue": 999}, "positions": [{"privateMarker": "PRIVATE-ONLY"}]}}
        self.status, self.content_type = 200, "application/json"
        self.raw_override = None

    def request(self, method, path, **kwargs):
        self.calls.append((method, path, kwargs))
        payload = self.numbers if path.endswith("accountNumbers") else self.detail
        raw = self.raw_override if self.raw_override is not None else json.dumps(payload).encode()
        return NitroHttpResponse(self.status, {"Content-Type": self.content_type}, raw)


def collect(manifest, transport=None, times=None):
    transport = transport or Transport()
    archives = []
    ticks = iter(times or [CUTOFF + 1000, CUTOFF + 2000])
    reading = nav.collect_account(manifest, nav.SchwabAccountClient(transport, lambda: "SYNTHETIC-TOKEN"),
        clock_ms=lambda: next(ticks), archive=lambda name, raw: archives.append((name, raw)))
    return reading, archives, transport


def evidence(manifest, *, pcr0=bytes.fromhex(PCRS["0"]), debug=False):
    keys = NitroStatementKeys.generate()
    # Shared cert helper uses 15:00; shift its module clock for the fixed daily fix.
    import test_yolo_nitro_attestation as fixtures
    old = fixtures.NOW
    fixtures.NOW = datetime.fromtimestamp((CUTOFF + 3000) / 1000, timezone.utc)
    try:
        certificates = _certificates()
    finally:
        fixtures.NOW = old
    class Nsm:
        def attest(self, *, user_data, nonce, public_key):
            user = json.loads(user_data)
            return _document(timestamp=datetime.fromtimestamp((CUTOFF + 3000) / 1000, timezone.utc),
                pcr0=pcr0, all_pcrs_zero=debug, nonce=nonce,
                statement_digest=user["statementBindingDigest"], statement_key=user["statementKey"],
                recipient_key=keys.recipient_private_key, certificates=certificates)[0]
    reading, _, _ = collect(manifest)
    return nav.attest_reading(manifest, reading, keys, Nsm()), certificates[1].public_bytes(serialization.Encoding.PEM)


def verify(value, root, manifest, now=CUTOFF + 4000):
    return nav.verify_reading(value, manifest=manifest, root_pem=root, expected_pcrs=PCRS, now_ms=now)


def test_exact_read_preserves_source_and_private_bodies(manifest):
    reading, archives, transport = collect(manifest)
    assert reading["liquidation_value_atoms"] == 419234350000
    assert [x[0] for x in archives] == ["account-list", "account-detail"]
    assert all(c[0] == "GET" for c in transport.calls)
    assert transport.calls[1][2]["params"] == {"fields": "positions"}
    public = json.dumps(nav.statement(manifest, reading))
    for private in ("SYNTHETIC-ACCOUNT-ONLY", "synthetic-account-hash", "SYNTHETIC-TOKEN", "PRIVATE-ONLY"):
        assert private not in public
    assert measured_policy(manifest) == measured_policy({**manifest, "observation_epoch": 4})


@pytest.mark.parametrize("mutation", ["wrong_account", "wrong_detail", "ambiguous", "missing", "negative", "zero", "fraction", "bool", "string", "overflow", "initial_only"])
def test_bad_account_value_refused(manifest, mutation):
    t = Transport()
    b = t.detail["securitiesAccount"]["currentBalances"]
    if mutation == "wrong_account": t.numbers[0]["hashValue"] = "another-hash"
    elif mutation == "wrong_detail": t.detail["securitiesAccount"]["accountNumber"] = "another-number"
    elif mutation == "ambiguous": t.numbers *= 2
    elif mutation in ("missing", "initial_only"): del b["liquidationValue"]
    else: b["liquidationValue"] = {"negative": -1, "zero": 0, "fraction": .0000001, "bool": True, "string": "12", "overflow": 2**64}[mutation]
    with pytest.raises(ValueError): collect(manifest, t)


@pytest.mark.parametrize("times", [[CUTOFF-1, CUTOFF], [CUTOFF, CUTOFF+60001], [CUTOFF+1000, CUTOFF]])
def test_wrong_cutoff_times(manifest, times):
    with pytest.raises(ValueError): collect(manifest, times=times)


def test_multiple_arbitrary_cutoffs_in_one_day(manifest):
    manifest['schedule'] = dict(opening_epoch=1, opening_unix_ms=CUTOFF-3600000,
                               first_daily_epoch=2, first_daily_unix_ms=CUTOFF)
    for epoch, cutoff in ((3,CUTOFF-1200000), (4,CUTOFF-600000)):
        current = {**manifest,'observation_epoch':epoch,'cutoff_unix_ms':cutoff}
        reading, _, _ = collect(current,times=[cutoff,cutoff+59999])
        assert reading['cutoff_unix_ms'] == cutoff
        assert measured_policy(current) == measured_policy(manifest)
        with pytest.raises(ValueError):collect(current,times=[cutoff-1,cutoff])
        with pytest.raises(ValueError):collect(current,times=[cutoff,cutoff+60001])


@pytest.mark.parametrize("raw", [b'{"x":1,"x":2}', b'{"x":NaN}', b'{', b'\xff'])
def test_ambiguous_or_malformed_json(manifest, raw):
    t = Transport(); t.raw_override = raw
    with pytest.raises(ValueError): collect(manifest, t)


@pytest.mark.parametrize("status,ctype", [(302,"application/json"),(401,"application/json"),(500,"application/json"),(200,"text/html")])
def test_http_error_no_statement(manifest, status, ctype):
    t = Transport(); t.status=status; t.content_type=ctype
    with pytest.raises(ValueError): collect(manifest, t)


def test_read_only_client_rejects_other_endpoints():
    client = nav.SchwabAccountClient(Transport(), lambda: "synthetic")
    for path in ("/trader/v1/accounts/unknown", "/trader/v1/accounts/x/orders", "/v1/oauth/token", "https://evil.invalid"):
        with pytest.raises(ValueError): client.get(path)


def test_native_verification_and_release4_abi(manifest):
    envelope, root = evidence(manifest)
    values = verify(envelope, root, manifest)
    encoded = nav.encode_reserve_values(values)
    assert len(encoded) == 584 and encoded[:12] == b"PFNAV001\0\0\0\1"
    assert struct.unpack_from(">6Q", encoded, 364) == (419234350000,0,419234350000,0,419234350000,0)
    assert struct.unpack_from(">7I", encoded, 412) == (1,0,1,0,0,1,0)
    assert encoded.hex() == envelope["body"]["public_values_hex"]


@pytest.mark.parametrize("attack", ["wrong_pcr", "debug", "altered_value", "altered_abi", "stale", "wrong_account", "wrong_epoch", "wrong_challenge", "wrong_signature", "wrong_document", "wrong_root"])
def test_tampering_is_rejected(manifest, attack):
    envelope, root = evidence(manifest, pcr0=b"\x88"*48 if attack=="wrong_pcr" else bytes.fromhex(PCRS["0"]), debug=attack=="debug")
    expected = copy.deepcopy(manifest)
    now = CUTOFF+4000
    if attack == "altered_value": envelope["body"]["reading"]["liquidation_value_atoms"] += 1
    elif attack == "altered_abi": envelope["body"]["public_values_hex"] = "00"*584
    elif attack == "stale": now += 300001
    elif attack == "wrong_account": expected["account_commitment_sha256"] = "bb"*32
    elif attack == "wrong_epoch": expected["observation_epoch"] += 1
    elif attack == "wrong_challenge": expected["challenge_sha256"] = "bb"*32
    elif attack == "wrong_signature": envelope["signature"] = base64.b64encode(bytes(64)).decode()
    elif attack == "wrong_document":
        raw=base64.b64decode(envelope["document_base64"]); envelope["document_base64"]=base64.b64encode(raw[:-1]+bytes([raw[-1]^1])).decode()
    elif attack == "wrong_root": root=_certificates()[1].public_bytes(serialization.Encoding.PEM)
    with pytest.raises(ValueError): verify(envelope, root, expected, now)


def test_rehashed_changed_statement_cannot_reuse_attestation(manifest):
    envelope, root=evidence(manifest)
    envelope["body"]["reading"]["liquidation_value_atoms"]+=1
    envelope["body"]=nav.statement(manifest,envelope["body"]["reading"])
    envelope["binding_digest"]=domain_sha256(nav.STATEMENT_SCHEMA,envelope["body"])
    with pytest.raises(ValueError): verify(envelope,root,manifest)
