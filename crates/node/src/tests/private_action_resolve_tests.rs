    // Operator rule for a stuck private action record (`private-action-resolve`).

    /// A reserved record left `unknown` by a crash between dispatch and the
    /// certified send.
    fn stuck_private_action(
        fixture: &PrivateActionFixture,
        amount: u64,
        tag: u8,
    ) -> (PrivateActionSubmitRequest, PrivateActionRecord) {
        let request = private_submit_request(fixture, &private_ingress_action_json(fixture, amount, tag));
        private_action_submit(&fixture.data_dir, &request).expect("reserve");
        let mut store = PrivateActionAdmissionStore::open(&fixture.data_dir).unwrap();
        let mut record = store.get(&request.intent_id).unwrap().clone();
        record.state = PrivateActionState::Unknown;
        record.dispatch_count = 1;
        store.write(record.clone()).unwrap();
        (request, record)
    }

    #[test]
    fn private_action_resolve_settles_from_the_chain_or_changes_nothing() {
        let fixture = private_action_fixture("postfiat-private-action-resolve-settle");
        let (landed, landed_record) = stuck_private_action(&fixture, 3, 71);
        let (lost, _) = stuck_private_action(&fixture, 4, 72);
        apply_reserved_batch(&fixture.data_dir, &landed_record);

        let settled = private_action_resolve(&fixture.data_dir, &landed.intent_id, None, 1_000)
            .expect("settle");
        assert_eq!(settled.action, "settled");
        assert_eq!(settled.record.state, PrivateActionState::Accepted);
        assert!(settled.record.outcome.as_ref().unwrap().inclusion.is_some());

        // Absent from the chain: the record is printed and nothing changes.
        let before = PrivateActionAdmissionStore::open(&fixture.data_dir)
            .unwrap()
            .history(&lost.intent_id)
            .unwrap();
        let unchanged = private_action_resolve(&fixture.data_dir, &lost.intent_id, None, 1_000)
            .expect("no-op");
        assert_eq!(unchanged.action, "unchanged");
        assert_eq!(unchanged.record.state, PrivateActionState::Unknown);
        assert_eq!(unchanged.history, before);

        assert_refused(
            private_action_resolve(&fixture.data_dir, &"ab".repeat(48), None, 1_000).unwrap_err(),
            "private_action_unknown_intent",
        );
        fs::remove_dir_all(fixture.data_dir).ok();
    }

    #[test]
    fn private_action_abandon_is_refused_on_chain_in_flight_or_final() {
        let fixture = private_action_fixture("postfiat-private-action-resolve-refuse");
        let (landed, landed_record) = stuck_private_action(&fixture, 3, 73);
        apply_reserved_batch(&fixture.data_dir, &landed_record);
        assert_refused(
            private_action_resolve(&fixture.data_dir, &landed.intent_id, Some("lost"), 1_000)
                .unwrap_err(),
            "private_action_on_chain",
        );
        let store = PrivateActionAdmissionStore::open(&fixture.data_dir).unwrap();
        assert_eq!(
            store.get(&landed.intent_id).unwrap().state,
            PrivateActionState::Unknown
        );
        drop(store);

        let (dispatched, mut record) = stuck_private_action(&fixture, 4, 74);
        record.state = PrivateActionState::Dispatched;
        PrivateActionAdmissionStore::open(&fixture.data_dir)
            .unwrap()
            .write(record)
            .unwrap();
        assert_refused(
            private_action_resolve(&fixture.data_dir, &dispatched.intent_id, Some("lost"), 1_000)
                .unwrap_err(),
            "private_action_round_in_flight",
        );
        let (blank, _) = stuck_private_action(&fixture, 5, 75);
        assert_refused(
            private_action_resolve(&fixture.data_dir, &blank.intent_id, Some("  "), 1_000)
                .unwrap_err(),
            "private_action_request_invalid",
        );
        private_action_resolve(&fixture.data_dir, &blank.intent_id, Some("lost"), 1_000)
            .expect("abandon");
        assert_refused(
            private_action_resolve(&fixture.data_dir, &blank.intent_id, Some("again"), 1_001)
                .unwrap_err(),
            "private_action_final",
        );
        fs::remove_dir_all(fixture.data_dir).ok();
    }

    #[test]
    fn private_action_abandon_then_resubmit_keeps_the_audit_trail() {
        let fixture = private_action_fixture("postfiat-private-action-resolve-resubmit");
        let (request, record) = stuck_private_action(&fixture, 6, 76);
        let abandoned = private_action_resolve(
            &fixture.data_dir,
            &request.intent_id,
            Some("lost after a crash before the certified send"),
            1_700_000_000,
        )
        .expect("abandon");
        assert_eq!(abandoned.action, "abandoned");
        assert_eq!(abandoned.record.state, PrivateActionState::Abandoned);
        let outcome = abandoned.record.outcome.as_ref().unwrap();
        assert_eq!(outcome.code, "private_action_abandoned");
        assert_eq!(outcome.state_effects, "none");
        assert!(outcome.message.contains("1700000000"), "{}", outcome.message);
        assert!(outcome.message.contains("lost after a crash"), "{}", outcome.message);

        // The round service never dispatches an abandoned record.
        let steps = private_action_service_step(&fixture.data_dir, |_| Ok(true), |_, _| {
            panic!("an abandoned record is never dispatched")
        })
        .expect("idle step");
        assert!(steps.is_empty());
        let receipt = private_action_receipt(&fixture.data_dir, &request.intent_id).unwrap();
        assert_eq!(receipt.state, "abandoned");
        assert_eq!(
            receipt.contract_receipt.unwrap().state,
            postfiat_types::PrivateAdmissionState::Unknown
        );

        // The wallet resubmits the same bytes: a fresh `queued` record supersedes
        // the abandoned one under the same intent, action hash and batch id.
        let resubmitted = private_action_submit(&fixture.data_dir, &request).expect("resubmit");
        assert!(!resubmitted.existing);
        assert_eq!(resubmitted.record.state, PrivateActionState::Queued);
        assert_eq!(resubmitted.record.dispatch_count, 0);
        assert_eq!(resubmitted.record.outcome, None);
        assert_eq!(resubmitted.record.batch_id, record.batch_id);
        assert_eq!(resubmitted.record.batch_json, record.batch_json);
        let again = private_action_submit(&fixture.data_dir, &request).expect("duplicate");
        assert!(again.existing);
        assert_eq!(again.record, resubmitted.record);

        let store = PrivateActionAdmissionStore::open(&fixture.data_dir).unwrap();
        assert_eq!(
            store.intent_for_action_hash(&record.action_hash),
            Some(request.intent_id.as_str())
        );
        assert_eq!(
            store.intent_for_batch_id(&record.batch_id),
            Some(request.intent_id.as_str())
        );
        let states = store
            .history(&request.intent_id)
            .unwrap()
            .into_iter()
            .map(|record| record.state)
            .collect::<Vec<_>>();
        assert_eq!(
            states,
            vec![
                PrivateActionState::Queued,
                PrivateActionState::Unknown,
                PrivateActionState::Abandoned,
                PrivateActionState::Queued,
            ]
        );
        drop(store);

        // The superseding record follows the chain like any other.
        let current = PrivateActionAdmissionStore::open(&fixture.data_dir)
            .unwrap()
            .get(&request.intent_id)
            .unwrap()
            .clone();
        apply_reserved_batch(&fixture.data_dir, &current);
        let settled = private_action_resolve(&fixture.data_dir, &request.intent_id, None, 1_700_000_100)
            .expect("settle superseding record");
        assert_eq!(settled.record.state, PrivateActionState::Accepted);
        assert_eq!(settled.history.len(), 5);
        fs::remove_dir_all(fixture.data_dir).ok();
    }
