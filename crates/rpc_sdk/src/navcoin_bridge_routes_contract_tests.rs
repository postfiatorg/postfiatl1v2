    /// Contract test: the SDK must accept what the real route-status producer
    /// serializes, so a producer/consumer schema mismatch fails here.
    fn navcoin_bridge_routes_producer_config() -> postfiat_bridge::PftlUniswapRouteConfig {
        postfiat_bridge::PftlUniswapRouteConfig {
            schema: "postfiat-pftl-uniswap-route-config-v1".to_string(),
            route_id: "pftl-a666-ethereum-wA666-usdc-v1".to_string(),
            route_family: postfiat_bridge::PFTL_UNISWAP_ROUTE_FAMILY_PRIMARY_MINT.to_string(),
            native_nav_asset_id: "a".repeat(96),
            settlement_asset_id: "8".repeat(96),
            wrapped_navcoin_token: "0x1111111111111111111111111111111111111111".to_string(),
            handoff_controller: "0x2222222222222222222222222222222222222222".to_string(),
            settlement_adapter: "0x3333333333333333333333333333333333333333".to_string(),
            verifier_mode: "threshold-controlled".to_string(),
            route_trust_class: postfiat_bridge::ROUTE_TRUST_CLASS_CONTROLLED.to_string(),
            uniswap_pool_id_or_path:
                "0x4444444444444444444444444444444444444444444444444444444444444444".to_string(),
            router: "0x5555555555555555555555555555555555555555".to_string(),
            failure_behavior: "refund_unconsumed_pftl_packet".to_string(),
            route_supply_cap_atoms: 10_000_000,
            packet_notional_cap_atoms: 1_000_000,
            seed_nav_epoch: 7,
            seed_usdc_atoms: 100_000_000,
            seed_wrapped_navcoin_atoms: 100_000,
            lp_recipient: "0x6666666666666666666666666666666666666666".to_string(),
            lp_custody_policy: "controlled_launch_lp".to_string(),
        }
    }

    #[test]
    fn navcoin_bridge_routes_sdk_accepts_real_bridge_producer_output() {
        let ledger = postfiat_bridge::pftl_uniswap_bridge_ledger_from_config(
            &navcoin_bridge_routes_producer_config(),
            1,
            7,
            64,
        )
        .expect("bridge ledger");
        for ledgers in [Vec::new(), vec![ledger]] {
            let report = postfiat_bridge::pftl_uniswap_bridge_routes_status(&ledgers)
                .expect("routes status");
            let response = success_response("navcoin-bridge-routes-contract", &report, vec![])
                .expect("routes response");
            validate_response_kind(&response, RpcResponseKind::NavcoinBridgeRoutes)
                .unwrap_or_else(|error| {
                    panic!("SDK rejects real producer output ({} rows): {error:?}", ledgers.len())
                });
        }
    }

    #[test]
    fn navcoin_bridge_routes_row_checks_run_on_v1_and_v2() {
        let ledger = postfiat_bridge::pftl_uniswap_bridge_ledger_from_config(
            &navcoin_bridge_routes_producer_config(),
            1,
            7,
            64,
        )
        .expect("bridge ledger");
        let report =
            postfiat_bridge::pftl_uniswap_bridge_routes_status(&[ledger]).expect("routes status");
        let real = serde_json::to_value(report).expect("report json");
        let validate = |result: &serde_json::Value| {
            let response = success_response("navcoin-bridge-routes-schema", result, vec![])
                .expect("routes response");
            validate_response_kind(&response, RpcResponseKind::NavcoinBridgeRoutes)
        };
        for schema in [
            "postfiat-pftl-uniswap-routes-status-v1",
            "postfiat-pftl-uniswap-routes-status-v2",
        ] {
            let mut result = real.clone();
            result["schema"] = json!(schema);
            validate(&result).unwrap_or_else(|error| panic!("{schema}: {error:?}"));
            // An old row field and a new authority field are both still checked.
            for (field, bad) in [
                ("ledger_hash", json!("not-hex")),
                ("authority_committee_current", json!("false")),
            ] {
                let mut bad_result = result.clone();
                bad_result["routes"][0][field] = bad;
                assert!(validate(&bad_result).is_err(), "{schema}: bad {field} accepted");
            }
        }
        let mut unknown = real;
        unknown["schema"] = json!("postfiat-pftl-uniswap-routes-status-v3");
        assert!(validate(&unknown).is_err(), "unknown schema accepted");
    }
