#!/bin/bash
# Run only from the owner's digest-verified private stage:
#   install-exit-system.sh [--allow-downgrade]   install or upgrade to the bundle's revision
#   install-exit-system.sh --rollback            put the previous tree back
set -euo pipefail
umask 022
operator=postfiat
account=pf-sol-exit
# The worker's own user and group ID, set here only. 0-1000 hold system and distribution
# accounts and the users of container images (databases run as 999, 70, 27, 101 or 1000), and
# 60000 up holds systemd's homed and dynamic users and nobody.
account_id=59900
svc=/var/lib/pf-sol-exit
base=/opt/postfiat
# The live tree is the symlink $app; each installed revision has its own tree under $trees, and
# $previous names the tree before the last switch. $history lists revisions in first-install order.
app=$base/sol-exit
trees=$base/sol-exit.d
previous=$base/sol-exit.previous
history=$base/sol-exit.history
ops=$app/ops/sol-live
# Bash ignores `set -e` inside a condition (`if`, `!`, `&&`, `||`) and in every function called
# from one, and a command substitution inside `[[ ]]` or an argument loses its status. Every step
# below therefore tests its own result, so a function is safe in any context.
unit=pft-sol-mainnet-withdraw.service
test_unit=pft-sol-exit-self-test.service
# The self-test unit records the revision it passed on; the worker starts only on that revision.
passed=/var/lib/pf-sol-exit-install/self-test-passed
unit_dir=/etc/systemd/system
# Masks here override unit files in /etc/systemd/system; `systemctl mask --runtime` writes to
# /run/systemd/system, which /etc overrides, so its mask would not hold for these units.
mask_dir=/run/systemd/system.control
masked=0

# A unit counts as stopped only when inactive or failed: activating, deactivating or reloading
# units still run code from the live tree.
require_stopped() {
    local current state
    for current in "$@"; do
        state=$(systemctl show -p ActiveState --value -- "$current") || {
            echo "Cannot read the state of $current." >&2; return 1; }
        case $state in
            inactive|failed) ;;
            *) echo "Stop $current before installing: it is ${state:-unknown}." >&2; return 1 ;;
        esac
    done
}

# A bundle is a downgrade when the installed tree names its revision as an ancestor (the tree's
# `ancestry` file, one revision per line, when the bundle carries one), or when this host first
# installed it before the installed revision.
check_not_downgrade() {
    local rev=$1 installed=$2 allow=$3 ancestry=$4 order=$5 at_rev="" at_installed=""
    if [[ -z $installed || $rev == "$installed" || $allow == 1 ]]; then return 0; fi
    if [[ -f $ancestry ]]; then
        at_rev=$(line_of "$rev" "$ancestry") || return 1
        if [[ -n $at_rev ]]; then
            echo "Refusing downgrade: $rev is an ancestor of the installed $installed. Pass --allow-downgrade to install it." >&2
            return 1
        fi
    fi
    if [[ -f $order ]]; then
        at_rev=$(line_of "$rev" "$order") || return 1
        at_installed=$(line_of "$installed" "$order") || return 1
        if [[ -n $at_rev && -n $at_installed ]] && (( at_rev < at_installed )); then
            echo "Refusing downgrade: $rev was installed here before $installed. Pass --allow-downgrade to install it." >&2
            return 1
        fi
    fi
    return 0
}

# The number of the first line of file $2 equal to $1, empty when there is none; fails when the
# file cannot be read, so a read error never counts as "not listed".
line_of() {
    awk -v want="$1" '$0 == want { print NR; exit }' "$2" || { echo "Cannot read $2." >&2; return 1; }
}

# Point $1 at $3 in one rename; $2 then names the tree $1 named before.
switch_tree() {
    local link=$1 before=$2 tree=$3 old=""
    if [[ -L $link ]]; then old=$(readlink -- "$link") || return 1; fi
    ln -sfnT -- "$tree" "$link.next" || return 1
    mv -T -- "$link.next" "$link" || return 1
    if [[ -n $old && $old != "$tree" ]]; then
        ln -sfnT -- "$old" "$before.next" || return 1
        mv -T -- "$before.next" "$before" || return 1
    fi
    return 0
}

