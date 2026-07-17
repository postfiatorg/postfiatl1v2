/// True when a proof_profile string is shaped like a registered profile id
/// (96 lowercase hex chars) rather than a legacy free-text label.
fn is_nav_profile_id_shaped(value: &str) -> bool {
    value.len() == NAV_PROFILE_ID_HEX_LEN
        && value
            .chars()
            .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c))
}

/// Resolve the registered proof profile governing a NAV asset, if any.
/// Legacy assets whose proof_profile is a free-text label resolve to None
/// and keep pre-profile semantics.
fn nav_profile_for_asset<'a>(
    ledger: &'a LedgerState,
    nav_asset: &NavTrackedAsset,
) -> Option<&'a NavProofProfile> {
    if !is_nav_profile_id_shaped(&nav_asset.proof_profile) {
        return None;
    }
    ledger.nav_proof_profile(&nav_asset.proof_profile)
}

fn vault_bridge_profile_for_asset<'a>(
    ledger: &'a LedgerState,
    nav_asset: &NavTrackedAsset,
) -> Result<&'a NavProofProfile, (&'static str, String)> {
    let profile = nav_profile_for_asset(ledger, nav_asset).ok_or_else(|| {
        (
            "missing_vault_bridge_profile",
            "vault bridge asset requires a registered NAV proof profile".to_string(),
        )
    })?;
    if !profile
        .source_class
        .starts_with(VAULT_BRIDGE_PROFILE_SOURCE_CLASS_PREFIX)
    {
        return Err((
            "nav_profile_not_vault_bridge",
            "vault bridge asset operation requires a vault_bridge:<source_domain> NAV proof profile"
                .to_string(),
        ));
    }
    let source_domain = profile
        .source_class
        .strip_prefix(VAULT_BRIDGE_PROFILE_SOURCE_CLASS_PREFIX)
        .unwrap_or_default();
    if source_domain.is_empty() {
        return Err((
            "bad_vault_bridge_profile",
            "vault bridge asset profile source_class must include a nonempty source domain"
                .to_string(),
        ));
    }
    if vault_bridge_route_policy_hash(profile).is_empty() {
        return Err((
            "bad_vault_bridge_profile",
            "vault bridge asset profile route policy hash must be nonempty".to_string(),
        ));
    }
    Ok(profile)
}

fn vault_bridge_route_policy_hash(profile: &NavProofProfile) -> &str {
    if profile.vault_bridge_route_policy_hash.is_empty() {
        &profile.valuation_policy_hash
    } else {
        &profile.vault_bridge_route_policy_hash
    }
}

/// Resolve an immutable historical vault-bridge policy pinned by an existing
/// deposit, receipt, bucket, or redemption. New ingress still resolves through
/// `vault_bridge_profile_for_asset`; this resolver exists only so an in-flight
/// lifecycle can finish safely after the asset's active route rotates.
fn vault_bridge_profile_for_pinned_policy<'a>(
    ledger: &'a LedgerState,
    nav_asset: &NavTrackedAsset,
    source_domain: &str,
    policy_hash: &str,
) -> Result<&'a NavProofProfile, (&'static str, String)> {
    ensure_vault_bridge_asset_registration(ledger, nav_asset)?;
    let expected_source_class =
        format!("{VAULT_BRIDGE_PROFILE_SOURCE_CLASS_PREFIX}{source_domain}");
    let mut matches = ledger.nav_proof_profiles.iter().filter(|profile| {
        profile.source_class == expected_source_class
            && vault_bridge_route_policy_hash(profile) == policy_hash
            && (profile.registered_by == nav_asset.issuer
                || profile.registered_by == nav_asset.reserve_operator)
    });
    let profile = matches.next().ok_or_else(|| {
        (
            "missing_vault_bridge_pinned_profile",
            "vault bridge lifecycle references a source/policy profile that is not registered"
                .to_string(),
        )
    })?;
    if matches.next().is_some() {
        return Err((
            "ambiguous_vault_bridge_pinned_profile",
            "vault bridge lifecycle source/policy profile resolves ambiguously".to_string(),
        ));
    }
    ensure_vault_bridge_source_policy(profile, source_domain, policy_hash)?;
    Ok(profile)
}

fn ensure_vault_bridge_source_policy(
    profile: &NavProofProfile,
    source_domain: &str,
    policy_hash: &str,
) -> Result<(), (&'static str, String)> {
    let Some(profile_source_domain) = profile
        .source_class
        .strip_prefix(VAULT_BRIDGE_PROFILE_SOURCE_CLASS_PREFIX)
    else {
        return Err((
            "nav_profile_not_vault_bridge",
            "vault bridge asset operation requires a vault_bridge:<source_domain> profile"
                .to_string(),
        ));
    };
    if profile_source_domain != source_domain {
        return Err((
            "vault_bridge_source_domain_mismatch",
            "vault bridge asset source_domain must match the NAV proof profile source_class"
                .to_string(),
        ));
    }
    if vault_bridge_route_policy_hash(profile) != policy_hash {
        return Err((
            "vault_bridge_policy_hash_mismatch",
            "vault bridge asset policy_hash must match the NAV proof profile route policy hash"
                .to_string(),
        ));
    }
    Ok(())
}
