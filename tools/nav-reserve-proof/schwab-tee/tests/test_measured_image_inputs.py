"""The measured image must not change when a commit leaves the reader's files alone."""
import hashlib
import json
import shutil
import subprocess
from pathlib import Path

import pytest

from pftl_schwab_tee import nitro_build

TEE = Path(__file__).resolve().parents[1]
ROOT = nitro_build.SOURCE_ROOT


def git(repo, *args):
    return subprocess.check_output(["git", "-C", str(repo), "-c", "user.name=t", "-c", "user.email=t@example.invalid",
                                    "-c", "commit.gpgsign=false", *args])


@pytest.fixture
def repo(tmp_path):
    repo = tmp_path / "repo"
    target = repo / ROOT
    for folder in ("pftl_schwab_tee", "enclave"):
        shutil.copytree(TEE / folder, target / folder, ignore=shutil.ignore_patterns("__pycache__"))
    nsm, root = tmp_path / "libnsm.so", tmp_path / "root.der"
    nsm.write_bytes(b"synthetic nsm"); root.write_bytes(b"synthetic root")
    pins = target / "enclave/build-inputs.json"
    value = json.loads(pins.read_text())
    value.update(nsmLibrarySha256=hashlib.sha256(nsm.read_bytes()).hexdigest(),
                 nitroRootDerSha256=hashlib.sha256(root.read_bytes()).hexdigest())
    pins.write_text(json.dumps(value, indent=2) + "\n")
    git(tmp_path, "init", "-q", str(repo)); git(repo, "add", "-A"); git(repo, "commit", "-q", "-m", "reader")
    return repo, nsm, root


def context(repo, nsm, root, out):
    nitro_build.main(["--repository", str(repo), "--revision", "HEAD", "--nsm-library", str(nsm), "--root-der", str(root),
                      "--output-directory", str(out), "--image-tag", "unused:tag", "--prepare-only"])
    return {str(p.relative_to(out / "context")): p.read_bytes() for p in sorted((out / "context").rglob("*")) if p.is_file()}


def test_a_commit_outside_the_reader_leaves_the_build_context_byte_identical(repo, tmp_path):
    repo, nsm, root = repo
    first = context(repo, nsm, root, tmp_path / "a")
    (repo / "elsewhere.txt").write_text("a change in another part of the repository\n")
    git(repo, "add", "-A")
    git(repo, "-c", "user.name=t", "commit", "-q", "-m", "elsewhere", "--date", "2030-01-01T00:00:00Z")
    second = context(repo, nsm, root, tmp_path / "b")
    assert first == second
    manifest = json.loads(first["build-inputs.json"])
    assert "navSourceRevision" not in manifest and manifest["sourceDateEpoch"] == json.loads((TEE / "enclave/build-inputs.json").read_text())["sourceDateEpoch"]
    command = json.loads((tmp_path / "b/command.json").read_text())["command"]
    assert f"SOURCE_DATE_EPOCH={manifest['sourceDateEpoch']}" in command


def test_a_change_to_a_measured_module_changes_the_build_context(repo, tmp_path):
    repo, nsm, root = repo
    first = context(repo, nsm, root, tmp_path / "a")
    module = repo / ROOT / "pftl_schwab_tee/canonical.py"
    module.write_text(module.read_text() + "\n# changed\n")
    git(repo, "add", "-A"); git(repo, "commit", "-q", "-m", "reader change")
    assert context(repo, nsm, root, tmp_path / "b") != first


def test_both_pin_files_carry_one_fixed_build_time():
    values = {json.loads((TEE / "enclave" / name).read_text())["sourceDateEpoch"] for name in ("build-inputs.json", "nav-build-inputs.json")}
    assert len(values) == 1 and all(type(v) is int and v > 0 for v in values)


def test_the_runner_accepts_a_later_commit_only_when_the_reader_files_are_the_same(repo, monkeypatch):
    import nav_reading_run as runner
    repo, _, _ = repo
    monkeypatch.setattr(runner, "REPO", repo)
    reviewed = git(repo, "rev-parse", "HEAD").decode().strip()
    (repo / "elsewhere.txt").write_text("x\n"); git(repo, "add", "-A"); git(repo, "commit", "-q", "-m", "elsewhere")
    assert runner.reader_tree("HEAD") == runner.reader_tree(reviewed)
    module = repo / ROOT / "pftl_schwab_tee/canonical.py"
    module.write_text(module.read_text() + "\n# changed\n"); git(repo, "add", "-A"); git(repo, "commit", "-q", "-m", "reader")
    assert runner.reader_tree("HEAD") != runner.reader_tree(reviewed)
