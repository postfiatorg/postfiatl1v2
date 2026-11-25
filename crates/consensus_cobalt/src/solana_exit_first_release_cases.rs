// Governance cases shared with the first release's code (origin/main a720cc80f): the same
// file is compiled into both trees, and its outputs on main are pinned in the tests.
mod first_release_cases {
    use super::*;

    fn wan() -> CobaltDomain {
        CobaltDomain {
            chain_id: "postfiat-wan-devnet-2".into(),
            genesis_hash: "ce22ca8c932da0998b484483a09647138a30e0bf44408dd49a8d6d452787ad25521aff3ed334da07e150a7233a3e90a9".into(),
            protocol_version: 1,
        }
    }

    /// A WAN decision at height 1900 signed by `signers` of six; `kind` replaces the signed
    /// kind afterwards where the first release could not ratify it.
    fn decision(signed_kind: &str, kind: &str, signers: usize) -> GovernanceAmendment {
        let validators: Vec<String> = (0..6).map(|i| format!("validator-{i}")).collect();
        let config = EssentialSubsetConfig { validators: validators.clone(), quorum: signers };
        let lifecycle = GovernanceAmendmentLifecycle {
            activation_height: 1900,
            ..GovernanceAmendmentLifecycle::immediate()
        };
        let mut amendment = ratify_governance_amendment_with_lifecycle(
            &wan(),
            &config,
            signed_kind,
            1,
            validators[..signers].to_vec(),
            lifecycle,
        )
        .unwrap();
        amendment.kind = kind.into();
        amendment
    }

    /// (case, verdict) of the consensus domain check for each case.
    pub(super) fn outcomes() -> Vec<(&'static str, String)> {
        use postfiat_types::solana_exit::{CLOSE, OPEN, PAID, RETIRE};
        let paid = format!("{PAID}{}:12345:0:{}", "11".repeat(48), "ab".repeat(64));
        let minimum = "solana_exit_min_withdrawal:300000";
        let cases = [
            ("close_5_of_6", decision(CLOSE, CLOSE, 5)),
            ("close_6_of_6", decision(CLOSE, CLOSE, 6)),
            ("paid_5_of_6", decision(&paid, &paid, 5)),
            ("open_5_of_6", decision(OPEN, OPEN, 5)),
            ("retire_5_of_6", decision(RETIRE, RETIRE, 5)),
            ("min_withdrawal_6_of_6", decision(CLOSE, minimum, 6)),
            ("min_withdrawal_5_of_6", decision(CLOSE, minimum, 5)),
        ];
        cases
            .into_iter()
            .map(|(name, amendment)| {
                let verdict = match verify_governance_amendment(&wan(), &amendment) {
                    Ok(()) => "accepted".to_string(),
                    Err(error) => error,
                };
                (name, verdict)
            })
            .collect()
    }
}