# Swap the live and previous trees, each in one rename.
rollback_tree() {
    local link=$1 before=$2 current earlier
    [[ -L $link && -L $before ]] || { echo "No previous tree to roll back to." >&2; return 1; }
    current=$(readlink -- "$link") || return 1
    earlier=$(readlink -- "$before") || return 1
    [[ -f $earlier/revision ]] || { echo "The previous tree has no recorded revision." >&2; return 1; }
    ln -sfnT -- "$earlier" "$link.next" || return 1
    mv -T -- "$link.next" "$link" || return 1
    ln -sfnT -- "$current" "$before.next" || return 1
    mv -T -- "$before.next" "$before" || return 1
}

# Record the revision of the live tree, only after the switch, and its first-install order.
record_revision() {
    local link=$1 rev=$2 order=$3 at=""
    printf '%s\n' "$rev" > "$link/revision.next" || return 1
    mv -T -- "$link/revision.next" "$link/revision" || return 1
    if [[ -f $order ]]; then at=$(line_of "$rev" "$order") || return 1; fi
    if [[ -z $at ]]; then printf '%s\n' "$rev" >> "$order" || return 1; fi
    return 0
}

# Install the bundle: build and check a staged tree beside the live one, then put its unit files
# and the self-test gate in place and load them, and only then switch the tree in. A failed build
# or check removes the staged tree and leaves the live tree, its revision, the units and the
# earlier self-test pass as they were. Both units stay masked throughout (mask_units).
install_tree() {
    local bundle=$1 rev=$2 payer=${3:-$svc/fee-payer.json}
    stage_tree "$bundle" "$rev" "$payer" || return 1
    rm -f -- "$passed" || return 1
    install_units "$staged" || return 1
    switch_in "$staged" "$rev" || return 1
}

# Build and check a tree under $trees; its path is left in $staged.
stage_tree() {
    local bundle=$1 rev=$2 payer=$3 stage ok=0
    staged=""
    install -d -m755 -o root -g root "$trees" || return 1
    stage=$(mktemp -d "$trees/$rev.XXXXXXXX") || return 1
    # Each step is run on its own and its status kept; the switch needs all of them to succeed.
    chmod 0755 "$stage" && ok=1
    if (( ok )); then ok=0; build_tree "$bundle" "$stage" && ok=1; fi
    if (( ok )); then ok=0; self_test_tree "$stage" "$payer" && ok=1; fi
    if (( ! ok )); then
        rm -rf -- "$stage"
        echo "The new tree failed to build or its self-test failed; the installed tree is unchanged." >&2
        return 1
    fi
    staged=$stage
}

# The unit files that belong to tree $1, and the gates, in place and loaded before $1 goes live,
# so the worker never runs a tree without its gate.
install_units() {
    local tree=$1 name
    install -d -m755 "$unit_dir" || return 1
    for name in "$unit" "$test_unit"; do
        cp -- "$tree/ops/sol-live/$name" "$unit_dir/$name.next" || return 1
        chmod 0644 "$unit_dir/$name.next" || return 1
        mv -T -- "$unit_dir/$name.next" "$unit_dir/$name" || return 1
    done
    write_gates "$unit_dir" || return 1
    systemctl daemon-reload || return 1
}

# Switch the checked tree in, checking again that both units are stopped, then record its revision.
switch_in() {
    local tree=$1 rev=$2
    require_stopped "$unit" "$test_unit" || return 1
    switch_tree "$app" "$previous" "$tree" || return 1
    record_revision "$app" "$rev" "$history" || return 1
}

# Nothing can start either unit while masked: an install and an ID move run with both masked.
mask_units() {
    local name
    install -d -m755 "$mask_dir" || return 1
    for name in "$unit" "$test_unit"; do
        ln -sfnT /dev/null "$mask_dir/$name" || return 1
        masked=1
    done
    systemctl daemon-reload || return 1
}

unmask_units() {
    local name status=0
    for name in "$unit" "$test_unit"; do
        rm -f -- "$mask_dir/$name" || status=1
    done
    systemctl daemon-reload || status=1
    if (( status == 0 )); then masked=0; fi
    return $status
}

build_tree() {
    local bundle=$1 tree=$2
    cp -R "$bundle/runtime/." "$tree/" || { echo "Copying the bundle into $tree failed." >&2; return 1; }
    python3 -I -m venv "$tree/venv" || { echo "Creating the venv in $tree failed." >&2; return 1; }
    PIP_CONFIG_FILE=/dev/null "$tree/venv/bin/python" -I -m pip --isolated --disable-pip-version-check install \
        --no-cache-dir --no-index --only-binary=:all: --require-hashes --find-links "$bundle/wheels" \
        -r "$tree/ops/sol-live/exit-requirements.lock" || { echo "Installing the wheels into $tree failed." >&2; return 1; }
}

