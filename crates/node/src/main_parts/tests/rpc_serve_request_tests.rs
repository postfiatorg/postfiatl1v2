#[cfg(test)]
mod rpc_serve_request_tests {
    use super::*;
    use postfiat_node::read_consensus_v2_safety_state;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn send_loopback_rpc(port: u16, request: &RpcRequest) -> RpcResponse {
        let mut stream = (0..100)
            .find_map(|_| match TcpStream::connect(("127.0.0.1", port)) {
                Ok(stream) => Some(stream),
                Err(_) => {
                    std::thread::sleep(Duration::from_millis(10));
                    None
                }
            })
            .expect("connect to loopback RPC server");
        stream
            .set_read_timeout(Some(Duration::from_secs(10)))
            .expect("set RPC read timeout");
        let mut request_line = serde_json::to_vec(request).expect("serialize RPC request");
        request_line.push(b'\n');
        stream.write_all(&request_line).expect("write RPC request");
        let mut response_line = String::new();
        BufReader::new(stream)
            .read_line(&mut response_line)
            .expect("read RPC response");
        serde_json::from_str(&response_line).expect("parse RPC response")
    }

    #[test]
    fn consensus_v2_timeout_vote_rpc_is_finality_gated_and_durably_signed() {
        let root = env::temp_dir().join(format!(
            "postfiat-timeout-vote-loopback-rpc-{}-{}",
            process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        init_consensus_v2(InitConsensusV2Options {
            data_dir: root.clone(),
            chain_id: "timeout-vote-rpc-test".to_string(),
            node_id: "validator-0".to_string(),
            validator_count: 4,
            activation_height: 1,
        })
        .expect("initialize timeout vote RPC fixture");
        let rpc_port = TcpListener::bind(("127.0.0.1", 0))
            .expect("reserve timeout RPC port")
            .local_addr()
            .expect("timeout RPC address")
            .port();
        let topology_file = root.join("topology.json");
        write_consensus_v2_topology(TopologyConsensusV2Options {
            chain_id: "timeout-vote-rpc-test".to_string(),
            validators: 4,
            base_port: rpc_port.saturating_add(20),
            rpc_base_port: Some(rpc_port),
            hosts: Some(vec!["127.0.0.1".to_string(); 4]),
            output_file: topology_file.clone(),
            activation_height: 1,
        })
        .expect("write timeout vote RPC topology");
        let local = status(NodeOptions {
            data_dir: root.clone(),
        })
        .expect("timeout vote parent status");
        let ready_file = root.join("readiness/timeout-vote-rpc.json");
        let server_root = root.clone();
        let server_topology = topology_file.clone();
        let server = std::thread::spawn(move || {
            rpc_serve(RpcServeOptions {
                data_dir: server_root.clone(),
                spool_dir: server_root.join("runtime/rpc-spool"),
                ready_file: server_root.join("readiness/timeout-vote-rpc.json"),
                bind_host: "127.0.0.1".to_string(),
                port: rpc_port,
                max_requests: 1,
                timeout_ms: 10_000,
                child_timeout_ms: 10_000,
                event_log: None,
                allow_mempool_submit: false,
                allow_mempool_submit_finality: true,
                allow_orchard_batch_create: false,
                owned_lane_enabled: true,
                finality_topology_file: server_topology,
                finality_key_file: server_root.join(VALIDATOR_KEYS_FILE),
                finality_proposal_key_file: Some(server_root.join(VALIDATOR_KEYS_FILE)),
                finality_artifact_root: server_root.join("finality-artifacts"),
                finality_timeout_ms: 10_000,
                finality_send_retries: 0,
                finality_retry_backoff_ms: 0,
                finality_quorum_early_full_propagation: false,
                max_mempool_submit_per_peer: 8,
                max_mempool_submit_total: 32,
                max_orchard_batch_create_per_peer: 2,
                max_orchard_batch_create_total: 8,
                max_orchard_batch_create_concurrent: 1,
                keep_alive: false,
            })
            .expect("serve timeout vote loopback RPC")
        });
        for _ in 0..100 {
            if ready_file.is_file() {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(ready_file.is_file(), "timeout vote RPC did not become ready");

        let response = send_loopback_rpc(
            rpc_port,
            &RpcRequest::empty("timeout-vote", RPC_FINALITY_TIMEOUT_VOTE_METHOD)
                .with_param("block_height", 1u64)
                .expect("timeout height")
                .with_param("view", 0u64)
                .expect("timeout view")
                .with_param("proxy_required_current_height", 0u64)
                .expect("timeout parent height")
                .with_param("proxy_required_parent_hash", &local.block_tip_hash)
                .expect("timeout parent hash")
                .with_param("proxy_required_state_root", &local.state_root)
                .expect("timeout parent root"),
        );
        assert!(response.ok, "{:?}", response.error);
        let vote = response
            .result_as::<BlockTimeoutVoteFile>()
            .expect("decode timeout vote");
        assert_eq!(vote.block_height, 1);
        assert_eq!(vote.view, 0);
        assert_eq!(vote.vote.validator, "validator-0");
        assert_eq!(
            vote.consensus_v2_vote
                .as_ref()
                .expect("activated v2 timeout vote")
                .validator,
            "validator-0"
        );
        server
            .join()
            .expect("join timeout vote RPC");
        let (domain, _) = live_consensus_v2_context(&root).expect("timeout domain");
        let safety = read_consensus_v2_safety_state(&root, &domain, 1)
            .expect("read timeout safety state");
        assert_eq!(
            safety
                .highest_timeout_round
                .expect("durable timeout high-water mark")
                .view,
            0
        );
        fs::remove_dir_all(root).expect("cleanup timeout vote RPC fixture");
    }

    #[test]
    fn fastpay_v3_loopback_rpc_exposes_live_capabilities_and_bounded_reads() {
        let root = env::temp_dir().join(format!(
            "postfiat-fastpay-v3-loopback-rpc-{}",
            process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        postfiat_node::init(postfiat_node::InitOptions {
            data_dir: root.clone(),
            chain_id: "fastpay-v3-rpc-test".to_string(),
            node_id: "validator-0".to_string(),
            validator_count: 4,
        })
        .expect("initialize FastPay v3 RPC fixture");
        let store = postfiat_storage::NodeStore::new(&root);
        let genesis = store.read_genesis().expect("read RPC fixture genesis");
        store
            .write_chain_tip(&postfiat_types::ChainTipState {
                schema: "postfiat-chain-tip-v1".to_string(),
                chain_id: genesis.chain_id.clone(),
                genesis_hash: postfiat_execution::genesis_hash(&genesis),
                protocol_version: genesis.protocol_version,
                height: 5,
                block_hash: "aa".repeat(48),
                state_root: "bb".repeat(48),
                ordered_batch_count: 0,
                receipt_count: 0,
                history_base_height: 0,
            })
            .expect("write RPC fixture tip");
        let owner = postfiat_crypto_provider::ml_dsa_65_keygen().expect("generate owner key");
        let owner_pubkey_hex = postfiat_crypto_provider::bytes_to_hex(&owner.public_key);
        let mut ledger = store.read_ledger().expect("read RPC fixture ledger");
        ledger.fastpay_recovery_policy = Some(postfiat_types::FastPayRecoveryPolicyV1 {
            schema: postfiat_types::FASTPAY_RECOVERY_POLICY_SCHEMA_V1.to_string(),
            activation_height: 1,
            max_validity_blocks: 20,
            max_recovery_blocks: 20,
        });
        let validator_registry: postfiat_node::ValidatorRegistry = serde_json::from_slice(
            &fs::read(root.join("validator_registry.json")).expect("read validator registry"),
        )
        .expect("decode validator registry");
        ledger.fastpay_recovery_committees.push(
            postfiat_types::FastPayRecoveryCommitteeV1::from_public_keys(
                genesis.chain_id.clone(),
                postfiat_execution::genesis_hash(&genesis),
                genesis.protocol_version,
                1,
                1,
                20,
                validator_registry
                    .validators
                    .iter()
                    .map(|record| (record.node_id.clone(), record.public_key_hex.clone()))
                    .collect(),
            )
            .expect("build RPC recovery committee"),
        );
        ledger.owned_objects.push(postfiat_types::OwnedObject {
            id: "fastpay-v3-rpc-input".to_string(),
            version: 7,
            owner_pubkey_hex: owner_pubkey_hex.clone(),
            value: 100,
            asset: "PFT".to_string(),
        });
        store.write_ledger(&ledger).expect("activate v3 RPC policy");

        let mut order = postfiat_types::OwnedTransferOrderV3 {
            domain: postfiat_node::owned_certificate_domain_v3(&root)
                .expect("read v3 certificate domain"),
            recovery: postfiat_types::FastPayOrderRecoveryV1 {
                schema: postfiat_types::FASTPAY_ORDER_RECOVERY_SCHEMA_V1.to_string(),
                committee_epoch: 1,
                lock_id: "00".repeat(48),
                valid_from_height: 5,
                expires_at_height: 10,
                recovery_closes_at_height: 15,
            },
            inputs: vec![postfiat_types::OwnedObjectRef {
                id: "fastpay-v3-rpc-input".to_string(),
                version: 7,
            }],
            outputs: vec![postfiat_types::OwnedOutputSpec {
                owner_pubkey_hex: "fastpay-v3-rpc-recipient".to_string(),
                value: 99,
                asset: "PFT".to_string(),
            }],
            fee: 1,
            nonce: 1,
            memos: Vec::new(),
        };
        order.recovery.lock_id = postfiat_types::fastpay_transfer_lock_id_v1(&order);
        let owner_signature = postfiat_crypto_provider::ml_dsa_65_sign_with_context(
            &owner.private_key,
            &postfiat_execution::owned_transfer_v3_signing_bytes(&order),
            postfiat_execution::OWNED_TRANSFER_CONTEXT_V3,
        )
        .expect("sign v3 RPC order");
        let signed_order = postfiat_types::SignedOwnedTransferOrderV3 {
            order: order.clone(),
            owner_pubkey_hex: owner_pubkey_hex.clone(),
            owner_signature_hex: postfiat_crypto_provider::bytes_to_hex(&owner_signature),
        };
        let signed_order_json = serde_json::to_string(&signed_order).expect("encode signed order");

        let port = TcpListener::bind(("127.0.0.1", 0))
            .expect("reserve loopback port")
            .local_addr()
            .expect("read loopback port")
            .port();
        let ready_file = root.join("readiness/fastpay-v3-rpc.json");
        let server_root = root.clone();
        let server = std::thread::spawn(move || {
            rpc_serve(RpcServeOptions {
                data_dir: server_root.clone(),
                spool_dir: server_root.join("runtime/rpc-spool"),
                ready_file: server_root.join("readiness/fastpay-v3-rpc.json"),
                bind_host: "127.0.0.1".to_string(),
                port,
                max_requests: 8,
                timeout_ms: 10_000,
                child_timeout_ms: 10_000,
                event_log: None,
                allow_mempool_submit: false,
                allow_mempool_submit_finality: false,
                allow_orchard_batch_create: false,
                owned_lane_enabled: true,
                finality_topology_file: server_root.join("unused-topology.json"),
                finality_key_file: server_root.join("validator_keys.json"),
                finality_proposal_key_file: None,
                finality_artifact_root: server_root.join("unused-finality-artifacts"),
                finality_timeout_ms: 10_000,
                finality_send_retries: 0,
                finality_retry_backoff_ms: 0,
                finality_quorum_early_full_propagation: false,
                max_mempool_submit_per_peer: 8,
                max_mempool_submit_total: 32,
                max_orchard_batch_create_per_peer: 2,
                max_orchard_batch_create_total: 8,
                max_orchard_batch_create_concurrent: 1,
                keep_alive: false,
            })
            .expect("serve FastPay v3 loopback RPC")
        });
        for _ in 0..100 {
            if ready_file.is_file() {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(ready_file.is_file(), "FastPay v3 RPC did not become ready");

        let capabilities = send_loopback_rpc(
            port,
            &RpcRequest::empty("v3-capabilities", "owned_recovery_capabilities"),
        );
        let capabilities = capabilities
            .result_as::<postfiat_types::FastPayRecoveryCapabilitiesV1>()
            .expect("decode FastPay v3 capabilities");
        capabilities.validate().expect("valid v3 capabilities");
        assert_eq!(capabilities.validator_count, 4);
        assert_eq!(capabilities.quorum, 3);
        assert_eq!(capabilities.policy.max_validity_blocks, 20);

        let votes = (0..3)
            .map(|validator_index| {
                let validator_id = format!("validator-{validator_index}");
                send_loopback_rpc(
                    port,
                    &RpcRequest::empty(format!("v3-sign-{validator_index}"), "owned_sign_v3")
                        .with_param("validator_id", &validator_id)
                        .and_then(|request| {
                            request.with_param("order_json", &signed_order_json)
                        })
                        .expect("signed vote request"),
                )
                .result_as::<postfiat_types::OwnedTransferVote>()
                .expect("decode validator vote")
            })
            .collect::<Vec<_>>();
        let certificate = postfiat_types::OwnedTransferCertificateV3 {
            order,
            owner_pubkey_hex,
            owner_signature_hex: signed_order.owner_signature_hex,
            votes,
        };
        let certificate_json = serde_json::to_string(&certificate).expect("encode certificate");
        let apply_ack = send_loopback_rpc(
            port,
            &RpcRequest::empty("v3-apply", "owned_apply_v3")
                .with_param("validator_id", "validator-0")
                .and_then(|request| request.with_param("cert_json", &certificate_json))
                .expect("apply request"),
        )
        .result_as::<postfiat_types::FastPayApplyAckV1>()
        .expect("decode durable apply acknowledgement");
        apply_ack.validate_shape().expect("valid durable apply ack");
        assert_eq!(apply_ack.lock_id, certificate.order.recovery.lock_id);
        let validator_public_key = validator_registry
            .validators
            .iter()
            .find(|record| record.node_id == "validator-0")
            .expect("validator-0 registry record")
            .public_key_hex
            .as_str();
        assert!(postfiat_execution::verify_fastpay_apply_ack_v1(
            &apply_ack,
            validator_public_key
        ));

        let applied = store.read_ledger().expect("read remotely applied ledger");
        assert!(applied
            .owned_objects
            .iter()
            .all(|object| object.id != "fastpay-v3-rpc-input"));
        assert_eq!(
            applied
                .owned_objects
                .iter()
                .find(|object| object.owner_pubkey_hex == "fastpay-v3-rpc-recipient")
                .map(|object| object.value),
            Some(99)
        );
        assert_eq!(
            applied
                .owned_objects
                .iter()
                .filter(|object| object.asset == "PFT")
                .map(|object| object.value)
                .sum::<u64>()
                + certificate.order.fee,
            100
        );

        let lock_id = certificate.order.recovery.lock_id.clone();
        let status = send_loopback_rpc(
            port,
            &RpcRequest::empty("v3-status", "owned_recovery_status")
                .with_param("lock_id", &lock_id)
                .expect("status parameter"),
        );
        assert!(status.ok);
        assert_eq!(
            status.result.expect("status result")["status"],
            serde_json::json!("confirmed")
        );

        let recovered_certificate = send_loopback_rpc(
            port,
            &RpcRequest::empty("v3-certificate", "owned_certificate")
                .with_param("certificate_digest", &apply_ack.certificate_digest)
                .expect("certificate parameter"),
        )
        .result_as::<postfiat_types::FastPayCertificateV1>()
        .expect("recover certificate by digest");
        assert_eq!(
            recovered_certificate,
            postfiat_types::FastPayCertificateV1::Transfer(certificate)
        );

        let malformed_sign = send_loopback_rpc(
            port,
            &RpcRequest::empty("v3-sign", "owned_sign_v3")
                .with_param("validator_id", "validator-0")
                .and_then(|request| request.with_param("order_json", "{}"))
                .expect("malformed sign request"),
        );
        assert!(!malformed_sign.ok);
        assert_eq!(
            malformed_sign.error.expect("malformed-sign error").code,
            "owned_sign_v3_failed"
        );

        server.join().expect("join FastPay v3 RPC server");
        fs::remove_dir_all(root).expect("remove FastPay v3 RPC fixture");
    }

    #[test]
    fn tx_not_found_is_a_typed_protocol_error_and_other_tx_errors_remain_generic() {
        let tx_id = "ab".repeat(48);
        let root = env::temp_dir().join(format!(
            "postfiat-rpc-typed-tx-absence-{}",
            process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        postfiat_node::init(postfiat_node::InitOptions {
            data_dir: root.clone(),
            chain_id: "postfiat-local".to_string(),
            node_id: "validator-0".to_string(),
            validator_count: 1,
        })
        .expect("initialize typed tx-absence fixture");
        let absence = postfiat_node::tx_finality(postfiat_node::TxFinalityQueryOptions {
            data_dir: root.clone(),
            tx_id: tx_id.clone(),
            audit_block_log: false,
        })
        .expect_err("unknown valid tx must be absent");
        assert!(postfiat_node::tx_finality_error_is_transaction_not_found(
            &absence
        ));
        let encoded = rpc_dispatch::rpc_tx_finality_error(absence);
        let (code, message) = rpc_dispatch::rpc_dispatch_error_response_parts(&encoded);
        let response = error_response(
            "missing-tx-query",
            code,
            message,
            vec![RpcEvent::new("error", code, "rpc request failed")],
        );

        response
            .validate_protocol()
            .expect("typed tx-not-found response must be protocol-valid");
        assert!(!response.ok);
        assert!(response.result.is_none());
        assert_eq!(
            response.error.as_ref().map(|error| error.code.as_str()),
            Some("rpc_tx_not_found")
        );
        let expected_message = format!("rpc tx failed: transaction `{tx_id}` has no receipt");
        assert_eq!(
            response.error.as_ref().map(|error| error.message.as_str()),
            Some(expected_message.as_str())
        );

        let missing_state = root.with_extension("missing-state");
        let _ = fs::remove_dir_all(&missing_state);
        let local_fault = postfiat_node::tx_finality(postfiat_node::TxFinalityQueryOptions {
            data_dir: missing_state,
            tx_id,
            audit_block_log: false,
        })
        .expect_err("missing node state must fail");
        assert_eq!(local_fault.kind(), std::io::ErrorKind::NotFound);
        assert!(!postfiat_node::tx_finality_error_is_transaction_not_found(
            &local_fault
        ));
        let local_fault = rpc_dispatch::rpc_tx_finality_error(local_fault);
        let (local_fault_code, _) =
            rpc_dispatch::rpc_dispatch_error_response_parts(&local_fault);
        assert_eq!(local_fault_code, "rpc_error");

        let generic = rpc_dispatch::rpc_tx_finality_error(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "corrupt finality index",
        ));
        let (generic_code, generic_message) =
            rpc_dispatch::rpc_dispatch_error_response_parts(&generic);
        assert_eq!(generic_code, "rpc_error");
        assert_eq!(generic_message, "rpc tx failed: corrupt finality index");
        fs::remove_dir_all(root).expect("remove typed tx-absence fixture");
    }

    #[test]
    fn atomic_swap_quote_rpc_preserves_typed_market_binding_codes() {
        for code in [
            "wrong_market_envelope",
            "wrong_nav_epoch",
            "nav_pair_not_supported",
        ] {
            let error = std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                postfiat_node::AtomicSwapFeeQuoteErrorDetail::new(code, "typed rejection"),
            );
            let (observed_code, message) = atomic_swap_fee_quote_rpc_error(error);
            assert_eq!(observed_code, code);
            assert!(message.contains("typed rejection"), "{message}");
        }

        let (generic_code, _) = atomic_swap_fee_quote_rpc_error(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "untyped failure",
        ));
        assert_eq!(generic_code, "rpc_atomic_swap_fee_quote_failed");
    }

    #[test]
    fn rpc_serve_rejects_oversized_request_lines_before_parse() {
        let at_limit = format!("{}\n", "x".repeat(MAX_RPC_REQUEST_BYTES));
        validate_rpc_serve_request_line(&at_limit).expect("request at limit");

        let oversized = format!("{}\n", "x".repeat(MAX_RPC_REQUEST_BYTES + 1));
        let error =
            validate_rpc_serve_request_line(&oversized).expect_err("oversized request line");
        assert!(error.contains("rpc request exceeded"), "{error}");
    }

    #[test]
    fn rpc_serve_internal_failures_do_not_expose_operator_paths() {
        let sensitive =
            "ledger read `/home/operator/private-validator/ledger.json` failed: permission denied";
        for code in [
            "rpc_internal",
            "rpc_server_error",
            "rpc_status_failed",
            "rpc_mempool_status_failed",
            "fastswap_unavailable",
        ] {
            let response = rpc_serve_error_response("path-leak", code, sensitive);
            let message = response.error.expect("error response").message;
            assert!(!message.contains("/home/operator"), "{code}: {message}");
            assert!(!message.contains("ledger.json"), "{code}: {message}");
        }

        let child = rpc_serve_child_error_response("child", sensitive);
        let child_message = child.error.expect("child error response").message;
        assert!(!child_message.contains("/home/operator"), "{child_message}");
        assert!(!child_message.contains("ledger.json"), "{child_message}");

        let typed = rpc_serve_error_response(
            "typed",
            "wrong_nav_epoch",
            "quote epoch 58 does not match active epoch 59",
        );
        assert_eq!(
            typed.error.expect("typed error response").message,
            "quote epoch 58 does not match active epoch 59"
        );
    }

    #[test]
    fn rpc_serve_rejects_public_bind_even_with_legacy_override() {
        let error = validate_rpc_serve_bind_host_with_override("0.0.0.0", false)
            .expect_err("public RPC bind must require an override");
        assert!(error.contains("rpc serve bind host rejected"), "{error}");
        validate_rpc_serve_bind_host_with_override("127.0.0.1", false)
            .expect("localhost RPC bind is allowed");
        let override_error = validate_rpc_serve_bind_host_with_override("0.0.0.0", true)
            .expect_err("a legacy environment override must not expose plaintext RPC");
        assert!(override_error.contains("direct public and wildcard binds are disabled"));
    }

    #[test]
    fn finality_parent_overshoot_is_immediately_typed_as_stale() {
        assert!(rpc_finality_parent_stale_error(
            "validator-0",
            10,
            Some("required-hash"),
            Some("required-root"),
            9,
            "behind-hash",
            "behind-root",
        )
        .is_none());
        assert!(rpc_finality_parent_stale_error(
            "validator-0",
            10,
            Some("required-hash"),
            Some("required-root"),
            10,
            "required-hash",
            "required-root",
        )
        .is_none());
        let (code, message) = rpc_finality_parent_stale_error(
            "validator-0",
            10,
            Some("required-hash"),
            Some("required-root"),
            11,
            "observed-hash",
            "observed-root",
        )
        .expect("overshot parent was allowed to wait");
        assert_eq!(code, "rpc_finality_parent_stale");
        for value in [
            "required-hash",
            "required-root",
            "observed-hash",
            "observed-root",
            "height 10",
            "height 11",
        ] {
            assert!(
                message.contains(value),
                "missing `{value}` from `{message}`"
            );
        }
    }

    #[test]
    fn rpc_serve_child_request_spool_uses_private_directory() {
        let spool_root =
            env::temp_dir().join(format!("postfiat-rpc-spool-root-test-{}", process::id()));
        prepare_rpc_serve_spool_root(&spool_root).expect("prepare RPC spool root");
        let spool_dir = create_rpc_serve_spool_dir(&spool_root, 42).expect("create RPC spool dir");
        let legacy_file =
            env::temp_dir().join(format!("postfiat-rpc-serve-{}-42.json", process::id()));

        assert_ne!(spool_dir, legacy_file);
        assert!(spool_dir.is_dir());
        assert!(spool_dir
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(
                |name| name.starts_with("postfiat-rpc-serve-") && !name.ends_with(".json")
            ));

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let mode = std::fs::metadata(&spool_dir)
                .expect("spool dir metadata")
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(
                mode & 0o077,
                0,
                "spool dir must not grant group/other access"
            );
        }

        std::fs::remove_dir_all(spool_dir).expect("cleanup spool dir");
        std::fs::remove_dir_all(spool_root).expect("cleanup spool root");
    }

    #[test]
    fn rpc_serve_spool_root_rejects_non_directory() {
        let path = env::temp_dir().join(format!("postfiat-rpc-spool-file-{}", process::id()));
        std::fs::write(&path, b"not a directory").expect("write spool root file");
        let error = prepare_rpc_serve_spool_root(&path).expect_err("reject file spool root");
        assert!(error.contains("not a directory"), "{error}");
        std::fs::remove_file(path).expect("cleanup spool root file");
    }

    #[cfg(unix)]
    #[test]
    fn rpc_serve_spool_root_rejects_symlink() {
        use std::os::unix::fs::symlink;

        let root = env::temp_dir().join(format!("postfiat-rpc-spool-link-test-{}", process::id()));
        let target = root.with_extension("target");
        std::fs::create_dir(&target).expect("create symlink target");
        symlink(&target, &root).expect("create spool symlink");
        let error = prepare_rpc_serve_spool_root(&root).expect_err("reject symlink spool root");
        assert!(error.contains("must not be a symlink"), "{error}");
        std::fs::remove_file(root).expect("cleanup spool symlink");
        std::fs::remove_dir(target).expect("cleanup spool target");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn rpc_serve_storage_failures_are_typed_for_operator_recovery() {
        let cases = [
            (28, "capacity_or_inode_exhausted"),
            (30, "read_only_filesystem"),
            (5, "io_failure"),
            (13, "permission_denied"),
        ];
        for (errno, class) in cases {
            let message = rpc_serve_storage_error(
                "fault injection",
                std::io::Error::from_raw_os_error(errno),
            );
            assert!(message.contains(&format!("[{class}]")), "{message}");
            assert!(message.contains("fault injection"), "{message}");
        }
    }

    #[test]
    fn rpc_serve_health_cache_stamp_invalidates_on_state_change() {
        let root = env::temp_dir().join(format!("postfiat-rpc-health-stamp-{}", process::id()));
        std::fs::create_dir(&root).expect("create health stamp root");
        for name in [
            postfiat_storage::MEMPOOL_FILE,
            postfiat_storage::CHAIN_TIP_FILE,
            postfiat_storage::NODE_STATE_FILE,
            postfiat_storage::GOVERNANCE_FILE,
        ] {
            std::fs::write(root.join(name), b"{}\n").expect("write health state file");
        }
        let first_status = rpc_serve_health_stamp(&root, true).expect("initial status stamp");
        let first_mempool = rpc_serve_health_stamp(&root, false).expect("initial mempool stamp");
        std::fs::write(
            root.join(postfiat_storage::MEMPOOL_FILE),
            b"{\"pending\":[]}\n",
        )
        .expect("change mempool state");
        let second_status = rpc_serve_health_stamp(&root, true).expect("changed status stamp");
        let second_mempool = rpc_serve_health_stamp(&root, false).expect("changed mempool stamp");
        assert_ne!(first_status, second_status);
        assert_ne!(first_mempool, second_mempool);
        std::fs::remove_dir_all(root).expect("cleanup health stamp root");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn rpc_serve_event_log_failure_degrades_without_failing_listener_path() {
        let ready_file = env::temp_dir().join(format!(
            "postfiat-rpc-ready-telemetry-test-{}.json",
            process::id()
        ));
        let readiness = Arc::new(Mutex::new(RpcServeReadinessReport {
            schema: "postfiat-rpc-serve-readiness-v1".to_string(),
            ready: true,
            degraded: false,
            node_id: "validator-0".to_string(),
            bind_address: "127.0.0.1:0".to_string(),
            data_dir: "/tmp/data".to_string(),
            data_dir_readable: true,
            spool_dir: "/tmp/spool".to_string(),
            spool_probe_ok: true,
            event_log: Some("/dev/full".to_string()),
            event_log_writable: true,
            local_state_loaded: true,
            listener_bound: true,
            finality_enabled: false,
            finality_topology_available: true,
            finality_key_available: true,
            telemetry_failure_count: 0,
            last_telemetry_error: None,
        }));
        let mut writer = Some(
            std::fs::OpenOptions::new()
                .write(true)
                .open("/dev/full")
                .expect("open /dev/full"),
        );
        let response = rpc_serve_error_response("telemetry", "test", "test");
        let event = rpc_serve_event(
            "validator-0",
            1,
            "127.0.0.1",
            "telemetry".to_string(),
            "status",
            None,
            None,
            &response,
        );

        write_rpc_serve_event_or_degrade(&mut writer, &event, &ready_file, &readiness);

        assert!(writer.is_none(), "failed telemetry writer must be disabled");
        let readiness = readiness.lock().expect("readiness lock");
        assert!(
            readiness.ready,
            "telemetry failure must not withdraw service readiness"
        );
        assert!(readiness.degraded);
        assert!(!readiness.event_log_writable);
        assert_eq!(readiness.telemetry_failure_count, 1);
        let persisted: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&ready_file).expect("read degraded readiness"))
                .expect("parse degraded readiness");
        assert_eq!(persisted["degraded"], true);
        std::fs::remove_file(ready_file).expect("cleanup readiness file");
    }

    #[test]
    fn rpc_serve_classifies_write_errors_for_edge_metrics() {
        let invalid_signature = rpc_serve_error_response(
            "bad-sig",
            "rpc_error",
            "rpc mempool_submit_signed_transfer failed: mempool admission rejected `bad_signature`: signature verification failed",
        );
        let invalid_event = rpc_serve_event(
            "validator-0",
            1,
            "127.0.0.1",
            "bad-sig".to_string(),
            "mempool_submit_signed_transfer",
            Some(RpcServeMempoolSubmitCounters {
                peer_count: 1,
                total_count: 1,
                active_count: 0,
            }),
            None,
            &invalid_signature,
        );
        assert_eq!(
            invalid_event.error_class.as_deref(),
            Some("invalid_signature")
        );

        let duplicate = rpc_serve_error_response(
            "duplicate",
            "rpc_error",
            "rpc mempool_submit_signed_transfer failed: transaction `abc` is already pending",
        );
        let duplicate_event = rpc_serve_event(
            "validator-0",
            2,
            "127.0.0.1",
            "duplicate".to_string(),
            "mempool_submit_signed_transfer",
            Some(RpcServeMempoolSubmitCounters {
                peer_count: 2,
                total_count: 2,
                active_count: 0,
            }),
            None,
            &duplicate,
        );
        assert_eq!(
            duplicate_event.error_class.as_deref(),
            Some("duplicate_transaction")
        );

        let disallowed = rpc_serve_error_response(
            "disallowed",
            "rpc_method_not_allowed",
            "rpc method `mempool_submit_transfer` is not enabled",
        );
        let disallowed_event = rpc_serve_event(
            "validator-0",
            3,
            "127.0.0.1",
            "disallowed".to_string(),
            "mempool_submit_transfer",
            None,
            None,
            &disallowed,
        );
        assert_eq!(
            disallowed_event.error_class.as_deref(),
            Some("method_not_allowed")
        );
        let events = vec![invalid_event, duplicate_event, disallowed_event];
        assert_eq!(rpc_serve_error_class_count(&events, "invalid_signature"), 1);
        assert_eq!(
            rpc_serve_error_class_count(&events, "duplicate_transaction"),
            1
        );
        assert_eq!(
            rpc_serve_error_class_count(&events, "method_not_allowed"),
            1
        );

        let rate_limited = rpc_serve_error_response(
            "rate-limited",
            "rpc_mempool_submit_rate_limited",
            "peer `127.0.0.1` exceeded mempool_submit_signed_transfer limit 1",
        );
        assert_eq!(
            rpc_serve_error_class("mempool_submit_signed_transfer", &rate_limited).as_deref(),
            Some("mempool_submit_rate_limited")
        );
        let global_rate_limited = rpc_serve_error_response(
            "global-rate-limited",
            "rpc_mempool_submit_global_rate_limited",
            "rpc serve exceeded mempool_submit_signed_transfer total limit 1",
        );
        assert_eq!(
            rpc_serve_error_class("mempool_submit_signed_transfer", &global_rate_limited)
                .as_deref(),
            Some("mempool_submit_global_rate_limited")
        );
        let orchard_rate_limited = rpc_serve_error_response(
            "orchard-rate-limited",
            "rpc_orchard_batch_create_rate_limited",
            "peer `127.0.0.1` exceeded Orchard batch-create limit 1",
        );
        assert_eq!(
            rpc_serve_error_class("shield_batch_orchard", &orchard_rate_limited).as_deref(),
            Some("orchard_batch_create_rate_limited")
        );
        let orchard_concurrency_limited = rpc_serve_error_response(
            "orchard-concurrency-limited",
            "rpc_orchard_batch_create_concurrency_limited",
            "rpc serve exceeded Orchard batch-create concurrent verifier limit 1",
        );
        assert_eq!(
            rpc_serve_error_class("shield_batch_orchard", &orchard_concurrency_limited).as_deref(),
            Some("orchard_batch_create_concurrency_limited")
        );
        let orchard_not_public_safe = rpc_serve_error_response(
            "orchard-not-public-safe",
            "rpc_orchard_batch_create_not_public_safe",
            "remote Orchard batch creation requires action_json, not action_file",
        );
        assert_eq!(
            rpc_serve_error_class("shield_batch_orchard", &orchard_not_public_safe).as_deref(),
            Some("orchard_batch_create_not_public_safe")
        );
        let child_timeout =
            rpc_serve_child_error_response("child-timeout", "rpc serve child timed out after 0 ms");
        assert_eq!(
            rpc_serve_error_class("status", &child_timeout).as_deref(),
            Some("rpc_child_timeout")
        );
        for (code, expected_class) in [
            ("rpc_finality_parent_stale", "finality_parent_stale"),
            ("rpc_finality_parent_not_ready", "finality_parent_not_ready"),
            (
                "rpc_finality_parent_wait_failed",
                "finality_parent_wait_failed",
            ),
        ] {
            let response = rpc_serve_error_response("typed", code, "bounded failure");
            assert_eq!(
                rpc_serve_error_class(
                    "mempool_submit_signed_atomic_swap_transaction_finality",
                    &response,
                )
                .as_deref(),
                Some(expected_class),
                "error code {code} fell through to a signed-input class"
            );
        }
        let request_too_large = rpc_serve_error_response(
            "remote-4",
            "rpc_request_too_large",
            "rpc request exceeded 65536 bytes",
        );
        let request_too_large_event = rpc_serve_event(
            "validator-0",
            4,
            "127.0.0.1",
            "remote-4".to_string(),
            "invalid",
            None,
            None,
            &request_too_large,
        );
        assert_eq!(
            request_too_large_event.error_class.as_deref(),
            Some("request_too_large")
        );

        let state = Arc::new(Mutex::new(RpcServeMempoolSubmitState::default()));
        assert_eq!(
            record_rpc_serve_mempool_submit_attempt(&state, "127.0.0.1")
                .expect("first counter")
                .peer_count,
            1
        );
        let counters =
            record_rpc_serve_mempool_submit_attempt(&state, "127.0.0.1").expect("second counter");
        assert_eq!(counters.peer_count, 2);
        assert_eq!(counters.total_count, 2);
        assert_eq!(counters.active_count, 0);
    }

    #[test]
    fn rpc_serve_metrics_include_direct_connection_saturation() {
        let mut result = serde_json::json!({"schema": "postfiat-node-metrics-v1"});
        merge_rpc_serve_runtime_metrics(
            &mut result,
            RpcServeRuntimeMetricsSnapshot {
                active_connections: 48,
                active_connection_limit: 64,
                peak_active_connections: 63,
                accepted_connection_count: 100,
            },
        );
        assert_eq!(result["rpc"]["active_connections"], 48);
        assert_eq!(result["rpc"]["active_connection_limit"], 64);
        assert_eq!(result["rpc"]["active_connection_utilization_ppm"], 750_000);
        assert_eq!(result["rpc"]["peak_active_connections"], 63);
        assert_eq!(result["rpc"]["accepted_connection_count"], 100);
    }

    #[test]
    fn rpc_serve_orchard_worker_counter_enforces_concurrency() {
        let state = Arc::new(Mutex::new(RpcServeMempoolSubmitState::default()));
        let (_first_guard, active_count) = try_acquire_rpc_serve_active_orchard_worker(&state, 1)
            .expect("first active Orchard worker");
        assert_eq!(active_count, 1);
        let second_error = try_acquire_rpc_serve_active_orchard_worker(&state, 1)
            .expect_err("second active Orchard worker must be rejected");
        assert!(
            second_error.contains("concurrent verifier limit 1"),
            "{second_error}"
        );
        drop(_first_guard);
        let (_third_guard, active_count) = try_acquire_rpc_serve_active_orchard_worker(&state, 1)
            .expect("worker slot released after guard drop");
        assert_eq!(active_count, 1);
    }

    #[test]
    fn rpc_serve_orchard_batch_create_policy_is_gated() {
        assert!(!rpc_serve_method_allowed(
            "shield_batch_orchard",
            false,
            false,
            false
        ));
        assert!(rpc_serve_method_allowed(
            "shield_batch_orchard",
            false,
            false,
            true
        ));
        assert!(!rpc_serve_method_allowed(
            "apply_shield_batch",
            false,
            false,
            true
        ));
        assert!(!rpc_serve_method_allowed(
            "mempool_submit_signed_transfer_finality",
            true,
            false,
            false
        ));
        assert!(rpc_serve_method_allowed(
            "mempool_submit_signed_transfer_finality",
            false,
            true,
            false
        ));
        assert!(!rpc_serve_method_allowed(
            "shield_batch_finality",
            true,
            false,
            true
        ));
        assert!(rpc_serve_method_allowed(
            "shield_batch_finality",
            false,
            true,
            false
        ));
        assert!(!rpc_serve_method_allowed(
            RPC_FINALITY_TIMEOUT_VOTE_METHOD,
            true,
            false,
            false
        ));
        assert!(rpc_serve_method_allowed(
            RPC_FINALITY_TIMEOUT_VOTE_METHOD,
            false,
            true,
            false
        ));

        let public_request = RpcRequest::empty("orchard-json", "shield_batch_orchard")
            .with_param("action_json", "{}")
            .expect("action json param");
        validate_rpc_serve_orchard_batch_create_request(&public_request)
            .expect("public Orchard batch-create shape");

        let file_request = RpcRequest::empty("orchard-file", "shield_batch_orchard")
            .with_param("action_file", "/tmp/action.json")
            .expect("action file param")
            .with_param("batch_file", "/tmp/batch.json")
            .expect("batch file param");
        let file_error = validate_rpc_serve_orchard_batch_create_request(&file_request)
            .expect_err("remote file-backed Orchard request rejected");
        assert!(
            file_error.contains("inline JSON, not client file paths"),
            "{file_error}"
        );

        let deposit_public_request =
            RpcRequest::empty("orchard-deposit-json", "shield_batch_orchard_deposit")
                .with_param("deposit_json", "{}")
                .expect("deposit json param");
        validate_rpc_serve_orchard_batch_create_request(&deposit_public_request)
            .expect("public Orchard deposit batch-create shape");

        let deposit_file_request =
            RpcRequest::empty("orchard-deposit-file", "shield_batch_orchard_deposit")
                .with_param("deposit_file", "/tmp/deposit.json")
                .expect("deposit file param")
                .with_param("batch_file", "/tmp/batch.json")
                .expect("batch file param");
        let deposit_file_error =
            validate_rpc_serve_orchard_batch_create_request(&deposit_file_request)
                .expect_err("remote file-backed Orchard deposit request rejected");
        assert!(
            deposit_file_error.contains("inline JSON, not client file paths"),
            "{deposit_file_error}"
        );

        let client_path_request = RpcRequest::empty("orchard-client-path", "shield_batch_orchard")
            .with_param("action_json", "{}")
            .expect("action json param")
            .with_param("batch_file", "/tmp/client-selected-batch.json")
            .expect("batch file param");
        let path_error = validate_rpc_serve_orchard_batch_create_request(&client_path_request)
            .expect_err("remote client-selected batch path rejected");
        assert!(
            path_error.contains("server-controlled batch spool"),
            "{path_error}"
        );
    }

    #[test]
    fn rpc_serve_allows_navswap_planner_read_methods() {
        assert!(rpc_serve_method_allowed(
            "market_ops_status",
            false,
            false,
            false
        ));
        assert!(rpc_serve_method_allowed(
            "vault_bridge_status",
            false,
            false,
            false
        ));
        assert!(rpc_serve_method_allowed(
            "navcoin_bridge_routes",
            false,
            false,
            false
        ));
        assert!(rpc_serve_method_allowed(
            "navcoin_bridge_packet",
            false,
            false,
            false
        ));
        assert!(rpc_serve_method_allowed(
            "navcoin_bridge_claims",
            false,
            false,
            false
        ));
        assert!(rpc_serve_method_allowed(
            "navcoin_bridge_supply_status",
            false,
            false,
            false
        ));
        assert!(rpc_serve_method_allowed(
            "navcoin_bridge_receipt_replay",
            false,
            false,
            false
        ));
    }

    #[test]
    fn rpc_owned_sign_accepts_bounded_gzip_transport_without_changing_payload() {
        let order_json = r#"{"order":{"inputs":[],"outputs":[]},"owner_pubkey_hex":"abcd","owner_signature_hex":"1234"}"#;
        let mut encoder = flate2::write::GzEncoder::new(
            Vec::new(),
            flate2::Compression::fast(),
        );
        encoder.write_all(order_json.as_bytes()).expect("gzip write");
        let encoded = BASE64_STANDARD.encode(encoder.finish().expect("gzip finish"));
        let compressed = RpcRequest::empty("owned-compressed", "owned_sign")
            .with_param("order_json_gzip_base64", encoded)
            .expect("compressed param");
        assert_eq!(
            rpc_owned_sign_order_json(&compressed).expect("compressed order"),
            order_json
        );

        let legacy = RpcRequest::empty("owned-legacy", "owned_sign")
            .with_param("order_json", order_json)
            .expect("legacy param");
        assert_eq!(
            rpc_owned_sign_order_json(&legacy).expect("legacy order"),
            order_json
        );
    }
}
