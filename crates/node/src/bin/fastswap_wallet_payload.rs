use postfiat_crypto_provider::{hash_bytes, hex_to_bytes};
use postfiat_types::{
    FastAssetIdV1, FastLaneDepositV1, FastLanePrimaryOperationV1,
    FastLanePrimaryTransactionV1, FastObjectIdV1, FastObjectKeyV1,
    SignedFastLaneDepositV1,
    FastSwapGovernanceBootstrapPayloadV1, FastSwapIntentV1, FastSwapPartyV1,
    FastSwapRfqHashV1,
};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn flag(args: &[String], name: &str) -> Result<String, String> {
    let index = args
        .iter()
        .position(|value| value == name)
        .ok_or_else(|| format!("missing {name}"))?;
    args.get(index + 1)
        .cloned()
        .ok_or_else(|| format!("missing value for {name}"))
}

fn read_payload(path: &Path) -> Result<FastSwapGovernanceBootstrapPayloadV1, String> {
    let bytes = fs::read(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let payload: FastSwapGovernanceBootstrapPayloadV1 = serde_json::from_slice(&bytes)
        .map_err(|error| format!("{}: {error}", path.display()))?;
    payload
        .validate_payload()
        .map_err(|error| format!("invalid bootstrap payload: {error:?}"))?;
    Ok(payload)
}

fn bytes<const N: usize>(value: &str, label: &str) -> Result<[u8; N], String> {
    hex_to_bytes(value.trim_start_matches("0x"))
        .map_err(|error| format!("invalid {label}: {error}"))?
        .try_into()
        .map_err(|_| format!("{label} must be {N} bytes"))
}

fn write_private(path: &Path, value: &impl serde::Serialize) -> Result<(), String> {
    if path.exists() {
        return Err(format!("{} already exists", path.display()));
    }
    let encoded = serde_json::to_vec_pretty(value).map_err(|error| error.to_string())?;
    fs::write(path, encoded).map_err(|error| format!("{}: {error}", path.display()))
}

fn deposit(args: &[String]) -> Result<(), String> {
    let payload = read_payload(&PathBuf::from(flag(args, "--payload")?))?;
    let asset_id = FastAssetIdV1(bytes(&flag(args, "--asset-id")?, "asset id")?);
    let rule = payload
        .asset_rules
        .iter()
        .find(|rule| rule.asset_id == asset_id)
        .ok_or_else(|| "asset is absent from the activated bootstrap payload".to_owned())?;
    let source_address = flag(args, "--source-address")?;
    let source_pubkey = hex_to_bytes(&flag(args, "--source-pubkey")?)
        .map_err(|error| format!("invalid source public key: {error}"))?;
    let sequence = flag(args, "--sequence")?
        .parse::<u64>()
        .map_err(|_| "--sequence must be a u64".to_owned())?;
    let amount_atoms = flag(args, "--amount-atoms")?
        .parse::<u64>()
        .map_err(|_| "--amount-atoms must be a u64".to_owned())?;
    if amount_atoms == 0 {
        return Err("deposit amount must be positive".to_owned());
    }
    let value = FastLaneDepositV1 {
        domain: payload.committee.domain.chain,
        source_address,
        source_pubkey: source_pubkey.clone(),
        sequence,
        fee_pft: 1,
        destination_owner_pubkey: source_pubkey,
        destination_holder_permit_id: None,
        asset_id,
        asset_rule_hash: rule
            .rule_hash()
            .map_err(|error| format!("asset rule hash: {error:?}"))?,
        amount_atoms,
        nonce: bytes(&flag(args, "--nonce")?, "deposit nonce")?,
    };
    value
        .signing_bytes()
        .map_err(|error| format!("deposit validation: {error:?}"))?;
    write_private(&PathBuf::from(flag(args, "--output")?), &value)
}

fn primary(args: &[String]) -> Result<(), String> {
    let input = PathBuf::from(flag(args, "--signed-deposit")?);
    let signed: SignedFastLaneDepositV1 = serde_json::from_slice(
        &fs::read(&input).map_err(|error| format!("{}: {error}", input.display()))?,
    )
    .map_err(|error| format!("{}: {error}", input.display()))?;
    let value = FastLanePrimaryTransactionV1 {
        operation: FastLanePrimaryOperationV1::Deposit { signed },
    };
    let tx_id = value
        .tx_id()
        .map_err(|error| format!("primary transaction validation: {error:?}"))?;
    write_private(&PathBuf::from(flag(args, "--output")?), &value)?;
    println!("{}", tx_id.0.iter().map(|byte| format!("{byte:02x}")).collect::<String>());
    Ok(())
}

struct PartyArgs {
    address: String,
    pubkey: Vec<u8>,
    object_id: [u8; 32],
    object_version: u64,
    amount: u64,
}

fn quote_asset_1(amount_0: u64, numerator: u128, denominator: u128) -> Result<u64, String> {
    if denominator == 0 {
        return Err("price denominator is zero".to_owned());
    }
    let quoted = u128::from(amount_0)
        .checked_mul(numerator)
        .ok_or_else(|| "quote multiplication overflow".to_owned())?
        / denominator;
    quoted
        .try_into()
        .map_err(|_| "governed quote exceeds u64 asset atoms".to_owned())
}

fn party_args(args: &[String], suffix: &str) -> Result<PartyArgs, String> {
    let amount = flag(args, &format!("--amount-{suffix}"))?
        .parse::<u64>()
        .map_err(|_| format!("--amount-{suffix} must be a u64"))?;
    if amount == 0 {
        return Err(format!("--amount-{suffix} must be positive"));
    }
    Ok(PartyArgs {
        address: flag(args, &format!("--owner-{suffix}-address"))?,
        pubkey: hex_to_bytes(&flag(args, &format!("--owner-{suffix}-pubkey"))?)
            .map_err(|error| format!("invalid owner {suffix} public key: {error}"))?,
        object_id: bytes(&flag(args, &format!("--object-{suffix}-id"))?, "object id")?,
        object_version: flag(args, &format!("--object-{suffix}-version"))?
            .parse::<u64>()
            .map_err(|_| format!("--object-{suffix}-version must be a u64"))?,
        amount,
    })
}

fn intent(args: &[String]) -> Result<(), String> {
    let payload = read_payload(&PathBuf::from(flag(args, "--payload")?))?;
    if payload.policies.len() != 1 {
        return Err("wallet helper requires one unambiguous active policy".to_owned());
    }
    let policy = &payload.policies[0];
    if policy.paused {
        return Err("FastSwap policy is paused".to_owned());
    }
    let first = party_args(args, "0")?;
    let second = party_args(args, "1")?;
    let expected_asset_1 = quote_asset_1(
        first.amount,
        policy.price_numerator,
        policy.price_denominator,
    )?;
    if expected_asset_1 != second.amount {
        return Err(format!(
            "amount-1 does not match the governed quote: expected {expected_asset_1}"
        ));
    }
    let amount_0 = first.amount;
    let amount_1 = second.amount;
    let make_party = |owner: PartyArgs,
                      offered_asset_id: FastAssetIdV1,
                      offered_rule_hash,
                      received_asset_id: FastAssetIdV1,
                      received_rule_hash,
                      received_amount: u64| FastSwapPartyV1 {
        owner_address: owner.address,
        owner_pubkey: owner.pubkey,
        offered_asset_id,
        offered_asset_rule_hash: offered_rule_hash,
        offered_amount: owner.amount,
        receives_asset_id: received_asset_id,
        receives_asset_rule_hash: received_rule_hash,
        receives_holder_permit_id: None,
        receives_amount: received_amount,
        asset_inputs: vec![FastObjectKeyV1 {
            object_id: FastObjectIdV1(owner.object_id),
            version: owner.object_version,
        }],
        fee_inputs: Vec::new(),
        asset_change: 0,
        fee_change: 0,
        fee_burn_pft: 0,
    };
    let expires_at_height = flag(args, "--expires-at-height")?
        .parse::<u64>()
        .map_err(|_| "--expires-at-height must be a u64".to_owned())?;
    if expires_at_height < policy.valid_from_height || expires_at_height > policy.valid_through_height {
        return Err("intent expiry is outside the governed policy window".to_owned());
    }
    let rfq: [u8; 48] = hash_bytes(
        "postfiat.fastswap.wallet_rfq.v1",
        flag(args, "--rfq")?.as_bytes(),
    )
    .try_into()
    .map_err(|_| "RFQ hash must be 48 bytes".to_owned())?;
    let value = FastSwapIntentV1 {
        domain: payload.committee.domain,
        policy_hash: policy.policy_hash,
        rfq_hash: FastSwapRfqHashV1(rfq),
        market_envelope_hash: policy.market_envelope_hash,
        nav_epoch: policy.nav_epoch,
        expires_at_height,
        nonce: bytes(&flag(args, "--nonce")?, "intent nonce")?,
        party_0: make_party(
            first,
            policy.pair_asset_0,
            policy.asset_rule_hash_0,
            policy.pair_asset_1,
            policy.asset_rule_hash_1,
            amount_1,
        ),
        party_1: make_party(
            second,
            policy.pair_asset_1,
            policy.asset_rule_hash_1,
            policy.pair_asset_0,
            policy.asset_rule_hash_0,
            amount_0,
        ),
    };
    value
        .canonical_bytes()
        .map_err(|error| format!("intent validation: {error:?}"))?;
    write_private(&PathBuf::from(flag(args, "--output")?), &value)
}

fn main() -> Result<(), String> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    match args.first().map(String::as_str) {
        Some("deposit") => deposit(&args[1..]),
        Some("primary") => primary(&args[1..]),
        Some("intent") => intent(&args[1..]),
        _ => Err("usage: fastswap_wallet_payload <deposit|primary|intent> [flags]".to_owned()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quote_uses_governed_down_rounding_without_narrowing_prices() {
        assert_eq!(quote_asset_1(9, 100_000_000, 820_102_177).unwrap(), 1);
        assert_eq!(quote_asset_1(10, 1, 1).unwrap(), 10);
        assert!(quote_asset_1(1, 1, 0).is_err());
        assert!(quote_asset_1(u64::MAX, u128::MAX, 1).is_err());
    }
}
