//! The Base lock contract's minimum Lock is the PFTL account reserve.
//!
//! `PFTLockV1` refuses a Lock below `minimumLockAmount`, and the lock deployment
//! script takes that value from `PFT_LOCK_MINIMUM_LOCK_AMOUNT` in
//! `crates/pft-token-contracts/script/pft-lock.env.example`. A smaller Lock to a
//! new PFTL account would be final on Base but refused on PFTL with
//! `below_account_reserve` (the release path's own check, which stays as the
//! second line of defence). Changing one value without the other fails here.

const LOCK_DEPLOYMENT_CONFIG: &str =
    include_str!("../../pft-token-contracts/script/pft-lock.env.example");

fn pinned_minimum_lock_amount(config: &str) -> u64 {
    let mut values = config
        .lines()
        .filter_map(|line| line.trim().strip_prefix("PFT_LOCK_MINIMUM_LOCK_AMOUNT="));
    let value = values
        .next()
        .expect("the lock deployment config pins PFT_LOCK_MINIMUM_LOCK_AMOUNT");
    assert!(
        values.next().is_none(),
        "PFT_LOCK_MINIMUM_LOCK_AMOUNT is pinned once"
    );
    value
        .trim()
        .parse()
        .expect("PFT_LOCK_MINIMUM_LOCK_AMOUNT is a number, not a placeholder")
}

#[test]
fn lock_deployment_minimum_equals_the_pftl_account_reserve() {
    assert_eq!(
        pinned_minimum_lock_amount(LOCK_DEPLOYMENT_CONFIG),
        crate::ACCOUNT_RESERVE,
        "PFTLockV1's minimum Lock must equal ACCOUNT_RESERVE (crates/execution/src/entrypoints.rs)"
    );
}

#[test]
fn a_lock_at_the_minimum_can_create_a_new_pftl_account() {
    let minimum = pinned_minimum_lock_amount(LOCK_DEPLOYMENT_CONFIG);
    let recipient = "pf0000000000000000000000000000000000000001";
    assert_eq!(crate::account_reserve_violation(recipient, minimum), None);
    assert!(crate::account_reserve_violation(recipient, minimum - 1).is_some());
}