as_worker() {
    local tree=$1
    shift
    runuser -u "$account" -- env -i PATH=/usr/bin:/bin HOME="$svc" PYTHONDONTWRITEBYTECODE=1 \
        PYTHONPATH="$tree/ops/sol-live" EXIT_PROOF_HOME="$tree/proof" "$tree/venv/bin/python" "$@"
}

self_test_tree() {
    local tree=$1 payer=${2:-$svc/fee-payer.json}
    as_worker "$tree" -c 'import exit_service; from exit_proof import verify_local_host; verify_local_host()' ||
        { echo "The local proof host check failed on $tree." >&2; return 1; }
    if [[ -f $payer ]]; then
        validate_payer "$tree" "$payer" || { echo "The payer check failed on $tree." >&2; return 1; }
    fi
    return 0
}

validate_payer() {
    as_worker "$1" -c 'from exit_setup import FEE_PAYER, load, require; import sys; require(load(sys.argv[1]).pubkey()==FEE_PAYER, "Wrong installed payer")' "$2"
}

# The service ID must be outside the reserved ranges and used by no other user or group.
check_account_id() {
    local id=$1 name=$2 user group
    if ! [[ $id =~ ^[1-9][0-9]*$ ]] || (( id <= 1000 || id >= 60000 )); then
        echo "Service ID $id is in a reserved range (system and container users up to 1000, systemd and nobody from 60000)." >&2
        return 1
    fi
    user=$(id_owner passwd "$id") || return 1
    group=$(id_owner group "$id") || return 1
    if [[ -n $user && $user != "$name" ]] || [[ -n $group && $group != "$name" ]]; then
        echo "Service ID $id already belongs to ${user:-$group}." >&2
        return 1
    fi
}

# The name that owns numeric ID $2 in database $1, empty when none; a lookup error fails.
id_owner() {
    local entry status=0
    entry=$(getent "$1" "$2") || status=$?
    case $status in
        0) printf '%s\n' "${entry%%:*}" ;;
        2) ;;
        *) echo "Looking up $1 ID $2 failed." >&2; return 1 ;;
    esac
}

# Move an existing account to its own ID, all units stopped: group ID, then user ID (usermod
# re-owns the files it owns in its home, $svc), then every file under $svc that still has the
# old group or user. Modes, including 600 on the key files, do not change.
migrate_account_id() {
    local name=$1 id=$2 home=$3 old_uid old_gid
    local left
    old_uid=$(id -u "$name") || return 1
    old_gid=$(getent group "$name" | cut -d: -f3) || return 1
    [[ $old_uid =~ ^[0-9]+$ && $old_gid =~ ^[0-9]+$ ]] || { echo "Cannot read the IDs of $name." >&2; return 1; }
    if [[ $old_uid == "$id" && $old_gid == "$id" ]]; then return 0; fi
    check_account_id "$id" "$name" || return 1
    echo "Moving $name from user ID $old_uid and group ID $old_gid to $id."
    groupmod -g "$id" "$name" || return 1
    usermod -u "$id" "$name" || return 1
    find "$home" -xdev -gid "$old_gid" -exec chgrp -h "$id" {} + || return 1
    find "$home" -xdev -uid "$old_uid" -exec chown -h "$id" {} + || return 1
    left=$(ids_left "$home" "$old_uid" "$old_gid") || { echo "Cannot check the IDs under $home." >&2; return 1; }
    if [[ -n $left ]]; then
        echo "Files under $home still have the old IDs." >&2
        return 1
    fi
}

ids_left() { find "$1" -xdev \( -uid "$2" -o -gid "$3" \) -print -quit; }

create_account() {
    local name=$1 id=$2 home=$3
    check_account_id "$id" "$name" || return 1
    groupadd --system --gid "$id" "$name" || return 1
    useradd --system --uid "$id" --gid "$id" --no-create-home --home-dir "$home" --shell /usr/sbin/nologin "$name" || return 1
}

