#!/usr/bin/env bash
set -euo pipefail

specs=${A666_WALLET_TUNNEL_SPECS:-}
identity=${A666_WALLET_SSH_IDENTITY_FILE:-}
ssh_user=${A666_WALLET_SSH_USER:-root}
ssh_bin=${A666_WALLET_SSH_BIN:-ssh}

if [[ -z $specs || -z $identity ]]; then
  echo 'A666 wallet tunnel specs and SSH identity file are required' >&2
  exit 2
fi
if [[ ! -f $identity || ! -r $identity || ! $ssh_user =~ ^[A-Za-z_][A-Za-z0-9_-]{0,31}$ ]] \
  || ! command -v "$ssh_bin" >/dev/null 2>&1; then
  echo 'A666 wallet tunnel SSH configuration is invalid' >&2
  exit 2
fi

declare -a tunnel_pids=()
declare -A required_names=(
  [validator-0]=1
  [validator-1]=1
  [validator-2]=1
  [validator-3]=1
  [validator-4]=1
  [validator-5]=1
  [private-swap]=1
)
declare -A seen_names=()
declare -A seen_local_ports=()
declare -A seen_remote_endpoints=()

stop_tunnels() {
  local pid
  for pid in "${tunnel_pids[@]}"; do
    kill -TERM "$pid" 2>/dev/null || true
  done
  wait "${tunnel_pids[@]}" 2>/dev/null || true
}
trap stop_tunnels EXIT INT TERM

IFS=';' read -r -a entries <<< "$specs"
for entry in "${entries[@]}"; do
  IFS=',' read -r name local_port ssh_host remote_port extra <<< "$entry"
  if [[ -n ${extra:-} || ! ${name:-} =~ ^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$ \
    || ! ${local_port:-} =~ ^[1-9][0-9]{0,4}$ || ! ${remote_port:-} =~ ^[1-9][0-9]{0,4}$ \
    || ! ${ssh_host:-} =~ ^[A-Za-z0-9][A-Za-z0-9._-]{0,252}$ \
    || $local_port -lt 1024 || $local_port -gt 65535 \
    || $remote_port -lt 1 || $remote_port -gt 65535 ]]; then
    echo "invalid bounded A666 wallet tunnel entry: ${name:-unnamed}" >&2
    exit 2
  fi
  remote_endpoint="${ssh_host}:${remote_port}"
  if [[ -z ${required_names[$name]+present} || -n ${seen_names[$name]+present} \
    || -n ${seen_local_ports[$local_port]+present} \
    || -n ${seen_remote_endpoints[$remote_endpoint]+present} ]]; then
    echo "duplicate or unexpected A666 wallet tunnel entry: $name" >&2
    exit 2
  fi
  seen_names[$name]=1
  seen_local_ports[$local_port]=1
  seen_remote_endpoints[$remote_endpoint]=1
  "$ssh_bin" -NT \
    -i "$identity" \
    -o BatchMode=yes \
    -o ExitOnForwardFailure=yes \
    -o ServerAliveInterval=15 \
    -o ServerAliveCountMax=3 \
    -o StrictHostKeyChecking=yes \
    -L "127.0.0.1:${local_port}:127.0.0.1:${remote_port}" \
    "${ssh_user}@${ssh_host}" &
  tunnel_pids+=("$!")
done

if [[ ${#tunnel_pids[@]} -ne ${#required_names[@]} ]]; then
  echo 'exactly six validator RPC tunnels and one private-swap tunnel are required' >&2
  exit 2
fi

set +e
wait -n "${tunnel_pids[@]}"
status=$?
set -e
echo 'an A666 wallet tunnel exited; restarting the complete bounded tunnel set' >&2
if [[ $status -eq 0 ]]; then
  exit 1
fi
exit "$status"
