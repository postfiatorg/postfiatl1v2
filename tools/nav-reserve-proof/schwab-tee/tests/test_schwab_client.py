import io
import json
import os
import stat
import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "mint_reader"))
import schwab_client


def client(tmp_path, stored, monkeypatch, reply):
    tokens = tmp_path / "tokens.json"
    tokens.write_text(json.dumps(stored))
    os.chmod(tokens, 0o600)
    credentials = tmp_path / "app.txt"
    credentials.write_text("client_id=a\nclient_secret=b\n")
    seen = {}

    def urlopen(request, timeout):
        # While the refresh is in flight no half-written or world-readable file exists.
        seen["during"] = sorted(p.name for p in tmp_path.iterdir())
        return io.BytesIO(json.dumps(reply).encode())

    monkeypatch.setattr(schwab_client.urllib.request, "urlopen", urlopen)
    return schwab_client.SchwabClient(tokens, credentials), tokens, seen


def test_refresh_keeps_fields_the_response_does_not_carry(tmp_path, monkeypatch):
    stored = {"access_token": "old", "refresh_token": "keep-me", "account_hash": "other-client-field"}
    c, tokens, _ = client(tmp_path, stored, monkeypatch, {"access_token": "new", "expires_in": 1800})
    assert c._access_token() == "new"
    saved = json.loads(tokens.read_text())
    assert saved["refresh_token"] == "keep-me" and saved["account_hash"] == "other-client-field"
    assert saved["access_token"] == "new" and saved["expires_in"] == 1800 and "obtained_at_utc" in saved


def test_refresh_takes_a_rotated_refresh_token(tmp_path, monkeypatch):
    c, tokens, _ = client(tmp_path, {"access_token": "old", "refresh_token": "r1"}, monkeypatch,
                          {"access_token": "new", "refresh_token": "r2"})
    c._access_token()
    assert json.loads(tokens.read_text())["refresh_token"] == "r2"


def test_token_file_is_owner_only_and_no_temporary_file_remains(tmp_path, monkeypatch):
    old = os.umask(0)
    try:
        c, tokens, _ = client(tmp_path, {"access_token": "old", "refresh_token": "r"}, monkeypatch, {"access_token": "new"})
        created = []
        real = os.open
        monkeypatch.setattr(schwab_client.os, "open", lambda path, flags, mode=0o777: created.append(mode) or real(path, flags, mode))
        c._access_token()
    finally:
        os.umask(old)
    assert created == [0o600]
    assert stat.S_IMODE(tokens.stat().st_mode) == 0o600
    assert sorted(p.name for p in tmp_path.iterdir()) == ["app.txt", "tokens.json"]


def test_a_reply_without_an_access_token_leaves_the_file_as_it_was(tmp_path, monkeypatch):
    stored = {"access_token": "old", "refresh_token": "r"}
    c, tokens, _ = client(tmp_path, stored, monkeypatch, {"error": "invalid_grant"})
    with pytest.raises(RuntimeError):
        c._access_token()
    assert json.loads(tokens.read_text()) == stored
