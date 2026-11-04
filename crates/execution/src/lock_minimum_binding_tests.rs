//! The Base lock contract's minimum Lock is the PFTL account reserve.
//!
//! `PFTLockV1` refuses a minimum below its constant `PFTL_ACCOUNT_RESERVE` and a
//! Lock below `minimumLockAmount`. The lock deployment script refuses any
//! `PFT_LOCK_MINIMUM_LOCK_AMOUNT` other than its constant
//! `PFTL_ACCOUNT_RESERVE_UNITS`, and `script/pft-lock.env.example` sets that
//! value. A smaller Lock to a new PFTL account would be final on Base but
//! refused on PFTL with `below_account_reserve` (the release path's own check,
//! which stays as the second line of defence). Changing one of the three
//! without `ACCOUNT_RESERVE` fails here.

const LOCK_CONTRACT: &str = include_str!("../../pft-token-contracts/src/PFTLockV1.sol");
const LOCK_DEPLOYMENT_SCRIPT: &str =
    include_str!("../../pft-token-contracts/script/DeployPFTLock.s.sol");
const LOCK_DEPLOYMENT_CONFIG: &str =
    include_str!("../../pft-token-contracts/script/pft-lock.env.example");

/// The single `<prefix><n>` in `source`, `<n>` ending at `terminator`.
fn single_value(source: &str, prefix: &str, terminator: char) -> u64 {
    let mut values = source.lines().filter_map(|line| {
        let (_, rest) = line.split_once(prefix)?;
        Some(rest.split(terminator).next().unwrap_or(rest).trim())
    });
    let value = values
        .next()
        .unwrap_or_else(|| panic!("`{prefix}` is set in the source"));
    assert!(values.next().is_none(), "`{prefix}` is set once");
    value
        .parse()
        .unwrap_or_else(|_| panic!("`{prefix}` is a number, not a placeholder: {value}"))
}

fn contract_reserve() -> u64 {
    single_value(LOCK_CONTRACT, "PFTL_ACCOUNT_RESERVE = ", ';')
}

fn script_reserve() -> u64 {
    single_value(LOCK_DEPLOYMENT_SCRIPT, "PFTL_ACCOUNT_RESERVE_UNITS = ", ';')
}

fn configured_minimum() -> u64 {
    let config: String = LOCK_DEPLOYMENT_CONFIG
        .lines()
        .filter(|line| line.starts_with("PFT_LOCK_MINIMUM_LOCK_AMOUNT="))
        .collect();
    single_value(&config, "PFT_LOCK_MINIMUM_LOCK_AMOUNT=", '#')
}

#[test]
fn lock_minimum_equals_the_pftl_account_reserve_in_contract_script_and_config() {
    let reserve = crate::ACCOUNT_RESERVE;
    let source = "ACCOUNT_RESERVE (crates/execution/src/entrypoints.rs)";
    assert_eq!(
        contract_reserve(),
        reserve,
        "PFTLockV1.PFTL_ACCOUNT_RESERVE must equal {source}"
    );
    assert_eq!(
        script_reserve(),
        reserve,
        "DeployPFTLock.PFTL_ACCOUNT_RESERVE_UNITS must equal {source}"
    );
    assert_eq!(
        configured_minimum(),
        reserve,
        "PFT_LOCK_MINIMUM_LOCK_AMOUNT in pft-lock.env.example must equal {source}"
    );
}

#[test]
fn a_lock_at_the_minimum_can_create_a_new_pftl_account() {
    let minimum = contract_reserve();
    let recipient = "pf0000000000000000000000000000000000000001";
    assert_eq!(crate::account_reserve_violation(recipient, minimum), None);
    assert!(crate::account_reserve_violation(recipient, minimum - 1).is_some());
}
