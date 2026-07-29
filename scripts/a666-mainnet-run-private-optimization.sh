#!/usr/bin/env bash
set -euo pipefail

repo=$(cd "$(dirname "$0")/.." && pwd)
phase_dir=

while (($#)); do
  case "$1" in
    --phase-dir) phase_dir=$2; shift 2 ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done
test -n "$phase_dir"

cd "$repo"
phase_dir=$(realpath "$phase_dir")
manifest="$phase_dir/run-manifest.json"
script_hashes="$phase_dir/script-sha256.json"
test -s "$manifest"
test -s "$script_hashes"
test -s "$phase_dir/pre-funding-sha256.txt"
test ! -e "$phase_dir/deposit"
test ! -e "$phase_dir/intervention-log.json"
git diff --quiet
git diff --cached --quiet

workflow_id=$(jq -er '.workflow_id' "$manifest")
deposit_atoms=$(jq -er '.amounts.deposit_atoms' "$manifest")
mint_atoms=$(jq -er '.amounts.mint_atoms' "$manifest")
redemption_output_atoms=$(jq -er '.amounts.redemption_output_atoms' "$manifest")
expected_pftl_height=$(jq -er '.expected_pftl_height' "$manifest")
expected_verifier_height=$(jq -er '.expected_verifier_height' "$manifest")
prior_checkpoint=$(jq -er '.prior_checkpoint_block_id' "$manifest")
wrapped_balance_before=$(jq -er '.expected_wrapped_balance_before' "$manifest")
wrapped_supply_before=$(jq -er '.expected_wrapped_supply_before' "$manifest")
lane_manifest=$(jq -er '.pfusdc_deposit_lane.deployment_manifest' "$manifest")
lane_manifest_sha=$(jq -er '.pfusdc_deposit_lane.deployment_manifest_sha256' "$manifest")

[[ "$workflow_id" =~ ^[a-z0-9][a-z0-9-]{0,39}$ ]]
[[ "$deposit_atoms" =~ ^[1-9][0-9]*$ ]]
[[ "$mint_atoms" =~ ^[1-9][0-9]*$ ]]
[[ "$redemption_output_atoms" =~ ^[1-9][0-9]*$ ]]
sha256sum -c "$phase_dir/pre-funding-sha256.txt" >/dev/null
while IFS=$'\t' read -r script expected_sha; do
  test "$(sha256sum "$script" | awk '{print $1}')" = "$expected_sha"
done < <(jq -r '.scripts | to_entries[] | [.key,.value] | @tsv' "$script_hashes")

started_unix=$(date +%s)
failure_file="$phase_dir/run-failure.json"
on_error() {
  code=$?
  jq -n \
    --arg workflow_id "$workflow_id" \
    --argjson started_unix "$started_unix" \
    --argjson failed_unix "$(date +%s)" \
    --argjson exit_code "$code" \
    '{
      schema:"postfiat.a666.optimization_runner_failure.v1",
      verdict:"FAIL",
      workflow_id:$workflow_id,
      started_unix:$started_unix,
      failed_unix:$failed_unix,
      exit_code:$exit_code,
      intervention_free_after_deposit:false
    }' > "$failure_file"
  exit "$code"
}
trap on_error ERR

mkdir -p "$phase_dir/deposit"
python3 scripts/a666-mainnet-pfusdc-deposit.py \
  --amount-atoms "$deposit_atoms" \
  --output "$phase_dir/deposit/deposit-result.json" \
  --deployment-manifest "$lane_manifest" \
  --expected-manifest-sha256 "$lane_manifest_sha"

bash scripts/a666-mainnet-transparent-issue-after-deposit.sh \
  --phase-dir "$phase_dir" \
  --workflow-id "$workflow_id" \
  --expected-pftl-height "$expected_pftl_height" \
  --prior-checkpoint-block-id "$prior_checkpoint" \
  --expected-verifier-height "$expected_verifier_height" \
  --expected-wrapped-balance-before "$wrapped_balance_before" \
  --expected-wrapped-supply-before "$wrapped_supply_before" \
  --private-middle

bash scripts/a666-mainnet-private-roundtrip-after-mint.sh \
  --phase-dir "$phase_dir" \
  --workflow-id "$workflow_id" \
  --nav-amount-atoms "$mint_atoms" \
  --settlement-output-atoms "$redemption_output_atoms"

completed_unix=$(date +%s)
trap - ERR
jq -n \
  --arg workflow_id "$workflow_id" \
  --argjson started_unix "$started_unix" \
  --argjson completed_unix "$completed_unix" \
  --slurpfile issue "$phase_dir/summary.json" \
  --slurpfile redemption "$phase_dir/private-roundtrip-summary.json" \
  '{
    schema:"postfiat.a666.private_optimization_runner.v1",
    verdict:"PASS",
    workflow_id:$workflow_id,
    started_unix:$started_unix,
    completed_unix:$completed_unix,
    elapsed_seconds:($completed_unix-$started_unix),
    intervention_free_after_deposit:true,
    issue:$issue[0],
    redemption:$redemption[0]
  }' > "$phase_dir/runner-summary.json"
python3 scripts/a666-score-private-optimization-run.py \
  --phase-dir "$phase_dir"
echo "A666_OPTIMIZATION_RUNNER: PASS workflow=$workflow_id"
