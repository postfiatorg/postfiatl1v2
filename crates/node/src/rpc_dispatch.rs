use super::*;

const RPC_TX_NOT_FOUND_ERROR_PREFIX: &str = "postfiat-rpc-dispatch-error-v1:rpc_tx_not_found:";

pub(super) fn rpc_dispatch_error_response_parts(error: &str) -> (&'static str, &str) {
    error
        .strip_prefix(RPC_TX_NOT_FOUND_ERROR_PREFIX)
        .map_or(("rpc_error", error), |message| {
            ("rpc_tx_not_found", message)
        })
}

pub(super) fn rpc_tx_finality_error(error: std::io::Error) -> String {
    let message = format!("rpc tx failed: {error}");
    if postfiat_node::tx_finality_error_is_transaction_not_found(&error) {
        format!("{RPC_TX_NOT_FOUND_ERROR_PREFIX}{message}")
    } else {
        message
    }
}

pub(super) fn run_rpc(flags: &[String]) -> Result<(), String> {
    if let Some(request_file) = flag_value(flags, "--request-file") {
        return run_rpc_request_file(flags, request_file);
    }

    let id = flag_value(flags, "--id").unwrap_or("local-1");
    let method = flag_value(flags, "--method").ok_or("missing --method")?;
    let data_dir = PathBuf::from(flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR));

    match method {
        "status" => {
            let report = status(NodeOptions { data_dir })
                .map_err(|error| format!("rpc status failed: {error}"))?;
            print_rpc_success(
                id,
                &report,
                vec![RpcEvent::new(
                    "status",
                    report.node_id.clone(),
                    "status queried",
                )],
            )
        }
        "owned_sign" | "owned-sign" => {
            let validator_id = flag_value(flags, "--validator-id")
                .ok_or("rpc owned_sign: missing --validator-id")?;
            let order_json = flag_value(flags, "--order-json")
                .map(|s| s.to_string())
                .or_else(|| {
                    flag_value(flags, "--order-file")
                        .and_then(|path| std::fs::read_to_string(path).ok())
                })
                .ok_or("rpc owned_sign: missing --order-json or --order-file")?;
            let vote = crate::owned_sign(NodeOptions { data_dir }, &order_json, &validator_id)
                .map_err(|error| format!("owned_sign failed: {error}"))?;
            let vote_value: serde_json::Value =
                serde_json::from_str(&vote).unwrap_or(serde_json::Value::String(vote));
            print_rpc_success(
                id,
                &vote_value,
                vec![RpcEvent::new(
                    "owned_sign",
                    validator_id,
                    "validator signed owned-transfer order",
                )],
            )
        }
        "owned_unwrap_sign" | "owned-unwrap-sign" => {
            let validator_id = flag_value(flags, "--validator-id")
                .ok_or("rpc owned_unwrap_sign: missing --validator-id")?;
            let order_json = flag_value(flags, "--order-json")
                .map(|s| s.to_string())
                .or_else(|| {
                    flag_value(flags, "--order-file")
                        .and_then(|path| std::fs::read_to_string(path).ok())
                })
                .ok_or("rpc owned_unwrap_sign: missing --order-json or --order-file")?;
            let vote =
                crate::owned_unwrap_sign(NodeOptions { data_dir }, &order_json, &validator_id)
                    .map_err(|error| format!("owned_unwrap_sign failed: {error}"))?;
            let vote_value: serde_json::Value =
                serde_json::from_str(&vote).unwrap_or(serde_json::Value::String(vote));
            print_rpc_success(
                id,
                &vote_value,
                vec![RpcEvent::new(
                    "owned_unwrap_sign",
                    validator_id,
                    "validator signed owned-unwrap order",
                )],
            )
        }
        "owned_unwrap_apply" | "owned-unwrap-apply" => {
            let cert_json = flag_value(flags, "--cert-json")
                .map(|s| s.to_string())
                .or_else(|| {
                    flag_value(flags, "--cert-file")
                        .and_then(|path| std::fs::read_to_string(path).ok())
                })
                .ok_or("rpc owned_unwrap_apply: missing --cert-json or --cert-file")?;
            let report = crate::owned_unwrap_apply_report(NodeOptions { data_dir }, &cert_json)
                .map_err(|error| format!("owned_unwrap_apply failed: {error}"))?;
            print_rpc_success(
                id,
                &report,
                vec![RpcEvent::new(
                    "owned_unwrap_apply",
                    "owned_unwrap_apply",
                    "certified owned-unwrap applied",
                )],
            )
        }
        "server_info" => {
            let report = rpc_server_info(data_dir)?;
            let node_id = report
                .get("node_id")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("unknown")
                .to_string();
            print_rpc_success(
                id,
                &report,
                vec![RpcEvent::new("server_info", node_id, "server info queried")],
            )
        }
        "metrics" => {
            let report = metrics(NodeOptions { data_dir })
                .map_err(|error| format!("rpc metrics failed: {error}"))?;
            print_rpc_success(
                id,
                &report,
                vec![RpcEvent::new(
                    "metrics",
                    report.node_id.clone(),
                    "metrics queried",
                )],
            )
        }
        "ledger" => {
            let limit = flag_value(flags, "--limit")
                .map(|value| {
                    value
                        .parse::<usize>()
                        .map_err(|_| "--limit must be a usize".to_string())
                })
                .transpose()?;
            let report = rpc_ledger_alias(data_dir, limit)?;
            let target = report
                .get("ledger_index")
                .and_then(serde_json::Value::as_u64)
                .map(|height| height.to_string())
                .unwrap_or_else(|| "unknown".to_string());
            print_rpc_success(
                id,
                &report,
                vec![RpcEvent::new("ledger", target, "ledger queried")],
            )
        }
        "faucet" => {
            let key = faucet_key(NodeOptions { data_dir })
                .map_err(|error| format!("rpc faucet failed: {error}"))?;
            print_rpc_success(
                id,
                &key,
                vec![RpcEvent::new(
                    "faucet",
                    key.address.clone(),
                    "faucet key queried",
                )],
            )
        }
        "validate_local_keys" => {
            let validators = flag_value(flags, "--validators")
                .unwrap_or("4")
                .parse::<u32>()
                .map_err(|_| "--validators must be a u32".to_string())?;
            let local_only = flag_present(flags, "--local-only");
            let report = validate_local_keys(ValidatorKeysOptions {
                data_dir,
                validators,
                local_only,
            })
            .map_err(|error| format!("rpc validate_local_keys failed: {error}"))?;
            print_rpc_success(
                id,
                &report,
                vec![RpcEvent::new(
                    "validate_local_keys",
                    report.node_id.clone(),
                    "local private keys validated",
                )],
            )
        }
        "account" => {
            let address = flag_value(flags, "--address").ok_or("missing --address")?;
            let account = account(NodeOptions { data_dir }, address)
                .map_err(|error| format!("rpc account failed: {error}"))?;
            print_rpc_success(
                id,
                &account,
                vec![RpcEvent::new(
                    "account",
                    account.address.clone(),
                    "account queried",
                )],
            )
        }
        "account_tx" => {
            let address = flag_value(flags, "--address").ok_or("missing --address")?;
            let from_height = flag_value(flags, "--from-height")
                .map(|value| {
                    value
                        .parse::<u64>()
                        .map_err(|_| "--from-height must be a u64".to_string())
                })
                .transpose()?;
            let to_height = flag_value(flags, "--to-height")
                .map(|value| {
                    value
                        .parse::<u64>()
                        .map_err(|_| "--to-height must be a u64".to_string())
                })
                .transpose()?;
            let limit = flag_value(flags, "--limit")
                .map(|value| {
                    value
                        .parse::<usize>()
                        .map_err(|_| "--limit must be a usize".to_string())
                })
                .transpose()?;
            let report = account_tx(AccountTxQueryOptions {
                data_dir,
                address: address.to_string(),
                from_height,
                to_height,
                limit,
            })
            .map_err(|error| format!("rpc account_tx failed: {error}"))?;
            print_rpc_success(
                id,
                &report,
                vec![RpcEvent::new(
                    "account_tx",
                    report.address.clone(),
                    "account transaction history queried",
                )],
            )
        }
        "account_tx_index_status" => {
            let report = account_tx_index_status(AccountTxIndexOptions { data_dir })
                .map_err(|error| format!("rpc account_tx_index_status failed: {error}"))?;
            print_rpc_success(
                id,
                &report,
                vec![RpcEvent::new(
                    "account_tx_index_status",
                    if report.index_usable {
                        "usable"
                    } else {
                        "unusable"
                    },
                    "account transaction index status queried",
                )],
            )
        }
        "fee" => {
            let report = rpc_fee_alias(data_dir)?;
            print_rpc_success(
                id,
                &report,
                vec![RpcEvent::new("fee", "policy", "fee policy queried")],
            )
        }
        "transfer_fee_quote" => {
            let from = flag_value(flags, "--from").ok_or("missing --from")?;
            let to = flag_value(flags, "--to").ok_or("missing --to")?;
            let amount = parse_u64_flag(flags, "--amount")?;
            let sequence = parse_optional_u64_flag(flags, "--sequence")?;
            let report = transfer_fee_quote(TransferFeeQuoteOptions {
                data_dir,
                from: from.to_string(),
                to: to.to_string(),
                amount,
                sequence,
                memo_type: flag_value(flags, "--memo-type").map(ToString::to_string),
                memo_format: flag_value(flags, "--memo-format").map(ToString::to_string),
                memo_data: flag_value(flags, "--memo-data").map(ToString::to_string),
            })
            .map_err(|error| format!("rpc transfer_fee_quote failed: {error}"))?;
            print_rpc_success(
                id,
                &report,
                vec![RpcEvent::new(
                    "transfer_fee_quote",
                    report.from.clone(),
                    "transfer fee quoted",
                )],
            )
        }
        "atomic_swap_fee_quote" => {
            let quote_leg = |prefix: &str| -> Result<AtomicSwapQuoteLegInput, String> {
                let owner_flag = format!("--{prefix}-owner");
                let recipient_flag = format!("--{prefix}-recipient");
                let issuer_flag = format!("--{prefix}-issuer");
                let asset_id_flag = format!("--{prefix}-asset-id");
                let amount_flag = format!("--{prefix}-amount");
                Ok(AtomicSwapQuoteLegInput {
                    owner: flag_value(flags, &owner_flag)
                        .ok_or_else(|| format!("missing {owner_flag}"))?
                        .to_string(),
                    recipient: flag_value(flags, &recipient_flag)
                        .ok_or_else(|| format!("missing {recipient_flag}"))?
                        .to_string(),
                    issuer: flag_value(flags, &issuer_flag)
                        .ok_or_else(|| format!("missing {issuer_flag}"))?
                        .to_string(),
                    asset_id: flag_value(flags, &asset_id_flag)
                        .ok_or_else(|| format!("missing {asset_id_flag}"))?
                        .to_string(),
                    amount: flag_value(flags, &amount_flag)
                        .ok_or_else(|| format!("missing {amount_flag}"))?
                        .parse::<u64>()
                        .map_err(|_| format!("{amount_flag} must be a u64"))?,
                })
            };
            let report = atomic_swap_fee_quote(AtomicSwapFeeQuoteOptions {
                data_dir,
                rfq_hash: flag_value(flags, "--rfq-hash")
                    .ok_or("missing --rfq-hash")?
                    .to_string(),
                market_envelope_hash: flag_value(flags, "--market-envelope-hash")
                    .ok_or("missing --market-envelope-hash")?
                    .to_string(),
                nav_epoch: parse_u64_flag(flags, "--nav-epoch")?,
                expires_at_height: parse_u64_flag(flags, "--expires-at-height")?,
                swap_nonce: flag_value(flags, "--swap-nonce")
                    .ok_or("missing --swap-nonce")?
                    .to_string(),
                leg_0: quote_leg("leg-0")?,
                leg_1: quote_leg("leg-1")?,
            })
            .map_err(|error| format!("rpc atomic_swap_fee_quote failed: {error}"))?;
            print_rpc_success(
                id,
                &report,
                vec![RpcEvent::new(
                    "atomic_swap_fee_quote",
                    report.unsigned_transaction.rfq_hash.clone(),
                    "atomic swap fee quoted for both owners",
                )],
            )
        }
        "asset_fee_quote" => {
            let source = flag_value(flags, "--source").ok_or("missing --source")?;
            let operation_json =
                flag_value(flags, "--operation-json").ok_or("missing --operation-json")?;
            let sequence = parse_optional_u64_flag(flags, "--sequence")?;
            let report = asset_fee_quote(AssetFeeQuoteOptions {
                data_dir,
                source: source.to_string(),
                operation_json: operation_json.to_string(),
                sequence,
            })
            .map_err(|error| format!("rpc asset_fee_quote failed: {error}"))?;
            print_rpc_success(
                id,
                &report,
                vec![RpcEvent::new(
                    "asset_fee_quote",
                    report.source.clone(),
                    "asset transaction fee quoted",
                )],
            )
        }
        "escrow_fee_quote" => {
            let source = flag_value(flags, "--source").ok_or("missing --source")?;
            let operation_json =
                flag_value(flags, "--operation-json").ok_or("missing --operation-json")?;
            let sequence = parse_optional_u64_flag(flags, "--sequence")?;
            let report = escrow_fee_quote(EscrowFeeQuoteOptions {
                data_dir,
                source: source.to_string(),
                operation_json: operation_json.to_string(),
                sequence,
            })
            .map_err(|error| format!("rpc escrow_fee_quote failed: {error}"))?;
            print_rpc_success(
                id,
                &report,
                vec![RpcEvent::new(
                    "escrow_fee_quote",
                    report.source.clone(),
                    "escrow transaction fee quoted",
                )],
            )
        }
        "nft_fee_quote" => {
            let source = flag_value(flags, "--source").ok_or("missing --source")?;
            let operation_json =
                flag_value(flags, "--operation-json").ok_or("missing --operation-json")?;
            let sequence = parse_optional_u64_flag(flags, "--sequence")?;
            let report = nft_fee_quote(NftFeeQuoteOptions {
                data_dir,
                source: source.to_string(),
                operation_json: operation_json.to_string(),
                sequence,
            })
            .map_err(|error| format!("rpc nft_fee_quote failed: {error}"))?;
            print_rpc_success(
                id,
                &report,
                vec![RpcEvent::new(
                    "nft_fee_quote",
                    report.source.clone(),
                    "nft transaction fee quoted",
                )],
            )
        }
        "offer_fee_quote" => {
            let source = flag_value(flags, "--source").ok_or("missing --source")?;
            let operation_json =
                flag_value(flags, "--operation-json").ok_or("missing --operation-json")?;
            let sequence = parse_optional_u64_flag(flags, "--sequence")?;
            let report = offer_fee_quote(OfferFeeQuoteOptions {
                data_dir,
                source: source.to_string(),
                operation_json: operation_json.to_string(),
                sequence,
            })
            .map_err(|error| format!("rpc offer_fee_quote failed: {error}"))?;
            print_rpc_success(
                id,
                &report,
                vec![RpcEvent::new(
                    "offer_fee_quote",
                    report.source.clone(),
                    "offer transaction fee quoted",
                )],
            )
        }
        "offer_info" => {
            let offer_id = flag_value(flags, "--offer-id").ok_or("missing --offer-id")?;
            let report = offer_info(OfferInfoOptions {
                data_dir,
                offer_id: offer_id.to_string(),
            })
            .map_err(|error| format!("rpc offer_info failed: {error}"))?;
            print_rpc_success(
                id,
                &report,
                vec![RpcEvent::new(
                    "offer_info",
                    report.offer_id.clone(),
                    "offer info queried",
                )],
            )
        }
        "account_offers" => {
            let account = flag_value(flags, "--account").ok_or("missing --account")?;
            let limit = flag_value(flags, "--limit")
                .map(|value| {
                    value
                        .parse::<usize>()
                        .map_err(|_| "--limit must be a usize".to_string())
                })
                .transpose()?;
            let report = account_offers(AccountOffersOptions {
                data_dir,
                account: account.to_string(),
                state: flag_value(flags, "--state").map(ToString::to_string),
                limit,
            })
            .map_err(|error| format!("rpc account_offers failed: {error}"))?;
            print_rpc_success(
                id,
                &report,
                vec![RpcEvent::new(
                    "account_offers",
                    report.account.clone(),
                    "account offers queried",
                )],
            )
        }
        "book_offers" => {
            let taker_gets_asset_id = flag_value(flags, "--taker-gets-asset-id")
                .ok_or("missing --taker-gets-asset-id")?;
            let taker_pays_asset_id = flag_value(flags, "--taker-pays-asset-id")
                .ok_or("missing --taker-pays-asset-id")?;
            let limit = flag_value(flags, "--limit")
                .map(|value| {
                    value
                        .parse::<usize>()
                        .map_err(|_| "--limit must be a usize".to_string())
                })
                .transpose()?;
            let report = book_offers(BookOffersOptions {
                data_dir,
                taker_gets_asset_id: taker_gets_asset_id.to_string(),
                taker_pays_asset_id: taker_pays_asset_id.to_string(),
                limit,
            })
            .map_err(|error| format!("rpc book_offers failed: {error}"))?;
            print_rpc_success(
                id,
                &report,
                vec![RpcEvent::new(
                    "book_offers",
                    format!(
                        "{}:{}",
                        report.taker_gets_asset_id, report.taker_pays_asset_id
                    ),
                    "book offers queried",
                )],
            )
        }
        "atomic_settlement_template" => {
            let report = atomic_settlement_template(AtomicSettlementTemplateOptions {
                data_dir,
                left_owner: flag_value(flags, "--left-owner")
                    .ok_or("missing --left-owner")?
                    .to_string(),
                left_recipient: flag_value(flags, "--left-recipient")
                    .ok_or("missing --left-recipient")?
                    .to_string(),
                left_asset_id: flag_value(flags, "--left-asset-id")
                    .ok_or("missing --left-asset-id")?
                    .to_string(),
                left_amount: parse_u64_flag(flags, "--left-amount")?,
                right_owner: flag_value(flags, "--right-owner")
                    .ok_or("missing --right-owner")?
                    .to_string(),
                right_recipient: flag_value(flags, "--right-recipient")
                    .ok_or("missing --right-recipient")?
                    .to_string(),
                right_asset_id: flag_value(flags, "--right-asset-id")
                    .ok_or("missing --right-asset-id")?
                    .to_string(),
                right_amount: parse_u64_flag(flags, "--right-amount")?,
                condition: flag_value(flags, "--condition")
                    .ok_or("missing --condition")?
                    .to_string(),
                finish_after: parse_optional_u64_flag(flags, "--finish-after")?.unwrap_or(0),
                cancel_after: parse_u64_flag(flags, "--cancel-after")?,
                left_sequence: parse_optional_u64_flag(flags, "--left-sequence")?,
                right_sequence: parse_optional_u64_flag(flags, "--right-sequence")?,
            })
            .map_err(|error| format!("rpc atomic_settlement_template failed: {error}"))?;
            print_rpc_success(
                id,
                &report,
                vec![RpcEvent::new(
                    "atomic_settlement_template",
                    report.settlement_id.clone(),
                    "atomic settlement template built",
                )],
            )
        }
        "asset_info" => {
            let asset_id = flag_value(flags, "--asset-id").ok_or("missing --asset-id")?;
            let report = asset_info(AssetInfoOptions {
                data_dir,
                asset_id: asset_id.to_string(),
            })
            .map_err(|error| format!("rpc asset_info failed: {error}"))?;
            print_rpc_success(
                id,
                &report,
                vec![RpcEvent::new(
                    "asset_info",
                    report.asset_id.clone(),
                    "asset info queried",
                )],
            )
        }
        "market_ops_status" => {
            let asset_id = flag_value(flags, "--asset-id").ok_or("missing --asset-id")?;
            let epoch = flag_value(flags, "--epoch")
                .map(|value| {
                    value
                        .parse::<u64>()
                        .map_err(|_| "--epoch must be a u64".to_string())
                })
                .transpose()?;
            let report = market_ops_status(MarketOpsStatusOptions {
                data_dir,
                asset_id: asset_id.to_string(),
                epoch,
            })
            .map_err(|error| format!("rpc market_ops_status failed: {error}"))?;
            print_rpc_success(
                id,
                &report,
                vec![RpcEvent::new(
                    "market_ops_status",
                    report.asset_id.clone(),
                    "market ops status queried",
                )],
            )
        }
        "vault_bridge_status" => {
            let asset_id = flag_value(flags, "--asset-id").ok_or("missing --asset-id")?;
            let report = vault_bridge_status(VaultBridgeStatusOptions {
                data_dir,
                asset_id: asset_id.to_string(),
            })
            .map_err(|error| format!("rpc vault_bridge_status failed: {error}"))?;
            print_rpc_success(
                id,
                &report,
                vec![RpcEvent::new(
                    "vault_bridge_status",
                    report.asset_id.clone(),
                    "vault bridge status queried",
                )],
            )
        }
        "vault_bridge_route" => {
            let asset_id = flag_value(flags, "--asset-id").ok_or("missing --asset-id")?;
            let report = vault_bridge_route(VaultBridgeRouteOptions {
                data_dir,
                asset_id: asset_id.to_string(),
            })
            .map_err(|error| format!("rpc vault_bridge_route failed: {error}"))?;
            print_rpc_success(
                id,
                &report,
                vec![RpcEvent::new(
                    "vault_bridge_route",
                    report.profile.route_id.clone(),
                    "governed vault bridge route verified",
                )],
            )
        }
        "navcoin_bridge_routes" => {
            let report = navcoin_bridge_routes(NavcoinBridgeRoutesOptions { data_dir })
                .map_err(|error| format!("rpc navcoin_bridge_routes failed: {error}"))?;
            print_rpc_success(
                id,
                &report,
                vec![RpcEvent::new(
                    "navcoin_bridge_routes",
                    "routes",
                    "NAVCoin bridge routes queried",
                )],
            )
        }
        "navcoin_bridge_packet" => {
            let route_id = flag_value(flags, "--route-id").ok_or("missing --route-id")?;
            let packet_hash = flag_value(flags, "--packet-hash").ok_or("missing --packet-hash")?;
            let report = navcoin_bridge_packet(NavcoinBridgePacketOptions {
                data_dir,
                route_id: route_id.to_string(),
                packet_hash: packet_hash.to_string(),
            })
            .map_err(|error| format!("rpc navcoin_bridge_packet failed: {error}"))?;
            print_rpc_success(
                id,
                &report,
                vec![RpcEvent::new(
                    "navcoin_bridge_packet",
                    report.packet_hash.clone(),
                    "NAVCoin bridge packet queried",
                )],
            )
        }
        "navcoin_bridge_claims" => {
            let route_id = flag_value(flags, "--route-id").ok_or("missing --route-id")?;
            let limit = parse_optional_u64_flag(flags, "--limit")?
                .map(|value| {
                    usize::try_from(value).map_err(|_| "--limit does not fit in usize".to_string())
                })
                .transpose()?;
            let report = navcoin_bridge_claims(NavcoinBridgeClaimsOptions {
                data_dir,
                route_id: route_id.to_string(),
                limit,
                include_terminal: flag_present(flags, "--include-terminal"),
            })
            .map_err(|error| format!("rpc navcoin_bridge_claims failed: {error}"))?;
            print_rpc_success(
                id,
                &report,
                vec![RpcEvent::new(
                    "navcoin_bridge_claims",
                    report.route_id.clone(),
                    "NAVCoin bridge claims queried",
                )],
            )
        }
        "navcoin_bridge_supply_status" => {
            let route_id = flag_value(flags, "--route-id").ok_or("missing --route-id")?;
            let report = navcoin_bridge_supply_status(NavcoinBridgeSupplyStatusOptions {
                data_dir,
                route_id: route_id.to_string(),
            })
            .map_err(|error| format!("rpc navcoin_bridge_supply_status failed: {error}"))?;
            print_rpc_success(
                id,
                &report,
                vec![RpcEvent::new(
                    "navcoin_bridge_supply_status",
                    report.route_id.clone(),
                    "NAVCoin bridge supply status queried",
                )],
            )
        }
        "navcoin_bridge_receipt_replay" => {
            let route_id = flag_value(flags, "--route-id").ok_or("missing --route-id")?;
            let report = navcoin_bridge_receipt_replay(NavcoinBridgeReceiptReplayOptions {
                data_dir,
                route_id: route_id.to_string(),
            })
            .map_err(|error| format!("rpc navcoin_bridge_receipt_replay failed: {error}"))?;
            print_rpc_success(
                id,
                &report,
                vec![RpcEvent::new(
                    "navcoin_bridge_receipt_replay",
                    report.route_id.clone(),
                    "NAVCoin bridge receipt replay verified",
                )],
            )
        }
        "navcoin_bridge_route_init" => {
            let config_file = flag_value(flags, "--config-file").ok_or("missing --config-file")?;
            let report = navcoin_bridge_route_init(NavcoinBridgeRouteInitOptions {
                data_dir,
                config_file: PathBuf::from(config_file),
                ethereum_chain_id: parse_u64_flag(flags, "--ethereum-chain-id")?,
                latest_finalized_nav_epoch: parse_u64_flag(flags, "--latest-finalized-nav-epoch")?,
                return_finality_blocks: parse_u64_flag(flags, "--return-finality-blocks")?,
                replace: flag_present(flags, "--replace"),
            })
            .map_err(|error| format!("rpc navcoin_bridge_route_init failed: {error}"))?;
            print_rpc_success(
                id,
                &report,
                vec![RpcEvent::new(
                    "navcoin_bridge_route_init",
                    report.route_id.clone(),
                    "NAVCoin bridge route initialized",
                )],
            )
        }
        "navcoin_bridge_launch_config_template" => {
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
                    format!("rpc navcoin_bridge_launch_config_template failed: {error}")
                })?;
            print_rpc_success(
                id,
                &report,
                vec![RpcEvent::new(
                    "navcoin_bridge_launch_config_template",
                    report.route_id.clone(),
                    "NAVCoin bridge launch config template generated",
                )],
            )
        }
        "navcoin_bridge_launch_config_init" => {
            let launch_config_file =
                flag_value(flags, "--launch-config-file").ok_or("missing --launch-config-file")?;
            let report = navcoin_bridge_launch_config_init(NavcoinBridgeLaunchConfigInitOptions {
                data_dir,
                launch_config_file: PathBuf::from(launch_config_file),
                replace: flag_present(flags, "--replace"),
            })
            .map_err(|error| format!("rpc navcoin_bridge_launch_config_init failed: {error}"))?;
            print_rpc_success(
                id,
                &report,
                vec![RpcEvent::new(
                    "navcoin_bridge_launch_config_init",
                    report.route_id.clone(),
                    "NAVCoin bridge launch config initialized",
                )],
            )
        }
        "navcoin_bridge_record_fork_rehearsal" => {
            let route_id = flag_value(flags, "--route-id").ok_or("missing --route-id")?;
            let evidence_file =
                flag_value(flags, "--evidence-file").ok_or("missing --evidence-file")?;
            let report =
                navcoin_bridge_record_fork_rehearsal(NavcoinBridgeRecordForkRehearsalOptions {
                    data_dir,
                    route_id: route_id.to_string(),
                    evidence_file: PathBuf::from(evidence_file),
                })
                .map_err(|error| {
                    format!("rpc navcoin_bridge_record_fork_rehearsal failed: {error}")
                })?;
            print_rpc_success(
                id,
                &report,
                vec![RpcEvent::new(
                    "navcoin_bridge_record_fork_rehearsal",
                    report.route_id.clone(),
                    "NAVCoin bridge fork rehearsal evidence recorded",
                )],
            )
        }
        "navcoin_bridge_packet_preflight" => {
            let route_id = flag_value(flags, "--route-id").ok_or("missing --route-id")?;
            let packet_file = flag_value(flags, "--packet-file").ok_or("missing --packet-file")?;
            let report = navcoin_bridge_packet_preflight(NavcoinBridgePacketPreflightOptions {
                data_dir,
                route_id: route_id.to_string(),
                packet_file: PathBuf::from(packet_file),
            })
            .map_err(|error| format!("rpc navcoin_bridge_packet_preflight failed: {error}"))?;
            print_rpc_success(
                id,
                &report,
                vec![RpcEvent::new(
                    "navcoin_bridge_packet_preflight",
                    report.route_id.clone(),
                    "NAVCoin bridge packet preflight passed",
                )],
            )
        }
        "navcoin_bridge_primary_subscribe" => {
            let request_file = flag_value(flags, "--transition-request-file")
                .ok_or("missing --transition-request-file")?;
            let report = navcoin_bridge_primary_subscribe(NavcoinBridgePrimarySubscribeOptions {
                data_dir,
                request_file: PathBuf::from(request_file),
            })
            .map_err(|error| format!("rpc navcoin_bridge_primary_subscribe failed: {error}"))?;
            print_rpc_success(
                id,
                &report,
                vec![RpcEvent::new(
                    "navcoin_bridge_primary_subscribe",
                    report.route_id.clone(),
                    "NAVCoin bridge primary subscription applied",
                )],
            )
        }
        "navcoin_bridge_export_debit" => {
            let request_file = flag_value(flags, "--transition-request-file")
                .ok_or("missing --transition-request-file")?;
            let report = navcoin_bridge_export_debit(NavcoinBridgeExportDebitOptions {
                data_dir,
                request_file: PathBuf::from(request_file),
            })
            .map_err(|error| format!("rpc navcoin_bridge_export_debit failed: {error}"))?;
            print_rpc_success(
                id,
                &report,
                vec![RpcEvent::new(
                    "navcoin_bridge_export_debit",
                    report.route_id.clone(),
                    "NAVCoin bridge export debit applied",
                )],
            )
        }
        "navcoin_bridge_destination_consume" => {
            let route_id = flag_value(flags, "--route-id").ok_or("missing --route-id")?;
            let packet_hash = flag_value(flags, "--packet-hash").ok_or("missing --packet-hash")?;
            let report =
                navcoin_bridge_destination_consume(NavcoinBridgeDestinationConsumeOptions {
                    data_dir,
                    route_id: route_id.to_string(),
                    packet_hash: packet_hash.to_string(),
                })
                .map_err(|error| {
                    format!("rpc navcoin_bridge_destination_consume failed: {error}")
                })?;
            print_rpc_success(
                id,
                &report,
                vec![RpcEvent::new(
                    "navcoin_bridge_destination_consume",
                    report.route_id.clone(),
                    "NAVCoin bridge destination consume applied",
                )],
            )
        }
        "navcoin_bridge_refund_source" => {
            let route_id = flag_value(flags, "--route-id").ok_or("missing --route-id")?;
            let request_file = flag_value(flags, "--transition-request-file")
                .ok_or("missing --transition-request-file")?;
            let report = navcoin_bridge_refund_source(NavcoinBridgeRefundSourceOptions {
                data_dir,
                route_id: route_id.to_string(),
                request_file: PathBuf::from(request_file),
            })
            .map_err(|error| format!("rpc navcoin_bridge_refund_source failed: {error}"))?;
            print_rpc_success(
                id,
                &report,
                vec![RpcEvent::new(
                    "navcoin_bridge_refund_source",
                    report.route_id.clone(),
                    "NAVCoin bridge source refund applied",
                )],
            )
        }
        "navcoin_bridge_record_return_burn" => {
            let route_id = flag_value(flags, "--route-id").ok_or("missing --route-id")?;
            let request_file = flag_value(flags, "--transition-request-file")
                .ok_or("missing --transition-request-file")?;
            let report = navcoin_bridge_record_return_burn(NavcoinBridgeRecordReturnBurnOptions {
                data_dir,
                route_id: route_id.to_string(),
                request_file: PathBuf::from(request_file),
            })
            .map_err(|error| format!("rpc navcoin_bridge_record_return_burn failed: {error}"))?;
            print_rpc_success(
                id,
                &report,
                vec![RpcEvent::new(
                    "navcoin_bridge_record_return_burn",
                    report.route_id.clone(),
                    "NAVCoin bridge return burn recorded",
                )],
            )
        }
        "navcoin_bridge_import_return" => {
            let route_id = flag_value(flags, "--route-id").ok_or("missing --route-id")?;
            let burn_event_hash =
                flag_value(flags, "--burn-event-hash").ok_or("missing --burn-event-hash")?;
            let pftl_recipient =
                flag_value(flags, "--pftl-recipient").ok_or("missing --pftl-recipient")?;
            let report = navcoin_bridge_import_return(NavcoinBridgeImportReturnOptions {
                data_dir,
                route_id: route_id.to_string(),
                burn_event_hash: burn_event_hash.to_string(),
                pftl_recipient: pftl_recipient.to_string(),
            })
            .map_err(|error| format!("rpc navcoin_bridge_import_return failed: {error}"))?;
            print_rpc_success(
                id,
                &report,
                vec![RpcEvent::new(
                    "navcoin_bridge_import_return",
                    report.route_id.clone(),
                    "NAVCoin bridge return import applied",
                )],
            )
        }
        "account_lines" => {
            let account = flag_value(flags, "--account").ok_or("missing --account")?;
            let limit = flag_value(flags, "--limit")
                .map(|value| {
                    value
                        .parse::<usize>()
                        .map_err(|_| "--limit must be a usize".to_string())
                })
                .transpose()?;
            let report = account_lines(AccountLinesOptions {
                data_dir,
                account: account.to_string(),
                issuer: flag_value(flags, "--issuer").map(ToString::to_string),
                asset_id: flag_value(flags, "--asset-id").map(ToString::to_string),
                limit,
            })
            .map_err(|error| format!("rpc account_lines failed: {error}"))?;
            print_rpc_success(
                id,
                &report,
                vec![RpcEvent::new(
                    "account_lines",
                    report.account.clone(),
                    "account trustlines queried",
                )],
            )
        }
        "account_assets" => {
            let account = flag_value(flags, "--account").ok_or("missing --account")?;
            let limit = flag_value(flags, "--limit")
                .map(|value| {
                    value
                        .parse::<usize>()
                        .map_err(|_| "--limit must be a usize".to_string())
                })
                .transpose()?;
            let report = account_assets(AccountAssetsOptions {
                data_dir,
                account: account.to_string(),
                asset_id: flag_value(flags, "--asset-id").map(ToString::to_string),
                limit,
            })
            .map_err(|error| format!("rpc account_assets failed: {error}"))?;
            print_rpc_success(
                id,
                &report,
                vec![RpcEvent::new(
                    "account_assets",
                    report.account.clone(),
                    "account issued assets queried",
                )],
            )
        }
        "owned_objects" => {
            let owner_public_key_hex = flag_value(flags, "--owner-public-key-hex")
                .ok_or("missing --owner-public-key-hex")?;
            let limit = flag_value(flags, "--limit")
                .map(|value| {
                    value
                        .parse::<usize>()
                        .map_err(|_| "--limit must be a usize".to_string())
                })
                .transpose()?;
            let report = owned_objects(OwnedObjectsOptions {
                data_dir,
                owner_public_key_hex: owner_public_key_hex.to_string(),
                asset: flag_value(flags, "--asset").map(ToString::to_string),
                limit,
            })
            .map_err(|error| format!("rpc owned_objects failed: {error}"))?;
            print_rpc_success(
                id,
                &report,
                vec![RpcEvent::new(
                    "owned_objects",
                    report.owner_public_key_hex.clone(),
                    "owned objects queried",
                )],
            )
        }
        "issuer_assets" => {
            let issuer = flag_value(flags, "--issuer").ok_or("missing --issuer")?;
            let limit = flag_value(flags, "--limit")
                .map(|value| {
                    value
                        .parse::<usize>()
                        .map_err(|_| "--limit must be a usize".to_string())
                })
                .transpose()?;
            let report = issuer_assets(IssuerAssetsOptions {
                data_dir,
                issuer: issuer.to_string(),
                limit,
            })
            .map_err(|error| format!("rpc issuer_assets failed: {error}"))?;
            print_rpc_success(
                id,
                &report,
                vec![RpcEvent::new(
                    "issuer_assets",
                    report.issuer.clone(),
                    "issuer assets queried",
                )],
            )
        }
        "escrow_info" => {
            let escrow_id = flag_value(flags, "--escrow-id").ok_or("missing --escrow-id")?;
            let report = escrow_info(EscrowInfoOptions {
                data_dir,
                escrow_id: escrow_id.to_string(),
            })
            .map_err(|error| format!("rpc escrow_info failed: {error}"))?;
            print_rpc_success(
                id,
                &report,
                vec![RpcEvent::new(
                    "escrow_info",
                    report.escrow_id.clone(),
                    "escrow info queried",
                )],
            )
        }
        "account_escrows" => {
            let account = flag_value(flags, "--account").ok_or("missing --account")?;
            let limit = flag_value(flags, "--limit")
                .map(|value| {
                    value
                        .parse::<usize>()
                        .map_err(|_| "--limit must be a usize".to_string())
                })
                .transpose()?;
            let report = account_escrows(AccountEscrowsOptions {
                data_dir,
                account: account.to_string(),
                role: flag_value(flags, "--role").map(ToString::to_string),
                state: flag_value(flags, "--state").map(ToString::to_string),
                limit,
            })
            .map_err(|error| format!("rpc account_escrows failed: {error}"))?;
            print_rpc_success(
                id,
                &report,
                vec![RpcEvent::new(
                    "account_escrows",
                    report.account.clone(),
                    "account escrows queried",
                )],
            )
        }
        "nft_info" => {
            let nft_id = flag_value(flags, "--nft-id").ok_or("missing --nft-id")?;
            let report = nft_info(NftInfoOptions {
                data_dir,
                nft_id: nft_id.to_string(),
            })
            .map_err(|error| format!("rpc nft_info failed: {error}"))?;
            print_rpc_success(
                id,
                &report,
                vec![RpcEvent::new(
                    "nft_info",
                    report.nft_id.clone(),
                    "nft info queried",
                )],
            )
        }
        "account_nfts" => {
            let account = flag_value(flags, "--account").ok_or("missing --account")?;
            let limit = flag_value(flags, "--limit")
                .map(|value| {
                    value
                        .parse::<usize>()
                        .map_err(|_| "--limit must be a usize".to_string())
                })
                .transpose()?;
            let report = account_nfts(AccountNftsOptions {
                data_dir,
                account: account.to_string(),
                include_burned: flag_present(flags, "--include-burned"),
                limit,
            })
            .map_err(|error| format!("rpc account_nfts failed: {error}"))?;
            print_rpc_success(
                id,
                &report,
                vec![RpcEvent::new(
                    "account_nfts",
                    report.account.clone(),
                    "account nfts queried",
                )],
            )
        }
        "issuer_nfts" => {
            let issuer = flag_value(flags, "--issuer").ok_or("missing --issuer")?;
            let limit = flag_value(flags, "--limit")
                .map(|value| {
                    value
                        .parse::<usize>()
                        .map_err(|_| "--limit must be a usize".to_string())
                })
                .transpose()?;
            let report = issuer_nfts(IssuerNftsOptions {
                data_dir,
                issuer: issuer.to_string(),
                collection_id: flag_value(flags, "--collection-id").map(ToString::to_string),
                include_burned: flag_present(flags, "--include-burned"),
                limit,
            })
            .map_err(|error| format!("rpc issuer_nfts failed: {error}"))?;
            print_rpc_success(
                id,
                &report,
                vec![RpcEvent::new(
                    "issuer_nfts",
                    report.issuer.clone(),
                    "issuer nfts queried",
                )],
            )
        }
        "receipts" => {
            let tx_id = flag_value(flags, "--tx-id").map(str::to_string);
            let limit = flag_value(flags, "--limit")
                .map(|value| {
                    value
                        .parse::<usize>()
                        .map_err(|_| "--limit must be a usize".to_string())
                })
                .transpose()?;
            let target = tx_id.clone().unwrap_or_else(|| "latest".to_string());
            let receipt_log = receipts(ReceiptQueryOptions {
                data_dir,
                tx_id,
                limit,
            })
            .map_err(|error| format!("rpc receipts failed: {error}"))?;
            print_rpc_success(
                id,
                &receipt_log,
                vec![RpcEvent::new("receipts", target, "receipts queried")],
            )
        }
        "tx" => {
            let tx_id = flag_value(flags, "--tx-id").ok_or("missing --tx-id")?;
            let report = tx_finality(TxFinalityQueryOptions {
                data_dir,
                tx_id: tx_id.to_string(),
                audit_block_log: flag_present(flags, "--audit-block-log"),
            })
            .map_err(rpc_tx_finality_error)?;
            print_rpc_success(
                id,
                &report,
                vec![RpcEvent::new(
                    "tx",
                    report.tx_id.clone(),
                    "transaction finality proof queried",
                )],
            )
        }
        "blocks" => {
            let from_height = flag_value(flags, "--from-height")
                .map(|value| {
                    value
                        .parse::<u64>()
                        .map_err(|_| "--from-height must be a u64".to_string())
                })
                .transpose()?;
            let limit = flag_value(flags, "--limit")
                .map(|value| {
                    value
                        .parse::<usize>()
                        .map_err(|_| "--limit must be a usize".to_string())
                })
                .transpose()?;
            let block_log = blocks(BlockQueryOptions {
                data_dir,
                from_height,
                limit,
            })
            .map_err(|error| format!("rpc blocks failed: {error}"))?;
            let target = block_log
                .last()
                .map(|block| block.header.height.to_string())
                .unwrap_or_else(|| "empty".to_string());
            print_rpc_success(
                id,
                &block_log,
                vec![RpcEvent::new("blocks", target, "blocks queried")],
            )
        }
        "pfusdc_egress_witness" => {
            let withdrawal_id =
                flag_value(flags, "--withdrawal-id").ok_or("missing --withdrawal-id")?;
            let witness = pfusdc_egress_witness(PfUsdcEgressWitnessOptions {
                data_dir,
                withdrawal_id: withdrawal_id.to_string(),
                prior_checkpoint_block_id: flag_value(flags, "--prior-checkpoint")
                    .map(str::to_string),
            })
            .map_err(|error| format!("rpc pfusdc_egress_witness failed: {error}"))?;
            print_rpc_success(
                id,
                &witness,
                vec![RpcEvent::new(
                    "pfusdc_egress_witness",
                    withdrawal_id,
                    "proof-ready pfUSDC egress witness exported",
                )],
            )
        }
        "pfusdc_checkpoint_witness" => {
            let prior_checkpoint_block_id =
                flag_value(flags, "--prior-checkpoint").ok_or("missing --prior-checkpoint")?;
            let target_block_id =
                flag_value(flags, "--target-block").ok_or("missing --target-block")?;
            let witness = pfusdc_checkpoint_witness(PfUsdcCheckpointWitnessOptions {
                data_dir,
                prior_checkpoint_block_id: prior_checkpoint_block_id.to_string(),
                target_block_id: target_block_id.to_string(),
            })
            .map_err(|error| format!("rpc pfusdc_checkpoint_witness failed: {error}"))?;
            print_rpc_success(
                id,
                &witness,
                vec![RpcEvent::new(
                    "pfusdc_checkpoint_witness",
                    target_block_id,
                    "proof-ready bounded PFTL checkpoint witness exported",
                )],
            )
        }
        "validators" => {
            let report = rpc_validators_alias(data_dir)?;
            let target = report
                .get("validator_count")
                .and_then(serde_json::Value::as_u64)
                .map(|count| count.to_string())
                .unwrap_or_else(|| "unknown".to_string());
            print_rpc_success(
                id,
                &report,
                vec![RpcEvent::new("validators", target, "validators queried")],
            )
        }
        "manifests" => {
            let report = rpc_manifests_alias(data_dir)?;
            let target = report
                .get("manifest_count")
                .and_then(serde_json::Value::as_u64)
                .map(|count| count.to_string())
                .unwrap_or_else(|| "unknown".to_string());
            print_rpc_success(
                id,
                &report,
                vec![RpcEvent::new(
                    "manifests",
                    target,
                    "operator manifests queried",
                )],
            )
        }
        "batch_archive" => {
            let batch_kind = flag_value(flags, "--batch-kind").map(str::to_string);
            let batch_id = flag_value(flags, "--batch-id").map(str::to_string);
            let limit = flag_value(flags, "--limit")
                .map(|value| {
                    value
                        .parse::<usize>()
                        .map_err(|_| "--limit must be a usize".to_string())
                })
                .transpose()?;
            let archive = batch_archive(BatchArchiveQueryOptions {
                data_dir,
                batch_kind,
                batch_id,
                limit,
            })
            .map_err(|error| format!("rpc batch_archive failed: {error}"))?;
            let target = archive
                .last()
                .map(|entry| entry.batch_id.clone())
                .unwrap_or_else(|| "empty".to_string());
            print_rpc_success(
                id,
                &archive,
                vec![RpcEvent::new(
                    "batch_archive",
                    target,
                    "batch archive queried",
                )],
            )
        }
        "archive_window" => {
            let from_height = parse_u64_flag(flags, "--from-height")?;
            let to_height = parse_u64_flag(flags, "--to-height")?;
            if to_height < from_height {
                return Err("rpc archive_window requires --to-height >= --from-height".to_string());
            }
            let window_len = to_height
                .checked_sub(from_height)
                .and_then(|value| value.checked_add(1))
                .ok_or_else(|| "rpc archive_window range overflow".to_string())?;
            if window_len > MAX_READ_QUERY_LIMIT as u64 {
                return Err(format!(
                    "rpc archive_window range must not exceed {MAX_READ_QUERY_LIMIT} blocks"
                ));
            }
            let archive_uri = flag_value(flags, "--archive-uri").map(str::to_string);
            let bundle = build_history_archive_window(HistoryArchiveWindowBuildOptions {
                data_dir,
                from_height,
                to_height,
                archive_uri,
            })
            .map_err(|error| format!("rpc archive_window failed: {error}"))?;
            let target = format!("{from_height}-{to_height}");
            print_rpc_success(
                id,
                &bundle,
                vec![RpcEvent::new(
                    "archive_window",
                    target,
                    "archive window queried",
                )],
            )
        }
        "verify_blocks" => {
            let report = verify_blocks(NodeOptions { data_dir })
                .map_err(|error| format!("rpc verify_blocks failed: {error}"))?;
            print_rpc_success(
                id,
                &report,
                vec![RpcEvent::new(
                    "verify_blocks",
                    report.tip_hash.clone(),
                    "block log verified",
                )],
            )
        }
        "verify_state" => {
            let report = verify_state(NodeOptions { data_dir })
                .map_err(|error| format!("rpc verify_state failed: {error}"))?;
            print_rpc_success(
                id,
                &report,
                vec![RpcEvent::new(
                    "verify_state",
                    report.block_log.tip_hash.clone(),
                    "node state verified",
                )],
            )
        }
        "verify_bridge" => {
            let report = verify_bridge(NodeOptions { data_dir })
                .map_err(|error| format!("rpc verify_bridge failed: {error}"))?;
            print_rpc_success(
                id,
                &report,
                vec![RpcEvent::new(
                    "verify_bridge",
                    report.latest_transfer_id.clone(),
                    "bridge state verified",
                )],
            )
        }
        "verify_mempool" => {
            let report = verify_mempool(NodeOptions { data_dir })
                .map_err(|error| format!("rpc verify_mempool failed: {error}"))?;
            print_rpc_success(
                id,
                &report,
                vec![RpcEvent::new(
                    "verify_mempool",
                    report.pending_count.to_string(),
                    "mempool state verified",
                )],
            )
        }
        "verify_shielded" => {
            let report = verify_shielded(NodeOptions { data_dir })
                .map_err(|error| format!("rpc verify_shielded failed: {error}"))?;
            print_rpc_success(
                id,
                &report,
                vec![RpcEvent::new(
                    "verify_shielded",
                    report.tree_root.clone(),
                    "shielded state verified",
                )],
            )
        }
        "orchard_pool_report" => {
            let report = orchard_pool_report(OrchardPoolReportOptions { data_dir })
                .map_err(|error| format!("rpc orchard_pool_report failed: {error}"))?;
            print_rpc_success(
                id,
                &report,
                vec![RpcEvent::new(
                    "orchard_pool_report",
                    report.pool_id.clone(),
                    "Orchard pool report generated",
                )],
            )
        }
        "transfer" => {
            require_direct_state_enabled("rpc transfer")?;
            let to = flag_value(flags, "--to").ok_or("missing --to")?;
            let amount = parse_u64_flag(flags, "--amount")?;
            let key_file = flag_value(flags, "--key-file").map(PathBuf::from);
            let receipt = transfer(TransferOptions {
                data_dir,
                key_file,
                to: to.to_string(),
                amount,
            })
            .map_err(|error| format!("rpc transfer failed: {error}"))?;
            print_rpc_success(
                id,
                &receipt,
                vec![RpcEvent::new(
                    "transfer",
                    receipt.tx_id.clone(),
                    "transparent transfer submitted",
                )],
            )
        }
        "batch_transfer" => {
            let to = flag_value(flags, "--to").ok_or("missing --to")?;
            let amount = parse_u64_flag(flags, "--amount")?;
            let batch_file = flag_value(flags, "--batch-file").ok_or("missing --batch-file")?;
            let key_file = flag_value(flags, "--key-file").map(PathBuf::from);
            let batch = create_transfer_batch(BatchTransferOptions {
                data_dir,
                key_file,
                to: to.to_string(),
                amount,
                batch_file: PathBuf::from(batch_file),
            })
            .map_err(|error| format!("rpc batch_transfer failed: {error}"))?;
            print_rpc_success(
                id,
                &batch,
                vec![RpcEvent::new(
                    "batch_transfer",
                    batch.batch_id.clone(),
                    "transparent transfer batch created",
                )],
            )
        }
        "mempool_submit_transfer" => {
            let to = flag_value(flags, "--to").ok_or("missing --to")?;
            let amount = parse_u64_flag(flags, "--amount")?;
            let key_file = flag_value(flags, "--key-file").map(PathBuf::from);
            let entry = submit_transfer_to_mempool(TransferOptions {
                data_dir,
                key_file,
                to: to.to_string(),
                amount,
            })
            .map_err(|error| format!("rpc mempool_submit_transfer failed: {error}"))?;
            print_rpc_success(
                id,
                &entry,
                vec![RpcEvent::new(
                    "mempool_submit_transfer",
                    entry.tx_id.clone(),
                    "transparent transfer admitted to mempool",
                )],
            )
        }
        "mempool_submit_signed_transfer" => {
            let has_transfer_file = flag_value(flags, "--transfer-file");
            let has_signed_transfer_json = flag_value(flags, "--signed-transfer-json");
            let entry = match (has_transfer_file, has_signed_transfer_json) {
                (Some(transfer_file), None) => {
                    submit_signed_transfer_to_mempool(SignedTransferSubmitOptions {
                        data_dir,
                        transfer_file: PathBuf::from(transfer_file),
                    })
                }
                (None, Some(signed_transfer_json)) => {
                    submit_signed_transfer_json_to_mempool(SignedTransferJsonSubmitOptions {
                        data_dir,
                        signed_transfer_json: signed_transfer_json.to_string(),
                    })
                }
                (None, None) => Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "missing --transfer-file or --signed-transfer-json",
                )),
                (Some(_), Some(_)) => Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "use only one of --transfer-file or --signed-transfer-json",
                )),
            }
            .map_err(|error| format!("rpc mempool_submit_signed_transfer failed: {error}"))?;
            print_rpc_success(
                id,
                &entry,
                vec![RpcEvent::new(
                    "mempool_submit_signed_transfer",
                    entry.tx_id.clone(),
                    "externally signed transparent transfer admitted to mempool",
                )],
            )
        }
        "mempool_submit_signed_payment_v2" => {
            let signed_payment_v2_json = flag_value(flags, "--signed-payment-v2-json")
                .ok_or("missing --signed-payment-v2-json")?;
            let entry =
                submit_signed_payment_v2_json_to_mempool(SignedPaymentV2JsonSubmitOptions {
                    data_dir,
                    signed_payment_v2_json: signed_payment_v2_json.to_string(),
                })
                .map_err(|error| format!("rpc mempool_submit_signed_payment_v2 failed: {error}"))?;
            print_rpc_success(
                id,
                &entry,
                vec![RpcEvent::new(
                    "mempool_submit_signed_payment_v2",
                    entry.tx_id.clone(),
                    "externally signed payment v2 admitted to mempool",
                )],
            )
        }
        "mempool_submit_signed_asset_transaction" => {
            let signed_asset_transaction_json =
                flag_value(flags, "--signed-asset-transaction-json")
                    .ok_or("missing --signed-asset-transaction-json")?;
            let entry = submit_signed_asset_transaction_json_to_mempool(
                SignedAssetTransactionJsonSubmitOptions {
                    data_dir,
                    signed_asset_transaction_json: signed_asset_transaction_json.to_string(),
                },
            )
            .map_err(|error| {
                format!("rpc mempool_submit_signed_asset_transaction failed: {error}")
            })?;
            print_rpc_success(
                id,
                &entry,
                vec![RpcEvent::new(
                    "mempool_submit_signed_asset_transaction",
                    entry.tx_id.clone(),
                    "externally signed asset transaction admitted to mempool",
                )],
            )
        }
        "mempool_submit_signed_atomic_swap_transaction" => {
            let signed_atomic_swap_transaction_json =
                flag_value(flags, "--signed-atomic-swap-transaction-json")
                    .ok_or("missing --signed-atomic-swap-transaction-json")?;
            let entry = submit_signed_atomic_swap_transaction_json_to_mempool(
                SignedAtomicSwapTransactionJsonSubmitOptions {
                    data_dir,
                    signed_atomic_swap_transaction_json: signed_atomic_swap_transaction_json
                        .to_string(),
                },
            )
            .map_err(|error| {
                format!("rpc mempool_submit_signed_atomic_swap_transaction failed: {error}")
            })?;
            print_rpc_success(
                id,
                &entry,
                vec![RpcEvent::new(
                    "mempool_submit_signed_atomic_swap_transaction",
                    entry.tx_id.clone(),
                    "externally dual-signed atomic swap admitted to mempool",
                )],
            )
        }
        "mempool_submit_fastlane_primary" => {
            let transaction_json = flag_value(flags, "--fastlane-primary-json")
                .ok_or("missing --fastlane-primary-json")?;
            let transaction: postfiat_types::FastLanePrimaryTransactionV1 =
                serde_json::from_str(transaction_json)
                    .map_err(|error| format!("FastLane primary JSON parse failed: {error}"))?;
            let entry = admit_fastlane_primary_to_mempool(&data_dir, transaction)
                .map_err(|error| format!("rpc mempool_submit_fastlane_primary failed: {error}"))?;
            print_rpc_success(
                id,
                &entry,
                vec![RpcEvent::new(
                    "mempool_submit_fastlane_primary",
                    entry.tx_id.clone(),
                    "FastLane canonical bridge transaction admitted to mempool",
                )],
            )
        }
        "mempool_submit_signed_escrow_transaction" => {
            let signed_escrow_transaction_json =
                flag_value(flags, "--signed-escrow-transaction-json")
                    .ok_or("missing --signed-escrow-transaction-json")?;
            let entry = submit_signed_escrow_transaction_json_to_mempool(
                SignedEscrowTransactionJsonSubmitOptions {
                    data_dir,
                    signed_escrow_transaction_json: signed_escrow_transaction_json.to_string(),
                },
            )
            .map_err(|error| {
                format!("rpc mempool_submit_signed_escrow_transaction failed: {error}")
            })?;
            print_rpc_success(
                id,
                &entry,
                vec![RpcEvent::new(
                    "mempool_submit_signed_escrow_transaction",
                    entry.tx_id.clone(),
                    "externally signed escrow transaction admitted to mempool",
                )],
            )
        }
        "mempool_submit_signed_nft_transaction" => {
            let signed_nft_transaction_json = flag_value(flags, "--signed-nft-transaction-json")
                .ok_or("missing --signed-nft-transaction-json")?;
            let entry = submit_signed_nft_transaction_json_to_mempool(
                SignedNftTransactionJsonSubmitOptions {
                    data_dir,
                    signed_nft_transaction_json: signed_nft_transaction_json.to_string(),
                },
            )
            .map_err(|error| {
                format!("rpc mempool_submit_signed_nft_transaction failed: {error}")
            })?;
            print_rpc_success(
                id,
                &entry,
                vec![RpcEvent::new(
                    "mempool_submit_signed_nft_transaction",
                    entry.tx_id.clone(),
                    "externally signed nft transaction admitted to mempool",
                )],
            )
        }
        "mempool_submit_signed_offer_transaction" => {
            let signed_offer_transaction_json =
                flag_value(flags, "--signed-offer-transaction-json")
                    .ok_or("missing --signed-offer-transaction-json")?;
            let entry = submit_signed_offer_transaction_json_to_mempool(
                SignedOfferTransactionJsonSubmitOptions {
                    data_dir,
                    signed_offer_transaction_json: signed_offer_transaction_json.to_string(),
                },
            )
            .map_err(|error| {
                format!("rpc mempool_submit_signed_offer_transaction failed: {error}")
            })?;
            print_rpc_success(
                id,
                &entry,
                vec![RpcEvent::new(
                    "mempool_submit_signed_offer_transaction",
                    entry.tx_id.clone(),
                    "externally signed offer transaction admitted to mempool",
                )],
            )
        }
        "mempool_batch" => {
            let batch_file = flag_value(flags, "--batch-file").ok_or("missing --batch-file")?;
            let max_transactions = flag_value(flags, "--max-transactions")
                .unwrap_or("100")
                .parse::<usize>()
                .map_err(|_| "--max-transactions must be a usize".to_string())?;
            let batch = create_mempool_batch(MempoolBatchOptions {
                data_dir,
                batch_file: PathBuf::from(batch_file),
                max_transactions,
            })
            .map_err(|error| format!("rpc mempool_batch failed: {error}"))?;
            print_rpc_success(
                id,
                &batch,
                vec![RpcEvent::new(
                    "mempool_batch",
                    batch.batch_id.clone(),
                    "mempool batch sealed",
                )],
            )
        }
        "mempool_status" => {
            let mempool = mempool_state(NodeOptions { data_dir })
                .map_err(|error| format!("rpc mempool_status failed: {error}"))?;
            print_rpc_success(
                id,
                &mempool,
                vec![RpcEvent::new(
                    "mempool_status",
                    mempool.len().to_string(),
                    "mempool queried",
                )],
            )
        }
        "apply_batch" => {
            let batch_file = flag_value(flags, "--batch-file").ok_or("missing --batch-file")?;
            let receipts = apply_batch(ApplyBatchOptions {
                data_dir,
                batch_file: PathBuf::from(batch_file),

                certificate_file: None,
            })
            .map_err(|error| format!("rpc apply_batch failed: {error}"))?;
            print_rpc_success(
                id,
                &receipts,
                vec![RpcEvent::new(
                    "apply_batch",
                    batch_file,
                    "transparent transfer batch applied",
                )],
            )
        }
        "shield_mint" => {
            require_direct_state_enabled("rpc shield_mint")?;
            let owner = flag_value(flags, "--owner").ok_or("missing --owner")?;
            let amount = parse_u64_flag(flags, "--amount")?;
            let asset_id = flag_value(flags, "--asset-id").unwrap_or(DEFAULT_SHIELDED_ASSET_ID);
            let memo = flag_value(flags, "--memo").unwrap_or("");
            let note = shield_mint(ShieldMintOptions {
                data_dir,
                owner: owner.to_string(),
                asset_id: asset_id.to_string(),
                amount,
                memo: memo.to_string(),
            })
            .map_err(|error| format!("rpc shield_mint failed: {error}"))?;
            print_rpc_success(
                id,
                &note,
                vec![RpcEvent::new(
                    "shield_mint",
                    note.note_id.clone(),
                    "shielded note minted",
                )],
            )
        }
        "shield_spend" => {
            require_direct_state_enabled("rpc shield_spend")?;
            let note_id = flag_value(flags, "--note-id").ok_or("missing --note-id")?;
            let to = flag_value(flags, "--to").ok_or("missing --to")?;
            let amount = parse_u64_flag(flags, "--amount")?;
            let memo = flag_value(flags, "--memo").unwrap_or("");
            let spend = shield_spend(ShieldSpendOptions {
                data_dir,
                note_id: note_id.to_string(),
                to: to.to_string(),
                amount,
                memo: memo.to_string(),
            })
            .map_err(|error| format!("rpc shield_spend failed: {error}"))?;
            print_rpc_success(
                id,
                &spend,
                vec![RpcEvent::new(
                    "shield_spend",
                    spend.spend_id.clone(),
                    "shielded spend submitted",
                )],
            )
        }
        "shield_batch_mint" => {
            let owner = flag_value(flags, "--owner").ok_or("missing --owner")?;
            let amount = parse_u64_flag(flags, "--amount")?;
            let asset_id = flag_value(flags, "--asset-id").unwrap_or(DEFAULT_SHIELDED_ASSET_ID);
            let memo = flag_value(flags, "--memo").unwrap_or("");
            let batch_file = flag_value(flags, "--batch-file").ok_or("missing --batch-file")?;
            let batch = create_shielded_mint_batch(ShieldMintBatchOptions {
                data_dir,
                owner: owner.to_string(),
                asset_id: asset_id.to_string(),
                amount,
                memo: memo.to_string(),
                batch_file: PathBuf::from(batch_file),
            })
            .map_err(|error| format!("rpc shield_batch_mint failed: {error}"))?;
            print_rpc_success(
                id,
                &batch,
                vec![RpcEvent::new(
                    "shield_batch_mint",
                    batch.batch_id.clone(),
                    "shielded mint batch created",
                )],
            )
        }
        "shield_batch_spend" => {
            let note_id = flag_value(flags, "--note-id").ok_or("missing --note-id")?;
            let to = flag_value(flags, "--to").ok_or("missing --to")?;
            let amount = parse_u64_flag(flags, "--amount")?;
            let memo = flag_value(flags, "--memo").unwrap_or("");
            let batch_file = flag_value(flags, "--batch-file").ok_or("missing --batch-file")?;
            let batch = create_shielded_spend_batch(ShieldSpendBatchOptions {
                data_dir,
                note_id: note_id.to_string(),
                to: to.to_string(),
                amount,
                memo: memo.to_string(),
                batch_file: PathBuf::from(batch_file),
            })
            .map_err(|error| format!("rpc shield_batch_spend failed: {error}"))?;
            print_rpc_success(
                id,
                &batch,
                vec![RpcEvent::new(
                    "shield_batch_spend",
                    batch.batch_id.clone(),
                    "shielded spend batch created",
                )],
            )
        }
        "shield_batch_migrate" => {
            let note_id = flag_value(flags, "--note-id").ok_or("missing --note-id")?;
            let target_pool = flag_value(flags, "--target-pool").ok_or("missing --target-pool")?;
            let memo = flag_value(flags, "--memo").unwrap_or("");
            let batch_file = flag_value(flags, "--batch-file").ok_or("missing --batch-file")?;
            let batch = create_shielded_migrate_batch(ShieldMigrateBatchOptions {
                data_dir,
                note_id: note_id.to_string(),
                target_pool: target_pool.to_string(),
                memo: memo.to_string(),
                batch_file: PathBuf::from(batch_file),
            })
            .map_err(|error| format!("rpc shield_batch_migrate failed: {error}"))?;
            print_rpc_success(
                id,
                &batch,
                vec![RpcEvent::new(
                    "shield_batch_migrate",
                    batch.batch_id.clone(),
                    "shielded turnstile migration batch created",
                )],
            )
        }
        "shield_batch_orchard" => {
            let action_file = rpc_orchard_action_source_file(&data_dir, id, flags)?;
            let batch_file = rpc_orchard_batch_file(&data_dir, id, flags, "orchard")?;
            let batch = create_orchard_action_batch(OrchardActionBatchOptions {
                data_dir,
                action_file: action_file.path,
                batch_file,
            })
            .map_err(|error| format!("rpc shield_batch_orchard failed: {error}"))?;
            cleanup_rpc_orchard_action_source(action_file.cleanup_path)?;
            print_rpc_success(
                id,
                &batch,
                vec![RpcEvent::new(
                    "shield_batch_orchard",
                    batch.batch_id.clone(),
                    "Orchard shielded action batch created",
                )],
            )
        }
        "shield_batch_orchard_deposit" => {
            let deposit_file = rpc_orchard_deposit_source_file(&data_dir, id, flags)?;
            let batch_file = rpc_orchard_batch_file(&data_dir, id, flags, "orchard-deposit")?;
            let batch = create_orchard_deposit_action_batch(OrchardDepositActionBatchOptions {
                data_dir,
                deposit_file: deposit_file.path,
                batch_file,
            })
            .map_err(|error| format!("rpc shield_batch_orchard_deposit failed: {error}"))?;
            cleanup_rpc_orchard_action_source(deposit_file.cleanup_path)?;
            print_rpc_success(
                id,
                &batch,
                vec![RpcEvent::new(
                    "shield_batch_orchard_deposit",
                    batch.batch_id.clone(),
                    "Orchard shielded deposit batch created",
                )],
            )
        }
        "shield_batch_asset_orchard_ingress" => {
            let ingress_file = rpc_asset_orchard_ingress_source_file(&data_dir, id, flags)?;
            let batch_file = rpc_orchard_batch_file(&data_dir, id, flags, "asset-orchard-ingress")?;
            let batch = create_asset_orchard_ingress_batch(AssetOrchardIngressBatchOptions {
                data_dir,
                ingress_file: ingress_file.path,
                batch_file,
            })
            .map_err(|error| format!("rpc shield_batch_asset_orchard_ingress failed: {error}"))?;
            cleanup_rpc_orchard_action_source(ingress_file.cleanup_path)?;
            print_rpc_success(
                id,
                &batch,
                vec![RpcEvent::new(
                    "shield_batch_asset_orchard_ingress",
                    batch.batch_id.clone(),
                    "AssetOrchard ingress batch created",
                )],
            )
        }
        "asset_orchard_swap_create" => {
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
                data_dir,
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
            .map_err(|error| format!("rpc asset_orchard_swap_create failed: {error}"))?;
            print_rpc_success(
                id,
                &report,
                vec![RpcEvent::new(
                    "asset_orchard_swap_create",
                    report.action_file.clone(),
                    "AssetOrchard swap action created",
                )],
            )
        }
        "shield_batch_orchard_withdraw" => {
            let action_file = rpc_orchard_action_source_file(&data_dir, id, flags)?;
            let to = flag_value(flags, "--to").ok_or("missing --to")?;
            let amount = parse_u64_flag(flags, "--amount")?;
            let fee = parse_u64_flag(flags, "--fee")?;
            let policy_id = flag_value(flags, "--policy-id").map(str::to_string);
            let disclosure_hash = flag_value(flags, "--disclosure-hash").map(str::to_string);
            let batch_file = rpc_orchard_batch_file(&data_dir, id, flags, "orchard-withdraw")?;
            let batch = create_orchard_withdraw_action_batch(OrchardWithdrawActionBatchOptions {
                data_dir,
                action_file: action_file.path,
                to: to.to_string(),
                amount,
                fee,
                policy_id,
                disclosure_hash,
                batch_file,
            })
            .map_err(|error| format!("rpc shield_batch_orchard_withdraw failed: {error}"))?;
            cleanup_rpc_orchard_action_source(action_file.cleanup_path)?;
            print_rpc_success(
                id,
                &batch,
                vec![RpcEvent::new(
                    "shield_batch_orchard_withdraw",
                    batch.batch_id.clone(),
                    "Orchard shielded withdraw batch created",
                )],
            )
        }
        "shield_batch_swap" => {
            let swap_file = rpc_shielded_swap_source_file(&data_dir, id, flags)?;
            let batch_file = rpc_orchard_batch_file(&data_dir, id, flags, "shielded-swap")?;
            let batch = create_shielded_swap_action_batch(ShieldedSwapActionBatchOptions {
                data_dir,
                swap_file: swap_file.path,
                batch_file,
            })
            .map_err(|error| format!("rpc shield_batch_swap failed: {error}"))?;
            cleanup_rpc_orchard_action_source(swap_file.cleanup_path)?;
            print_rpc_success(
                id,
                &batch,
                vec![RpcEvent::new(
                    "shield_batch_swap",
                    batch.batch_id.clone(),
                    "ShieldedSwap batch created",
                )],
            )
        }
        "apply_shield_batch" => {
            let batch_file = flag_value(flags, "--batch-file").ok_or("missing --batch-file")?;
            let receipts = apply_shielded_batch(ApplyBatchOptions {
                data_dir,
                batch_file: PathBuf::from(batch_file),

                certificate_file: None,
            })
            .map_err(|error| format!("rpc apply_shield_batch failed: {error}"))?;
            print_rpc_success(
                id,
                &receipts,
                vec![RpcEvent::new(
                    "apply_shield_batch",
                    batch_file,
                    "shielded action batch applied",
                )],
            )
        }
        "shield_scan" => {
            let owner = flag_value(flags, "--owner").ok_or("missing --owner")?;
            let notes = shield_scan(NodeOptions { data_dir }, owner)
                .map_err(|error| format!("rpc shield_scan failed: {error}"))?;
            print_rpc_success(
                id,
                &notes,
                vec![RpcEvent::new(
                    "shield_scan",
                    owner,
                    "shielded notes scanned",
                )],
            )
        }
        "shield_disclose" => {
            let note_id = flag_value(flags, "--note-id").ok_or("missing --note-id")?;
            let disclosure = shield_disclose(NodeOptions { data_dir }, note_id)
                .map_err(|error| format!("rpc shield_disclose failed: {error}"))?;
            print_rpc_success(
                id,
                &disclosure,
                vec![RpcEvent::new(
                    "shield_disclose",
                    note_id,
                    "shielded note disclosed",
                )],
            )
        }
        "shield_turnstile" => {
            let summary = shield_turnstile(NodeOptions { data_dir })
                .map_err(|error| format!("rpc shield_turnstile failed: {error}"))?;
            print_rpc_success(
                id,
                &summary,
                vec![RpcEvent::new(
                    "shield_turnstile",
                    summary.event_count.to_string(),
                    "shielded turnstile accounting queried",
                )],
            )
        }
        "bridge_domain" => {
            require_direct_state_enabled("rpc bridge_domain")?;
            let domain_id = flag_value(flags, "--domain-id").unwrap_or(DEFAULT_BRIDGE_DOMAIN_ID);
            let name = flag_value(flags, "--name").unwrap_or("Local Simulation");
            let source_chain = flag_value(flags, "--source-chain").unwrap_or(domain_id);
            let target_chain = flag_value(flags, "--target-chain").unwrap_or(DEFAULT_CHAIN_ID);
            let bridge_id = flag_value(flags, "--bridge-id").unwrap_or(domain_id);
            let door_account = flag_value(flags, "--door-account")
                .map(str::to_string)
                .unwrap_or_else(|| format!("door:{domain_id}"));
            let inbound_cap = parse_u64_flag(flags, "--inbound-cap")?;
            let outbound_cap = parse_u64_flag(flags, "--outbound-cap")?;
            let domain = bridge_upsert_domain(BridgeDomainOptions {
                data_dir,
                domain_id: domain_id.to_string(),
                name: name.to_string(),
                source_chain: source_chain.to_string(),
                target_chain: target_chain.to_string(),
                bridge_id: bridge_id.to_string(),
                door_account,
                inbound_cap,
                outbound_cap,
            })
            .map_err(|error| format!("rpc bridge_domain failed: {error}"))?;
            print_rpc_success(
                id,
                &domain,
                vec![RpcEvent::new(
                    "bridge_domain",
                    domain.domain_id.clone(),
                    "bridge domain upserted",
                )],
            )
        }
        "bridge_transfer" => {
            require_direct_state_enabled("rpc bridge_transfer")?;
            let domain_id = flag_value(flags, "--domain-id").unwrap_or(DEFAULT_BRIDGE_DOMAIN_ID);
            let direction = flag_value(flags, "--direction").unwrap_or(BRIDGE_DIRECTION_INBOUND);
            let from = flag_value(flags, "--from").ok_or("missing --from")?;
            let to = flag_value(flags, "--to").ok_or("missing --to")?;
            let asset_id = flag_value(flags, "--asset-id").unwrap_or(DEFAULT_SHIELDED_ASSET_ID);
            let amount = parse_u64_flag(flags, "--amount")?;
            let witness_id = flag_value(flags, "--witness-id").ok_or("missing --witness-id")?;
            let witness_epoch = parse_optional_u32_flag(flags, "--witness-epoch")?;
            let witness_signer =
                flag_value(flags, "--witness-signer").unwrap_or(DEFAULT_BRIDGE_WITNESS_SIGNER);
            let transfer = bridge_transfer(BridgeTransferOptions {
                data_dir,
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
            .map_err(|error| format!("rpc bridge_transfer failed: {error}"))?;
            print_rpc_success(
                id,
                &transfer,
                vec![RpcEvent::new(
                    "bridge_transfer",
                    transfer.transfer_id.clone(),
                    "bridge transfer submitted",
                )],
            )
        }
        "bridge_pause" | "bridge_resume" => {
            require_direct_state_enabled(&format!("rpc {method}"))?;
            let domain_id = flag_value(flags, "--domain-id").unwrap_or(DEFAULT_BRIDGE_DOMAIN_ID);
            let paused = method == "bridge_pause";
            let domain = bridge_pause(BridgePauseOptions {
                data_dir,
                domain_id: domain_id.to_string(),
                paused,
            })
            .map_err(|error| format!("rpc {method} failed: {error}"))?;
            print_rpc_success(
                id,
                &domain,
                vec![RpcEvent::new(
                    method,
                    domain.domain_id.clone(),
                    if paused {
                        "bridge domain paused"
                    } else {
                        "bridge domain resumed"
                    },
                )],
            )
        }
        "bridge_status" => {
            let bridge = bridge_state(NodeOptions { data_dir })
                .map_err(|error| format!("rpc bridge_status failed: {error}"))?;
            print_rpc_success(
                id,
                &bridge,
                vec![RpcEvent::new(
                    "bridge_status",
                    "bridge",
                    "bridge state queried",
                )],
            )
        }
        "bridge_batch_domain" => {
            let domain_id = flag_value(flags, "--domain-id").unwrap_or(DEFAULT_BRIDGE_DOMAIN_ID);
            let name = flag_value(flags, "--name").unwrap_or("Local Simulation");
            let source_chain = flag_value(flags, "--source-chain").unwrap_or(domain_id);
            let target_chain = flag_value(flags, "--target-chain").unwrap_or(DEFAULT_CHAIN_ID);
            let bridge_id = flag_value(flags, "--bridge-id").unwrap_or(domain_id);
            let door_account = flag_value(flags, "--door-account")
                .map(str::to_string)
                .unwrap_or_else(|| format!("door:{domain_id}"));
            let inbound_cap = parse_u64_flag(flags, "--inbound-cap")?;
            let outbound_cap = parse_u64_flag(flags, "--outbound-cap")?;
            let batch_file = flag_value(flags, "--batch-file").ok_or("missing --batch-file")?;
            let batch = create_bridge_domain_batch(BridgeDomainBatchOptions {
                data_dir,
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
            .map_err(|error| format!("rpc bridge_batch_domain failed: {error}"))?;
            print_rpc_success(
                id,
                &batch,
                vec![RpcEvent::new(
                    "bridge_batch_domain",
                    batch.batch_id.clone(),
                    "bridge domain batch created",
                )],
            )
        }
        "bridge_batch_transfer" => {
            let domain_id = flag_value(flags, "--domain-id").unwrap_or(DEFAULT_BRIDGE_DOMAIN_ID);
            let direction = flag_value(flags, "--direction").unwrap_or(BRIDGE_DIRECTION_INBOUND);
            let from = flag_value(flags, "--from").ok_or("missing --from")?;
            let to = flag_value(flags, "--to").ok_or("missing --to")?;
            let asset_id = flag_value(flags, "--asset-id").unwrap_or(DEFAULT_SHIELDED_ASSET_ID);
            let amount = parse_u64_flag(flags, "--amount")?;
            let witness_id = flag_value(flags, "--witness-id").ok_or("missing --witness-id")?;
            let witness_epoch = parse_optional_u32_flag(flags, "--witness-epoch")?;
            let witness_signer =
                flag_value(flags, "--witness-signer").unwrap_or(DEFAULT_BRIDGE_WITNESS_SIGNER);
            let batch_file = flag_value(flags, "--batch-file").ok_or("missing --batch-file")?;
            let batch = create_bridge_transfer_batch(BridgeTransferBatchOptions {
                data_dir,
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
            .map_err(|error| format!("rpc bridge_batch_transfer failed: {error}"))?;
            print_rpc_success(
                id,
                &batch,
                vec![RpcEvent::new(
                    "bridge_batch_transfer",
                    batch.batch_id.clone(),
                    "bridge transfer batch created",
                )],
            )
        }
        "bridge_batch_pause" | "bridge_batch_resume" => {
            let domain_id = flag_value(flags, "--domain-id").unwrap_or(DEFAULT_BRIDGE_DOMAIN_ID);
            let batch_file = flag_value(flags, "--batch-file").ok_or("missing --batch-file")?;
            let paused = method == "bridge_batch_pause";
            let batch = create_bridge_pause_batch(BridgePauseBatchOptions {
                data_dir,
                domain_id: domain_id.to_string(),
                paused,
                batch_file: PathBuf::from(batch_file),
            })
            .map_err(|error| format!("rpc {method} failed: {error}"))?;
            print_rpc_success(
                id,
                &batch,
                vec![RpcEvent::new(
                    method,
                    batch.batch_id.clone(),
                    if paused {
                        "bridge pause batch created"
                    } else {
                        "bridge resume batch created"
                    },
                )],
            )
        }
        "apply_bridge_batch" => {
            let batch_file = flag_value(flags, "--batch-file").ok_or("missing --batch-file")?;
            let receipts = apply_bridge_batch(ApplyBatchOptions {
                data_dir,
                batch_file: PathBuf::from(batch_file),

                certificate_file: None,
            })
            .map_err(|error| format!("rpc apply_bridge_batch failed: {error}"))?;
            print_rpc_success(
                id,
                &receipts,
                vec![RpcEvent::new(
                    "apply_bridge_batch",
                    batch_file,
                    "bridge action batch applied",
                )],
            )
        }
        other => Err(format!("unknown rpc method `{other}`")),
    }
}

fn rpc_server_info(data_dir: PathBuf) -> Result<serde_json::Value, String> {
    let status_report = status(NodeOptions {
        data_dir: data_dir.clone(),
    })
    .map_err(|error| format!("rpc server_info status failed: {error}"))?;
    let metrics_report = metrics(NodeOptions { data_dir });
    let (metrics_report, metrics_error) = match metrics_report {
        Ok(report) => (Some(report), None),
        Err(error) => (
            None,
            Some(format!("rpc server_info metrics failed: {error}")),
        ),
    };
    Ok(rpc_server_info_response(
        &status_report,
        metrics_report.as_ref(),
        metrics_error.as_deref(),
    ))
}

fn rpc_server_info_response(
    status_report: &StatusReport,
    metrics_report: Option<&postfiat_node::NodeMetrics>,
    metrics_error: Option<&str>,
) -> serde_json::Value {
    let registry_update_count = metrics_report
        .map(|report| serde_json::json!(report.consensus.validator_registry_update_count))
        .unwrap_or(serde_json::Value::Null);
    let minimum_transfer_fee = metrics_report
        .map(|report| report.execution.minimum_transfer_fee)
        .unwrap_or(postfiat_execution::MIN_TRANSFER_FEE);
    let account_reserve = metrics_report
        .map(|report| report.execution.account_reserve)
        .unwrap_or(postfiat_execution::ACCOUNT_RESERVE);
    let transfer_account_creation_fee = metrics_report
        .map(|report| report.execution.transfer_account_creation_fee)
        .unwrap_or(postfiat_execution::TRANSFER_ACCOUNT_CREATION_FEE);
    let mut response = serde_json::json!({
        "schema": "postfiat-server-info-v1",
        "chain_id": &status_report.chain_id,
        "genesis_hash": &status_report.genesis_hash,
        "protocol_version": status_report.protocol_version,
        "node_id": &status_report.node_id,
        "status": &status_report.status,
        "ledger": {
            "height": status_report.block_height,
            "hash": &status_report.block_tip_hash,
            "state_root": &status_report.state_root
        },
        "validators": {
            "active_count": status_report.validator_count,
            "registry_update_count": registry_update_count
        },
        "fees": {
            "minimum_transfer_fee": minimum_transfer_fee,
            "account_reserve": account_reserve,
            "transfer_account_creation_fee": transfer_account_creation_fee
        },
        "mempool": {
            "pending": status_report.mempool_pending
        },
        "rpc": {
            "version": postfiat_rpc_sdk::RPC_VERSION,
            "read_aliases": [
                "server_info",
                "ledger",
                "account",
                "tx",
                "validators",
                "manifests",
                "fee",
                "transfer_fee_quote"
            ]
        },
        "metrics": {
            "ok": metrics_report.is_some()
        }
    });
    if let Some(error) = metrics_error {
        response["metrics"]["error"] = serde_json::json!(error);
        response["warnings"] = serde_json::json!([{
            "code": "server_info_metrics_unavailable",
            "message": error
        }]);
    }
    response
}

fn rpc_ledger_alias(data_dir: PathBuf, limit: Option<usize>) -> Result<serde_json::Value, String> {
    let metrics_report = metrics(NodeOptions {
        data_dir: data_dir.clone(),
    })
    .map_err(|error| format!("rpc ledger metrics failed: {error}"))?;
    let block_log = blocks(BlockQueryOptions {
        data_dir,
        from_height: None,
        limit,
    })
    .map_err(|error| format!("rpc ledger blocks failed: {error}"))?;
    Ok(serde_json::json!({
        "schema": "postfiat-ledger-v1",
        "chain_id": metrics_report.chain_id,
        "genesis_hash": metrics_report.genesis_hash,
        "protocol_version": metrics_report.protocol_version,
        "ledger_index": metrics_report.ordering.block_height,
        "ledger_hash": metrics_report.ordering.block_tip_hash,
        "state_root": metrics_report.execution.state_root,
        "account_count": metrics_report.execution.account_count,
        "receipt_count": metrics_report.execution.receipt_count,
        "burned_fee_total": metrics_report.execution.burned_fee_total,
        "returned_block_count": block_log.len() as u64,
        "blocks": block_log
    }))
}

fn rpc_fee_alias(data_dir: PathBuf) -> Result<serde_json::Value, String> {
    let metrics_report =
        metrics(NodeOptions { data_dir }).map_err(|error| format!("rpc fee failed: {error}"))?;
    Ok(serde_json::json!({
        "schema": "postfiat-fee-v1",
        "chain_id": metrics_report.chain_id,
        "genesis_hash": metrics_report.genesis_hash,
        "protocol_version": metrics_report.protocol_version,
        "minimum_transfer_fee": metrics_report.execution.minimum_transfer_fee,
        "account_reserve": metrics_report.execution.account_reserve,
        "transfer_account_creation_fee": metrics_report.execution.transfer_account_creation_fee,
        "transfer_fee_byte_quantum": metrics_report.execution.transfer_fee_byte_quantum,
        "transfer_fee_per_quantum": metrics_report.execution.transfer_fee_per_quantum,
        "burned_fee_total": metrics_report.execution.burned_fee_total
    }))
}

fn rpc_validators_alias(data_dir: PathBuf) -> Result<serde_json::Value, String> {
    let status_report = status(NodeOptions {
        data_dir: data_dir.clone(),
    })
    .map_err(|error| format!("rpc validators status failed: {error}"))?;
    let registry_path = data_dir.join(VALIDATOR_REGISTRY_FILE);
    let raw = std::fs::read_to_string(&registry_path)
        .map_err(|error| format!("rpc validators registry read failed: {error}"))?;
    let registry: serde_json::Value = serde_json::from_str(&raw)
        .map_err(|error| format!("rpc validators registry parse failed: {error}"))?;
    let records = registry
        .get("validators")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| "rpc validators registry missing validators array".to_string())?;
    let validators = records
        .iter()
        .map(|record| {
            record
                .get("node_id")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
                .ok_or_else(|| "rpc validators registry record missing node_id".to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    let root = validator_registry_root_report(ValidatorRegistryRootOptions {
        data_dir,
        registry_file: Some(registry_path.clone()),
        validators,
    })
    .map_err(|error| format!("rpc validators registry root failed: {error}"))?;
    Ok(serde_json::json!({
        "schema": "postfiat-validators-v1",
        "chain_id": status_report.chain_id,
        "genesis_hash": status_report.genesis_hash,
        "protocol_version": status_report.protocol_version,
        "validator_count": root.validator_count,
        "registry_root": root.registry_root,
        "source_file": registry_path.display().to_string(),
        "validators": records
    }))
}

fn rpc_manifests_alias(data_dir: PathBuf) -> Result<serde_json::Value, String> {
    let status_report = status(NodeOptions {
        data_dir: data_dir.clone(),
    })
    .map_err(|error| format!("rpc manifests status failed: {error}"))?;
    let bundle_file = data_dir.join("governance-genesis-bundle.json");
    if !bundle_file.is_file() {
        return Ok(serde_json::json!({
            "schema": "postfiat-manifests-v1",
            "chain_id": status_report.chain_id,
            "genesis_hash": status_report.genesis_hash,
            "protocol_version": status_report.protocol_version,
            "available": false,
            "source": bundle_file.display().to_string(),
            "network": "",
            "quorum": 0,
            "bundle_hash": "",
            "manifest_count": 0,
            "manifests": []
        }));
    }
    let verification = verify_governance_genesis_bundle(GovernanceGenesisVerifyOptions {
        data_dir,
        bundle_file: bundle_file.clone(),
    })
    .map_err(|error| format!("rpc manifests governance genesis verification failed: {error}"))?;
    let raw = std::fs::read_to_string(&bundle_file)
        .map_err(|error| format!("rpc manifests bundle read failed: {error}"))?;
    let bundle: serde_json::Value = serde_json::from_str(&raw)
        .map_err(|error| format!("rpc manifests bundle parse failed: {error}"))?;
    let manifests = bundle
        .get("operator_manifests")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| "rpc manifests bundle missing operator_manifests array".to_string())?;
    Ok(serde_json::json!({
        "schema": "postfiat-manifests-v1",
        "chain_id": verification.chain_id,
        "genesis_hash": verification.genesis_hash,
        "protocol_version": verification.protocol_version,
        "available": true,
        "source": verification.bundle_file,
        "network": verification.network,
        "quorum": verification.quorum,
        "bundle_hash": verification.bundle_hash,
        "manifest_count": verification.operator_manifest_count,
        "manifests": manifests
    }))
}

fn run_rpc_request_file(flags: &[String], request_file: &str) -> Result<(), String> {
    let request = read_request_file(request_file)
        .map_err(|error| format!("rpc request read failed: {error}"))?;

    let params = request
        .params
        .as_object()
        .ok_or("rpc request params must be an object")?;
    let mut rpc_flags = vec![
        "--id".to_string(),
        request.id,
        "--method".to_string(),
        request.method,
    ];
    if !params.contains_key("data_dir") {
        if let Some(data_dir) = flag_value(flags, "--data-dir") {
            rpc_flags.push("--data-dir".to_string());
            rpc_flags.push(data_dir.to_string());
        }
    }

    let mut keys = params.keys().collect::<Vec<_>>();
    keys.sort();
    for key in keys {
        append_rpc_param_flags(&mut rpc_flags, key, &params[key])?;
    }

    run_rpc(&rpc_flags)
}

fn append_rpc_param_flags(
    rpc_flags: &mut Vec<String>,
    key: &str,
    value: &serde_json::Value,
) -> Result<(), String> {
    if value.is_null() {
        return Ok(());
    }
    let flag = format!("--{}", key.replace('_', "-"));
    if let Some(value) = value.as_bool() {
        if value {
            rpc_flags.push(flag);
        }
        return Ok(());
    }
    rpc_flags.push(flag);
    rpc_flags.push(rpc_param_value(value)?);
    Ok(())
}

struct RpcOrchardActionSource {
    path: PathBuf,
    cleanup_path: Option<PathBuf>,
}

fn rpc_orchard_action_source_file(
    data_dir: &Path,
    rpc_id: &str,
    flags: &[String],
) -> Result<RpcOrchardActionSource, String> {
    let action_file = flag_value(flags, "--action-file");
    let action_json = flag_value(flags, "--action-json");
    match (action_file, action_json) {
        (Some(action_file), None) => Ok(RpcOrchardActionSource {
            path: PathBuf::from(action_file),
            cleanup_path: None,
        }),
        (None, Some(action_json)) => {
            let path = rpc_orchard_spool_file(
                data_dir,
                RPC_ORCHARD_ACTION_SPOOL_DIR,
                rpc_id,
                "action.json",
            )?;
            rpc_orchard_write_spooled_json(
                data_dir,
                RPC_ORCHARD_ACTION_SPOOL_DIR,
                &path,
                "rpc Orchard action",
                action_json,
            )?;
            Ok(RpcOrchardActionSource {
                path: path.clone(),
                cleanup_path: Some(path),
            })
        }
        (None, None) => Err("missing --action-file or --action-json".to_string()),
        (Some(_), Some(_)) => Err("use only one of --action-file or --action-json".to_string()),
    }
}

fn rpc_orchard_deposit_source_file(
    data_dir: &Path,
    rpc_id: &str,
    flags: &[String],
) -> Result<RpcOrchardActionSource, String> {
    let deposit_file = flag_value(flags, "--deposit-file");
    let deposit_json = flag_value(flags, "--deposit-json");
    match (deposit_file, deposit_json) {
        (Some(deposit_file), None) => Ok(RpcOrchardActionSource {
            path: PathBuf::from(deposit_file),
            cleanup_path: None,
        }),
        (None, Some(deposit_json)) => {
            let path = rpc_orchard_spool_file(
                data_dir,
                RPC_ORCHARD_ACTION_SPOOL_DIR,
                rpc_id,
                "deposit.json",
            )?;
            rpc_orchard_write_spooled_json(
                data_dir,
                RPC_ORCHARD_ACTION_SPOOL_DIR,
                &path,
                "rpc Orchard deposit",
                deposit_json,
            )?;
            Ok(RpcOrchardActionSource {
                path: path.clone(),
                cleanup_path: Some(path),
            })
        }
        (None, None) => Err("missing --deposit-file or --deposit-json".to_string()),
        (Some(_), Some(_)) => Err("use only one of --deposit-file or --deposit-json".to_string()),
    }
}

fn rpc_asset_orchard_ingress_source_file(
    data_dir: &Path,
    rpc_id: &str,
    flags: &[String],
) -> Result<RpcOrchardActionSource, String> {
    let ingress_file = flag_value(flags, "--ingress-file");
    let ingress_json = flag_value(flags, "--ingress-json");
    match (ingress_file, ingress_json) {
        (Some(ingress_file), None) => Ok(RpcOrchardActionSource {
            path: PathBuf::from(ingress_file),
            cleanup_path: None,
        }),
        (None, Some(ingress_json)) => {
            let path = rpc_orchard_spool_file(
                data_dir,
                RPC_ORCHARD_ACTION_SPOOL_DIR,
                rpc_id,
                "asset-orchard-ingress.json",
            )?;
            rpc_orchard_write_spooled_json(
                data_dir,
                RPC_ORCHARD_ACTION_SPOOL_DIR,
                &path,
                "rpc AssetOrchard ingress",
                ingress_json,
            )?;
            Ok(RpcOrchardActionSource {
                path: path.clone(),
                cleanup_path: Some(path),
            })
        }
        (None, None) => Err("missing --ingress-file or --ingress-json".to_string()),
        (Some(_), Some(_)) => Err("use only one of --ingress-file or --ingress-json".to_string()),
    }
}

fn rpc_shielded_swap_source_file(
    data_dir: &Path,
    rpc_id: &str,
    flags: &[String],
) -> Result<RpcOrchardActionSource, String> {
    let swap_file = flag_value(flags, "--swap-file");
    let swap_json = flag_value(flags, "--swap-json");
    match (swap_file, swap_json) {
        (Some(swap_file), None) => Ok(RpcOrchardActionSource {
            path: PathBuf::from(swap_file),
            cleanup_path: None,
        }),
        (None, Some(swap_json)) => {
            let path = rpc_orchard_spool_file(
                data_dir,
                RPC_ORCHARD_ACTION_SPOOL_DIR,
                rpc_id,
                "swap.json",
            )?;
            rpc_orchard_write_spooled_json(
                data_dir,
                RPC_ORCHARD_ACTION_SPOOL_DIR,
                &path,
                "rpc ShieldedSwap",
                swap_json,
            )?;
            Ok(RpcOrchardActionSource {
                path: path.clone(),
                cleanup_path: Some(path),
            })
        }
        (None, None) => Err("missing --swap-file or --swap-json".to_string()),
        (Some(_), Some(_)) => Err("use only one of --swap-file or --swap-json".to_string()),
    }
}

fn rpc_orchard_batch_file(
    data_dir: &Path,
    rpc_id: &str,
    flags: &[String],
    kind: &str,
) -> Result<PathBuf, String> {
    if let Some(batch_file) = flag_value(flags, "--batch-file") {
        return Ok(PathBuf::from(batch_file));
    }
    rpc_orchard_spool_file(data_dir, RPC_ORCHARD_BATCH_SPOOL_DIR, rpc_id, kind)
}

fn rpc_orchard_spool_file(
    data_dir: &Path,
    dir_name: &str,
    rpc_id: &str,
    suffix: &str,
) -> Result<PathBuf, String> {
    let dir = data_dir.join(dir_name);
    std::fs::create_dir_all(&dir)
        .map_err(|error| format!("rpc Orchard spool dir create failed: {error}"))?;
    let safe_id = sanitize_rpc_spool_component(rpc_id);
    Ok(dir.join(format!("{safe_id}-{}-{suffix}", process::id())))
}

fn rpc_orchard_write_spooled_json(
    data_dir: &Path,
    dir_name: &str,
    path: &Path,
    label: &str,
    json: &str,
) -> Result<(), String> {
    let incoming_bytes = u64::try_from(json.len()).unwrap_or(u64::MAX);
    rpc_orchard_check_spool_quota(data_dir, dir_name, incoming_bytes)?;
    std::fs::write(path, json.as_bytes())
        .map_err(|error| format!("{label} spool write failed: {error}"))
}

fn rpc_orchard_check_spool_quota(
    data_dir: &Path,
    dir_name: &str,
    incoming_bytes: u64,
) -> Result<(), String> {
    rpc_orchard_check_spool_quota_with_limits(
        data_dir,
        dir_name,
        incoming_bytes,
        RPC_ORCHARD_ACTION_SPOOL_MAX_REQUEST_BYTES,
        RPC_ORCHARD_ACTION_SPOOL_MAX_TOTAL_BYTES,
    )
}

fn rpc_orchard_check_spool_quota_with_limits(
    data_dir: &Path,
    dir_name: &str,
    incoming_bytes: u64,
    max_request_bytes: u64,
    max_total_bytes: u64,
) -> Result<(), String> {
    if incoming_bytes > max_request_bytes {
        return Err(format!(
            "rpc Orchard action spool request has {incoming_bytes} bytes, max {max_request_bytes}"
        ));
    }
    let spool_dir = data_dir.join(dir_name);
    let current_bytes = rpc_orchard_spool_usage_bytes(&spool_dir)?;
    if current_bytes.saturating_add(incoming_bytes) > max_total_bytes {
        return Err(format!(
            "rpc Orchard action spool would use {} bytes, max {max_total_bytes}",
            current_bytes.saturating_add(incoming_bytes)
        ));
    }
    Ok(())
}

fn rpc_orchard_spool_usage_bytes(dir: &Path) -> Result<u64, String> {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(error) => {
            return Err(format!(
                "rpc Orchard action spool usage read failed `{}`: {error}",
                dir.display()
            ));
        }
    };
    let mut total = 0_u64;
    for entry in entries {
        let entry = entry.map_err(|error| {
            format!(
                "rpc Orchard action spool usage entry read failed `{}`: {error}",
                dir.display()
            )
        })?;
        let metadata = entry.metadata().map_err(|error| {
            format!(
                "rpc Orchard action spool usage metadata failed `{}`: {error}",
                entry.path().display()
            )
        })?;
        if metadata.is_file() {
            total = total.saturating_add(metadata.len());
        }
    }
    Ok(total)
}

fn sanitize_rpc_spool_component(value: &str) -> String {
    let sanitized = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();
    if sanitized.is_empty() {
        "rpc".to_string()
    } else {
        sanitized.chars().take(80).collect()
    }
}

fn cleanup_rpc_orchard_action_source(path: Option<PathBuf>) -> Result<(), String> {
    if let Some(path) = path {
        std::fs::remove_file(&path)
            .map_err(|error| format!("rpc Orchard action spool cleanup failed: {error}"))?;
    }
    Ok(())
}

fn rpc_param_value(value: &serde_json::Value) -> Result<String, String> {
    match value {
        serde_json::Value::String(value) => Ok(value.clone()),
        serde_json::Value::Number(value) => Ok(value.to_string()),
        serde_json::Value::Bool(value) => Ok(value.to_string()),
        serde_json::Value::Array(values) => values
            .iter()
            .map(rpc_param_value)
            .collect::<Result<Vec<_>, _>>()
            .map(|values| values.join(",")),
        serde_json::Value::Null => Ok(String::new()),
        serde_json::Value::Object(_) => Err("rpc params cannot contain nested objects".to_string()),
    }
}

#[cfg(test)]
mod rpc_cli_tests {
    use super::*;

    #[test]
    fn persistent_rpc_reader_preserves_pipelined_read_ahead() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind test listener");
        let address = listener.local_addr().expect("test listener address");
        let client = thread::spawn(move || {
            let mut stream = TcpStream::connect(address).expect("connect test client");
            stream
                .write_all(b"first-frame\nsecond-frame\n")
                .expect("write pipelined frames");
        });
        let (mut stream, _) = listener.accept().expect("accept test client");
        let mut reader = BufReader::new(&mut stream);
        assert_eq!(
            read_rpc_line(&mut reader, "first frame").expect("read first frame"),
            "first-frame\n"
        );
        assert_eq!(
            read_rpc_line(&mut reader, "second frame").expect("read second frame"),
            "second-frame\n"
        );
        client.join().expect("join test client");
    }

    fn rpc_cli_test_data_dir(prefix: &str) -> PathBuf {
        let unique = RPC_SERVE_SPOOL_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = env::temp_dir().join(format!("{prefix}-{}-{unique}", process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("create rpc cli test data dir");
        path
    }

    #[test]
    fn rpc_catch_up_final_verification_fails_closed_on_history_mismatch() {
        let data_dir = rpc_cli_test_data_dir("rpc-catch-up-final-verification");
        init(InitOptions {
            data_dir: data_dir.clone(),
            chain_id: "postfiat-rpc-catch-up-test".to_string(),
            node_id: "validator-0".to_string(),
            validator_count: 3,
        })
        .expect("init catch-up verification state");

        rpc_catch_up_verify_completed_state(&data_dir)
            .expect("clean initialized state must verify");
        std::fs::write(
            data_dir.join("ordered_batches.json"),
            b"[\"orphaned-catch-up-batch\"]\n",
        )
        .expect("write mismatched ordered batch state");

        let error = rpc_catch_up_verify_completed_state(&data_dir)
            .expect_err("history mismatch must fail catch-up completion");
        assert!(
            error.contains("rpc catch-up final state verification failed"),
            "unexpected catch-up verification error: {error}"
        );

        std::fs::remove_dir_all(data_dir).expect("cleanup catch-up verification state");
    }

    #[test]
    fn rpc_catch_up_terminal_audit_catches_corruption_left_by_deferred_steps() {
        let data_dir = rpc_cli_test_data_dir("rpc-catch-up-terminal-audit");
        init(InitOptions {
            data_dir: data_dir.clone(),
            chain_id: "postfiat-rpc-catch-up-terminal-audit-test".to_string(),
            node_id: "validator-0".to_string(),
            validator_count: 3,
        })
        .expect("init terminal audit state");

        // A certified-delta step deliberately does not call verify_state. Model
        // corruption that remains latent until the mandatory terminal audit.
        std::fs::write(
            data_dir.join("ordered_batches.json"),
            b"[\"corrupt-intermediate-certified-delta\"]\n",
        )
        .expect("write corrupted intermediate state");

        let error = rpc_catch_up_verify_completed_state(&data_dir)
            .expect_err("terminal full audit must reject intermediate corruption");
        assert!(
            error.contains("rpc catch-up final state verification failed"),
            "unexpected terminal audit error: {error}"
        );

        std::fs::remove_dir_all(data_dir).expect("cleanup terminal audit state");
    }

    #[test]
    fn false_bool_rpc_param_is_omitted() {
        let mut flags = Vec::new();
        append_rpc_param_flags(&mut flags, "audit_block_log", &serde_json::json!(false))
            .expect("append false bool param");
        assert!(flags.is_empty());
    }

    #[test]
    fn true_bool_rpc_param_is_flag_only() {
        let mut flags = Vec::new();
        append_rpc_param_flags(&mut flags, "audit_block_log", &serde_json::json!(true))
            .expect("append true bool param");
        assert_eq!(flags, vec!["--audit-block-log"]);
    }

    #[test]
    fn non_bool_rpc_param_keeps_flag_value_pair() {
        let mut flags = Vec::new();
        append_rpc_param_flags(&mut flags, "tx_id", &serde_json::json!("abc"))
            .expect("append string param");
        assert_eq!(flags, vec!["--tx-id", "abc"]);
    }

    #[test]
    fn orchard_action_spool_quota_rejects_oversized_request_and_aggregate() {
        let data_dir = rpc_cli_test_data_dir("rpc-orchard-spool-quota");
        let per_request_error =
            rpc_orchard_check_spool_quota_with_limits(&data_dir, "spool", 11, 10, 100)
                .expect_err("oversized request must be rejected");
        assert!(
            per_request_error.contains("has 11 bytes, max 10"),
            "unexpected per-request quota error: {per_request_error}"
        );

        let spool_dir = data_dir.join("spool");
        std::fs::create_dir_all(&spool_dir).expect("create spool dir");
        std::fs::write(spool_dir.join("existing.json"), b"12345678").expect("seed spool file");
        rpc_orchard_check_spool_quota_with_limits(&data_dir, "spool", 2, 10, 10)
            .expect("aggregate at limit is allowed");
        let aggregate_error =
            rpc_orchard_check_spool_quota_with_limits(&data_dir, "spool", 3, 10, 10)
                .expect_err("aggregate quota overflow must be rejected");
        assert!(
            aggregate_error.contains("would use 11 bytes, max 10"),
            "unexpected aggregate quota error: {aggregate_error}"
        );

        std::fs::remove_dir_all(data_dir).expect("cleanup rpc cli test data dir");
    }

    #[test]
    fn mempool_submit_signed_payment_v2_finality_uses_finality_classifier() {
        assert!(is_mempool_submit_signed_transfer_finality_method(
            "mempool_submit_signed_transfer_finality"
        ));
        assert!(is_mempool_submit_signed_transfer_finality_method(
            "mempool_submit_signed_payment_v2_finality"
        ));
        assert!(is_mempool_submit_signed_transfer_finality_method(
            "mempool_submit_signed_asset_transaction_finality"
        ));
        assert!(is_mempool_submit_signed_transfer_finality_method(
            "mempool_submit_signed_atomic_swap_transaction_finality"
        ));
        assert!(is_mempool_submit_signed_transfer_finality_method(
            "mempool_submit_signed_escrow_transaction_finality"
        ));
        assert!(is_mempool_submit_signed_transfer_finality_method(
            "mempool_submit_fastlane_primary_finality"
        ));
        assert!(!is_mempool_submit_signed_transfer_finality_method(
            "mempool_submit_signed_payment_v2"
        ));
    }

    #[test]
    fn atomic_swap_rpc_methods_have_exact_capability_gates_and_classifiers() {
        const QUOTE: &str = "atomic_swap_fee_quote";
        const RAW: &str = "mempool_submit_signed_atomic_swap_transaction";
        const FINALITY: &str = "mempool_submit_signed_atomic_swap_transaction_finality";
        const RAW_PAYMENT_V2: &str = "mempool_submit_signed_payment_v2";

        assert!(!is_mempool_submit_signed_method(QUOTE));
        assert!(!is_mempool_submit_signed_transfer_finality_method(QUOTE));
        assert!(rpc_serve_method_allowed(QUOTE, false, false, false));

        assert!(is_mempool_submit_signed_method(RAW));
        assert!(!is_mempool_submit_signed_transfer_finality_method(RAW));
        assert!(!rpc_serve_method_allowed(RAW, false, false, false));
        assert!(rpc_serve_method_allowed(RAW, true, false, false));
        assert!(!rpc_serve_method_allowed(RAW, false, true, false));

        assert!(is_mempool_submit_signed_method(FINALITY));
        assert!(is_mempool_submit_signed_transfer_finality_method(FINALITY));
        assert!(!rpc_serve_method_allowed(FINALITY, false, false, false));
        assert!(!rpc_serve_method_allowed(FINALITY, true, false, false));
        assert!(rpc_serve_method_allowed(FINALITY, false, true, false));

        assert!(is_mempool_submit_signed_method(RAW_PAYMENT_V2));
        assert!(!is_mempool_submit_signed_transfer_finality_method(
            RAW_PAYMENT_V2
        ));
        assert!(rpc_serve_method_allowed(RAW_PAYMENT_V2, true, false, false));
        assert!(!rpc_serve_method_allowed(
            RAW_PAYMENT_V2,
            false,
            true,
            false
        ));
    }

    #[test]
    fn mempool_submit_signed_payment_v2_finality_allowed_under_finality_flag() {
        assert!(
            rpc_serve_method_allowed(
                "mempool_submit_signed_payment_v2_finality",
                false,
                true,
                false,
            ),
            "payment_v2 finality must be allowed with finality flag"
        );
        assert!(
            !rpc_serve_method_allowed(
                "mempool_submit_signed_payment_v2_finality",
                false,
                false,
                false,
            ),
            "payment_v2 finality must be blocked in read-only mode"
        );
        assert!(
            !rpc_serve_method_allowed(
                "mempool_submit_signed_payment_v2_finality",
                true,
                false,
                false,
            ),
            "payment_v2 finality must not be enabled by the generic submit flag"
        );
    }

    #[test]
    fn fastlane_primary_finality_is_exactly_finality_gated() {
        const METHOD: &str = "mempool_submit_fastlane_primary_finality";
        assert!(is_mempool_submit_signed_method(METHOD));
        assert!(is_mempool_submit_signed_transfer_finality_method(METHOD));
        assert!(!rpc_serve_method_allowed(METHOD, false, false, false));
        assert!(!rpc_serve_method_allowed(METHOD, true, false, false));
        assert!(rpc_serve_method_allowed(METHOD, false, true, false));
    }

    #[test]
    fn server_info_keeps_wallet_capabilities_when_metrics_are_unavailable() {
        let status_report = StatusReport {
            chain_id: "postfiat-wan-devnet".to_string(),
            genesis_hash: "a".repeat(96),
            protocol_version: 1,
            rpc_schema: "postfiat-local-rpc-v1".to_string(),
            build_git_revision: "test-revision".to_string(),
            build_profile: "test".to_string(),
            active_nav_profiles: Vec::new(),
            deployment_manifest_sha256: None,
            deployment_validator_id: None,
            deployment_service_artifacts: Vec::new(),
            deployment_runtime_artifacts: None,
            validator_count: 6,
            node_id: "validator-0".to_string(),
            status: "running".to_string(),
            last_run_unix: 1,
            state_root: "b".repeat(96),
            block_height: 1,
            block_tip_hash: "c".repeat(96),
            mempool_pending: 0,
        };
        let result = rpc_server_info_response(
            &status_report,
            None,
            Some("rpc server_info metrics failed: failed to parse batch archive append"),
        );

        assert_eq!(result["metrics"]["ok"], serde_json::json!(false));
        assert_eq!(result["validators"]["active_count"], serde_json::json!(6));
        assert!(result["validators"]["registry_update_count"].is_null());
        assert_eq!(
            result["fees"]["minimum_transfer_fee"],
            serde_json::json!(postfiat_execution::MIN_TRANSFER_FEE)
        );
        assert_eq!(
            result["fees"]["account_reserve"],
            serde_json::json!(postfiat_execution::ACCOUNT_RESERVE)
        );
        assert_eq!(
            result["fees"]["transfer_account_creation_fee"],
            serde_json::json!(postfiat_execution::TRANSFER_ACCOUNT_CREATION_FEE)
        );
        assert_eq!(
            result["warnings"][0]["code"],
            serde_json::json!("server_info_metrics_unavailable")
        );

        let response = postfiat_rpc_sdk::success_response("server-info", &result, vec![]).unwrap();
        postfiat_rpc_sdk::validate_response_kind(
            &response,
            postfiat_rpc_sdk::RpcResponseKind::ServerInfo,
        )
        .expect("server_info fallback remains RPC-SDK valid");
    }

    #[test]
    fn remote_server_info_capability_merge_preserves_read_aliases() {
        let mut result = serde_json::json!({
            "rpc": {
                "read_aliases": ["server_info", "ledger", "atomic_swap_fee_quote"],
                "upstream_capability": "preserve-me",
            },
        });
        let expected_aliases = result["rpc"]["read_aliases"].clone();
        let owned_domain = postfiat_types::OwnedCertificateDomain {
            schema: postfiat_types::OWNED_CERTIFICATE_DOMAIN_SCHEMA_V2.to_string(),
            chain_id: "postfiat-test".to_string(),
            genesis_hash: "ab".repeat(48),
            protocol_version: 1,
            registry_id: "cd".repeat(48),
        };

        merge_rpc_serve_server_info_capabilities(
            &mut result,
            false,
            true,
            false,
            false,
            &owned_domain,
            7,
            11,
        );

        assert_eq!(result["rpc"]["read_aliases"], expected_aliases);
        assert_eq!(
            result["rpc"]["upstream_capability"],
            serde_json::json!("preserve-me")
        );
        assert_eq!(
            result["rpc"]["mempool_submit_atomic_swap_finality_enabled"],
            serde_json::json!(true)
        );
        assert_eq!(result["rpc"]["read_only"], serde_json::json!(false));
        assert_eq!(
            result["rpc"]["owned_certificate_domain"],
            serde_json::json!(owned_domain)
        );
        assert_eq!(
            result["rpc"]["owned_lane_enabled"],
            serde_json::json!(false)
        );
        assert_eq!(
            result["rpc"]["max_mempool_submit_per_peer"],
            serde_json::json!(7)
        );
        assert_eq!(
            result["rpc"]["max_mempool_submit_total"],
            serde_json::json!(11)
        );
    }

    #[test]
    fn rpc_finality_required_u64_param_accepts_unsigned_integer_only() {
        let params_value = serde_json::json!({
            "proxy_required_current_height": 479,
            "proxy_readiness_timeout_ms": "1000",
        });
        let params = params_value.as_object().expect("object params");

        assert_eq!(
            rpc_finality_required_u64_param(params, "proxy_required_current_height")
                .expect("height parse"),
            Some(479)
        );
        assert_eq!(
            rpc_finality_required_u64_param(params, "missing").expect("missing parse"),
            None
        );
        let error = rpc_finality_required_u64_param(params, "proxy_readiness_timeout_ms")
            .expect_err("string timeout must fail");
        assert_eq!(error.0, "rpc_protocol_error");
        assert!(error
            .1
            .contains("proxy_readiness_timeout_ms must be an unsigned integer"));
    }

    #[test]
    fn rpc_finality_wrong_proposer_is_typed_before_signing() {
        assert!(rpc_finality_proposer_mismatch("validator-4", "validator-4", 580, 0).is_none());
        let error = rpc_finality_proposer_mismatch("validator-0", "validator-4", 580, 0)
            .expect("wrong endpoint must return a typed proposer error");
        assert_eq!(error.0, "rpc_finality_wrong_proposer");
        assert!(error
            .1
            .contains("retry the signed request at `validator-4`"));
        let response = rpc_serve_error_response("wrong-proposer", &error.0, &error.1);
        assert_eq!(
            rpc_serve_error_class(
                "mempool_submit_signed_asset_transaction_finality",
                &response,
            ),
            Some("finality_wrong_proposer".to_string())
        );
    }

    #[test]
    fn rpc_proxy_parent_wait_is_limited_to_quote_methods() {
        assert!(rpc_serve_method_accepts_proxy_parent_wait(
            "transfer_fee_quote"
        ));
        assert!(rpc_serve_method_accepts_proxy_parent_wait(
            "atomic_swap_fee_quote"
        ));
        assert!(rpc_serve_method_accepts_proxy_parent_wait(
            "asset_fee_quote"
        ));
        assert!(rpc_serve_method_accepts_proxy_parent_wait(
            "offer_fee_quote"
        ));
        assert!(!rpc_serve_method_accepts_proxy_parent_wait("status"));
        assert!(!rpc_serve_method_accepts_proxy_parent_wait(
            "mempool_submit_signed_transfer_finality"
        ));
    }

    #[test]
    fn rpc_serve_rate_limit_window_prunes_old_attempts() {
        let now = Instant::now();
        let old = now
            .checked_sub(RPC_SERVE_RATE_LIMIT_WINDOW + Duration::from_secs(1))
            .expect("old timestamp");
        let recent = now
            .checked_sub(Duration::from_secs(1))
            .expect("recent timestamp");
        let mut state = RpcServeMempoolSubmitState::default();
        state.total_timestamps.push_back(old);
        state.total_timestamps.push_back(recent);
        let mut peer_timestamps = std::collections::VecDeque::new();
        peer_timestamps.push_back(old);
        peer_timestamps.push_back(recent);
        state
            .counts_by_peer
            .insert("127.0.0.1".to_string(), peer_timestamps);

        prune_rpc_serve_rate_limit_window(&mut state, now);

        assert_eq!(state.total_timestamps.len(), 1);
        assert_eq!(
            state
                .counts_by_peer
                .get("127.0.0.1")
                .expect("peer timestamps")
                .len(),
            1
        );
    }

    #[test]
    fn payment_v2_allowed_under_finality_flag() {
        // Raw payment_v2 bypasses the certified-round lock and must not be
        // exposed by a finality-only service. Callers use payment_v2_finality.
        assert!(
            !rpc_serve_method_allowed("mempool_submit_signed_payment_v2", false, true, false,),
            "raw payment_v2 must be blocked by the finality-only flag"
        );
        // payment_v2 must NOT be allowed when both flags are false (read-only mode).
        assert!(
            !rpc_serve_method_allowed("mempool_submit_signed_payment_v2", false, false, false,),
            "payment_v2 must be blocked in read-only mode"
        );
        // payment_v2 must also be allowed under the generic submit flag.
        assert!(
            rpc_serve_method_allowed("mempool_submit_signed_payment_v2", true, false, false,),
            "payment_v2 must be allowed with generic submit flag"
        );
        // transfer_finality must still be allowed under finality flag.
        assert!(
            rpc_serve_method_allowed(
                "mempool_submit_signed_transfer_finality",
                false,
                true,
                false,
            ),
            "transfer_finality must be allowed with finality flag"
        );
        assert!(rpc_serve_method_allowed(
            "mempool_submit_signed_atomic_swap_transaction_finality",
            false,
            true,
            false,
        ));
        assert!(!rpc_serve_method_allowed(
            "mempool_submit_signed_atomic_swap_transaction",
            false,
            true,
            false,
        ));
        // Other generic methods (asset/escrow/nft) must NOT be allowed under finality-only.
        assert!(
            !rpc_serve_method_allowed(
                "mempool_submit_signed_asset_transaction",
                false,
                true,
                false,
            ),
            "raw asset transactions must not be allowed under finality-only flag"
        );
        assert!(
            rpc_serve_method_allowed(
                "mempool_submit_signed_asset_transaction_finality",
                false,
                true,
                false,
            ),
            "asset transaction finality must be allowed with finality flag"
        );
        assert!(
            !rpc_serve_method_allowed(
                "mempool_submit_signed_asset_transaction_finality",
                false,
                false,
                false,
            ),
            "asset transaction finality must be blocked in read-only mode"
        );
        assert!(
            !rpc_serve_method_allowed(
                "mempool_submit_signed_asset_transaction_finality",
                true,
                false,
                false,
            ),
            "asset transaction finality must not be enabled by the generic submit flag"
        );
        assert!(
            !rpc_serve_method_allowed(
                "mempool_submit_signed_escrow_transaction",
                false,
                true,
                false,
            ),
            "raw escrow transactions must not be allowed under finality-only flag"
        );
        assert!(
            rpc_serve_method_allowed(
                "mempool_submit_signed_escrow_transaction_finality",
                false,
                true,
                false,
            ),
            "escrow transaction finality must be allowed with finality flag"
        );
        assert!(
            !rpc_serve_method_allowed(
                "mempool_submit_signed_escrow_transaction_finality",
                false,
                false,
                false,
            ),
            "escrow transaction finality must be blocked in read-only mode"
        );
        assert!(
            !rpc_serve_method_allowed(
                "mempool_submit_signed_escrow_transaction_finality",
                true,
                false,
                false,
            ),
            "escrow transaction finality must not be enabled by the generic submit flag"
        );
    }
}
