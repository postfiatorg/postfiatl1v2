#!/bin/bash
# Run only from the owner's digest-verified private stage.
set -euo pipefail
umask 022
[[ $EUID == 0 && $# == 0 ]]
bundle=$(cd -- "$(dirname -- "$0")" && pwd -P)
[[ $(stat -c '%u:%a' "$bundle") == 0:700 && $bundle == /var/lib/pf-sol-install.* ]]
cd /
exec 9>/run/pf-sol-exit-install.lock
flock -n 9
operator=postfiat
operator_uid=$(id -u "$operator")
operator_home=$(getent passwd "$operator" | cut -d: -f6)
account=pf-sol-exit
svc=/var/lib/pf-sol-exit
app=/opt/postfiat/sol-exit
ops=$app/ops/sol-live
unit=pft-sol-mainnet-withdraw.service
test_unit=pft-sol-exit-self-test.service
rev=$(cat "$bundle/revision")
[[ $rev =~ ^[0-9a-f]{40}$ ]]
for dir in /opt/postfiat "$app" "$svc" /var/log/pf-sol-exit; do
    [[ ! -L "$dir" ]]
    if [[ -e "$dir" ]]; then
        [[ -d "$dir" && $(stat -c %u "$dir") == 0 && -z $(find "$dir" -maxdepth 0 -perm /022 -print) ]]
    fi
done
if [[ -f "$app/revision" ]]; then
    [[ $(cat "$app/revision") == "$rev" ]]
fi
state=$(systemctl list-unit-files --type=service --no-legend --no-pager | awk -v unit="$unit" '$1 == unit {print $2}')
[[ -z "$state" || $state == disabled ]]
for current in "$unit" "$test_unit"; do
    if systemctl is-active --quiet "$current"; then echo "Stop $current before installing." >&2; exit 1; fi
done
check_port() {
    local listeners
    listeners=$(ss -H -ltn 'sport = :18089') || return 1
    [[ -z "$listeners" ]]
}
check_port
# Installation begins here. A failed preflight preserves the existing proof.
install -d -m755 -o root -g root /var/log/pf-sol-exit
install -m644 /dev/null /var/log/pf-sol-exit/self-test.log
if ! id "$account" >/dev/null 2>&1; then
    useradd --system --user-group --no-create-home --home-dir "$svc" --shell /usr/sbin/nologin "$account"
fi
[[ $(getent passwd "$account" | cut -d: -f6,7) == "$svc:/usr/sbin/nologin" ]]
[[ $(id -Gn "$account") == "$account" && $(id -u "$account") -ne 0 ]]
install -d -m755 -o root -g root "$app"
install -d -m750 -o root -g "$account" "$svc"
install -d -m700 -o "$account" -g "$account" "$svc/jobs"
cp -R "$bundle/runtime/." "$app/"
install -m640 -o root -g "$account" "$bundle/known_hosts" "$svc/known_hosts"
bash "$ops/check-prover-host.sh" "$svc/known_hosts"
python3 -I -m venv "$app/venv"
PIP_CONFIG_FILE=/dev/null "$app/venv/bin/python" -I -m pip --isolated --disable-pip-version-check install \
    --no-cache-dir --no-index --only-binary=:all: --require-hashes --find-links "$bundle/wheels" -r "$ops/exit-requirements.lock"
as_worker=(runuser -u "$account" -- env -i PATH=/usr/bin:/bin HOME="$svc" PYTHONDONTWRITEBYTECODE=1
           PYTHONPATH="$ops" EXIT_PROOF_HOME="$app/proof" "$app/venv/bin/python")
"${as_worker[@]}" -c 'import exit_service; from exit_proof import verify_local_host; verify_local_host()'
tmp=$(mktemp -d "$svc/.install.XXXXXXXX")
trap 'rm -rf -- "$tmp"; if [[ -n ${sudoers:-} ]]; then rm -f -- "$sudoers"; fi' EXIT
chgrp "$account" "$tmp"
chmod 0710 "$tmp"
secrets=$operator_home/.config/postfiat/sol-bridge
stage_credential() {
    python3 -I "$bundle/exit_install_credential.py" "$secrets/$1" "$tmp/$2" "$operator_uid"
    chown "$account:$account" "$tmp/$2"
}
stage_credential exit-prover-ssh prover
runuser -u "$account" -- bash "$ops/check-prover-identity.sh" "$tmp/prover" "$(cat "$ops/exit-prover.fingerprint")"
mv -T "$tmp/prover" "$svc/prover"
check_port
validate_payer() {
    "${as_worker[@]}" -c 'from exit_setup import FEE_PAYER, load, require; import sys; require(load(sys.argv[1]).pubkey()==FEE_PAYER, "Wrong installed payer")' "$1"
}
if [[ -f "$svc/fee-payer.json" ]]; then
    [[ ! -L "$svc/fee-payer.json" && $(stat -c '%u:%a:%h' "$svc/fee-payer.json") == "$(id -u "$account"):600:1" ]]
    validate_payer "$svc/fee-payer.json"
else
    [[ ! -e "$svc/fee-payer.json" && ! -L "$svc/fee-payer.json" ]]
    stage_credential exit-fee-payer.json fee-payer.json
    validate_payer "$tmp/fee-payer.json"
    mv -T "$tmp/fee-payer.json" "$svc/fee-payer.json"
fi
install -m644 "$ops/$unit" "$ops/$test_unit" /etc/systemd/system/
sudoers=$(mktemp /etc/sudoers.d/.pf-sol-exit.XXXXXXXX)
cat > "$sudoers" <<RULES
$operator ALL=(root) NOPASSWD: /usr/bin/systemctl start pft-sol-exit-self-test.service, /usr/bin/systemctl start $unit, /usr/bin/systemctl stop $unit, /usr/bin/systemctl --no-pager status $unit
RULES
chmod 0440 "$sudoers"
visudo -cf "$sudoers"
mv -T "$sudoers" /etc/sudoers.d/pf-sol-exit
systemctl daemon-reload
[[ $(systemctl is-enabled "$unit") == disabled ]]
printf '%s\n' "$rev" > "$app/revision"
echo "Installed disabled worker at $rev. Run the archive self-test."