# The pass names the tree the self-test ran on (its directory under $trees) and that tree's
# revision; the worker starts only when the live tree is that same tree. A self-test that saw the
# link move while it ran records nothing.
write_gates() {
    local dir=$1 state=${passed%/*}
    install -d -m755 "$dir/$unit.d" "$dir/$test_unit.d" || return 1
    cat > "$dir/$unit.d/self-test-gate.conf" <<GATE || return 1
[Service]
ExecStartPre=+/bin/sh -c 'tree=\$\$(readlink -e $app) && test -s $passed && test "\$\$(cat $passed)" = "\$\$tree \$\$(cat \$\$tree/revision)"'
GATE
    cat > "$dir/$test_unit.d/record-pass.conf" <<PASS || return 1
[Service]
ExecStartPre=+/bin/sh -c 'install -d -m755 $state && rm -f $passed && readlink -e $app > $state/self-test-tree'
ExecStartPost=+/bin/sh -c 'tree=\$\$(readlink -e $app) && test "\$\$tree" = "\$\$(cat $state/self-test-tree)" && echo "\$\$tree \$\$(cat \$\$tree/revision)" > $passed'
PASS
    chmod 0644 "$dir/$unit.d/self-test-gate.conf" "$dir/$test_unit.d/record-pass.conf" || return 1
}

check_port() {
    local listeners
    listeners=$(ss -H -ltn 'sport = :18089') || return 1
    [[ -z "$listeners" ]]
}

# The exit trap: removes the private staging directory and an unfinished sudoers file, and
# unmasks both units, on success and on any failure.
cleanup() {
    local status=$?
    if [[ -n ${tmp:-} ]]; then rm -rf -- "$tmp"; fi
    if [[ -n ${sudoers:-} ]]; then rm -f -- "$sudoers"; fi
    if (( masked )) && ! unmask_units; then
        echo "Unmask by hand: rm $mask_dir/$unit $mask_dir/$test_unit; systemctl daemon-reload" >&2
        status=1
    fi
    exit "$status"
}

# Tests source this file for the functions above.
if [[ ${BASH_SOURCE[0]} != "$0" ]]; then return 0; fi

allow_downgrade=0 rollback=0
for arg in "$@"; do
    case $arg in
        --allow-downgrade) allow_downgrade=1 ;;
        --rollback) rollback=1 ;;
        *) echo "Unknown argument $arg." >&2; exit 2 ;;
    esac
done
[[ $EUID == 0 ]]
bundle=$(cd -- "$(dirname -- "$0")" && pwd -P)
owner_mode=$(stat -c '%u:%a' "$bundle")
[[ $owner_mode == 0:700 && $bundle == /var/lib/pf-sol-install.* ]]
cd /
exec 9>/run/pf-sol-exit-install.lock
flock -n 9
operator_uid=$(id -u "$operator")
operator_home=$(getent passwd "$operator" | cut -d: -f6)
rev=$(cat "$bundle/revision")
[[ $rev =~ ^[0-9a-f]{40}$ ]]
for dir in "$base" "$trees" "$svc" /var/log/pf-sol-exit; do
    [[ ! -L "$dir" ]]
    if [[ -e "$dir" ]]; then
        dir_owner=$(stat -c %u "$dir")
        writable=$(find "$dir" -maxdepth 0 -perm /022 -print)
        [[ -d "$dir" && $dir_owner == 0 && -z $writable ]]
    fi
done
# The live tree is a root-owned symlink into $trees, or a directory from an installer before it.
if [[ -L $app ]]; then [[ $(readlink -- "$app") == "$trees"/* ]]; fi
state=$(systemctl list-unit-files --type=service --no-legend --no-pager | awk -v unit="$unit" '$1 == unit {print $2}')
[[ -z "$state" || $state == disabled ]]
require_stopped "$unit" "$test_unit"
# From here to the end both units are masked; the exit trap unmasks them on success and on any failure.
trap cleanup EXIT
mask_units
require_stopped "$unit" "$test_unit"
if (( rollback )); then
    rm -f -- "$passed"
    rollback_tree "$app" "$previous"
    restored=$(cat "$app/revision")
    echo "Rolled back to $restored. Run the archive self-test before starting the worker."
    exit 0
fi
installed=""
if [[ -f "$app/revision" ]]; then installed=$(cat "$app/revision"); fi
check_not_downgrade "$rev" "$installed" "$allow_downgrade" "$app/ancestry" "$history"
if [[ -n $installed && $installed != "$rev" ]]; then
    echo "Upgrading installed worker from $installed to $rev."
fi
check_port
# Installation begins here. A failed preflight preserves the existing proof.
install -d -m755 -o root -g root /var/log/pf-sol-exit
install -m644 /dev/null /var/log/pf-sol-exit/self-test.log
require_stopped "$unit" "$test_unit"
if id "$account" >/dev/null 2>&1; then
    migrate_account_id "$account" "$account_id" "$svc"
else
    create_account "$account" "$account_id" "$svc"
fi
[[ $(getent passwd "$account" | cut -d: -f6,7) == "$svc:/usr/sbin/nologin" ]]
[[ $(id -Gn "$account") == "$account" && $(id -u "$account") == "$account_id" ]]
install -d -m750 -o root -g "$account" "$svc"
install -d -m700 -o "$account" -g "$account" "$svc/jobs"
install -m640 -o root -g "$account" "$bundle/known_hosts" "$svc/known_hosts"
bash "$bundle/runtime/ops/sol-live/check-prover-host.sh" "$svc/known_hosts"
# A tree from an installer before this one becomes the first entry under $trees, in one rename.
if [[ -d $app && ! -L $app ]]; then
    install -d -m755 -o root -g root "$trees"
    legacy="$trees/${installed:-legacy}.legacy"
    mv -T -- "$app" "$legacy"
    ln -sT -- "$legacy" "$app"
    if [[ -n $installed ]]; then
        at=""
        if [[ -f $history ]]; then at=$(line_of "$installed" "$history"); fi
        if [[ -z $at ]]; then printf '%s\n' "$installed" >> "$history"; fi
    fi
fi
# Credentials go through a private directory in $svc (root and the worker's group only), never
# through a world-readable place, and end at mode 600 owned by the worker.
tmp=$(mktemp -d "$svc/.install.XXXXXXXX")
chgrp "$account" "$tmp"
chmod 0710 "$tmp"
secrets=$operator_home/.config/postfiat/sol-bridge
stage_credential() {
    python3 -I "$bundle/exit_install_credential.py" "$secrets/$1" "$tmp/$2" "$operator_uid" || return 1
    chown "$account:$account" "$tmp/$2" || return 1
}
stage_credential exit-prover-ssh prover
fingerprint=$(cat "$bundle/runtime/ops/sol-live/exit-prover.fingerprint")
[[ -n $fingerprint ]]
runuser -u "$account" -- bash "$bundle/runtime/ops/sol-live/check-prover-identity.sh" "$tmp/prover" "$fingerprint"
mv -T "$tmp/prover" "$svc/prover"
check_port
if [[ -f "$svc/fee-payer.json" ]]; then
    [[ ! -L "$svc/fee-payer.json" && $(stat -c '%u:%a:%h' "$svc/fee-payer.json") == "$account_id:600:1" ]]
else
    [[ ! -e "$svc/fee-payer.json" && ! -L "$svc/fee-payer.json" ]]
    stage_credential exit-fee-payer.json fee-payer.json
fi
sudoers=$(mktemp /etc/sudoers.d/.pf-sol-exit.XXXXXXXX)
cat > "$sudoers" <<RULES
$operator ALL=(root) NOPASSWD: /usr/bin/systemctl start pft-sol-exit-self-test.service, /usr/bin/systemctl start $unit, /usr/bin/systemctl stop $unit, /usr/bin/systemctl --no-pager status $unit
RULES
chmod 0440 "$sudoers"
visudo -cf "$sudoers"
mv -T "$sudoers" /etc/sudoers.d/pf-sol-exit
sudoers=""
# The new tree is built and checked beside the live one, its units and gate are loaded, and only
# then is it switched in.
payer=$svc/fee-payer.json
if [[ -f "$tmp/fee-payer.json" ]]; then payer=$tmp/fee-payer.json; fi
install_tree "$bundle" "$rev" "$payer"
if [[ -f "$tmp/fee-payer.json" ]]; then
    mv -T "$tmp/fee-payer.json" "$svc/fee-payer.json"
fi
enabled=$(systemctl is-enabled "$unit" || true)
[[ $enabled == disabled ]]
echo "Installed disabled worker at $rev. Run the archive self-test; the worker starts only after it passed on $rev."
