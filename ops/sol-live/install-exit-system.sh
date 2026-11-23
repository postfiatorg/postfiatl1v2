#!/bin/bash
# The owner runs this once from the reviewed release. No worker start or signing.
set -euo pipefail
umask 022
export PATH=/usr/sbin:/usr/bin:/sbin:/bin
unset PYTHONHOME PYTHONPATH
if [[ ${1:-} == --help ]]; then
    echo 'sudo bash ops/sol-live/install-exit-system.sh RELEASE_KIT STAGED_WHEELS'
    exit
fi
[[ $EUID == 0 && $# == 2 ]]
exec 9>/run/lock/postfiat-sol-exit-install.lock
flock -n 9
operator=postfiat
operator_uid=$(id -u "$operator")
operator_home=$(getent passwd "$operator" | cut -d: -f6)
account=pf-sol-exit
svc=/var/lib/pf-sol-exit
app=/opt/postfiat/sol-exit
ops=$app/ops/sol-live
kit=$(realpath -e "$1")
wheels=$(realpath -e "$2")
repo=$(cd "$(dirname "$0")/../.." && pwd -P)
as_operator=(runuser -u "$operator" --)
repo_git=("${as_operator[@]}" git -C "$repo")
rev=$("${repo_git[@]}" rev-parse HEAD)
"${repo_git[@]}" diff --quiet HEAD -- ops/sol-live solana/deploy solana/pfusdc-egress-verifier/bootstrap.py crates/types/src/solana_exit_profile.json
unit=pft-sol-mainnet-withdraw.service
state=$(systemctl list-unit-files --type=service --no-legend --no-pager | awk -v unit="$unit" '$1 == unit {print $2}')
[[ -z "$state" || $state == disabled ]]
if systemctl is-active --quiet "$unit"; then echo 'Stop the worker before installing.' >&2; exit 1; fi
check_port() {
    local listeners
    listeners=$(ss -H -ltn 'sport = :18089') || return 1
    [[ -z "$listeners" ]]
}
check_port
secrets=$operator_home/.config/postfiat/sol-bridge
identity=$secrets/exit-prover-ssh
trust=$secrets/exit-prover-known_hosts
fee=$secrets/exit-fee-payer.json
proof=$operator_home/repos/b23-coord/evidence/withdraw
if ! id "$account" >/dev/null 2>&1; then
    useradd --system --user-group --no-create-home --home-dir "$svc" --shell /usr/sbin/nologin "$account"
fi
[[ $(getent passwd "$account" | cut -d: -f6,7) == "$svc:/usr/sbin/nologin" ]]
[[ $(id -Gn "$account") == "$account" && $(id -u "$account") -ne 0 ]]
echo "Verified dedicated account $account"
# Only jobs is worker-writable. Root controls every destination parent.
for dir in "$app" "$svc"; do
    [[ ! -L "$dir" ]]
    if [[ -e "$dir" ]]; then
        [[ -d "$dir" && $(stat -c %u "$dir") == 0 ]]
        [[ -z $(find "$dir" -maxdepth 0 -perm /022 -print) ]]
    fi
done
install -d -m755 -o root -g root "$app"
install -d -m750 -o root -g "$account" "$svc"
install -d -m700 -o "$account" -g "$account" "$svc/jobs"
if [[ -f "$app/revision" ]]; then
    [[ $(cat "$app/revision") == "$rev" ]]
fi
# Extract only committed code. Install packages offline from hash-pinned binary wheels.
"${repo_git[@]}" archive HEAD ops/sol-live solana/deploy solana/pfusdc-egress-verifier/bootstrap.py crates/types/src/solana_exit_profile.json |
    tar -xf - -C "$app" --no-same-owner --no-same-permissions
install -d -m755 "$app/proof/bin" "$app/proof/historical"
install -m755 "$proof/bin/proof-host" "$app/proof/bin/proof-host"
install -m644 "$proof/historical/egress.elf" "$app/proof/historical/egress.elf"
(cd "$app/proof" && sha256sum -c <<'HASHES'
1fa81b51967ccf72e91ce2edde135968abd519395047fd1f304b9d5aec25a600  bin/proof-host
4d5f84493c9b02b0d2a082c446229e30ce6645210a00c271dfb125b2761c67e0  historical/egress.elf
HASHES
)
install -m644 "$kit/evidence/solana/anchor-witness.json" "$app/anchor-witness.json"
install -m644 "$kit/evidence/solana/roles.json" "$app/roles.json"
python3 -I -m venv "$app/venv"
PIP_CONFIG_FILE=/dev/null "$app/venv/bin/python" -I -m pip --isolated --disable-pip-version-check install \
    --no-cache-dir --no-index --only-binary=:all: --require-hashes --find-links "$wheels" -r "$ops/exit-requirements.lock"
as_worker=(runuser -u "$account" -- env -i PATH=/usr/bin:/bin HOME="$svc" PYTHONDONTWRITEBYTECODE=1
           PYTHONPATH="$ops" EXIT_PROOF_HOME="$app/proof" "$app/venv/bin/python")
(cd / && "${as_worker[@]}" -c 'import exit_service; from exit_proof import verify_local_host; verify_local_host()')
echo "Installed root-owned runtime for $rev"
fp=$(cat "$ops/exit-prover.fingerprint")
"${as_operator[@]}" bash "$ops/check-prover-identity.sh" "$identity" "$fp"
"${as_operator[@]}" bash "$ops/check-prover-host.sh" "$trust"
install -m600 -o "$account" -g "$account" "$identity" "$svc/prover"
install -m640 -o root -g "$account" "$trust" "$svc/known_hosts"
runuser -u "$account" -- bash "$ops/check-prover-identity.sh" "$svc/prover" "$fp"
echo "Installed approved prover identity $fp and host trust"
check_port
if [[ -f "$svc/fee-payer.json" ]]; then
    [[ ! -L "$svc/fee-payer.json" && ! -e "$fee" && ! -L "$fee" ]]
    owner=$(stat -c %u "$svc/fee-payer.json")
    [[ $owner == "$operator_uid" || $owner == "$(id -u "$account")" ]]
    [[ $(stat -c '%a:%h' "$svc/fee-payer.json") == 600:1 ]]
else
    [[ ! -e "$svc/fee-payer.json" && ! -L "$svc/fee-payer.json" && -f "$fee" && ! -L "$fee" ]]
    [[ $(stat -c '%u:%a:%h' "$fee") == "$operator_uid:600:1" ]]
    [[ $(stat -c %d "$fee") == "$(stat -c %d "$svc")" ]]
    (cd / && "${as_operator[@]}" env PYTHONPATH="$ops" "$app/venv/bin/python" -B -c \
        'from exit_setup import FEE_PAYER, load, require; import sys; require(load(sys.argv[1]).pubkey()==FEE_PAYER, "Wrong source payer")' "$fee")
    mv -T "$fee" "$svc/fee-payer.json"
fi
chown "$account:$account" "$svc/fee-payer.json"
(cd / && "${as_worker[@]}" -c 'from exit_setup import FEE_PAYER, load, require; import sys; require(load(sys.argv[1]).pubkey()==FEE_PAYER, "Wrong installed payer")' "$svc/fee-payer.json")
echo 'Verified service-owned fee payer'
install -d -m755 -o root -g root /var/log/pf-sol-exit
install -m644 /dev/null /var/log/pf-sol-exit/self-test.log
install -m644 "$ops/$unit" "$ops/pft-sol-exit-self-test.service" /etc/systemd/system/
# Fixed arguments only. The rebuild runs as the service user with its normal key path.
sudoers=$(mktemp /etc/sudoers.d/.pf-sol-exit.XXXXXXXX)
trap 'rm -f -- "$sudoers"' EXIT
cat > "$sudoers" <<RULES
$operator ALL=(root) NOPASSWD: /usr/bin/systemctl start pft-sol-exit-self-test.service, /usr/bin/systemctl enable --now $unit, /usr/bin/systemctl start $unit, /usr/bin/systemctl stop $unit, /usr/bin/systemctl --no-pager status $unit
$operator ALL=($account) NOPASSWD: $app/venv/bin/python -I $ops/exit_rebuild.py
RULES
chmod 0440 "$sudoers"
visudo -cf "$sudoers"
mv -T "$sudoers" /etc/sudoers.d/pf-sol-exit
systemctl daemon-reload
[[ $(systemctl is-enabled "$unit") == disabled ]]
printf '%s\n' "$rev" > "$app/revision"
echo "Installed disabled $unit and passwordless, fixed operator commands. Run the archive self-test next."
