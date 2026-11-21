#!/usr/bin/env bash
# Stage only the pinned, non-secret release files in a new deployment directory.
set -euo pipefail
if [[ $# != 3 ]]; then
    echo 'Usage: stage-runtime.sh LIVE_SOURCE RECORDER_SOURCE NEW_CHECKOUT' >&2
    exit 2
fi
live=$(realpath -e -- "$1")
recorder=$(realpath -e -- "$2")
dest=$(realpath -e -- "$3")
manifest=$(realpath -e -- "$(dirname -- "$0")/runtime-files.sha256")
[[ $dest != "$live" && $dest != "$recorder" ]]
while read -r digest path; do
    case "$path" in
        programs/pfusdc-sol-mainnet-ingress/elf/*)
            source="$live/programs/pfusdc-sol-ingress/elf/${path##*/}" ;;
        ops/sol-recorder/target/*)
            source="$recorder/${path#ops/sol-recorder/}" ;;
        target/task-l/*)
            source="$live/$path" ;;
        *) echo "Unexpected release path: $path" >&2; exit 1 ;;
    esac
    if [[ -e "$dest/$path" || -L "$dest/$path" ]]; then
        (cd "$dest" && printf '%s  %s\n' "$digest" "$path" | sha256sum --check --status)
        continue
    fi
    printf '%s  %s\n' "$digest" "$source" | sha256sum --check --status
    mkdir -p -- "$(dirname -- "$dest/$path")"
    temp=$(mktemp -- "$dest/$path.XXXXXXXX")
    trap 'rm -f -- "$temp"' EXIT
    cp --preserve=mode -- "$source" "$temp"
    printf '%s  %s\n' "$digest" "$temp" | sha256sum --check --status
    # Publish a complete file without replacing an existing artifact.
    ln -- "$temp" "$dest/$path"
    rm -f -- "$temp"
    trap - EXIT
done < "$manifest"
(cd "$dest" && sha256sum --check - < "$manifest")
