fn run_cli_group_05(command: &str, flags: &[String]) -> Result<(), String> {
    match command {
        "market-ops-status" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let asset_id = flag_value(flags, "--asset-id").ok_or("missing --asset-id")?;
            let epoch = flag_value(flags, "--epoch")
                .map(|value| {
                    value
                        .parse::<u64>()
                        .map_err(|_| "--epoch must be a u64".to_string())
                })
                .transpose()?;
            let report = market_ops_status(MarketOpsStatusOptions {
                data_dir: PathBuf::from(data_dir),
                asset_id: asset_id.to_string(),
                epoch,
            })
            .map_err(|error| format!("market-ops-status failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report)
                .map_err(|error| format!("market ops status serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "market-ops-operation-bundle" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let asset_id = flag_value(flags, "--asset-id").ok_or("missing --asset-id")?;
            let policy_file = flag_value(flags, "--policy-file").ok_or("missing --policy-file")?;
            let policy_inputs_file =
                flag_value(flags, "--policy-inputs-file").ok_or("missing --policy-inputs-file")?;
            let bundle_dir = flag_value(flags, "--bundle").ok_or("missing --bundle")?;
            let evm_chain_id = parse_u64_flag(flags, "--evm-chain-id")?;
            let adapter_address =
                flag_value(flags, "--adapter-address").ok_or("missing --adapter-address")?;
            let vault_address =
                flag_value(flags, "--vault-address").ok_or("missing --vault-address")?;
            let mint_controller_address = flag_value(flags, "--mint-controller-address")
                .ok_or("missing --mint-controller-address")?;
            let data_window_start = parse_u64_flag(flags, "--data-window-start")?;
            let data_window_end = parse_u64_flag(flags, "--data-window-end")?;
            let valid_after = parse_u64_flag(flags, "--valid-after")?;
            let expires_at = parse_u64_flag(flags, "--expires-at")?;
            let funded_alignment_reserve_usd_e8 =
                parse_u128_flag(flags, "--funded-alignment-reserve-usd-e8")?;
            let bundle = market_ops_operation_bundle(MarketOpsOperationBundleOptions {
                data_dir: PathBuf::from(data_dir),
                asset_id: asset_id.to_string(),
                issuer: flag_value(flags, "--issuer").map(str::to_string),
                epoch: parse_optional_u64_flag(flags, "--epoch")?,
                policy_file: PathBuf::from(policy_file),
                policy_inputs_file: PathBuf::from(policy_inputs_file),
                bundle_dir: PathBuf::from(bundle_dir),
                overwrite: flag_present(flags, "--overwrite"),
                encoding_version: parse_optional_u32_flag(flags, "--encoding-version")?
                    .unwrap_or(1),
                evm_chain_id,
                adapter_address: adapter_address.to_string(),
                vault_address: vault_address.to_string(),
                mint_controller_address: mint_controller_address.to_string(),
                funded_alignment_reserve_usd_e8,
                discount_trigger_bps: parse_optional_u32_flag(flags, "--discount-trigger-bps")?
                    .unwrap_or(100),
                premium_trigger_bps: parse_optional_u32_flag(flags, "--premium-trigger-bps")?
                    .unwrap_or(100),
                data_window_start,
                data_window_end,
                valid_after,
                expires_at,
                cooldown_seconds: parse_optional_u64_flag(flags, "--cooldown-seconds")?
                    .unwrap_or(0),
                nonce: flag_value(flags, "--nonce").map(str::to_string),
                previous_market_state_hash: flag_value(flags, "--previous-market-state-hash")
                    .map(str::to_string),
            })
            .map_err(|error| format!("market-ops-operation-bundle failed: {error}"))?;
            let json = serde_json::to_string_pretty(&bundle).map_err(|error| {
                format!("market ops operation bundle serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
        "vault-bridge-status" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let asset_id = flag_value(flags, "--asset-id").ok_or("missing --asset-id")?;
            let report = vault_bridge_status(VaultBridgeStatusOptions {
                data_dir: PathBuf::from(data_dir),
                asset_id: asset_id.to_string(),
            })
            .map_err(|error| format!("vault-bridge-status failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report).map_err(|error| {
                format!("vault bridge asset status serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
        "vault-bridge-conservation-audit" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let asset_id = flag_value(flags, "--asset-id").ok_or("missing --asset-id")?;
            let source_rpc_url =
                flag_value(flags, "--source-rpc-url").ok_or("missing --source-rpc-url")?;
            let cast_binary = flag_value(flags, "--cast-bin").unwrap_or("cast");
            let report = vault_bridge_conservation_audit(VaultBridgeConservationOptions {
                data_dir: PathBuf::from(data_dir),
                asset_id: asset_id.to_string(),
                source_rpc_url: source_rpc_url.to_string(),
                cast_binary: PathBuf::from(cast_binary),
            })
            .map_err(|error| format!("vault-bridge-conservation-audit failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report).map_err(|error| {
                format!("vault bridge conservation serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
        "navcoin-bridge-routes" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let report = navcoin_bridge_routes(NavcoinBridgeRoutesOptions {
                data_dir: PathBuf::from(data_dir),
            })
            .map_err(|error| format!("navcoin-bridge-routes failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report)
                .map_err(|error| format!("navcoin bridge routes serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "navcoin-bridge-packet" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let route_id = flag_value(flags, "--route-id").ok_or("missing --route-id")?;
            let packet_hash = flag_value(flags, "--packet-hash").ok_or("missing --packet-hash")?;
            let report = navcoin_bridge_packet(NavcoinBridgePacketOptions {
                data_dir: PathBuf::from(data_dir),
                route_id: route_id.to_string(),
                packet_hash: packet_hash.to_string(),
            })
            .map_err(|error| format!("navcoin-bridge-packet failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report)
                .map_err(|error| format!("navcoin bridge packet serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "navcoin-bridge-claims" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let route_id = flag_value(flags, "--route-id").ok_or("missing --route-id")?;
            let limit = parse_optional_u64_flag(flags, "--limit")?
                .map(|value| {
                    usize::try_from(value).map_err(|_| "--limit does not fit in usize".to_string())
                })
                .transpose()?;
            let report = navcoin_bridge_claims(NavcoinBridgeClaimsOptions {
                data_dir: PathBuf::from(data_dir),
                route_id: route_id.to_string(),
                limit,
                include_terminal: flag_present(flags, "--include-terminal"),
            })
            .map_err(|error| format!("navcoin-bridge-claims failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report)
                .map_err(|error| format!("navcoin bridge claims serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "navcoin-bridge-supply-status" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let route_id = flag_value(flags, "--route-id").ok_or("missing --route-id")?;
            let report = navcoin_bridge_supply_status(NavcoinBridgeSupplyStatusOptions {
                data_dir: PathBuf::from(data_dir),
                route_id: route_id.to_string(),
            })
            .map_err(|error| format!("navcoin-bridge-supply-status failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report).map_err(|error| {
                format!("navcoin bridge supply status serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
        "navcoin-bridge-receipt-replay" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let route_id = flag_value(flags, "--route-id").ok_or("missing --route-id")?;
            let report = navcoin_bridge_receipt_replay(NavcoinBridgeReceiptReplayOptions {
                data_dir: PathBuf::from(data_dir),
                route_id: route_id.to_string(),
            })
            .map_err(|error| format!("navcoin-bridge-receipt-replay failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report).map_err(|error| {
                format!("navcoin bridge receipt replay serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
        "navcoin-bridge-route-init" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let config_file = flag_value(flags, "--config-file").ok_or("missing --config-file")?;
            let report = navcoin_bridge_route_init(NavcoinBridgeRouteInitOptions {
                data_dir: PathBuf::from(data_dir),
                config_file: PathBuf::from(config_file),
                ethereum_chain_id: parse_u64_flag(flags, "--ethereum-chain-id")?,
                latest_finalized_nav_epoch: parse_u64_flag(flags, "--latest-finalized-nav-epoch")?,
                return_finality_blocks: parse_u64_flag(flags, "--return-finality-blocks")?,
                replace: flag_present(flags, "--replace"),
            })
            .map_err(|error| format!("navcoin-bridge-route-init failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report).map_err(|error| {
                format!("navcoin bridge route init serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
        "navcoin-bridge-launch-config-template" => {
            let route_config_file =
                flag_value(flags, "--route-config-file").ok_or("missing --route-config-file")?;
            let official_uniswap_file = flag_value(flags, "--official-uniswap-file")
                .ok_or("missing --official-uniswap-file")?;
            let usdc_token = flag_value(flags, "--usdc-token").ok_or("missing --usdc-token")?;
            let receipt_verifier =
                flag_value(flags, "--receipt-verifier").ok_or("missing --receipt-verifier")?;
            let uniswap_pool_key_hash = flag_value(flags, "--uniswap-pool-key-hash")
                .ok_or("missing --uniswap-pool-key-hash")?;
            let pricing_reserve_packet_hash = flag_value(flags, "--pricing-reserve-packet-hash")
                .ok_or("missing --pricing-reserve-packet-hash")?;
            let position_recipient =
                flag_value(flags, "--position-recipient").ok_or("missing --position-recipient")?;
            let output_file = flag_value(flags, "--output-file").ok_or("missing --output-file")?;
            let report =
                navcoin_bridge_launch_config_template(NavcoinBridgeLaunchConfigTemplateOptions {
                    route_config_file: PathBuf::from(route_config_file),
                    official_uniswap_file: PathBuf::from(official_uniswap_file),
                    usdc_token: usdc_token.to_string(),
                    receipt_verifier: receipt_verifier.to_string(),
                    uniswap_pool_key_hash: uniswap_pool_key_hash.to_string(),
                    pricing_reserve_packet_hash: pricing_reserve_packet_hash.to_string(),
                    nav_price_settlement_atoms_per_nav_atom: parse_u64_flag(
                        flags,
                        "--nav-price-settlement-atoms-per-nav-atom",
                    )?,
                    tick_lower: parse_i32_flag(flags, "--tick-lower")?,
                    tick_upper: parse_i32_flag(flags, "--tick-upper")?,
                    fee_pips: parse_u32_flag(flags, "--fee-pips")?,
                    position_recipient: position_recipient.to_string(),
                    output_file: PathBuf::from(output_file),
                    overwrite: flag_present(flags, "--overwrite"),
                })
                .map_err(|error| {
                    format!("navcoin-bridge-launch-config-template failed: {error}")
                })?;
            let json = serde_json::to_string_pretty(&report).map_err(|error| {
                format!("navcoin bridge launch config template serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
        "navcoin-bridge-launch-config-init" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let launch_config_file =
                flag_value(flags, "--launch-config-file").ok_or("missing --launch-config-file")?;
            let report = navcoin_bridge_launch_config_init(NavcoinBridgeLaunchConfigInitOptions {
                data_dir: PathBuf::from(data_dir),
                launch_config_file: PathBuf::from(launch_config_file),
                replace: flag_present(flags, "--replace"),
            })
            .map_err(|error| format!("navcoin-bridge-launch-config-init failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report).map_err(|error| {
                format!("navcoin bridge launch config init serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
        "navcoin-bridge-record-fork-rehearsal" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let route_id = flag_value(flags, "--route-id").ok_or("missing --route-id")?;
            let evidence_file =
                flag_value(flags, "--evidence-file").ok_or("missing --evidence-file")?;
            let report =
                navcoin_bridge_record_fork_rehearsal(NavcoinBridgeRecordForkRehearsalOptions {
                    data_dir: PathBuf::from(data_dir),
                    route_id: route_id.to_string(),
                    evidence_file: PathBuf::from(evidence_file),
                })
                .map_err(|error| format!("navcoin-bridge-record-fork-rehearsal failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report).map_err(|error| {
                format!("navcoin bridge fork rehearsal serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
        "navcoin-bridge-packet-preflight" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let route_id = flag_value(flags, "--route-id").ok_or("missing --route-id")?;
            let packet_file = flag_value(flags, "--packet-file").ok_or("missing --packet-file")?;
            let report = navcoin_bridge_packet_preflight(NavcoinBridgePacketPreflightOptions {
                data_dir: PathBuf::from(data_dir),
                route_id: route_id.to_string(),
                packet_file: PathBuf::from(packet_file),
            })
            .map_err(|error| format!("navcoin-bridge-packet-preflight failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report).map_err(|error| {
                format!("navcoin bridge packet preflight serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
        "navcoin-bridge-primary-subscribe" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let request_file =
                flag_value(flags, "--request-file").ok_or("missing --request-file")?;
            let report = navcoin_bridge_primary_subscribe(NavcoinBridgePrimarySubscribeOptions {
                data_dir: PathBuf::from(data_dir),
                request_file: PathBuf::from(request_file),
            })
            .map_err(|error| format!("navcoin-bridge-primary-subscribe failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report).map_err(|error| {
                format!("navcoin bridge primary subscribe serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
        "navcoin-bridge-export-debit" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let request_file =
                flag_value(flags, "--request-file").ok_or("missing --request-file")?;
            let report = navcoin_bridge_export_debit(NavcoinBridgeExportDebitOptions {
                data_dir: PathBuf::from(data_dir),
                request_file: PathBuf::from(request_file),
            })
            .map_err(|error| format!("navcoin-bridge-export-debit failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report).map_err(|error| {
                format!("navcoin bridge export debit serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
        "navcoin-bridge-destination-consume" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let route_id = flag_value(flags, "--route-id").ok_or("missing --route-id")?;
            let packet_hash = flag_value(flags, "--packet-hash").ok_or("missing --packet-hash")?;
            let report =
                navcoin_bridge_destination_consume(NavcoinBridgeDestinationConsumeOptions {
                    data_dir: PathBuf::from(data_dir),
                    route_id: route_id.to_string(),
                    packet_hash: packet_hash.to_string(),
                })
                .map_err(|error| format!("navcoin-bridge-destination-consume failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report).map_err(|error| {
                format!("navcoin bridge destination consume serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
        "navcoin-bridge-refund-source" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let route_id = flag_value(flags, "--route-id").ok_or("missing --route-id")?;
            let request_file =
                flag_value(flags, "--request-file").ok_or("missing --request-file")?;
            let report = navcoin_bridge_refund_source(NavcoinBridgeRefundSourceOptions {
                data_dir: PathBuf::from(data_dir),
                route_id: route_id.to_string(),
                request_file: PathBuf::from(request_file),
            })
            .map_err(|error| format!("navcoin-bridge-refund-source failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report).map_err(|error| {
                format!("navcoin bridge refund source serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
        "navcoin-bridge-record-return-burn" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let route_id = flag_value(flags, "--route-id").ok_or("missing --route-id")?;
            let request_file =
                flag_value(flags, "--request-file").ok_or("missing --request-file")?;
            let report = navcoin_bridge_record_return_burn(NavcoinBridgeRecordReturnBurnOptions {
                data_dir: PathBuf::from(data_dir),
                route_id: route_id.to_string(),
                request_file: PathBuf::from(request_file),
            })
            .map_err(|error| format!("navcoin-bridge-record-return-burn failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report).map_err(|error| {
                format!("navcoin bridge record return burn serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
        "navcoin-bridge-return-burn-request" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let route_id = flag_value(flags, "--route-id").ok_or("missing --route-id")?;
            let ethereum_sender =
                flag_value(flags, "--ethereum-sender").ok_or("missing --ethereum-sender")?;
            let pftl_recipient =
                flag_value(flags, "--pftl-recipient").ok_or("missing --pftl-recipient")?;
            let return_nonce =
                flag_value(flags, "--return-nonce").ok_or("missing --return-nonce")?;
            let output_file = flag_value(flags, "--output-file").ok_or("missing --output-file")?;
            let report =
                navcoin_bridge_return_burn_request(NavcoinBridgeReturnBurnRequestOptions {
                    data_dir: PathBuf::from(data_dir),
                    route_id: route_id.to_string(),
                    ethereum_sender: ethereum_sender.to_string(),
                    pftl_recipient: pftl_recipient.to_string(),
                    amount_atoms: parse_u64_flag(flags, "--amount-atoms")?,
                    return_nonce: return_nonce.to_string(),
                    burn_height: parse_u64_flag(flags, "--burn-height")?,
                    output_file: PathBuf::from(output_file),
                    overwrite: flag_present(flags, "--overwrite"),
                })
                .map_err(|error| format!("navcoin-bridge-return-burn-request failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report).map_err(|error| {
                format!("navcoin bridge return burn request serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
        "navcoin-bridge-import-return" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let route_id = flag_value(flags, "--route-id").ok_or("missing --route-id")?;
            let burn_event_hash =
                flag_value(flags, "--burn-event-hash").ok_or("missing --burn-event-hash")?;
            let pftl_recipient =
                flag_value(flags, "--pftl-recipient").ok_or("missing --pftl-recipient")?;
            let report = navcoin_bridge_import_return(NavcoinBridgeImportReturnOptions {
                data_dir: PathBuf::from(data_dir),
                route_id: route_id.to_string(),
                burn_event_hash: burn_event_hash.to_string(),
                pftl_recipient: pftl_recipient.to_string(),
            })
            .map_err(|error| format!("navcoin-bridge-import-return failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report).map_err(|error| {
                format!("navcoin bridge import return serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
        "vault-bridge-receipts" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let asset_id = flag_value(flags, "--asset-id").ok_or("missing --asset-id")?;
            let report = vault_bridge_receipts(VaultBridgeReceiptsOptions {
                data_dir: PathBuf::from(data_dir),
                asset_id: asset_id.to_string(),
                bucket_id: flag_value(flags, "--bucket-id").map(str::to_string),
            })
            .map_err(|error| format!("vault-bridge-receipts failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report).map_err(|error| {
                format!("vault bridge asset receipts serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
        "vault-bridge-asset-id" => {
            let pftl_chain_id =
                flag_value(flags, "--pftl-chain-id").ok_or("missing --pftl-chain-id")?;
            let issuer = flag_value(flags, "--issuer").ok_or("missing --issuer")?;
            let asset_code = flag_value(flags, "--asset-code").ok_or("missing --asset-code")?;
            let asset_version = flag_value(flags, "--asset-version")
                .unwrap_or("1")
                .parse::<u32>()
                .map_err(|_| "--asset-version must be a u32".to_string())?;
            let report = vault_bridge_asset_id(VaultBridgeAssetIdOptions {
                pftl_chain_id: pftl_chain_id.to_string(),
                issuer: issuer.to_string(),
                asset_code: asset_code.to_string(),
                asset_version,
                env_file: flag_value(flags, "--env-file").map(PathBuf::from),
                overwrite: flag_present(flags, "--overwrite"),
            })
            .map_err(|error| format!("vault-bridge-asset-id failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report)
                .map_err(|error| format!("vault bridge asset id serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "vault-bridge-bootstrap-bundle" => {
            let pftl_chain_id =
                flag_value(flags, "--pftl-chain-id").ok_or("missing --pftl-chain-id")?;
            let source_chain_id = flag_value(flags, "--source-chain-id")
                .ok_or("missing --source-chain-id")?
                .parse::<u64>()
                .map_err(|_| "--source-chain-id must be a u64".to_string())?;
            let vault_address =
                flag_value(flags, "--vault-address").ok_or("missing --vault-address")?;
            let token_address =
                flag_value(flags, "--token-address").ok_or("missing --token-address")?;
            let issuer = flag_value(flags, "--issuer").ok_or("missing --issuer")?;
            let asset_code = flag_value(flags, "--asset-code").ok_or("missing --asset-code")?;
            let valuation_policy_hash = flag_value(flags, "--valuation-policy-hash")
                .ok_or("missing --valuation-policy-hash")?;
            let bundle_dir = flag_value(flags, "--bundle").ok_or("missing --bundle")?;
            let asset_version = flag_value(flags, "--asset-version")
                .unwrap_or("1")
                .parse::<u32>()
                .map_err(|_| "--asset-version must be a u32".to_string())?;
            let asset_precision = flag_value(flags, "--asset-precision")
                .ok_or("missing --asset-precision")?
                .parse::<u8>()
                .map_err(|_| "--asset-precision must be a u8".to_string())?;
            let valuation_unit =
                flag_value(flags, "--valuation-unit").ok_or("missing --valuation-unit")?;
            let max_supply = flag_value(flags, "--max-supply")
                .map(|value| {
                    value
                        .parse::<u64>()
                        .map_err(|_| "--max-supply must be a u64".to_string())
                })
                .transpose()?;
            let max_snapshot_age_blocks = flag_value(flags, "--max-snapshot-age-blocks")
                .unwrap_or("100")
                .parse::<u64>()
                .map_err(|_| "--max-snapshot-age-blocks must be a u64".to_string())?;
            let challenge_window_blocks = flag_value(flags, "--challenge-window-blocks")
                .unwrap_or("1")
                .parse::<u64>()
                .map_err(|_| "--challenge-window-blocks must be a u64".to_string())?;
            let max_epoch_gap_blocks = flag_value(flags, "--max-epoch-gap-blocks")
                .unwrap_or("100")
                .parse::<u64>()
                .map_err(|_| "--max-epoch-gap-blocks must be a u64".to_string())?;
            let settle_deadline_blocks = flag_value(flags, "--settle-deadline-blocks")
                .unwrap_or("0")
                .parse::<u64>()
                .map_err(|_| "--settle-deadline-blocks must be a u64".to_string())?;
            let min_challenge_bond = flag_value(flags, "--min-challenge-bond")
                .unwrap_or("0")
                .parse::<u64>()
                .map_err(|_| "--min-challenge-bond must be a u64".to_string())?;
            let min_attestations = flag_value(flags, "--min-attestations")
                .unwrap_or("1")
                .parse::<u64>()
                .map_err(|_| "--min-attestations must be a u64".to_string())?;
            let tolerance_bp = flag_value(flags, "--tolerance-bp")
                .unwrap_or("0")
                .parse::<u64>()
                .map_err(|_| "--tolerance-bp must be a u64".to_string())?;
            let bridge_observer_min_confirmations =
                flag_value(flags, "--bridge-observer-min-confirmations")
                    .unwrap_or("0")
                    .parse::<u64>()
                    .map_err(|_| "--bridge-observer-min-confirmations must be a u64".to_string())?;
            let trust_limit = flag_value(flags, "--trust-limit")
                .unwrap_or("1000000000000000")
                .parse::<u64>()
                .map_err(|_| "--trust-limit must be a u64".to_string())?;
            let trust_reserve_paid = flag_value(flags, "--trust-reserve-paid")
                .unwrap_or("10")
                .parse::<u64>()
                .map_err(|_| "--trust-reserve-paid must be a u64".to_string())?;
            let trust_accounts = flag_value(flags, "--trust-accounts")
                .map(|value| {
                    value
                        .split(',')
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .map(str::to_string)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let report = vault_bridge_bootstrap_bundle(VaultBridgeBootstrapBundleOptions {
                pftl_chain_id: pftl_chain_id.to_string(),
                source_chain_id,
                vault_address: vault_address.to_string(),
                token_address: token_address.to_string(),
                issuer: issuer.to_string(),
                reserve_operator: flag_value(flags, "--reserve-operator")
                    .unwrap_or(issuer)
                    .to_string(),
                redemption_account: flag_value(flags, "--redemption-account")
                    .unwrap_or(issuer)
                    .to_string(),
                asset_code: asset_code.to_string(),
                asset_version,
                asset_precision,
                asset_display_name: flag_value(flags, "--asset-display-name")
                    .unwrap_or(asset_code)
                    .to_string(),
                max_supply,
                valuation_unit: valuation_unit.to_string(),
                verifier_kind: flag_value(flags, "--verifier-kind")
                    .unwrap_or(NAV_PROFILE_VERIFIER_MULTI_FETCH)
                    .to_string(),
                max_snapshot_age_blocks,
                challenge_window_blocks,
                max_epoch_gap_blocks,
                settle_deadline_blocks,
                min_challenge_bond,
                min_attestations,
                tolerance_bp,
                bridge_observer_min_confirmations,
                valuation_policy_hash: valuation_policy_hash.to_string(),
                trust_accounts,
                trust_limit,
                trust_reserve_paid,
                bundle_dir: PathBuf::from(bundle_dir),
                overwrite: flag_present(flags, "--overwrite"),
            })
            .map_err(|error| format!("vault-bridge-bootstrap-bundle failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report).map_err(|error| {
                format!("vault bridge bootstrap bundle serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
        "vault-bridge-deposit-intent" => {
            let source_chain_id = flag_value(flags, "--source-chain-id")
                .ok_or("missing --source-chain-id")?
                .parse::<u64>()
                .map_err(|_| "--source-chain-id must be a u64".to_string())?;
            let vault_address =
                flag_value(flags, "--vault-address").ok_or("missing --vault-address")?;
            let token_address =
                flag_value(flags, "--token-address").ok_or("missing --token-address")?;
            let depositor = flag_value(flags, "--depositor").ok_or("missing --depositor")?;
            let amount_atoms = flag_value(flags, "--amount-atoms")
                .ok_or("missing --amount-atoms")?
                .parse::<u64>()
                .map_err(|_| "--amount-atoms must be a u64".to_string())?;
            let pftl_recipient =
                flag_value(flags, "--pftl-recipient").ok_or("missing --pftl-recipient")?;
            let nonce = flag_value(flags, "--nonce").ok_or("missing --nonce")?;
            let asset_id = flag_value(flags, "--asset-id").ok_or("missing --asset-id")?;
            let policy_hash = flag_value(flags, "--policy-hash").ok_or("missing --policy-hash")?;
            let route_epoch = flag_value(flags, "--route-epoch")
                .ok_or("missing --route-epoch")?
                .parse::<u32>()
                .map_err(|_| "--route-epoch must be a u32".to_string())?;
            let expires_at_height = flag_value(flags, "--expires-at-height")
                .map(|value| {
                    value
                        .parse::<u64>()
                        .map_err(|_| "--expires-at-height must be a u64".to_string())
                })
                .transpose()?;
            let report = vault_bridge_deposit_intent(VaultBridgeDepositIntentOptions {
                source_chain_id,
                vault_address: vault_address.to_string(),
                token_address: token_address.to_string(),
                depositor: depositor.to_string(),
                amount_atoms,
                pftl_recipient: pftl_recipient.to_string(),
                nonce: nonce.to_string(),
                asset_id: asset_id.to_string(),
                policy_hash: policy_hash.to_string(),
                route_epoch,
                proposer: flag_value(flags, "--proposer").map(str::to_string),
                expires_at_height,
                bundle_dir: flag_value(flags, "--bundle").map(PathBuf::from),
            })
            .map_err(|error| format!("vault-bridge-deposit-intent failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report).map_err(|error| {
                format!("vault bridge asset deposit intent serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
        "vault-bridge-deposit-plan" => {
            let log_file = flag_value(flags, "--log-file").map(PathBuf::from);
            let receipt_file = flag_value(flags, "--receipt-file").map(PathBuf::from);
            if log_file.is_none() && receipt_file.is_none() {
                return Err("missing --log-file or --receipt-file".to_string());
            }
            if log_file.is_some() && receipt_file.is_some() {
                return Err("use only one of --log-file or --receipt-file".to_string());
            }
            let asset_id = flag_value(flags, "--asset-id").ok_or("missing --asset-id")?;
            let policy_hash = flag_value(flags, "--policy-hash").ok_or("missing --policy-hash")?;
            let proposer = flag_value(flags, "--proposer").ok_or("missing --proposer")?;
            let finalizer = flag_value(flags, "--finalizer").unwrap_or(proposer);
            let claimer = flag_value(flags, "--claimer").unwrap_or(proposer);
            let expires_at_height = flag_value(flags, "--expires-at-height")
                .ok_or("missing --expires-at-height")?
                .parse::<u64>()
                .map_err(|_| "--expires-at-height must be a u64".to_string())?;
            let observer_confirmation_depth = flag_value(flags, "--observer-confirmation-depth")
                .map(|value| {
                    value
                        .parse::<u64>()
                        .map_err(|_| "--observer-confirmation-depth must be a u64".to_string())
                })
                .transpose()?;
            let report = vault_bridge_deposit_plan(VaultBridgeDepositPlanOptions {
                log_file,
                receipt_file,
                vault_address: flag_value(flags, "--vault-address").map(str::to_string),
                token_address: flag_value(flags, "--token-address").map(str::to_string),
                asset_id: asset_id.to_string(),
                policy_hash: policy_hash.to_string(),
                proposer: proposer.to_string(),
                finalizer: finalizer.to_string(),
                claimer: claimer.to_string(),
                attestor: flag_value(flags, "--attestor").map(str::to_string),
                observer_confirmation_depth,
                expires_at_height,
                source_proof_kind: flag_value(flags, "--source-proof-kind").map(str::to_string),
                source_proof_hash: flag_value(flags, "--source-proof-hash").map(str::to_string),
                source_public_values_hash: flag_value(flags, "--source-public-values-hash")
                    .map(str::to_string),
                source_proof_file: flag_value(flags, "--source-proof-file").map(PathBuf::from),
                source_public_values_file: flag_value(flags, "--source-public-values-file")
                    .map(PathBuf::from),
            })
            .map_err(|error| format!("vault-bridge-deposit-plan failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report).map_err(|error| {
                format!("vault bridge asset bridge deposit plan serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
        "vault-bridge-deposit-relay-bundle" => {
            let log_file = flag_value(flags, "--log-file").map(PathBuf::from);
            let receipt_file = flag_value(flags, "--receipt-file").map(PathBuf::from);
            if log_file.is_none() && receipt_file.is_none() {
                return Err("missing --log-file or --receipt-file".to_string());
            }
            if log_file.is_some() && receipt_file.is_some() {
                return Err("use only one of --log-file or --receipt-file".to_string());
            }
            let asset_id = flag_value(flags, "--asset-id").ok_or("missing --asset-id")?;
            let policy_hash = flag_value(flags, "--policy-hash").ok_or("missing --policy-hash")?;
            let proposer = flag_value(flags, "--proposer").ok_or("missing --proposer")?;
            let finalizer = flag_value(flags, "--finalizer").unwrap_or(proposer);
            let claimer = flag_value(flags, "--claimer").unwrap_or(proposer);
            let expires_at_height = flag_value(flags, "--expires-at-height")
                .ok_or("missing --expires-at-height")?
                .parse::<u64>()
                .map_err(|_| "--expires-at-height must be a u64".to_string())?;
            let observer_confirmation_depth = flag_value(flags, "--observer-confirmation-depth")
                .map(|value| {
                    value
                        .parse::<u64>()
                        .map_err(|_| "--observer-confirmation-depth must be a u64".to_string())
                })
                .transpose()?;
            let bundle_dir = flag_value(flags, "--bundle").ok_or("missing --bundle")?;
            let report = vault_bridge_deposit_relay_bundle(VaultBridgeDepositRelayBundleOptions {
                plan_options: VaultBridgeDepositPlanOptions {
                    log_file,
                    receipt_file,
                    vault_address: flag_value(flags, "--vault-address").map(str::to_string),
                    token_address: flag_value(flags, "--token-address").map(str::to_string),
                    asset_id: asset_id.to_string(),
                    policy_hash: policy_hash.to_string(),
                    proposer: proposer.to_string(),
                    finalizer: finalizer.to_string(),
                    claimer: claimer.to_string(),
                    attestor: flag_value(flags, "--attestor").map(str::to_string),
                    observer_confirmation_depth,
                    expires_at_height,
                    source_proof_kind: flag_value(flags, "--source-proof-kind").map(str::to_string),
                    source_proof_hash: flag_value(flags, "--source-proof-hash").map(str::to_string),
                    source_public_values_hash: flag_value(flags, "--source-public-values-hash")
                        .map(str::to_string),
                    source_proof_file: flag_value(flags, "--source-proof-file")
                        .map(PathBuf::from),
                    source_public_values_file: flag_value(flags, "--source-public-values-file")
                        .map(PathBuf::from),
                },
                bundle_dir: PathBuf::from(bundle_dir),
                overwrite: flag_present(flags, "--overwrite"),
            })
            .map_err(|error| format!("vault-bridge-deposit-relay-bundle failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report).map_err(|error| {
                format!("vault bridge asset deposit relay bundle serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
        "vault-bridge-deposit-relay-rpc-bundle" => {
            let source_rpc_url =
                flag_value(flags, "--source-rpc-url").ok_or("missing --source-rpc-url")?;
            let tx_hash = flag_value(flags, "--tx-hash").ok_or("missing --tx-hash")?;
            let asset_id = flag_value(flags, "--asset-id").ok_or("missing --asset-id")?;
            let policy_hash = flag_value(flags, "--policy-hash").ok_or("missing --policy-hash")?;
            let proposer = flag_value(flags, "--proposer").ok_or("missing --proposer")?;
            let finalizer = flag_value(flags, "--finalizer").unwrap_or(proposer);
            let claimer = flag_value(flags, "--claimer").unwrap_or(proposer);
            let expires_at_height = flag_value(flags, "--expires-at-height")
                .ok_or("missing --expires-at-height")?
                .parse::<u64>()
                .map_err(|_| "--expires-at-height must be a u64".to_string())?;
            let bundle_dir = flag_value(flags, "--bundle").ok_or("missing --bundle")?;
            let report =
                vault_bridge_deposit_relay_rpc_bundle(VaultBridgeDepositRelayRpcBundleOptions {
                    source_rpc_url: source_rpc_url.to_string(),
                    tx_hash: tx_hash.to_string(),
                    cast_binary: flag_value(flags, "--cast-bin")
                        .unwrap_or("cast")
                        .to_string(),
                    plan_options: VaultBridgeDepositPlanOptions {
                        log_file: None,
                        receipt_file: None,
                        vault_address: flag_value(flags, "--vault-address").map(str::to_string),
                        token_address: flag_value(flags, "--token-address").map(str::to_string),
                        asset_id: asset_id.to_string(),
                        policy_hash: policy_hash.to_string(),
                        proposer: proposer.to_string(),
                        finalizer: finalizer.to_string(),
                        claimer: claimer.to_string(),
                        attestor: flag_value(flags, "--attestor").map(str::to_string),
                        observer_confirmation_depth: None,
                        expires_at_height,
                        source_proof_kind: flag_value(flags, "--source-proof-kind")
                            .map(str::to_string),
                        source_proof_hash: flag_value(flags, "--source-proof-hash")
                            .map(str::to_string),
                        source_public_values_hash: flag_value(flags, "--source-public-values-hash")
                            .map(str::to_string),
                        source_proof_file: flag_value(flags, "--source-proof-file")
                            .map(PathBuf::from),
                        source_public_values_file: flag_value(
                            flags,
                            "--source-public-values-file",
                        )
                        .map(PathBuf::from),
                    },
                    bundle_dir: PathBuf::from(bundle_dir),
                    overwrite: flag_present(flags, "--overwrite"),
                })
                .map_err(|error| {
                    format!("vault-bridge-deposit-relay-rpc-bundle failed: {error}")
                })?;
            let json = serde_json::to_string_pretty(&report).map_err(|error| {
                format!("vault bridge asset RPC deposit relay bundle serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
        "vault-bridge-burn-to-redeem-bundle" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let owner = flag_value(flags, "--owner").ok_or("missing --owner")?;
            let asset_id = flag_value(flags, "--asset-id").ok_or("missing --asset-id")?;
            let amount_atoms = flag_value(flags, "--amount-atoms")
                .ok_or("missing --amount-atoms")?
                .parse::<u64>()
                .map_err(|_| "--amount-atoms must be a u64".to_string())?;
            let destination_ref =
                flag_value(flags, "--destination-ref").ok_or("missing --destination-ref")?;
            let bundle_dir = flag_value(flags, "--bundle").ok_or("missing --bundle")?;
            let epoch = flag_value(flags, "--epoch")
                .map(|value| {
                    value
                        .parse::<u64>()
                        .map_err(|_| "--epoch must be a u64".to_string())
                })
                .transpose()?;
            let report = vault_bridge_burn_to_redeem_bundle(VaultBridgeBurnToRedeemBundleOptions {
                data_dir: PathBuf::from(data_dir),
                owner: owner.to_string(),
                issuer: flag_value(flags, "--issuer").map(str::to_string),
                asset_id: asset_id.to_string(),
                bucket_id: flag_value(flags, "--bucket-id").map(str::to_string),
                amount_atoms,
                epoch,
                reserve_packet_hash: flag_value(flags, "--reserve-packet-hash").map(str::to_string),
                destination_ref: destination_ref.to_string(),
                bundle_dir: PathBuf::from(bundle_dir),
                overwrite: flag_present(flags, "--overwrite"),
            })
            .map_err(|error| format!("vault-bridge-burn-to-redeem-bundle failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report).map_err(|error| {
                format!("vault bridge burn-to-redeem bundle serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
        "vault-bridge-withdrawal-plan" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let asset_id = flag_value(flags, "--asset-id").ok_or("missing --asset-id")?;
            let redemption_id =
                flag_value(flags, "--redemption-id").ok_or("missing --redemption-id")?;
            let pftl_finalized_height = flag_value(flags, "--pftl-finalized-height")
                .map(|value| {
                    value
                        .parse::<u64>()
                        .map_err(|_| "--pftl-finalized-height must be a u64".to_string())
                })
                .transpose()?;
            let evm_chain_id = flag_value(flags, "--evm-chain-id")
                .map(|value| {
                    value
                        .parse::<u64>()
                        .map_err(|_| "--evm-chain-id must be a u64".to_string())
                })
                .transpose()?;
            let report = vault_bridge_withdrawal_plan(VaultBridgeWithdrawalPlanOptions {
                data_dir: PathBuf::from(data_dir),
                asset_id: asset_id.to_string(),
                redemption_id: redemption_id.to_string(),
                pftl_finalized_height,
                evm_chain_id,
                verifier_address: flag_value(flags, "--verifier-address").map(str::to_string),
                signatures_file: flag_value(flags, "--signatures-file").map(PathBuf::from),
            })
            .map_err(|error| format!("vault-bridge-withdrawal-plan failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report).map_err(|error| {
                format!("vault bridge asset withdrawal plan serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
        "vault-bridge-withdrawal-signature-bundle" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let asset_id = flag_value(flags, "--asset-id").ok_or("missing --asset-id")?;
            let redemption_id =
                flag_value(flags, "--redemption-id").ok_or("missing --redemption-id")?;
            let bundle_dir = flag_value(flags, "--bundle").ok_or("missing --bundle")?;
            let pftl_finalized_height = flag_value(flags, "--pftl-finalized-height")
                .map(|value| {
                    value
                        .parse::<u64>()
                        .map_err(|_| "--pftl-finalized-height must be a u64".to_string())
                })
                .transpose()?;
            let evm_chain_id = flag_value(flags, "--evm-chain-id")
                .ok_or("missing --evm-chain-id")?
                .parse::<u64>()
                .map_err(|_| "--evm-chain-id must be a u64".to_string())?;
            let verifier_address =
                flag_value(flags, "--verifier-address").ok_or("missing --verifier-address")?;
            let report = vault_bridge_withdrawal_signature_bundle(
                VaultBridgeWithdrawalSignatureBundleOptions {
                    plan_options: VaultBridgeWithdrawalPlanOptions {
                        data_dir: PathBuf::from(data_dir),
                        asset_id: asset_id.to_string(),
                        redemption_id: redemption_id.to_string(),
                        pftl_finalized_height,
                        evm_chain_id: Some(evm_chain_id),
                        verifier_address: Some(verifier_address.to_string()),
                        signatures_file: None,
                    },
                    bundle_dir: PathBuf::from(bundle_dir),
                    relay_bundle_dir: flag_value(flags, "--relay-bundle").map(PathBuf::from),
                    overwrite: flag_present(flags, "--overwrite"),
                },
            )
            .map_err(|error| format!("vault-bridge-withdrawal-signature-bundle failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report).map_err(|error| {
                format!(
                    "vault bridge asset withdrawal signature bundle serialization failed: {error}"
                )
            })?;
            println!("{json}");
            Ok(())
        }
        "vault-bridge-withdrawal-relay-bundle" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let asset_id = flag_value(flags, "--asset-id").ok_or("missing --asset-id")?;
            let redemption_id =
                flag_value(flags, "--redemption-id").ok_or("missing --redemption-id")?;
            let bundle_dir = flag_value(flags, "--bundle").ok_or("missing --bundle")?;
            let pftl_finalized_height = flag_value(flags, "--pftl-finalized-height")
                .map(|value| {
                    value
                        .parse::<u64>()
                        .map_err(|_| "--pftl-finalized-height must be a u64".to_string())
                })
                .transpose()?;
            let evm_chain_id = flag_value(flags, "--evm-chain-id")
                .map(|value| {
                    value
                        .parse::<u64>()
                        .map_err(|_| "--evm-chain-id must be a u64".to_string())
                })
                .transpose()?;
            let report =
                vault_bridge_withdrawal_relay_bundle(VaultBridgeWithdrawalRelayBundleOptions {
                    plan_options: VaultBridgeWithdrawalPlanOptions {
                        data_dir: PathBuf::from(data_dir),
                        asset_id: asset_id.to_string(),
                        redemption_id: redemption_id.to_string(),
                        pftl_finalized_height,
                        evm_chain_id,
                        verifier_address: flag_value(flags, "--verifier-address")
                            .map(str::to_string),
                        signatures_file: flag_value(flags, "--signatures-file").map(PathBuf::from),
                    },
                    bundle_dir: PathBuf::from(bundle_dir),
                    overwrite: flag_present(flags, "--overwrite"),
                })
                .map_err(|error| format!("vault-bridge-withdrawal-relay-bundle failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report).map_err(|error| {
                format!("vault bridge asset withdrawal relay bundle serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
        "vault-bridge-export-reserve-packet" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let asset_id = flag_value(flags, "--asset-id").ok_or("missing --asset-id")?;
            let epoch = flag_value(flags, "--epoch")
                .ok_or("missing --epoch")?
                .parse::<u64>()
                .map_err(|_| "--epoch must be a u64".to_string())?;
            let bundle_dir = flag_value(flags, "--bundle").ok_or("missing --bundle")?;
            let bundle = export_vault_bridge_reserve_replay_bundle(
                VaultBridgeReserveReplayBundleExportOptions {
                    data_dir: PathBuf::from(data_dir),
                    asset_id: asset_id.to_string(),
                    epoch,
                    bundle_dir: PathBuf::from(bundle_dir),
                    overwrite: flag_present(flags, "--overwrite"),
                },
            )
            .map_err(|error| format!("vault-bridge-export-reserve-packet failed: {error}"))?;
            let json = serde_json::to_string_pretty(&bundle).map_err(|error| {
                format!("vault bridge asset reserve replay bundle serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
        "vault-bridge-replay-reserve-packet" => {
            let bundle_dir = flag_value(flags, "--bundle").ok_or("missing --bundle")?;
            let report =
                replay_vault_bridge_reserve_bundle(VaultBridgeReserveReplayBundleVerifyOptions {
                    bundle_dir: PathBuf::from(bundle_dir),
                })
                .map_err(|error| format!("vault-bridge-replay-reserve-packet failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report).map_err(|error| {
                format!("vault bridge asset reserve replay report serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
        "verify-blocks" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let report = verify_blocks(NodeOptions {
                data_dir: PathBuf::from(data_dir),
            })
            .map_err(|error| format!("verify-blocks failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report)
                .map_err(|error| format!("block verification serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "verify-state" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let report = verify_state(NodeOptions {
                data_dir: PathBuf::from(data_dir),
            })
            .map_err(|error| format!("verify-state failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report)
                .map_err(|error| format!("state verification serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "verify-governance" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let cobalt_mode = flag_value(flags, "--cobalt-mode")
                .unwrap_or("canonical")
                .to_string();
            let trust_graph_root = flag_value(flags, "--trust-graph-root").map(str::to_string);
            let report = verify_governance_with_options(GovernanceVerifyOptions {
                data_dir: PathBuf::from(data_dir),
                cobalt_mode,
                trust_graph_root,
            })
            .map_err(|error| format!("verify-governance failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report).map_err(|error| {
                format!("governance verification serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
        "verify-bridge" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let report = verify_bridge(NodeOptions {
                data_dir: PathBuf::from(data_dir),
            })
            .map_err(|error| format!("verify-bridge failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report)
                .map_err(|error| format!("bridge verification serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "verify-mempool" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let report = verify_mempool(NodeOptions {
                data_dir: PathBuf::from(data_dir),
            })
            .map_err(|error| format!("verify-mempool failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report)
                .map_err(|error| format!("mempool verification serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "verify-shielded" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let report = verify_shielded(NodeOptions {
                data_dir: PathBuf::from(data_dir),
            })
            .map_err(|error| format!("verify-shielded failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report)
                .map_err(|error| format!("shielded verification serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "orchard-action" => {
            let apply = flag_present(flags, "--apply");
            if apply {
                require_direct_state_enabled("orchard-action --apply")?;
            }
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let action_file = flag_value(flags, "--action-file").ok_or("missing --action-file")?;
            let report = verify_or_apply_orchard_action(OrchardActionOptions {
                data_dir: PathBuf::from(data_dir),
                action_file: PathBuf::from(action_file),
                apply,
            })
            .map_err(|error| format!("orchard-action failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report)
                .map_err(|error| format!("Orchard action report serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "orchard-operator-policy" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let privacy_enabled = flag_present(flags, "--privacy-enabled");
            let max_concurrent_verifiers = flag_value(flags, "--max-concurrent-verifiers")
                .unwrap_or("1")
                .parse::<usize>()
                .map_err(|_| "--max-concurrent-verifiers must be a usize".to_string())?;
            let verifier_timeout_ms = flag_value(flags, "--verifier-timeout-ms")
                .unwrap_or("30000")
                .parse::<u64>()
                .map_err(|_| "--verifier-timeout-ms must be a u64".to_string())?;
            let root_retention_roots = flag_value(flags, "--root-retention-roots")
                .unwrap_or("50000")
                .parse::<u64>()
                .map_err(|_| "--root-retention-roots must be a u64".to_string())?;
            let indexing_role = flag_value(flags, "--indexing-role")
                .unwrap_or("local")
                .to_string();
            let report = orchard_operator_policy(OrchardOperatorPolicyOptions {
                data_dir: PathBuf::from(data_dir),
                privacy_enabled,
                max_concurrent_verifiers,
                verifier_timeout_ms,
                root_retention_roots,
                indexing_role,
            })
            .map_err(|error| format!("orchard-operator-policy failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report).map_err(|error| {
                format!("Orchard operator policy serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
        "orchard-fee-resource-policy" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let report = orchard_fee_resource_policy(OrchardFeeResourcePolicyOptions {
                data_dir: PathBuf::from(data_dir),
            })
            .map_err(|error| format!("orchard-fee-resource-policy failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report).map_err(|error| {
                format!("Orchard fee/resource policy serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
        "orchard-frontier-cache-warm" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let report = orchard_frontier_cache_warm(NodeOptions {
                data_dir: PathBuf::from(data_dir),
            })
            .map_err(|error| format!("orchard-frontier-cache-warm failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report).map_err(|error| {
                format!("Orchard frontier cache warm serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
        "orchard-pool-report" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let report = orchard_pool_report(OrchardPoolReportOptions {
                data_dir: PathBuf::from(data_dir),
            })
            .map_err(|error| format!("orchard-pool-report failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report)
                .map_err(|error| format!("Orchard pool report serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "orchard-output-create" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let action_file = flag_value(flags, "--action-file").ok_or("missing --action-file")?;
            let recipient_address_raw_hex =
                flag_value(flags, "--recipient-address-raw-hex").map(str::to_string);
            let recipient_key_file = flag_value(flags, "--recipient-key-file").map(PathBuf::from);
            let recipient_view_key_file =
                flag_value(flags, "--recipient-view-key-file").map(PathBuf::from);
            let memo_hex = flag_value(flags, "--memo-hex").map(str::to_string);
            let value = flag_value(flags, "--value")
                .unwrap_or("0")
                .parse::<u64>()
                .map_err(|_| "--value must be a u64".to_string())?;
            let fee = flag_value(flags, "--fee")
                .unwrap_or("0")
                .parse::<u64>()
                .map_err(|_| "--fee must be a u64".to_string())?;
            let report = create_orchard_output_action(OrchardOutputActionOptions {
                data_dir: PathBuf::from(data_dir),
                recipient_address_raw_hex,
                recipient_key_file,
                recipient_view_key_file,
                memo_hex,
                value,
                fee,
                action_file: PathBuf::from(action_file),
                overwrite: flag_present(flags, "--overwrite"),
            })
            .map_err(|error| format!("orchard-output-create failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report).map_err(|error| {
                format!("Orchard output action report serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
        "orchard-deposit-create" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let deposit_file =
                flag_value(flags, "--deposit-file").ok_or("missing --deposit-file")?;
            let key_file = flag_value(flags, "--key-file").map(PathBuf::from);
            let recipient_address_raw_hex =
                flag_value(flags, "--recipient-address-raw-hex").map(str::to_string);
            let recipient_key_file = flag_value(flags, "--recipient-key-file").map(PathBuf::from);
            let recipient_view_key_file =
                flag_value(flags, "--recipient-view-key-file").map(PathBuf::from);
            let memo_hex = flag_value(flags, "--memo-hex").map(str::to_string);
            let amount = flag_value(flags, "--amount")
                .ok_or("missing --amount")?
                .parse::<u64>()
                .map_err(|_| "--amount must be a u64".to_string())?;
            let fee = flag_value(flags, "--fee")
                .unwrap_or("0")
                .parse::<u64>()
                .map_err(|_| "--fee must be a u64".to_string())?;
            let policy_id = flag_value(flags, "--policy-id").map(str::to_string);
            let disclosure_hash = flag_value(flags, "--disclosure-hash").map(str::to_string);
            let report = create_orchard_deposit_action(OrchardDepositActionOptions {
                data_dir: PathBuf::from(data_dir),
                key_file,
                recipient_address_raw_hex,
                recipient_key_file,
                recipient_view_key_file,
                memo_hex,
                amount,
                fee,
                policy_id,
                disclosure_hash,
                deposit_file: PathBuf::from(deposit_file),
                overwrite: flag_present(flags, "--overwrite"),
            })
            .map_err(|error| format!("orchard-deposit-create failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report).map_err(|error| {
                format!("Orchard deposit action report serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
        "asset-orchard-ingress-create" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let key_file = flag_value(flags, "--key-file").ok_or("missing --key-file")?;
            let asset_id = flag_value(flags, "--asset-id")
                .ok_or("missing --asset-id")?
                .to_string();
            let amount = flag_value(flags, "--amount")
                .ok_or("missing --amount")?
                .parse::<u64>()
                .map_err(|_| "--amount must be a u64".to_string())?;
            let fee = flag_value(flags, "--fee")
                .unwrap_or("0")
                .parse::<u64>()
                .map_err(|_| "--fee must be a u64".to_string())?;
            let note_seed_hex = flag_value(flags, "--note-seed-hex")
                .ok_or("missing --note-seed-hex")?
                .to_string();
            let encrypted_output_hex =
                flag_value(flags, "--encrypted-output-hex").map(str::to_string);
            let ingress_file =
                flag_value(flags, "--ingress-file").ok_or("missing --ingress-file")?;
            let note_file = flag_value(flags, "--note-file").ok_or("missing --note-file")?;
            let report = create_asset_orchard_ingress(AssetOrchardIngressCreateOptions {
                data_dir: PathBuf::from(data_dir),
                key_file: PathBuf::from(key_file),
                asset_id,
                amount,
                fee,
                note_seed_hex,
                encrypted_output_hex,
                ingress_file: PathBuf::from(ingress_file),
                note_file: PathBuf::from(note_file),
                overwrite: flag_present(flags, "--overwrite"),
            })
            .map_err(|error| format!("asset-orchard-ingress-create failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report).map_err(|error| {
                format!("AssetOrchard ingress report serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
        "asset-orchard-egress-create" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let note_file = flag_value(flags, "--note-file").ok_or("missing --note-file")?;
            let to = flag_value(flags, "--to").ok_or("missing --to")?.to_string();
            let amount = flag_value(flags, "--amount")
                .map(|value| {
                    value
                        .parse::<u64>()
                        .map_err(|_| "--amount must be a u64".to_string())
                })
                .transpose()?;
            let egress_file = flag_value(flags, "--egress-file").ok_or("missing --egress-file")?;
            let report = create_asset_orchard_egress(AssetOrchardEgressCreateOptions {
                data_dir: PathBuf::from(data_dir),
                note_file: PathBuf::from(note_file),
                to,
                amount,
                egress_file: PathBuf::from(egress_file),
                overwrite: flag_present(flags, "--overwrite"),
            })
            .map_err(|error| format!("asset-orchard-egress-create failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report).map_err(|error| {
                format!("AssetOrchard egress report serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
        "asset-orchard-private-egress-create" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let note_file = flag_value(flags, "--note-file").ok_or("missing --note-file")?;
            let to = flag_value(flags, "--to").ok_or("missing --to")?.to_string();
            let asset_id = flag_value(flags, "--asset-id").map(str::to_string);
            let amount = flag_value(flags, "--amount")
                .map(|value| {
                    value
                        .parse::<u64>()
                        .map_err(|_| "--amount must be a u64".to_string())
                })
                .transpose()?;
            let fee = flag_value(flags, "--fee")
                .unwrap_or("0")
                .parse::<u64>()
                .map_err(|_| "--fee must be a u64".to_string())?;
            let policy_id = flag_value(flags, "--policy-id")
                .ok_or("missing --policy-id")?
                .to_string();
            let disclosure_hash = flag_value(flags, "--disclosure-hash")
                .ok_or("missing --disclosure-hash")?
                .to_string();
            let egress_file = flag_value(flags, "--egress-file").ok_or("missing --egress-file")?;
            let report =
                create_asset_orchard_private_egress(AssetOrchardPrivateEgressCreateOptions {
                    data_dir: PathBuf::from(data_dir),
                    note_file: PathBuf::from(note_file),
                    to,
                    asset_id,
                    amount,
                    fee,
                    policy_id,
                    disclosure_hash,
                    egress_file: PathBuf::from(egress_file),
                    overwrite: flag_present(flags, "--overwrite"),
                })
                .map_err(|error| format!("asset-orchard-private-egress-create failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report).map_err(|error| {
                format!("AssetOrchard private egress report serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
        "asset-orchard-note-status" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let note_file = flag_value(flags, "--note-file").ok_or("missing --note-file")?;
            let report = asset_orchard_note_status(AssetOrchardNoteStatusOptions {
                data_dir: PathBuf::from(data_dir),
                note_file: PathBuf::from(note_file),
            })
            .map_err(|error| format!("asset-orchard-note-status failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report).map_err(|error| {
                format!("AssetOrchard note status report serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
        "asset-orchard-scan" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let note_seed_hex = flag_value(flags, "--note-seed-hex")
                .ok_or("missing --note-seed-hex")?
                .to_string();
            let note_file = flag_value(flags, "--note-file").ok_or("missing --note-file")?;
            let report = asset_orchard_scan(AssetOrchardScanOptions {
                data_dir: PathBuf::from(data_dir),
                note_seed_hex,
                note_file: PathBuf::from(note_file),
                overwrite: flag_present(flags, "--overwrite"),
            })
            .map_err(|error| format!("asset-orchard-scan failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report).map_err(|error| {
                format!("AssetOrchard scan report serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
        "asset-orchard-swap-create" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let input_note_file_a =
                flag_value(flags, "--input-note-file-a").ok_or("missing --input-note-file-a")?;
            let input_note_file_b =
                flag_value(flags, "--input-note-file-b").ok_or("missing --input-note-file-b")?;
            let output_note_seed_hex_a = flag_value(flags, "--output-note-seed-hex-a")
                .ok_or("missing --output-note-seed-hex-a")?
                .to_string();
            let output_note_seed_hex_b = flag_value(flags, "--output-note-seed-hex-b")
                .ok_or("missing --output-note-seed-hex-b")?
                .to_string();
            let action_file = flag_value(flags, "--action-file").ok_or("missing --action-file")?;
            let pricing_claim_file =
                flag_value(flags, "--pricing-claim-file").ok_or("missing --pricing-claim-file")?;
            let output_note_file_a =
                flag_value(flags, "--output-note-file-a").ok_or("missing --output-note-file-a")?;
            let output_note_file_b =
                flag_value(flags, "--output-note-file-b").ok_or("missing --output-note-file-b")?;
            let report = create_asset_orchard_swap_action(AssetOrchardSwapCreateOptions {
                data_dir: PathBuf::from(data_dir),
                input_note_files: [
                    PathBuf::from(input_note_file_a),
                    PathBuf::from(input_note_file_b),
                ],
                output_note_seed_hexes: [output_note_seed_hex_a, output_note_seed_hex_b],
                pricing_claim_file: PathBuf::from(pricing_claim_file),
                action_file: PathBuf::from(action_file),
                output_note_files: [
                    PathBuf::from(output_note_file_a),
                    PathBuf::from(output_note_file_b),
                ],
                overwrite: flag_present(flags, "--overwrite"),
            })
            .map_err(|error| format!("asset-orchard-swap-create failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report).map_err(|error| {
                format!("AssetOrchard swap create report serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
        "asset-orchard-swap-live-round" => {
            let total_start = Instant::now();
            let data_dir =
                PathBuf::from(flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR));
            let topology_file =
                PathBuf::from(flag_value(flags, "--topology").ok_or("missing --topology")?);
            let input_note_file_a =
                flag_value(flags, "--input-note-file-a").ok_or("missing --input-note-file-a")?;
            let input_note_file_b =
                flag_value(flags, "--input-note-file-b").ok_or("missing --input-note-file-b")?;
            let output_note_seed_hex_a = flag_value(flags, "--output-note-seed-hex-a")
                .ok_or("missing --output-note-seed-hex-a")?
                .to_string();
            let output_note_seed_hex_b = flag_value(flags, "--output-note-seed-hex-b")
                .ok_or("missing --output-note-seed-hex-b")?
                .to_string();
            let action_file =
                PathBuf::from(flag_value(flags, "--action-file").ok_or("missing --action-file")?);
            let pricing_claim_file = PathBuf::from(
                flag_value(flags, "--pricing-claim-file").ok_or("missing --pricing-claim-file")?,
            );
            let output_note_file_a = PathBuf::from(
                flag_value(flags, "--output-note-file-a").ok_or("missing --output-note-file-a")?,
            );
            let output_note_file_b = PathBuf::from(
                flag_value(flags, "--output-note-file-b").ok_or("missing --output-note-file-b")?,
            );
            let batch_file =
                PathBuf::from(flag_value(flags, "--batch-file").ok_or("missing --batch-file")?);
            let key_file =
                PathBuf::from(flag_value(flags, "--key-file").ok_or("missing --key-file")?);
            let proposal_key_file = flag_value(flags, "--proposal-key-file")
                .map(PathBuf::from)
                .or_else(|| Some(key_file.clone()));
            let require_local_proposer = flag_present(flags, "--require-local-proposer");
            let require_signed_proposal = !flag_present(flags, "--allow-unsigned-proposal");
            let allow_peer_failures = flag_present(flags, "--allow-peer-failures");
            let quorum_early_full_propagation =
                flag_present(flags, "--quorum-early-full-propagation");
            let local_apply_before_certified_send =
                flag_present(flags, "--local-apply-before-certified-send");
            let defer_certified_sends = flag_present(flags, "--defer-certified-sends");
            let artifact_dir =
                PathBuf::from(flag_value(flags, "--artifact-dir").ok_or("missing --artifact-dir")?);
            let block_height = flag_value(flags, "--height")
                .map(|value| {
                    value
                        .parse::<u64>()
                        .map_err(|_| "--height must be a u64".to_string())
                })
                .transpose()?;
            let view = flag_value(flags, "--view")
                .map(|value| {
                    value
                        .parse::<u64>()
                        .map_err(|_| "--view must be a u64".to_string())
                })
                .transpose()?;
            let timeout_certificate_file =
                flag_value(flags, "--timeout-certificate-file").map(PathBuf::from);
            let timeout_ms = flag_value(flags, "--timeout-ms")
                .unwrap_or("5000")
                .parse::<u64>()
                .map_err(|_| "--timeout-ms must be a u64".to_string())?;
            let send_retries = flag_value(flags, "--send-retries")
                .unwrap_or("0")
                .parse::<usize>()
                .map_err(|_| "--send-retries must be a usize".to_string())?;
            let retry_backoff_ms = flag_value(flags, "--retry-backoff-ms")
                .unwrap_or("250")
                .parse::<u64>()
                .map_err(|_| "--retry-backoff-ms must be a u64".to_string())?;
            let report_file = flag_value(flags, "--report-file").map(PathBuf::from);
            let prewarm_prover_cache = !flag_present(flags, "--no-prewarm-prover-cache");
            let prewarm_ready_file = flag_value(flags, "--prewarm-ready-file").map(PathBuf::from);
            let start_signal_file = flag_value(flags, "--start-signal-file").map(PathBuf::from);
            let start_signal_timeout_ms = flag_value(flags, "--start-signal-timeout-ms")
                .unwrap_or("0")
                .parse::<u64>()
                .map_err(|_| "--start-signal-timeout-ms must be a u64".to_string())?;

            let prewarm_prover_cache_ms = if prewarm_prover_cache {
                let prewarm_start = Instant::now();
                AssetOrchardSwapProvingKey::cached().map_err(|error| {
                    format!("AssetOrchard swap proving-key prewarm failed: {error}")
                })?;
                AssetOrchardSwapVerifyingKey::cached().map_err(|error| {
                    format!("AssetOrchard swap verifying-key prewarm failed: {error}")
                })?;
                Some(monotonic_elapsed_ms(prewarm_start))
            } else {
                None
            };

            if let Some(path) = prewarm_ready_file.as_ref() {
                write_json_file(
                    path,
                    &serde_json::json!({
                        "schema": "postfiat-asset-orchard-swap-live-round-prewarm-ready-v1",
                        "prewarm_prover_cache": prewarm_prover_cache,
                        "prewarm_prover_cache_ms": prewarm_prover_cache_ms,
                    }),
                )?;
            }
            if let Some(path) = start_signal_file.as_ref() {
                wait_for_asset_orchard_swap_start_signal(path, start_signal_timeout_ms)?;
            }

            let swap_create_start = Instant::now();
            let (swap_create, verified_action) =
                create_asset_orchard_swap_action_verified(AssetOrchardSwapCreateOptions {
                    data_dir: data_dir.clone(),
                    input_note_files: [
                        PathBuf::from(input_note_file_a),
                        PathBuf::from(input_note_file_b),
                    ],
                    output_note_seed_hexes: [output_note_seed_hex_a, output_note_seed_hex_b],
                    pricing_claim_file,
                    action_file: action_file.clone(),
                    output_note_files: [output_note_file_a.clone(), output_note_file_b.clone()],
                    overwrite: flag_present(flags, "--overwrite"),
                })
                .map_err(|error| {
                    format!("asset-orchard-swap-live-round action create failed: {error}")
                })?;
            let swap_create_ms = monotonic_elapsed_ms(swap_create_start);

            let batch_wrap_start = Instant::now();
            let batch = create_verified_asset_orchard_swap_action_batch(
                data_dir.clone(),
                &verified_action,
                batch_file.clone(),
            )
            .map_err(|error| {
                format!("asset-orchard-swap-live-round batch create failed: {error}")
            })?;
            let batch_wrap_ms = monotonic_elapsed_ms(batch_wrap_start);

            let transport_start = Instant::now();
            let transport =
                transport_peer_certified_batch_round(TransportPeerCertifiedBatchRoundOptions {
                    data_dir: data_dir.clone(),
                    topology_file: topology_file.clone(),
                    batch_kind: Some("shielded".to_string()),
                    batch_file: batch_file.clone(),
                    key_file,
                    proposal_key_file,
                    require_local_proposer,
                    require_signed_proposal,
                    allow_peer_failures,
                    quorum_early_full_propagation,
                    artifact_dir: artifact_dir.clone(),
                    block_height,
                    view,
                    timeout_certificate_file,
                    timeout_ms,
                    send_retries,
                    retry_backoff_ms,
                    local_apply_before_certified_send,
                    defer_certified_sends,
                    required_parent: None,
                })?;
            let transport_ms = monotonic_elapsed_ms(transport_start);
            let round_ok = transport.round_ok;

            let report = AssetOrchardSwapLiveRoundReport {
                schema: "postfiat-asset-orchard-swap-live-round-v1".to_string(),
                data_dir: data_dir.display().to_string(),
                topology_file: topology_file.display().to_string(),
                action_file: action_file.display().to_string(),
                batch_file: batch_file.display().to_string(),
                output_note_files: [
                    output_note_file_a.display().to_string(),
                    output_note_file_b.display().to_string(),
                ],
                artifact_dir: artifact_dir.display().to_string(),
                prewarm_prover_cache,
                duplicate_local_verification_skipped: true,
                swap_create,
                batch_id: batch.batch_id,
                batch_action_count: batch.actions.len(),
                transport,
                timings: AssetOrchardSwapLiveRoundTimingReport {
                    total_ms: monotonic_elapsed_ms(total_start),
                    prewarm_prover_cache_ms,
                    swap_create_ms,
                    batch_wrap_ms,
                    transport_ms,
                },
                report_file: report_file.as_ref().map(|path| path.display().to_string()),
                round_ok,
            };
            if let Some(path) = report_file.as_ref() {
                write_json_file(path, &report)?;
            }
            let json = serde_json::to_string_pretty(&report).map_err(|error| {
                format!("AssetOrchard swap live round report serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
        "orchard-spend-create" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let action_file = flag_value(flags, "--action-file").ok_or("missing --action-file")?;
            let spending_key_hex = flag_value(flags, "--spending-key-hex").map(str::to_string);
            let key_file = flag_value(flags, "--key-file").map(PathBuf::from);
            let input_output_index = flag_value(flags, "--input-output-index")
                .ok_or("missing --input-output-index")?
                .parse::<usize>()
                .map_err(|_| "--input-output-index must be a usize".to_string())?;
            let amount = flag_value(flags, "--amount")
                .map(|value| {
                    value
                        .parse::<u64>()
                        .map_err(|_| "--amount must be a u64".to_string())
                })
                .transpose()?;
            let recipient_address_raw_hex =
                flag_value(flags, "--recipient-address-raw-hex").map(str::to_string);
            let recipient_key_file = flag_value(flags, "--recipient-key-file").map(PathBuf::from);
            let recipient_view_key_file =
                flag_value(flags, "--recipient-view-key-file").map(PathBuf::from);
            let change_address_raw_hex =
                flag_value(flags, "--change-recipient-address-raw-hex").map(str::to_string);
            let change_key_file =
                flag_value(flags, "--change-recipient-key-file").map(PathBuf::from);
            let change_view_key_file =
                flag_value(flags, "--change-recipient-view-key-file").map(PathBuf::from);
            let memo_hex = flag_value(flags, "--memo-hex").map(str::to_string);
            let fee = flag_value(flags, "--fee")
                .unwrap_or("0")
                .parse::<u64>()
                .map_err(|_| "--fee must be a u64".to_string())?;
            let report = create_orchard_spend_action(OrchardSpendActionOptions {
                data_dir: PathBuf::from(data_dir),
                spending_key_hex,
                key_file,
                input_output_index,
                amount,
                recipient_address_raw_hex,
                recipient_key_file,
                recipient_view_key_file,
                change_address_raw_hex,
                change_key_file,
                change_view_key_file,
                memo_hex,
                fee,
                action_file: PathBuf::from(action_file),
                overwrite: flag_present(flags, "--overwrite"),
            })
            .map_err(|error| format!("orchard-spend-create failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report).map_err(|error| {
                format!("Orchard spend action report serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
        "orchard-withdraw-create" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let action_file = flag_value(flags, "--action-file").ok_or("missing --action-file")?;
            let spending_key_hex = flag_value(flags, "--spending-key-hex").map(str::to_string);
            let key_file = flag_value(flags, "--key-file").map(PathBuf::from);
            let input_output_index = flag_value(flags, "--input-output-index")
                .ok_or("missing --input-output-index")?
                .parse::<usize>()
                .map_err(|_| "--input-output-index must be a usize".to_string())?;
            let to = flag_value(flags, "--to").ok_or("missing --to")?;
            let amount = flag_value(flags, "--amount")
                .ok_or("missing --amount")?
                .parse::<u64>()
                .map_err(|_| "--amount must be a u64".to_string())?;
            let change_address_raw_hex =
                flag_value(flags, "--change-recipient-address-raw-hex").map(str::to_string);
            let change_key_file =
                flag_value(flags, "--change-recipient-key-file").map(PathBuf::from);
            let change_view_key_file =
                flag_value(flags, "--change-recipient-view-key-file").map(PathBuf::from);
            let memo_hex = flag_value(flags, "--memo-hex").map(str::to_string);
            let fee = flag_value(flags, "--fee")
                .unwrap_or("0")
                .parse::<u64>()
                .map_err(|_| "--fee must be a u64".to_string())?;
            let policy_id = flag_value(flags, "--policy-id").map(str::to_string);
            let disclosure_hash = flag_value(flags, "--disclosure-hash").map(str::to_string);
            let report = create_orchard_withdraw_action(OrchardWithdrawActionOptions {
                data_dir: PathBuf::from(data_dir),
                spending_key_hex,
                key_file,
                input_output_index,
                to: to.to_string(),
                amount,
                change_address_raw_hex,
                change_key_file,
                change_view_key_file,
                memo_hex,
                fee,
                policy_id,
                disclosure_hash,
                action_file: PathBuf::from(action_file),
                overwrite: flag_present(flags, "--overwrite"),
            })
            .map_err(|error| format!("orchard-withdraw-create failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report).map_err(|error| {
                format!("Orchard withdraw action report serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
        "orchard-keygen" => {
            let master_seed_hex = required_secret_flag(
                flags,
                "--master-seed-hex",
                "--master-seed-hex-file",
                "Orchard wallet master seed",
            )?;
            let account_index = flag_value(flags, "--account-index")
                .unwrap_or("0")
                .parse::<u32>()
                .map_err(|_| "--account-index must be a u32".to_string())?;
            let key_file = flag_value(flags, "--key-file").ok_or("missing --key-file")?;
            let report = orchard_wallet_keygen(OrchardWalletKeygenOptions {
                master_seed_hex: master_seed_hex.to_string(),
                account_index,
                key_file: PathBuf::from(key_file),
                overwrite: flag_present(flags, "--overwrite"),
            })
            .map_err(|error| format!("orchard-keygen failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report)
                .map_err(|error| format!("Orchard key report serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "orchard-view-key-export" => {
            let key_file = flag_value(flags, "--key-file").ok_or("missing --key-file")?;
            let view_key_file =
                flag_value(flags, "--view-key-file").ok_or("missing --view-key-file")?;
            let report = orchard_view_key_export(OrchardViewKeyExportOptions {
                key_file: PathBuf::from(key_file),
                view_key_file: PathBuf::from(view_key_file),
                overwrite: flag_present(flags, "--overwrite"),
            })
            .map_err(|error| format!("orchard-view-key-export failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report).map_err(|error| {
                format!("Orchard view-key report serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
        "orchard-scan" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let spending_key_hex = flag_value(flags, "--spending-key-hex").map(str::to_string);
            let key_file = flag_value(flags, "--key-file").map(PathBuf::from);
            let view_key_file = flag_value(flags, "--view-key-file").map(PathBuf::from);
            let report = orchard_wallet_scan(OrchardWalletScanOptions {
                data_dir: PathBuf::from(data_dir),
                spending_key_hex,
                key_file,
                view_key_file,
            })
            .map_err(|error| format!("orchard-scan failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report)
                .map_err(|error| format!("Orchard scan report serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "orchard-disclose" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let spending_key_hex = flag_value(flags, "--spending-key-hex").map(str::to_string);
            let key_file = flag_value(flags, "--key-file").map(PathBuf::from);
            let view_key_file = flag_value(flags, "--view-key-file").map(PathBuf::from);
            let output_index = flag_value(flags, "--output-index")
                .ok_or("missing --output-index")?
                .parse::<usize>()
                .map_err(|_| "--output-index must be a usize".to_string())?;
            let packet_file = flag_value(flags, "--packet-file").ok_or("missing --packet-file")?;
            let packet = orchard_disclosure_packet(OrchardDisclosureOptions {
                data_dir: PathBuf::from(data_dir),
                spending_key_hex,
                key_file,
                view_key_file,
                output_index,
                packet_file: PathBuf::from(packet_file),
                overwrite: flag_present(flags, "--overwrite"),
            })
            .map_err(|error| format!("orchard-disclose failed: {error}"))?;
            let json = serde_json::to_string_pretty(&packet).map_err(|error| {
                format!("Orchard disclosure packet serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
        "orchard-disclosure-verify" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let packet_file = flag_value(flags, "--packet-file").ok_or("missing --packet-file")?;
            let report = orchard_disclosure_verify(OrchardDisclosureVerifyOptions {
                data_dir: PathBuf::from(data_dir),
                packet_file: PathBuf::from(packet_file),
            })
            .map_err(|error| format!("orchard-disclosure-verify failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report).map_err(|error| {
                format!("Orchard disclosure verify report serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
        "shield-mint" => {
            require_direct_state_enabled("shield-mint")?;
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let owner = flag_value(flags, "--owner").ok_or("missing --owner")?;
            let amount = flag_value(flags, "--amount")
                .ok_or("missing --amount")?
                .parse::<u64>()
                .map_err(|_| "--amount must be a u64".to_string())?;
            let asset_id = flag_value(flags, "--asset-id").unwrap_or(DEFAULT_SHIELDED_ASSET_ID);
            let memo = flag_value(flags, "--memo").unwrap_or("");
            let note = shield_mint(ShieldMintOptions {
                data_dir: PathBuf::from(data_dir),
                owner: owner.to_string(),
                asset_id: asset_id.to_string(),
                amount,
                memo: memo.to_string(),
            })
            .map_err(|error| format!("shield-mint failed: {error}"))?;
            let json = serde_json::to_string_pretty(&note)
                .map_err(|error| format!("note serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "shield-spend" => {
            require_direct_state_enabled("shield-spend")?;
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let note_id = flag_value(flags, "--note-id").ok_or("missing --note-id")?;
            let to = flag_value(flags, "--to").ok_or("missing --to")?;
            let amount = flag_value(flags, "--amount")
                .ok_or("missing --amount")?
                .parse::<u64>()
                .map_err(|_| "--amount must be a u64".to_string())?;
            let memo = flag_value(flags, "--memo").unwrap_or("");
            let result = shield_spend(ShieldSpendOptions {
                data_dir: PathBuf::from(data_dir),
                note_id: note_id.to_string(),
                to: to.to_string(),
                amount,
                memo: memo.to_string(),
            })
            .map_err(|error| format!("shield-spend failed: {error}"))?;
            let json = serde_json::to_string_pretty(&result)
                .map_err(|error| format!("spend serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "shield-batch-mint" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let owner = flag_value(flags, "--owner").ok_or("missing --owner")?;
            let amount = flag_value(flags, "--amount")
                .ok_or("missing --amount")?
                .parse::<u64>()
                .map_err(|_| "--amount must be a u64".to_string())?;
            let asset_id = flag_value(flags, "--asset-id").unwrap_or(DEFAULT_SHIELDED_ASSET_ID);
            let memo = flag_value(flags, "--memo").unwrap_or("");
            let batch_file = flag_value(flags, "--batch-file").ok_or("missing --batch-file")?;
            let batch = create_shielded_mint_batch(ShieldMintBatchOptions {
                data_dir: PathBuf::from(data_dir),
                owner: owner.to_string(),
                asset_id: asset_id.to_string(),
                amount,
                memo: memo.to_string(),
                batch_file: PathBuf::from(batch_file),
            })
            .map_err(|error| format!("shield-batch-mint failed: {error}"))?;
            let json = serde_json::to_string_pretty(&batch)
                .map_err(|error| format!("shielded batch serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "shield-batch-spend" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let note_id = flag_value(flags, "--note-id").ok_or("missing --note-id")?;
            let to = flag_value(flags, "--to").ok_or("missing --to")?;
            let amount = flag_value(flags, "--amount")
                .ok_or("missing --amount")?
                .parse::<u64>()
                .map_err(|_| "--amount must be a u64".to_string())?;
            let memo = flag_value(flags, "--memo").unwrap_or("");
            let batch_file = flag_value(flags, "--batch-file").ok_or("missing --batch-file")?;
            let batch = create_shielded_spend_batch(ShieldSpendBatchOptions {
                data_dir: PathBuf::from(data_dir),
                note_id: note_id.to_string(),
                to: to.to_string(),
                amount,
                memo: memo.to_string(),
                batch_file: PathBuf::from(batch_file),
            })
            .map_err(|error| format!("shield-batch-spend failed: {error}"))?;
            let json = serde_json::to_string_pretty(&batch)
                .map_err(|error| format!("shielded batch serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "shield-batch-migrate" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let note_id = flag_value(flags, "--note-id").ok_or("missing --note-id")?;
            let target_pool = flag_value(flags, "--target-pool").ok_or("missing --target-pool")?;
            let memo = flag_value(flags, "--memo").unwrap_or("");
            let batch_file = flag_value(flags, "--batch-file").ok_or("missing --batch-file")?;
            let batch = create_shielded_migrate_batch(ShieldMigrateBatchOptions {
                data_dir: PathBuf::from(data_dir),
                note_id: note_id.to_string(),
                target_pool: target_pool.to_string(),
                memo: memo.to_string(),
                batch_file: PathBuf::from(batch_file),
            })
            .map_err(|error| format!("shield-batch-migrate failed: {error}"))?;
            let json = serde_json::to_string_pretty(&batch)
                .map_err(|error| format!("shielded batch serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "shield-batch-orchard" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let action_file = flag_value(flags, "--action-file").ok_or("missing --action-file")?;
            let batch_file = flag_value(flags, "--batch-file").ok_or("missing --batch-file")?;
            let batch = create_orchard_action_batch(OrchardActionBatchOptions {
                data_dir: PathBuf::from(data_dir),
                action_file: PathBuf::from(action_file),
                batch_file: PathBuf::from(batch_file),
            })
            .map_err(|error| format!("shield-batch-orchard failed: {error}"))?;
            let json = serde_json::to_string_pretty(&batch)
                .map_err(|error| format!("Orchard shielded batch serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "shield-batch-orchard-deposit" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let deposit_file =
                flag_value(flags, "--deposit-file").ok_or("missing --deposit-file")?;
            let batch_file = flag_value(flags, "--batch-file").ok_or("missing --batch-file")?;
            let batch = create_orchard_deposit_action_batch(OrchardDepositActionBatchOptions {
                data_dir: PathBuf::from(data_dir),
                deposit_file: PathBuf::from(deposit_file),
                batch_file: PathBuf::from(batch_file),
            })
            .map_err(|error| format!("shield-batch-orchard-deposit failed: {error}"))?;
            let json = serde_json::to_string_pretty(&batch).map_err(|error| {
                format!("Orchard deposit shielded batch serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
        "shield-batch-asset-orchard-ingress" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let ingress_file =
                flag_value(flags, "--ingress-file").ok_or("missing --ingress-file")?;
            let batch_file = flag_value(flags, "--batch-file").ok_or("missing --batch-file")?;
            let batch = create_asset_orchard_ingress_batch(AssetOrchardIngressBatchOptions {
                data_dir: PathBuf::from(data_dir),
                ingress_file: PathBuf::from(ingress_file),
                batch_file: PathBuf::from(batch_file),
            })
            .map_err(|error| format!("shield-batch-asset-orchard-ingress failed: {error}"))?;
            let json = serde_json::to_string_pretty(&batch).map_err(|error| {
                format!("AssetOrchard ingress shielded batch serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
        "shield-batch-asset-orchard-egress" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let egress_file = flag_value(flags, "--egress-file").ok_or("missing --egress-file")?;
            let batch_file = flag_value(flags, "--batch-file").ok_or("missing --batch-file")?;
            let batch = create_asset_orchard_egress_batch(AssetOrchardEgressBatchOptions {
                data_dir: PathBuf::from(data_dir),
                egress_file: PathBuf::from(egress_file),
                batch_file: PathBuf::from(batch_file),
            })
            .map_err(|error| format!("shield-batch-asset-orchard-egress failed: {error}"))?;
            let json = serde_json::to_string_pretty(&batch).map_err(|error| {
                format!("AssetOrchard egress shielded batch serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
        "shield-batch-asset-orchard-private-egress" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let egress_file = flag_value(flags, "--egress-file").ok_or("missing --egress-file")?;
            let batch_file = flag_value(flags, "--batch-file").ok_or("missing --batch-file")?;
            let batch =
                create_asset_orchard_private_egress_batch(AssetOrchardPrivateEgressBatchOptions {
                    data_dir: PathBuf::from(data_dir),
                    egress_file: PathBuf::from(egress_file),
                    batch_file: PathBuf::from(batch_file),
                })
                .map_err(|error| {
                    format!("shield-batch-asset-orchard-private-egress failed: {error}")
                })?;
            let json = serde_json::to_string_pretty(&batch).map_err(|error| {
                format!("AssetOrchard private egress shielded batch serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
        "shield-batch-orchard-withdraw" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let action_file = flag_value(flags, "--action-file").ok_or("missing --action-file")?;
            let to = flag_value(flags, "--to").ok_or("missing --to")?;
            let amount = flag_value(flags, "--amount")
                .ok_or("missing --amount")?
                .parse::<u64>()
                .map_err(|_| "--amount must be a u64".to_string())?;
            let fee = flag_value(flags, "--fee")
                .ok_or("missing --fee")?
                .parse::<u64>()
                .map_err(|_| "--fee must be a u64".to_string())?;
            let policy_id = flag_value(flags, "--policy-id").map(str::to_string);
            let disclosure_hash = flag_value(flags, "--disclosure-hash").map(str::to_string);
            let batch_file = flag_value(flags, "--batch-file").ok_or("missing --batch-file")?;
            let batch = create_orchard_withdraw_action_batch(OrchardWithdrawActionBatchOptions {
                data_dir: PathBuf::from(data_dir),
                action_file: PathBuf::from(action_file),
                to: to.to_string(),
                amount,
                fee,
                policy_id,
                disclosure_hash,
                batch_file: PathBuf::from(batch_file),
            })
            .map_err(|error| format!("shield-batch-orchard-withdraw failed: {error}"))?;
            let json = serde_json::to_string_pretty(&batch).map_err(|error| {
                format!("Orchard withdraw shielded batch serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
        "shield-batch-swap" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let swap_file = flag_value(flags, "--swap-file").ok_or("missing --swap-file")?;
            let batch_file = flag_value(flags, "--batch-file").ok_or("missing --batch-file")?;
            let batch = create_shielded_swap_action_batch(ShieldedSwapActionBatchOptions {
                data_dir: PathBuf::from(data_dir),
                swap_file: PathBuf::from(swap_file),
                batch_file: PathBuf::from(batch_file),
            })
            .map_err(|error| format!("shield-batch-swap failed: {error}"))?;
            let json = serde_json::to_string_pretty(&batch)
                .map_err(|error| format!("ShieldedSwap batch serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "apply-shield-batch" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let batch_file = flag_value(flags, "--batch-file").ok_or("missing --batch-file")?;
            let certificate_file = flag_value(flags, "--certificate-file").map(PathBuf::from);
            let receipts = apply_shielded_batch(ApplyBatchOptions {
                data_dir: PathBuf::from(data_dir),
                batch_file: PathBuf::from(batch_file),
                certificate_file,
            })
            .map_err(|error| format!("apply-shield-batch failed: {error}"))?;
            let json = serde_json::to_string_pretty(&receipts)
                .map_err(|error| format!("receipt serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "shield-scan" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let owner = flag_value(flags, "--owner").ok_or("missing --owner")?;
            let notes = shield_scan(
                NodeOptions {
                    data_dir: PathBuf::from(data_dir),
                },
                owner,
            )
            .map_err(|error| format!("shield-scan failed: {error}"))?;
            let json = serde_json::to_string_pretty(&notes)
                .map_err(|error| format!("scan serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "shield-disclose" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let note_id = flag_value(flags, "--note-id").ok_or("missing --note-id")?;
            let disclosure = shield_disclose(
                NodeOptions {
                    data_dir: PathBuf::from(data_dir),
                },
                note_id,
            )
            .map_err(|error| format!("shield-disclose failed: {error}"))?;
            let json = serde_json::to_string_pretty(&disclosure)
                .map_err(|error| format!("disclosure serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "shield-turnstile" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let summary = shield_turnstile(NodeOptions {
                data_dir: PathBuf::from(data_dir),
            })
            .map_err(|error| format!("shield-turnstile failed: {error}"))?;
            let json = serde_json::to_string_pretty(&summary)
                .map_err(|error| format!("turnstile serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "shield-root" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let root = shielded_tree_root(NodeOptions {
                data_dir: PathBuf::from(data_dir),
            })
            .map_err(|error| format!("shield-root failed: {error}"))?;
            println!("{root}");
            Ok(())
        }
        "bridge-domain" => {
            require_direct_state_enabled("bridge-domain")?;
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let domain_id = flag_value(flags, "--domain-id").unwrap_or(DEFAULT_BRIDGE_DOMAIN_ID);
            let name = flag_value(flags, "--name").unwrap_or("Local Simulation");
            let source_chain = flag_value(flags, "--source-chain").unwrap_or(domain_id);
            let target_chain = flag_value(flags, "--target-chain").unwrap_or(DEFAULT_CHAIN_ID);
            let bridge_id = flag_value(flags, "--bridge-id").unwrap_or(domain_id);
            let door_account = flag_value(flags, "--door-account")
                .map(str::to_string)
                .unwrap_or_else(|| format!("door:{domain_id}"));
            let inbound_cap = flag_value(flags, "--inbound-cap")
                .ok_or("missing --inbound-cap")?
                .parse::<u64>()
                .map_err(|_| "--inbound-cap must be a u64".to_string())?;
            let outbound_cap = flag_value(flags, "--outbound-cap")
                .ok_or("missing --outbound-cap")?
                .parse::<u64>()
                .map_err(|_| "--outbound-cap must be a u64".to_string())?;
            let domain = bridge_upsert_domain(BridgeDomainOptions {
                data_dir: PathBuf::from(data_dir),
                domain_id: domain_id.to_string(),
                name: name.to_string(),
                source_chain: source_chain.to_string(),
                target_chain: target_chain.to_string(),
                bridge_id: bridge_id.to_string(),
                door_account,
                inbound_cap,
                outbound_cap,
            })
            .map_err(|error| format!("bridge-domain failed: {error}"))?;
            let json = serde_json::to_string_pretty(&domain)
                .map_err(|error| format!("bridge domain serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "bridge-transfer" => {
            require_direct_state_enabled("bridge-transfer")?;
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let domain_id = flag_value(flags, "--domain-id").unwrap_or(DEFAULT_BRIDGE_DOMAIN_ID);
            let direction = flag_value(flags, "--direction").unwrap_or(BRIDGE_DIRECTION_INBOUND);
            let from = flag_value(flags, "--from").ok_or("missing --from")?;
            let to = flag_value(flags, "--to").ok_or("missing --to")?;
            let asset_id = flag_value(flags, "--asset-id").unwrap_or(DEFAULT_SHIELDED_ASSET_ID);
            let amount = flag_value(flags, "--amount")
                .ok_or("missing --amount")?
                .parse::<u64>()
                .map_err(|_| "--amount must be a u64".to_string())?;
            let witness_id = flag_value(flags, "--witness-id").ok_or("missing --witness-id")?;
            let witness_epoch = parse_optional_u32_flag(flags, "--witness-epoch")?;
            let witness_signer =
                flag_value(flags, "--witness-signer").unwrap_or(DEFAULT_BRIDGE_WITNESS_SIGNER);
            let transfer = bridge_transfer(BridgeTransferOptions {
                data_dir: PathBuf::from(data_dir),
                domain_id: domain_id.to_string(),
                direction: direction.to_string(),
                from: from.to_string(),
                to: to.to_string(),
                asset_id: asset_id.to_string(),
                amount,
                witness_id: witness_id.to_string(),
                witness_epoch,
                witness_signer: witness_signer.to_string(),
            })
            .map_err(|error| format!("bridge-transfer failed: {error}"))?;
            let json = serde_json::to_string_pretty(&transfer)
                .map_err(|error| format!("bridge transfer serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "bridge-pause" => {
            require_direct_state_enabled("bridge-pause")?;
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let domain_id = flag_value(flags, "--domain-id").unwrap_or(DEFAULT_BRIDGE_DOMAIN_ID);
            let domain = bridge_pause(BridgePauseOptions {
                data_dir: PathBuf::from(data_dir),
                domain_id: domain_id.to_string(),
                paused: true,
            })
            .map_err(|error| format!("bridge-pause failed: {error}"))?;
            let json = serde_json::to_string_pretty(&domain)
                .map_err(|error| format!("bridge domain serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "bridge-resume" => {
            require_direct_state_enabled("bridge-resume")?;
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let domain_id = flag_value(flags, "--domain-id").unwrap_or(DEFAULT_BRIDGE_DOMAIN_ID);
            let domain = bridge_pause(BridgePauseOptions {
                data_dir: PathBuf::from(data_dir),
                domain_id: domain_id.to_string(),
                paused: false,
            })
            .map_err(|error| format!("bridge-resume failed: {error}"))?;
            let json = serde_json::to_string_pretty(&domain)
                .map_err(|error| format!("bridge domain serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "bridge-status" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let bridge = bridge_state(NodeOptions {
                data_dir: PathBuf::from(data_dir),
            })
            .map_err(|error| format!("bridge-status failed: {error}"))?;
            let json = serde_json::to_string_pretty(&bridge)
                .map_err(|error| format!("bridge serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "bridge-batch-domain" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let domain_id = flag_value(flags, "--domain-id").unwrap_or(DEFAULT_BRIDGE_DOMAIN_ID);
            let name = flag_value(flags, "--name").unwrap_or("Local Simulation");
            let source_chain = flag_value(flags, "--source-chain").unwrap_or(domain_id);
            let target_chain = flag_value(flags, "--target-chain").unwrap_or(DEFAULT_CHAIN_ID);
            let bridge_id = flag_value(flags, "--bridge-id").unwrap_or(domain_id);
            let door_account = flag_value(flags, "--door-account")
                .map(str::to_string)
                .unwrap_or_else(|| format!("door:{domain_id}"));
            let inbound_cap = flag_value(flags, "--inbound-cap")
                .ok_or("missing --inbound-cap")?
                .parse::<u64>()
                .map_err(|_| "--inbound-cap must be a u64".to_string())?;
            let outbound_cap = flag_value(flags, "--outbound-cap")
                .ok_or("missing --outbound-cap")?
                .parse::<u64>()
                .map_err(|_| "--outbound-cap must be a u64".to_string())?;
            let batch_file = flag_value(flags, "--batch-file").ok_or("missing --batch-file")?;
            let batch = create_bridge_domain_batch(BridgeDomainBatchOptions {
                data_dir: PathBuf::from(data_dir),
                domain_id: domain_id.to_string(),
                name: name.to_string(),
                source_chain: source_chain.to_string(),
                target_chain: target_chain.to_string(),
                bridge_id: bridge_id.to_string(),
                door_account,
                inbound_cap,
                outbound_cap,
                batch_file: PathBuf::from(batch_file),
            })
            .map_err(|error| format!("bridge-batch-domain failed: {error}"))?;
            let json = serde_json::to_string_pretty(&batch)
                .map_err(|error| format!("bridge batch serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "bridge-batch-transfer" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let domain_id = flag_value(flags, "--domain-id").unwrap_or(DEFAULT_BRIDGE_DOMAIN_ID);
            let direction = flag_value(flags, "--direction").unwrap_or(BRIDGE_DIRECTION_INBOUND);
            let from = flag_value(flags, "--from").ok_or("missing --from")?;
            let to = flag_value(flags, "--to").ok_or("missing --to")?;
            let asset_id = flag_value(flags, "--asset-id").unwrap_or(DEFAULT_SHIELDED_ASSET_ID);
            let amount = flag_value(flags, "--amount")
                .ok_or("missing --amount")?
                .parse::<u64>()
                .map_err(|_| "--amount must be a u64".to_string())?;
            let witness_id = flag_value(flags, "--witness-id").ok_or("missing --witness-id")?;
            let witness_epoch = parse_optional_u32_flag(flags, "--witness-epoch")?;
            let witness_signer =
                flag_value(flags, "--witness-signer").unwrap_or(DEFAULT_BRIDGE_WITNESS_SIGNER);
            let batch_file = flag_value(flags, "--batch-file").ok_or("missing --batch-file")?;
            let batch = create_bridge_transfer_batch(BridgeTransferBatchOptions {
                data_dir: PathBuf::from(data_dir),
                domain_id: domain_id.to_string(),
                direction: direction.to_string(),
                from: from.to_string(),
                to: to.to_string(),
                asset_id: asset_id.to_string(),
                amount,
                witness_id: witness_id.to_string(),
                witness_epoch,
                witness_signer: witness_signer.to_string(),
                batch_file: PathBuf::from(batch_file),
            })
            .map_err(|error| format!("bridge-batch-transfer failed: {error}"))?;
            let json = serde_json::to_string_pretty(&batch)
                .map_err(|error| format!("bridge batch serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "bridge-batch-pause" | "bridge-batch-resume" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let domain_id = flag_value(flags, "--domain-id").unwrap_or(DEFAULT_BRIDGE_DOMAIN_ID);
            let batch_file = flag_value(flags, "--batch-file").ok_or("missing --batch-file")?;
            let batch = create_bridge_pause_batch(BridgePauseBatchOptions {
                data_dir: PathBuf::from(data_dir),
                domain_id: domain_id.to_string(),
                paused: command == "bridge-batch-pause",
                batch_file: PathBuf::from(batch_file),
            })
            .map_err(|error| format!("{command} failed: {error}"))?;
            let json = serde_json::to_string_pretty(&batch)
                .map_err(|error| format!("bridge batch serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "apply-bridge-batch" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let batch_file = flag_value(flags, "--batch-file").ok_or("missing --batch-file")?;
            let certificate_file = flag_value(flags, "--certificate-file").map(PathBuf::from);
            let receipts = apply_bridge_batch(ApplyBatchOptions {
                data_dir: PathBuf::from(data_dir),
                batch_file: PathBuf::from(batch_file),
                certificate_file,
            })
            .map_err(|error| format!("apply-bridge-batch failed: {error}"))?;
            let json = serde_json::to_string_pretty(&receipts)
                .map_err(|error| format!("receipt serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "deployment-publisher-key-create" => {
            let publisher_key_file =
                flag_value(flags, "--publisher-key-file").ok_or("missing --publisher-key-file")?;
            let public =
                create_deployment_publisher_private_key(DeploymentPublisherKeyCreateOptions {
                    publisher_key_file: PathBuf::from(publisher_key_file),
                })
                .map_err(|error| format!("deployment-publisher-key-create failed: {error}"))?;
            let json = serde_json::to_string_pretty(&public).map_err(|error| {
                format!("deployment publisher key serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
        "deployment-publisher-key-export" => {
            let publisher_key_file =
                flag_value(flags, "--publisher-key-file").ok_or("missing --publisher-key-file")?;
            let public_key_file =
                flag_value(flags, "--public-key-file").ok_or("missing --public-key-file")?;
            let public =
                export_deployment_publisher_public_key(DeploymentPublisherKeyExportOptions {
                    publisher_key_file: PathBuf::from(publisher_key_file),
                    public_key_file: PathBuf::from(public_key_file),
                })
                .map_err(|error| format!("deployment-publisher-key-export failed: {error}"))?;
            let json = serde_json::to_string_pretty(&public).map_err(|error| {
                format!("deployment publisher key serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
        "deployment-validator-units-stage" => {
            let required = |name: &str| {
                flag_value(flags, name)
                    .map(str::to_string)
                    .ok_or_else(|| format!("missing {name}"))
            };
            let report = stage_deployment_validator_units(DeploymentValidatorUnitsStageOptions {
                release_id: required("--release-id")?,
                topology_file: PathBuf::from(required("--topology-file")?),
                binary_file: PathBuf::from(required("--binary-file")?),
                swap_circuit_metadata_file: PathBuf::from(required(
                    "--swap-circuit-metadata-file",
                )?),
                private_egress_circuit_metadata_file: PathBuf::from(required(
                    "--private-egress-circuit-metadata-file",
                )?),
                output_dir: PathBuf::from(required("--output-dir")?),
            })
            .map_err(|error| format!("deployment-validator-units-stage failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report).map_err(|error| {
                format!("deployment validator stage serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
        "deployment-manifest-create" => {
            let required = |name: &str| {
                flag_value(flags, name)
                    .map(str::to_string)
                    .ok_or_else(|| format!("missing {name}"))
            };
            let valid_from_unix = required("--valid-from-unix")?
                .parse::<u64>()
                .map_err(|_| "--valid-from-unix must be a u64".to_string())?;
            let valid_until_unix = required("--valid-until-unix")?
                .parse::<u64>()
                .map_err(|_| "--valid-until-unix must be a u64".to_string())?;
            let protocol_version = required("--protocol-version")?
                .parse::<u32>()
                .map_err(|_| "--protocol-version must be a u32".to_string())?;
            let manifest = create_deployment_manifest(DeploymentManifestCreateOptions {
                deployment_id: required("--deployment-id")?,
                valid_from_unix,
                valid_until_unix,
                chain_id: required("--chain-id")?,
                genesis_hash: required("--genesis-hash")?,
                git_revision: required("--git-revision")?,
                binary_file: PathBuf::from(required("--binary-file")?),
                build_profile: required("--build-profile")?,
                build_features: required("--build-features")?
                    .split(',')
                    .map(str::to_string)
                    .collect(),
                protocol_version,
                rpc_schema: required("--rpc-schema")?,
                service_unit_file: PathBuf::from(required("--service-unit-file")?),
                environment_file: PathBuf::from(required("--environment-file")?),
                validator_bindings_file: PathBuf::from(required("--validator-bindings-file")?),
                topology_file: PathBuf::from(required("--topology-file")?),
                swap_circuit_metadata_file: PathBuf::from(required(
                    "--swap-circuit-metadata-file",
                )?),
                private_egress_circuit_metadata_file: PathBuf::from(required(
                    "--private-egress-circuit-metadata-file",
                )?),
                publisher_key_file: PathBuf::from(required("--publisher-key-file")?),
                manifest_file: PathBuf::from(required("--manifest-file")?),
            })
            .map_err(|error| format!("deployment-manifest-create failed: {error}"))?;
            let json = serde_json::to_string_pretty(&manifest)
                .map_err(|error| format!("deployment manifest serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "deployment-manifest-verify" => {
            let manifest_file =
                flag_value(flags, "--manifest-file").ok_or("missing --manifest-file")?;
            let trusted_publisher_key_file = flag_value(flags, "--trusted-publisher-key-file")
                .ok_or("missing --trusted-publisher-key-file")?;
            let now_unix = flag_value(flags, "--now-unix")
                .map(|value| {
                    value
                        .parse::<u64>()
                        .map_err(|_| "--now-unix must be a u64".to_string())
                })
                .transpose()?;
            let validator_id = flag_value(flags, "--validator-id").map(str::to_string);
            let validator_bindings_file =
                flag_value(flags, "--validator-bindings-file").map(PathBuf::from);
            let runtime_binary_file = flag_value(flags, "--runtime-binary-file").map(PathBuf::from);
            let runtime_topology_file =
                flag_value(flags, "--runtime-topology-file").map(PathBuf::from);
            let runtime_swap_circuit_metadata_file =
                flag_value(flags, "--runtime-swap-circuit-metadata-file").map(PathBuf::from);
            let runtime_private_egress_circuit_metadata_file =
                flag_value(flags, "--runtime-private-egress-circuit-metadata-file")
                    .map(PathBuf::from);
            let manifest = verify_deployment_manifest(DeploymentManifestVerifyOptions {
                manifest_file: PathBuf::from(manifest_file),
                trusted_publisher_key_file: PathBuf::from(trusted_publisher_key_file),
                now_unix,
                validator_id,
                validator_bindings_file,
                runtime_binary_file,
                runtime_topology_file,
                runtime_swap_circuit_metadata_file,
                runtime_private_egress_circuit_metadata_file,
            })
            .map_err(|error| format!("deployment-manifest-verify failed: {error}"))?;
            let json = serde_json::to_string_pretty(&manifest)
                .map_err(|error| format!("deployment manifest serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "snapshot-publisher-key-export" => {
            let publisher_key_file =
                flag_value(flags, "--publisher-key-file").ok_or("missing --publisher-key-file")?;
            let public_key_file =
                flag_value(flags, "--public-key-file").ok_or("missing --public-key-file")?;
            let public = export_snapshot_publisher_public_key(SnapshotPublisherKeyExportOptions {
                publisher_key_file: PathBuf::from(publisher_key_file),
                public_key_file: PathBuf::from(public_key_file),
            })
            .map_err(|error| format!("snapshot-publisher-key-export failed: {error}"))?;
            let json = serde_json::to_string_pretty(&public)
                .map_err(|error| format!("snapshot publisher key serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "snapshot-export-signed" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let snapshot_dir =
                flag_value(flags, "--snapshot-dir").ok_or("missing --snapshot-dir")?;
            let publisher_key_file =
                flag_value(flags, "--publisher-key-file").ok_or("missing --publisher-key-file")?;
            let manifest = export_signed_snapshot(SignedSnapshotExportOptions {
                data_dir: PathBuf::from(data_dir),
                snapshot_dir: PathBuf::from(snapshot_dir),
                publisher_key_file: PathBuf::from(publisher_key_file),
            })
            .map_err(|error| format!("snapshot-export-signed failed: {error}"))?;
            let json = serde_json::to_string_pretty(&manifest)
                .map_err(|error| format!("signed snapshot serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "snapshot-import-signed" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let snapshot_dir =
                flag_value(flags, "--snapshot-dir").ok_or("missing --snapshot-dir")?;
            let trusted_publisher_key_file = flag_value(flags, "--trusted-publisher-key-file")
                .ok_or("missing --trusted-publisher-key-file")?;
            let node_id = flag_value(flags, "--node-id").map(str::to_string);
            let report = import_signed_snapshot(SignedSnapshotImportOptions {
                data_dir: PathBuf::from(data_dir),
                snapshot_dir: PathBuf::from(snapshot_dir),
                trusted_publisher_key_file: PathBuf::from(trusted_publisher_key_file),
                node_id,
            })
            .map_err(|error| format!("snapshot-import-signed failed: {error}"))?;
            let json = report.to_json().map_err(|error| {
                format!("signed snapshot import report serialization failed: {error}")
            })?;
            print!("{json}");
            Ok(())
        }
        "snapshot-export" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let snapshot_dir =
                flag_value(flags, "--snapshot-dir").ok_or("missing --snapshot-dir")?;
            let manifest = export_snapshot(SnapshotExportOptions {
                data_dir: PathBuf::from(data_dir),
                snapshot_dir: PathBuf::from(snapshot_dir),
            })
            .map_err(|error| format!("snapshot-export failed: {error}"))?;
            let json = serde_json::to_string_pretty(&manifest)
                .map_err(|error| format!("snapshot manifest serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "snapshot-import" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let snapshot_dir =
                flag_value(flags, "--snapshot-dir").ok_or("missing --snapshot-dir")?;
            let node_id = flag_value(flags, "--node-id").map(str::to_string);
            let report = import_snapshot(SnapshotImportOptions {
                data_dir: PathBuf::from(data_dir),
                snapshot_dir: PathBuf::from(snapshot_dir),
                node_id,
            })
            .map_err(|error| format!("snapshot-import failed: {error}"))?;
            let json = report
                .to_json()
                .map_err(|error| format!("snapshot import report serialization failed: {error}"))?;
            print!("{json}");
            Ok(())
        }
        "rpc" => run_rpc(flags),
        "help" | "--help" | "-h" => {
            print_usage();
            Ok(())
        }
        _ => unreachable!("run_cli_group_05 dispatch mismatch"),
    }
}
