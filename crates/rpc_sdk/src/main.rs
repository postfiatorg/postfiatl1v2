use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::io::Write;
use std::process;

use zeroize::Zeroizing;

use postfiat_rpc_sdk::{
    account_escrows_request, account_nfts_request, account_offers_request, account_request,
    account_tx_request, apply_batch_request, apply_bridge_batch_request,
    apply_shield_batch_request, archive_window_request, atomic_settlement_template_request,
    atomic_swap_fee_quote_request, atomic_swap_transaction_tx_id, batch_archive_request,
    blocks_request_from_height, book_offers_request, bridge_batch_domain_request,
    bridge_batch_pause_request, bridge_batch_resume_request, bridge_batch_transfer_request,
    bridge_status_request, decode_atomic_swap_fee_quote_summary,
    decode_atomic_swap_finality_summary, decode_atomic_swap_mempool_submit_entry,
    decode_transfer_fee_quote_summary, escrow_fee_quote_request, escrow_info_request,
    fastlane_asset_control_apply_request, fastlane_asset_control_catch_up_request,
    fastlane_asset_control_prepare_request, fastlane_asset_control_preview_request,
    fastlane_exit_request, fastswap_apply_request, fastswap_cancel_apply_request,
    fastswap_capabilities_request, fastswap_catch_up_request, fastswap_checkpoint_status_request,
    fastswap_commit_request, fastswap_commit_round_request, fastswap_effects_request,
    fastswap_new_round_vote_request, fastswap_objects_request, fastswap_policy_by_hash_request,
    fastswap_policy_by_pair_request, fastswap_precommit_request, fastswap_prepare_request,
    fastswap_preview_request, fastswap_propose_round_request, fastswap_status_request,
    fastswap_votes_request, fee_request, issuer_nfts_request, ledger_request, manifests_request,
    mempool_batch_request, mempool_status_request,
    mempool_submit_fastlane_primary_finality_request, mempool_submit_fastlane_primary_request,
    mempool_submit_signed_atomic_swap_transaction_finality_request,
    mempool_submit_signed_atomic_swap_transaction_json_request,
    mempool_submit_signed_escrow_transaction_json_request,
    mempool_submit_signed_offer_transaction_json_request,
    mempool_submit_signed_payment_v2_json_request, mempool_submit_signed_transfer_json_request,
    mempool_submit_signed_transfer_request, mempool_submit_transfer_request, metrics_request,
    navcoin_bridge_claims_request, navcoin_bridge_packet_preflight_request,
    navcoin_bridge_packet_request, navcoin_bridge_receipt_replay_request,
    navcoin_bridge_routes_request, navcoin_bridge_supply_status_request, nft_info_request,
    offer_fee_quote_request, offer_info_request, parse_fastswap_phase, read_request_file,
    read_response_file, receipts_request, server_info_request, shield_batch_migrate_request,
    shield_batch_mint_request, shield_batch_orchard_deposit_json_request,
    shield_batch_orchard_deposit_request, shield_batch_orchard_request,
    shield_batch_orchard_withdraw_request, shield_batch_spend_request,
    shield_batch_swap_json_request, shield_batch_swap_request, shield_disclose_request,
    shield_scan_request, shield_turnstile_request, status_request, transfer_fee_quote_request,
    tx_request_with_audit, validate_local_keys_request, validate_request_file,
    validate_response_file, validate_response_kind_with_context, validators_request,
    verify_state_request, wallet_backup_from_master_seed, wallet_dual_sign_fastswap_intent,
    wallet_fastpay_transfer_certificate_digest_v3, wallet_fastpay_transfer_lock_id_v1,
    wallet_fastpay_unwrap_certificate_digest_v3, wallet_fastpay_unwrap_lock_id_v1,
    wallet_identity_from_backup, wallet_sign_asset_transaction_from_quote,
    wallet_sign_atomic_swap_from_quote, wallet_sign_escrow_transaction_from_quote,
    wallet_sign_fast_asset_control_command, wallet_sign_fastlane_deposit,
    wallet_sign_nft_transaction_from_quote, wallet_sign_offer_transaction_from_quote,
    wallet_sign_owned_deposit, wallet_sign_owned_transfer_order,
    wallet_sign_owned_transfer_order_v3, wallet_sign_owned_unwrap_order,
    wallet_sign_owned_unwrap_order_v3, wallet_sign_payment_v2_from_fields,
    wallet_sign_transfer_from_quote, wallet_verify_fastpay_apply_ack_v1, write_request_file,
    BatchArchiveValidationContext, BridgeBatchDomainParams, BridgeBatchTransferParams,
    NavcoinBridgeClaimsParams, NavcoinBridgePacketParams, NavcoinBridgePacketPreflightParams,
    NavcoinBridgeReceiptReplayParams, NavcoinBridgeSupplyStatusParams, RpcRequest, RpcRequestKind,
    RpcResponseKind, WalletBackupFile, WalletSignPaymentV2Fields, METHOD_ACCOUNT,
    METHOD_ACCOUNT_ESCROWS, METHOD_ACCOUNT_NFTS, METHOD_ACCOUNT_OFFERS, METHOD_ACCOUNT_TX,
    METHOD_APPLY_BATCH, METHOD_APPLY_BRIDGE_BATCH, METHOD_APPLY_SHIELD_BATCH,
    METHOD_ARCHIVE_WINDOW, METHOD_ATOMIC_SETTLEMENT_TEMPLATE, METHOD_ATOMIC_SWAP_FEE_QUOTE,
    METHOD_BATCH_ARCHIVE, METHOD_BLOCKS, METHOD_BOOK_OFFERS, METHOD_BRIDGE_BATCH_DOMAIN,
    METHOD_BRIDGE_BATCH_PAUSE, METHOD_BRIDGE_BATCH_RESUME, METHOD_BRIDGE_BATCH_TRANSFER,
    METHOD_BRIDGE_STATUS, METHOD_ESCROW_FEE_QUOTE, METHOD_ESCROW_INFO,
    METHOD_FASTLANE_ASSET_CONTROL_APPLY, METHOD_FASTLANE_ASSET_CONTROL_CATCH_UP,
    METHOD_FASTLANE_ASSET_CONTROL_PREPARE, METHOD_FASTLANE_ASSET_CONTROL_PREVIEW,
    METHOD_FASTLANE_EXIT, METHOD_FASTSWAP_APPLY, METHOD_FASTSWAP_CANCEL_APPLY,
    METHOD_FASTSWAP_CAPABILITIES, METHOD_FASTSWAP_CATCH_UP, METHOD_FASTSWAP_CHECKPOINT_STATUS,
    METHOD_FASTSWAP_COMMIT, METHOD_FASTSWAP_COMMIT_ROUND, METHOD_FASTSWAP_EFFECTS,
    METHOD_FASTSWAP_NEW_ROUND_VOTE, METHOD_FASTSWAP_OBJECTS, METHOD_FASTSWAP_POLICY,
    METHOD_FASTSWAP_PRECOMMIT, METHOD_FASTSWAP_PREPARE, METHOD_FASTSWAP_PREVIEW,
    METHOD_FASTSWAP_PROPOSE_ROUND, METHOD_FASTSWAP_STATUS, METHOD_FASTSWAP_VOTES, METHOD_FEE,
    METHOD_ISSUER_NFTS, METHOD_LEDGER, METHOD_MANIFESTS, METHOD_MEMPOOL_BATCH,
    METHOD_MEMPOOL_STATUS, METHOD_MEMPOOL_SUBMIT_FASTLANE_PRIMARY,
    METHOD_MEMPOOL_SUBMIT_FASTLANE_PRIMARY_FINALITY,
    METHOD_MEMPOOL_SUBMIT_SIGNED_ATOMIC_SWAP_TRANSACTION,
    METHOD_MEMPOOL_SUBMIT_SIGNED_ATOMIC_SWAP_TRANSACTION_FINALITY,
    METHOD_MEMPOOL_SUBMIT_SIGNED_ESCROW_TRANSACTION,
    METHOD_MEMPOOL_SUBMIT_SIGNED_OFFER_TRANSACTION, METHOD_MEMPOOL_SUBMIT_SIGNED_PAYMENT_V2,
    METHOD_MEMPOOL_SUBMIT_SIGNED_TRANSFER, METHOD_MEMPOOL_SUBMIT_TRANSFER, METHOD_METRICS,
    METHOD_NAVCOIN_BRIDGE_CLAIMS, METHOD_NAVCOIN_BRIDGE_PACKET,
    METHOD_NAVCOIN_BRIDGE_PACKET_PREFLIGHT, METHOD_NAVCOIN_BRIDGE_RECEIPT_REPLAY,
    METHOD_NAVCOIN_BRIDGE_ROUTES, METHOD_NAVCOIN_BRIDGE_SUPPLY_STATUS, METHOD_NFT_INFO,
    METHOD_OFFER_FEE_QUOTE, METHOD_OFFER_INFO, METHOD_RECEIPTS, METHOD_SERVER_INFO,
    METHOD_SHIELD_BATCH_MIGRATE, METHOD_SHIELD_BATCH_MINT, METHOD_SHIELD_BATCH_ORCHARD,
    METHOD_SHIELD_BATCH_ORCHARD_DEPOSIT, METHOD_SHIELD_BATCH_ORCHARD_WITHDRAW,
    METHOD_SHIELD_BATCH_SPEND, METHOD_SHIELD_BATCH_SWAP, METHOD_SHIELD_DISCLOSE,
    METHOD_SHIELD_SCAN, METHOD_SHIELD_TURNSTILE, METHOD_STATUS, METHOD_TRANSFER_FEE_QUOTE,
    METHOD_TX, METHOD_VALIDATE_LOCAL_KEYS, METHOD_VALIDATORS, METHOD_VERIFY_STATE,
};
use postfiat_types::PaymentMemo;

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error}");
        print_usage();
        process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    let Some(command) = args.first().map(String::as_str) else {
        return Err("missing command".to_string());
    };
    let flags = &args[1..];
    match command {
        "request" => write_request(flags),
        "validate-request" => validate_request(flags),
        "validate-response" => validate_response(flags),
        "wallet-backup" => write_wallet_backup(flags),
        "wallet-identity" => write_wallet_identity(flags),
        "wallet-sign-quote" => write_wallet_signed_quote(flags),
        "wallet-sign-payment-v2" => write_wallet_signed_payment_v2(flags),
        "wallet-sign-asset-transaction" => write_wallet_signed_asset_transaction(flags),
        "wallet-sign-escrow-transaction" => write_wallet_signed_escrow_transaction(flags),
        "wallet-sign-nft-transaction" => write_wallet_signed_nft_transaction(flags),
        "wallet-sign-offer-transaction" => write_wallet_signed_offer_transaction(flags),
        "wallet-sign-atomic-swap" => write_wallet_signed_atomic_swap(flags),
        "wallet-sign-fastswap-intent" => write_wallet_signed_fastswap_intent(flags),
        "wallet-sign-fastlane-deposit" => write_wallet_signed_fastlane_deposit(flags),
        "wallet-sign-owned-deposit" => write_wallet_signed_owned_deposit(flags),
        "wallet-sign-fastlane-asset-control" => write_wallet_signed_fast_asset_control(flags),
        "atomic-swap-tx-id" => write_atomic_swap_tx_id(flags),
        "wallet-sign-owned-transfer" => write_wallet_signed_owned_transfer(flags),
        "wallet-sign-owned-unwrap" => write_wallet_signed_owned_unwrap(flags),
        "wallet-sign-owned-transfer-v3" => write_wallet_signed_owned_transfer_v3(flags),
        "wallet-sign-owned-unwrap-v3" => write_wallet_signed_owned_unwrap_v3(flags),
        "wallet-verify-fastpay-apply-v3" => write_verified_fastpay_apply_v3(flags),
        "help" | "--help" | "-h" => {
            print_usage();
            Ok(())
        }
        other => Err(format!("unknown command `{other}`")),
    }
}

fn write_request(flags: &[String]) -> Result<(), String> {
    let id = flag_value(flags, "--id").unwrap_or("local-1");
    let method = flag_value(flags, "--method").ok_or("missing --method")?;
    let output = flag_value(flags, "--output").ok_or("missing --output")?;
    let request = match method {
        METHOD_STATUS => status_request(id),
        METHOD_SERVER_INFO => server_info_request(id),
        METHOD_METRICS => metrics_request(id),
        METHOD_LEDGER => ledger_request(id, optional_usize_flag(flags, "--limit")?),
        METHOD_VERIFY_STATE => verify_state_request(id),
        METHOD_VALIDATE_LOCAL_KEYS => {
            let validators = flag_value(flags, "--validators")
                .unwrap_or("4")
                .parse::<u32>()
                .map_err(|_| "--validators must be a u32".to_string())?;
            validate_local_keys_request(id, validators)
        }
        METHOD_ACCOUNT => {
            let address = flag_value(flags, "--address").ok_or("missing --address")?;
            account_request(id, address)
        }
        METHOD_ACCOUNT_TX => {
            let address = flag_value(flags, "--address").ok_or("missing --address")?;
            account_tx_request(
                id,
                address,
                optional_u64_flag(flags, "--from-height")?,
                optional_u64_flag(flags, "--to-height")?,
                optional_usize_flag(flags, "--limit")?,
            )
        }
        METHOD_FEE => fee_request(id),
        METHOD_TRANSFER_FEE_QUOTE => {
            let from = flag_value(flags, "--from").ok_or("missing --from")?;
            let to = flag_value(flags, "--to").ok_or("missing --to")?;
            let amount = u64_flag(flags, "--amount")?;
            let mut request = transfer_fee_quote_request(
                id,
                from,
                to,
                amount,
                optional_u64_flag(flags, "--sequence")?,
            );
            for (param, flag) in [
                ("memo_type", "--memo-type"),
                ("memo_format", "--memo-format"),
                ("memo_data", "--memo-data"),
            ] {
                if let Some(value) = flag_value(flags, flag) {
                    request = request
                        .with_param(param, value)
                        .map_err(|error| format!("request serialization failed: {error}"))?;
                }
            }
            request
        }
        METHOD_ATOMIC_SWAP_FEE_QUOTE => atomic_swap_fee_quote_request(
            id,
            flag_value(flags, "--rfq-hash").ok_or("missing --rfq-hash")?,
            flag_value(flags, "--market-envelope-hash").ok_or("missing --market-envelope-hash")?,
            u64_flag(flags, "--nav-epoch")?,
            u64_flag(flags, "--expires-at-height")?,
            flag_value(flags, "--swap-nonce").ok_or("missing --swap-nonce")?,
            flag_value(flags, "--leg-0-owner").ok_or("missing --leg-0-owner")?,
            flag_value(flags, "--leg-0-recipient").ok_or("missing --leg-0-recipient")?,
            flag_value(flags, "--leg-0-issuer").ok_or("missing --leg-0-issuer")?,
            flag_value(flags, "--leg-0-asset-id").ok_or("missing --leg-0-asset-id")?,
            u64_flag(flags, "--leg-0-amount")?,
            flag_value(flags, "--leg-1-owner").ok_or("missing --leg-1-owner")?,
            flag_value(flags, "--leg-1-recipient").ok_or("missing --leg-1-recipient")?,
            flag_value(flags, "--leg-1-issuer").ok_or("missing --leg-1-issuer")?,
            flag_value(flags, "--leg-1-asset-id").ok_or("missing --leg-1-asset-id")?,
            u64_flag(flags, "--leg-1-amount")?,
        ),
        METHOD_ESCROW_FEE_QUOTE => {
            let source = flag_value(flags, "--source").ok_or("missing --source")?;
            let operation_json =
                flag_value(flags, "--operation-json").ok_or("missing --operation-json")?;
            escrow_fee_quote_request(
                id,
                source,
                operation_json,
                optional_u64_flag(flags, "--sequence")?,
            )
        }
        METHOD_OFFER_FEE_QUOTE => {
            let source = flag_value(flags, "--source").ok_or("missing --source")?;
            let operation_json =
                flag_value(flags, "--operation-json").ok_or("missing --operation-json")?;
            offer_fee_quote_request(
                id,
                source,
                operation_json,
                optional_u64_flag(flags, "--sequence")?,
            )
        }
        METHOD_ATOMIC_SETTLEMENT_TEMPLATE => atomic_settlement_template_request(
            id,
            flag_value(flags, "--left-owner").ok_or("missing --left-owner")?,
            flag_value(flags, "--left-recipient").ok_or("missing --left-recipient")?,
            flag_value(flags, "--left-asset-id").ok_or("missing --left-asset-id")?,
            u64_flag(flags, "--left-amount")?,
            flag_value(flags, "--right-owner").ok_or("missing --right-owner")?,
            flag_value(flags, "--right-recipient").ok_or("missing --right-recipient")?,
            flag_value(flags, "--right-asset-id").ok_or("missing --right-asset-id")?,
            u64_flag(flags, "--right-amount")?,
            flag_value(flags, "--condition").ok_or("missing --condition")?,
            optional_u64_flag(flags, "--finish-after")?.unwrap_or(0),
            u64_flag(flags, "--cancel-after")?,
            optional_u64_flag(flags, "--left-sequence")?,
            optional_u64_flag(flags, "--right-sequence")?,
        ),
        METHOD_OFFER_INFO => {
            let offer_id = flag_value(flags, "--offer-id").ok_or("missing --offer-id")?;
            offer_info_request(id, offer_id)
        }
        METHOD_ACCOUNT_OFFERS => {
            let account = flag_value(flags, "--account").ok_or("missing --account")?;
            account_offers_request(
                id,
                account,
                flag_value(flags, "--state"),
                optional_usize_flag(flags, "--limit")?,
            )
        }
        METHOD_BOOK_OFFERS => {
            let taker_gets_asset_id = flag_value(flags, "--taker-gets-asset-id")
                .ok_or("missing --taker-gets-asset-id")?;
            let taker_pays_asset_id = flag_value(flags, "--taker-pays-asset-id")
                .ok_or("missing --taker-pays-asset-id")?;
            book_offers_request(
                id,
                taker_gets_asset_id,
                taker_pays_asset_id,
                optional_usize_flag(flags, "--limit")?,
            )
        }
        METHOD_ESCROW_INFO => {
            let escrow_id = flag_value(flags, "--escrow-id").ok_or("missing --escrow-id")?;
            escrow_info_request(id, escrow_id)
        }
        METHOD_ACCOUNT_ESCROWS => {
            let account = flag_value(flags, "--account").ok_or("missing --account")?;
            account_escrows_request(
                id,
                account,
                flag_value(flags, "--role"),
                flag_value(flags, "--state"),
                optional_usize_flag(flags, "--limit")?,
            )
        }
        METHOD_NFT_INFO => {
            let nft_id = flag_value(flags, "--nft-id").ok_or("missing --nft-id")?;
            nft_info_request(id, nft_id)
        }
        METHOD_ACCOUNT_NFTS => {
            let account = flag_value(flags, "--account").ok_or("missing --account")?;
            account_nfts_request(
                id,
                account,
                Some(has_flag(flags, "--include-burned")),
                optional_usize_flag(flags, "--limit")?,
            )
        }
        METHOD_ISSUER_NFTS => {
            let issuer = flag_value(flags, "--issuer").ok_or("missing --issuer")?;
            issuer_nfts_request(
                id,
                issuer,
                flag_value(flags, "--collection-id"),
                Some(has_flag(flags, "--include-burned")),
                optional_usize_flag(flags, "--limit")?,
            )
        }
        METHOD_RECEIPTS => receipts_request(
            id,
            flag_value(flags, "--tx-id"),
            optional_usize_flag(flags, "--limit")?,
        ),
        METHOD_TX => {
            let tx_id = flag_value(flags, "--tx-id").ok_or("missing --tx-id")?;
            tx_request_with_audit(id, tx_id, has_flag(flags, "--audit-block-log"))
        }
        METHOD_BLOCKS => blocks_request_from_height(
            id,
            optional_u64_flag(flags, "--from-height")?,
            optional_usize_flag(flags, "--limit")?,
        ),
        METHOD_VALIDATORS => validators_request(id),
        METHOD_MANIFESTS => manifests_request(id),
        METHOD_BATCH_ARCHIVE => batch_archive_request(
            id,
            flag_value(flags, "--batch-kind"),
            flag_value(flags, "--batch-id"),
            optional_usize_flag(flags, "--limit")?,
        ),
        METHOD_ARCHIVE_WINDOW => archive_window_request(
            id,
            u64_flag(flags, "--from-height")?,
            u64_flag(flags, "--to-height")?,
            flag_value(flags, "--archive-uri"),
        ),
        METHOD_MEMPOOL_SUBMIT_TRANSFER => {
            let to = flag_value(flags, "--to").ok_or("missing --to")?;
            let amount = u64_flag(flags, "--amount")?;
            mempool_submit_transfer_request(id, to, amount, flag_value(flags, "--key-file"))
        }
        METHOD_MEMPOOL_SUBMIT_SIGNED_TRANSFER => {
            match (
                flag_value(flags, "--transfer-file"),
                flag_value(flags, "--signed-transfer-json"),
                flag_value(flags, "--signed-transfer-json-file"),
            ) {
                (Some(transfer_file), None, None) => {
                    mempool_submit_signed_transfer_request(id, transfer_file)
                }
                (None, Some(signed_transfer_json), None) => {
                    mempool_submit_signed_transfer_json_request(id, signed_transfer_json)
                }
                (None, None, Some(signed_transfer_json_file)) => {
                    let signed_transfer_json = fs::read_to_string(signed_transfer_json_file)
                        .map_err(|error| {
                            format!(
                                "signed transfer JSON read failed at {signed_transfer_json_file}: {error}"
                            )
                        })?;
                    mempool_submit_signed_transfer_json_request(id, signed_transfer_json)
                }
                _ => {
                    return Err(
                        "use exactly one of --transfer-file, --signed-transfer-json, or --signed-transfer-json-file"
                            .to_string(),
                    );
                }
            }
        }
        METHOD_MEMPOOL_SUBMIT_SIGNED_PAYMENT_V2 => {
            match (
                flag_value(flags, "--signed-payment-v2-json"),
                flag_value(flags, "--signed-payment-v2-json-file"),
            ) {
                (Some(signed_payment_v2_json), None) => {
                    mempool_submit_signed_payment_v2_json_request(id, signed_payment_v2_json)
                }
                (None, Some(signed_payment_v2_json_file)) => {
                    let signed_payment_v2_json =
                        fs::read_to_string(signed_payment_v2_json_file).map_err(|error| {
                            format!(
                                "signed payment v2 JSON read failed at {signed_payment_v2_json_file}: {error}"
                            )
                        })?;
                    mempool_submit_signed_payment_v2_json_request(id, signed_payment_v2_json)
                }
                _ => {
                    return Err(
                        "use exactly one of --signed-payment-v2-json or --signed-payment-v2-json-file"
                            .to_string(),
                    );
                }
            }
        }
        METHOD_MEMPOOL_SUBMIT_SIGNED_ATOMIC_SWAP_TRANSACTION => {
            let signed_json = required_json_input(
                flags,
                "--signed-atomic-swap-transaction-json",
                "--signed-atomic-swap-transaction-json-file",
                "signed atomic swap transaction",
            )?;
            mempool_submit_signed_atomic_swap_transaction_json_request(id, signed_json)
        }
        METHOD_MEMPOOL_SUBMIT_SIGNED_ATOMIC_SWAP_TRANSACTION_FINALITY => {
            let signed_json = required_json_input(
                flags,
                "--signed-atomic-swap-transaction-json",
                "--signed-atomic-swap-transaction-json-file",
                "signed atomic swap transaction",
            )?;
            mempool_submit_signed_atomic_swap_transaction_finality_request(
                id,
                signed_json,
                u64_flag(flags, "--proxy-required-current-height")?,
                flag_value(flags, "--proxy-required-state-root")
                    .ok_or("missing --proxy-required-state-root")?,
                flag_value(flags, "--proxy-required-parent-hash")
                    .ok_or("missing --proxy-required-parent-hash")?,
                optional_u64_flag(flags, "--proxy-readiness-timeout-ms")?,
            )
        }
        METHOD_MEMPOOL_SUBMIT_SIGNED_ESCROW_TRANSACTION => {
            match (
                flag_value(flags, "--signed-escrow-transaction-json"),
                flag_value(flags, "--signed-escrow-transaction-json-file"),
            ) {
                (Some(signed_escrow_transaction_json), None) => {
                    mempool_submit_signed_escrow_transaction_json_request(
                        id,
                        signed_escrow_transaction_json,
                    )
                }
                (None, Some(signed_escrow_transaction_json_file)) => {
                    let signed_escrow_transaction_json =
                        fs::read_to_string(signed_escrow_transaction_json_file).map_err(|error| {
                            format!(
                                "signed escrow transaction JSON read failed at {signed_escrow_transaction_json_file}: {error}"
                            )
                        })?;
                    mempool_submit_signed_escrow_transaction_json_request(
                        id,
                        signed_escrow_transaction_json,
                    )
                }
                _ => {
                    return Err(
                        "use exactly one of --signed-escrow-transaction-json or --signed-escrow-transaction-json-file"
                            .to_string(),
                    );
                }
            }
        }
        METHOD_MEMPOOL_SUBMIT_SIGNED_OFFER_TRANSACTION => {
            match (
                flag_value(flags, "--signed-offer-transaction-json"),
                flag_value(flags, "--signed-offer-transaction-json-file"),
            ) {
                (Some(signed_offer_transaction_json), None) => {
                    mempool_submit_signed_offer_transaction_json_request(
                        id,
                        signed_offer_transaction_json,
                    )
                }
                (None, Some(signed_offer_transaction_json_file)) => {
                    let signed_offer_transaction_json =
                        fs::read_to_string(signed_offer_transaction_json_file).map_err(
                            |error| {
                                format!(
                                    "signed offer transaction JSON read failed at {signed_offer_transaction_json_file}: {error}"
                                )
                            },
                        )?;
                    mempool_submit_signed_offer_transaction_json_request(
                        id,
                        signed_offer_transaction_json,
                    )
                }
                _ => {
                    return Err(
                        "use exactly one of --signed-offer-transaction-json or --signed-offer-transaction-json-file"
                            .to_string(),
                    );
                }
            }
        }
        METHOD_MEMPOOL_STATUS => mempool_status_request(id),
        METHOD_MEMPOOL_BATCH => {
            let batch_file = flag_value(flags, "--batch-file").ok_or("missing --batch-file")?;
            mempool_batch_request(
                id,
                batch_file,
                optional_usize_flag(flags, "--max-transactions")?,
            )
        }
        METHOD_APPLY_BATCH => {
            let batch_file = flag_value(flags, "--batch-file").ok_or("missing --batch-file")?;
            apply_batch_request(id, batch_file)
        }
        METHOD_SHIELD_BATCH_MINT => {
            let owner = flag_value(flags, "--owner").ok_or("missing --owner")?;
            let amount = u64_flag(flags, "--amount")?;
            let batch_file = flag_value(flags, "--batch-file").ok_or("missing --batch-file")?;
            shield_batch_mint_request(
                id,
                owner,
                amount,
                flag_value(flags, "--asset-id"),
                flag_value(flags, "--memo"),
                batch_file,
            )
        }
        METHOD_SHIELD_BATCH_SPEND => {
            let note_id = flag_value(flags, "--note-id").ok_or("missing --note-id")?;
            let to = flag_value(flags, "--to").ok_or("missing --to")?;
            let amount = u64_flag(flags, "--amount")?;
            let batch_file = flag_value(flags, "--batch-file").ok_or("missing --batch-file")?;
            shield_batch_spend_request(
                id,
                note_id,
                to,
                amount,
                flag_value(flags, "--memo"),
                batch_file,
            )
        }
        METHOD_SHIELD_BATCH_MIGRATE => {
            let note_id = flag_value(flags, "--note-id").ok_or("missing --note-id")?;
            let target_pool = flag_value(flags, "--target-pool").ok_or("missing --target-pool")?;
            let batch_file = flag_value(flags, "--batch-file").ok_or("missing --batch-file")?;
            shield_batch_migrate_request(
                id,
                note_id,
                target_pool,
                flag_value(flags, "--memo"),
                batch_file,
            )
        }
        METHOD_SHIELD_BATCH_ORCHARD => {
            let action_file = flag_value(flags, "--action-file").ok_or("missing --action-file")?;
            let batch_file = flag_value(flags, "--batch-file").ok_or("missing --batch-file")?;
            shield_batch_orchard_request(id, action_file, batch_file)
        }
        METHOD_SHIELD_BATCH_ORCHARD_DEPOSIT => {
            let batch_file = flag_value(flags, "--batch-file");
            match (
                flag_value(flags, "--deposit-file"),
                flag_value(flags, "--deposit-json"),
                flag_value(flags, "--deposit-json-file"),
            ) {
                (Some(deposit_file), None, None) => {
                    let batch_file = batch_file.ok_or("missing --batch-file")?;
                    shield_batch_orchard_deposit_request(id, deposit_file, batch_file)
                }
                (None, Some(deposit_json), None) => {
                    let mut request = shield_batch_orchard_deposit_json_request(id, deposit_json);
                    if let Some(batch_file) = batch_file {
                        request = request
                            .with_param("batch_file", batch_file)
                            .map_err(|error| format!("request serialization failed: {error}"))?;
                    }
                    request
                }
                (None, None, Some(deposit_json_file)) => {
                    let deposit_json = fs::read_to_string(deposit_json_file).map_err(|error| {
                        format!("deposit JSON read failed at {deposit_json_file}: {error}")
                    })?;
                    let mut request = shield_batch_orchard_deposit_json_request(id, deposit_json);
                    if let Some(batch_file) = batch_file {
                        request = request
                            .with_param("batch_file", batch_file)
                            .map_err(|error| format!("request serialization failed: {error}"))?;
                    }
                    request
                }
                _ => {
                    return Err(
                        "use exactly one of --deposit-file, --deposit-json, or --deposit-json-file"
                            .to_string(),
                    );
                }
            }
        }
        METHOD_SHIELD_BATCH_ORCHARD_WITHDRAW => {
            let action_file = flag_value(flags, "--action-file").ok_or("missing --action-file")?;
            let to = flag_value(flags, "--to").ok_or("missing --to")?;
            let amount = u64_flag(flags, "--amount")?;
            let fee = u64_flag(flags, "--fee")?;
            let batch_file = flag_value(flags, "--batch-file").ok_or("missing --batch-file")?;
            shield_batch_orchard_withdraw_request(
                id,
                action_file,
                to,
                amount,
                fee,
                flag_value(flags, "--policy-id"),
                flag_value(flags, "--disclosure-hash"),
                batch_file,
            )
        }
        METHOD_SHIELD_BATCH_SWAP => {
            let batch_file = flag_value(flags, "--batch-file");
            match (
                flag_value(flags, "--swap-file"),
                flag_value(flags, "--swap-json"),
                flag_value(flags, "--swap-json-file"),
            ) {
                (Some(swap_file), None, None) => {
                    let batch_file = batch_file.ok_or("missing --batch-file")?;
                    shield_batch_swap_request(id, swap_file, batch_file)
                }
                (None, Some(swap_json), None) => {
                    let mut request = shield_batch_swap_json_request(id, swap_json);
                    if let Some(batch_file) = batch_file {
                        request = request
                            .with_param("batch_file", batch_file)
                            .map_err(|error| format!("request serialization failed: {error}"))?;
                    }
                    request
                }
                (None, None, Some(swap_json_file)) => {
                    let swap_json = fs::read_to_string(swap_json_file).map_err(|error| {
                        format!("swap JSON read failed at {swap_json_file}: {error}")
                    })?;
                    let mut request = shield_batch_swap_json_request(id, swap_json);
                    if let Some(batch_file) = batch_file {
                        request = request
                            .with_param("batch_file", batch_file)
                            .map_err(|error| format!("request serialization failed: {error}"))?;
                    }
                    request
                }
                _ => {
                    return Err(
                        "use exactly one of --swap-file, --swap-json, or --swap-json-file"
                            .to_string(),
                    );
                }
            }
        }
        METHOD_APPLY_SHIELD_BATCH => {
            let batch_file = flag_value(flags, "--batch-file").ok_or("missing --batch-file")?;
            apply_shield_batch_request(id, batch_file)
        }
        METHOD_SHIELD_SCAN => {
            let owner = flag_value(flags, "--owner").ok_or("missing --owner")?;
            shield_scan_request(id, owner)
        }
        METHOD_SHIELD_DISCLOSE => {
            let note_id = flag_value(flags, "--note-id").ok_or("missing --note-id")?;
            shield_disclose_request(id, note_id)
        }
        METHOD_SHIELD_TURNSTILE => shield_turnstile_request(id),
        METHOD_BRIDGE_STATUS => bridge_status_request(id),
        METHOD_NAVCOIN_BRIDGE_ROUTES => navcoin_bridge_routes_request(id),
        METHOD_NAVCOIN_BRIDGE_PACKET => {
            let route_id = flag_value(flags, "--route-id").ok_or("missing --route-id")?;
            let packet_hash = flag_value(flags, "--packet-hash").ok_or("missing --packet-hash")?;
            navcoin_bridge_packet_request(
                id,
                NavcoinBridgePacketParams {
                    route_id: route_id.to_string(),
                    packet_hash: packet_hash.to_string(),
                },
            )
        }
        METHOD_NAVCOIN_BRIDGE_CLAIMS => {
            let route_id = flag_value(flags, "--route-id").ok_or("missing --route-id")?;
            navcoin_bridge_claims_request(
                id,
                NavcoinBridgeClaimsParams {
                    route_id: route_id.to_string(),
                    limit: optional_usize_flag(flags, "--limit")?,
                    include_terminal: if has_flag(flags, "--include-terminal") {
                        Some(true)
                    } else {
                        None
                    },
                },
            )
        }
        METHOD_NAVCOIN_BRIDGE_SUPPLY_STATUS => {
            let route_id = flag_value(flags, "--route-id").ok_or("missing --route-id")?;
            navcoin_bridge_supply_status_request(
                id,
                NavcoinBridgeSupplyStatusParams {
                    route_id: route_id.to_string(),
                },
            )
        }
        METHOD_NAVCOIN_BRIDGE_RECEIPT_REPLAY => {
            let route_id = flag_value(flags, "--route-id").ok_or("missing --route-id")?;
            navcoin_bridge_receipt_replay_request(
                id,
                NavcoinBridgeReceiptReplayParams {
                    route_id: route_id.to_string(),
                },
            )
        }
        METHOD_NAVCOIN_BRIDGE_PACKET_PREFLIGHT => {
            let route_id = flag_value(flags, "--route-id").ok_or("missing --route-id")?;
            let packet_file = flag_value(flags, "--packet-file").ok_or("missing --packet-file")?;
            navcoin_bridge_packet_preflight_request(
                id,
                NavcoinBridgePacketPreflightParams {
                    route_id: route_id.to_string(),
                    packet_file: packet_file.to_string(),
                },
            )
        }
        METHOD_BRIDGE_BATCH_DOMAIN => {
            let domain_id = flag_value(flags, "--domain-id").ok_or("missing --domain-id")?;
            let name = flag_value(flags, "--name").ok_or("missing --name")?;
            let inbound_cap = u64_flag(flags, "--inbound-cap")?;
            let outbound_cap = u64_flag(flags, "--outbound-cap")?;
            let batch_file = flag_value(flags, "--batch-file").ok_or("missing --batch-file")?;
            bridge_batch_domain_request(
                id,
                BridgeBatchDomainParams {
                    domain_id: domain_id.to_string(),
                    name: name.to_string(),
                    source_chain: optional_string(flags, "--source-chain"),
                    target_chain: optional_string(flags, "--target-chain"),
                    bridge_id: optional_string(flags, "--bridge-id"),
                    door_account: optional_string(flags, "--door-account"),
                    inbound_cap,
                    outbound_cap,
                    batch_file: batch_file.to_string(),
                },
            )
        }
        METHOD_BRIDGE_BATCH_TRANSFER => {
            let domain_id = flag_value(flags, "--domain-id").ok_or("missing --domain-id")?;
            let direction = flag_value(flags, "--direction").ok_or("missing --direction")?;
            let from = flag_value(flags, "--from").ok_or("missing --from")?;
            let to = flag_value(flags, "--to").ok_or("missing --to")?;
            let asset_id = flag_value(flags, "--asset-id").ok_or("missing --asset-id")?;
            let amount = u64_flag(flags, "--amount")?;
            let witness_id = flag_value(flags, "--witness-id").ok_or("missing --witness-id")?;
            let batch_file = flag_value(flags, "--batch-file").ok_or("missing --batch-file")?;
            bridge_batch_transfer_request(
                id,
                BridgeBatchTransferParams {
                    domain_id: domain_id.to_string(),
                    direction: direction.to_string(),
                    from: from.to_string(),
                    to: to.to_string(),
                    asset_id: asset_id.to_string(),
                    amount,
                    witness_id: witness_id.to_string(),
                    witness_epoch: optional_u32_flag(flags, "--witness-epoch")?,
                    witness_signer: optional_string(flags, "--witness-signer"),
                    batch_file: batch_file.to_string(),
                },
            )
        }
        METHOD_BRIDGE_BATCH_PAUSE => {
            let domain_id = flag_value(flags, "--domain-id").ok_or("missing --domain-id")?;
            let batch_file = flag_value(flags, "--batch-file").ok_or("missing --batch-file")?;
            bridge_batch_pause_request(id, domain_id, batch_file)
        }
        METHOD_BRIDGE_BATCH_RESUME => {
            let domain_id = flag_value(flags, "--domain-id").ok_or("missing --domain-id")?;
            let batch_file = flag_value(flags, "--batch-file").ok_or("missing --batch-file")?;
            bridge_batch_resume_request(id, domain_id, batch_file)
        }
        METHOD_APPLY_BRIDGE_BATCH => {
            let batch_file = flag_value(flags, "--batch-file").ok_or("missing --batch-file")?;
            apply_bridge_batch_request(id, batch_file)
        }
        METHOD_MEMPOOL_SUBMIT_FASTLANE_PRIMARY => mempool_submit_fastlane_primary_request(
            id,
            required_json_input(
                flags,
                "--fastlane-primary-json",
                "--fastlane-primary-json-file",
                "FastLane primary transaction",
            )?,
        ),
        METHOD_MEMPOOL_SUBMIT_FASTLANE_PRIMARY_FINALITY => {
            mempool_submit_fastlane_primary_finality_request(
                id,
                required_json_input(
                    flags,
                    "--fastlane-primary-json",
                    "--fastlane-primary-json-file",
                    "FastLane primary transaction",
                )?,
            )
        }
        METHOD_FASTSWAP_CAPABILITIES => fastswap_capabilities_request(id),
        METHOD_FASTSWAP_PREVIEW | METHOD_FASTSWAP_PREPARE => {
            let signed = required_json_input(
                flags,
                "--signed-intent-json",
                "--signed-intent-json-file",
                "signed FastSwap intent",
            )?;
            if method == METHOD_FASTSWAP_PREVIEW {
                fastswap_preview_request(id, signed)
            } else {
                fastswap_prepare_request(id, signed)
            }
        }
        METHOD_FASTSWAP_COMMIT => fastswap_commit_request(
            id,
            required_json_input(flags, "--lock-qc-json", "--lock-qc-json-file", "LockQC")?,
        ),
        METHOD_FASTSWAP_APPLY => fastswap_apply_request(
            id,
            required_json_input(
                flags,
                "--decision-qc-json",
                "--decision-qc-json-file",
                "DecisionQC",
            )?,
            required_json_input(
                flags,
                "--signed-intent-json",
                "--signed-intent-json-file",
                "signed FastSwap intent",
            )?,
        ),
        METHOD_FASTSWAP_CATCH_UP => fastswap_catch_up_request(
            id,
            required_json_input(flags, "--lock-qc-json", "--lock-qc-json-file", "LockQC")?,
            required_json_input(
                flags,
                "--decision-qc-json",
                "--decision-qc-json-file",
                "DecisionQC",
            )?,
            required_json_input(
                flags,
                "--signed-intent-json",
                "--signed-intent-json-file",
                "signed FastSwap intent",
            )?,
        ),
        METHOD_FASTSWAP_STATUS | METHOD_FASTSWAP_EFFECTS => {
            let swap_id = flag_value(flags, "--swap-id").ok_or("missing --swap-id")?;
            if method == METHOD_FASTSWAP_STATUS {
                fastswap_status_request(id, swap_id)
            } else {
                fastswap_effects_request(id, swap_id)
            }
        }
        METHOD_FASTSWAP_VOTES => fastswap_votes_request(
            id,
            flag_value(flags, "--swap-id").ok_or("missing --swap-id")?,
            flag_value(flags, "--phase")
                .and_then(parse_fastswap_phase)
                .ok_or("missing or invalid --phase")?,
            u64_flag(flags, "--round")?,
        ),
        METHOD_FASTSWAP_NEW_ROUND_VOTE => fastswap_new_round_vote_request(
            id,
            flag_value(flags, "--swap-id").ok_or("missing --swap-id")?,
            u64_flag(flags, "--target-round")?,
        ),
        METHOD_FASTSWAP_PROPOSE_ROUND | METHOD_FASTSWAP_PRECOMMIT => {
            let proposal = required_json_input(
                flags,
                "--proposal-json",
                "--proposal-json-file",
                "FastSwap proposal",
            )?;
            if method == METHOD_FASTSWAP_PROPOSE_ROUND {
                fastswap_propose_round_request(id, proposal)
            } else {
                fastswap_precommit_request(id, proposal)
            }
        }
        METHOD_FASTSWAP_COMMIT_ROUND => fastswap_commit_round_request(
            id,
            required_json_input(
                flags,
                "--precommit-qc-json",
                "--precommit-qc-json-file",
                "precommit QC",
            )?,
        ),
        METHOD_FASTSWAP_CANCEL_APPLY => fastswap_cancel_apply_request(
            id,
            required_json_input(
                flags,
                "--decision-qc-json",
                "--decision-qc-json-file",
                "cancel DecisionQC",
            )?,
        ),
        METHOD_FASTLANE_EXIT => fastlane_exit_request(
            id,
            required_json_input(
                flags,
                "--signed-exit-json",
                "--signed-exit-json-file",
                "signed FastLane exit",
            )?,
        ),
        METHOD_FASTSWAP_CHECKPOINT_STATUS => fastswap_checkpoint_status_request(
            id,
            optional_string(flags, "--previous-checkpoint-id"),
        ),
        METHOD_FASTSWAP_OBJECTS => {
            let cursor_id = optional_string(flags, "--cursor-object-id");
            let cursor_version = optional_u64_flag(flags, "--cursor-version")?;
            if cursor_id.is_some() != cursor_version.is_some() {
                return Err(
                    "--cursor-object-id and --cursor-version are required together".to_owned(),
                );
            }
            fastswap_objects_request(
                id,
                flag_value(flags, "--owner-pubkey").ok_or("missing --owner-pubkey")?,
                optional_string(flags, "--asset-id"),
                cursor_id.zip(cursor_version),
                u64_flag(flags, "--limit")?,
            )
        }
        METHOD_FASTSWAP_POLICY => {
            match (
                flag_value(flags, "--policy-hash"),
                flag_value(flags, "--asset-0"),
                flag_value(flags, "--asset-1"),
            ) {
                (Some(hash), None, None) => fastswap_policy_by_hash_request(id, hash),
                (None, Some(asset_0), Some(asset_1)) => {
                    fastswap_policy_by_pair_request(id, asset_0, asset_1)
                }
                _ => return Err(
                    "FastSwap policy requires exactly --policy-hash or --asset-0 plus --asset-1"
                        .to_owned(),
                ),
            }
        }
        METHOD_FASTLANE_ASSET_CONTROL_PREVIEW | METHOD_FASTLANE_ASSET_CONTROL_PREPARE => {
            let signed = required_json_input(
                flags,
                "--signed-command-json",
                "--signed-command-json-file",
                "signed asset-control command",
            )?;
            if method == METHOD_FASTLANE_ASSET_CONTROL_PREVIEW {
                fastlane_asset_control_preview_request(id, signed)
            } else {
                fastlane_asset_control_prepare_request(id, signed)
            }
        }
        METHOD_FASTLANE_ASSET_CONTROL_APPLY => fastlane_asset_control_apply_request(
            id,
            required_json_input(
                flags,
                "--decision-qc-json",
                "--decision-qc-json-file",
                "DecisionQC",
            )?,
            required_json_input(
                flags,
                "--signed-command-json",
                "--signed-command-json-file",
                "signed asset-control command",
            )?,
        ),
        METHOD_FASTLANE_ASSET_CONTROL_CATCH_UP => fastlane_asset_control_catch_up_request(
            id,
            required_json_input(flags, "--lock-qc-json", "--lock-qc-json-file", "LockQC")?,
            required_json_input(
                flags,
                "--decision-qc-json",
                "--decision-qc-json-file",
                "DecisionQC",
            )?,
            required_json_input(
                flags,
                "--signed-command-json",
                "--signed-command-json-file",
                "signed asset-control command",
            )?,
        ),
        other => {
            return Err(format!(
                "unsupported request builder method `{other}`; supported: {METHOD_STATUS}, {METHOD_METRICS}, {METHOD_VERIFY_STATE}, {METHOD_VALIDATE_LOCAL_KEYS}, {METHOD_ACCOUNT}, {METHOD_TRANSFER_FEE_QUOTE}, {METHOD_RECEIPTS}, {METHOD_BLOCKS}, {METHOD_BATCH_ARCHIVE}, {METHOD_MEMPOOL_SUBMIT_TRANSFER}, {METHOD_MEMPOOL_SUBMIT_SIGNED_TRANSFER}, {METHOD_MEMPOOL_STATUS}, {METHOD_MEMPOOL_BATCH}, {METHOD_APPLY_BATCH}, {METHOD_SHIELD_BATCH_MINT}, {METHOD_SHIELD_BATCH_SPEND}, {METHOD_SHIELD_BATCH_MIGRATE}, {METHOD_SHIELD_BATCH_ORCHARD}, {METHOD_SHIELD_BATCH_ORCHARD_DEPOSIT}, {METHOD_SHIELD_BATCH_ORCHARD_WITHDRAW}, {METHOD_SHIELD_BATCH_SWAP}, {METHOD_APPLY_SHIELD_BATCH}, {METHOD_SHIELD_SCAN}, {METHOD_SHIELD_DISCLOSE}, {METHOD_SHIELD_TURNSTILE}, {METHOD_BRIDGE_STATUS}, {METHOD_NAVCOIN_BRIDGE_ROUTES}, {METHOD_NAVCOIN_BRIDGE_PACKET}, {METHOD_NAVCOIN_BRIDGE_CLAIMS}, {METHOD_NAVCOIN_BRIDGE_SUPPLY_STATUS}, {METHOD_NAVCOIN_BRIDGE_RECEIPT_REPLAY}, {METHOD_NAVCOIN_BRIDGE_PACKET_PREFLIGHT}, {METHOD_BRIDGE_BATCH_DOMAIN}, {METHOD_BRIDGE_BATCH_TRANSFER}, {METHOD_BRIDGE_BATCH_PAUSE}, {METHOD_BRIDGE_BATCH_RESUME}, {METHOD_APPLY_BRIDGE_BATCH}"
            ));
        }
    };
    write_request_output(output, &request)
}

fn write_request_output(output: &str, request: &RpcRequest) -> Result<(), String> {
    if output == "-" {
        let json = request
            .to_pretty_json()
            .map_err(|error| format!("request serialization failed: {error}"))?;
        println!("{json}");
        return Ok(());
    }
    write_request_file(output, request)
        .map_err(|error| format!("request file write failed at {output}: {error}"))
}

fn validate_request(flags: &[String]) -> Result<(), String> {
    let input = flag_value(flags, "--input").ok_or("missing --input")?;
    let expected_id = flag_value(flags, "--expect-id");
    let expected_kind = request_kind(flags)?;
    let request = validate_request_file(input, expected_id, expected_kind)
        .map_err(|error| format!("request validation failed at {input}: {error}"))?;
    println!("rpc_request=ok id={} method={}", request.id, request.method);
    Ok(())
}

fn validate_response(flags: &[String]) -> Result<(), String> {
    let input = flag_value(flags, "--input").ok_or("missing --input")?;
    let expected_id = flag_value(flags, "--expect-id");
    let require_ok = has_flag(flags, "--require-ok");
    let expected_kind = response_kind(flags)?;
    let domain_context = response_domain_context(flags)?;
    let archive_context = batch_archive_context(expected_kind, domain_context.as_ref())?;
    let response = validate_response_file(input, expected_id, require_ok)
        .map_err(|error| format!("response validation failed at {input}: {error}"))?;
    if let Some(expected_kind) = expected_kind {
        if expected_kind == RpcResponseKind::AtomicSwapFeeQuote {
            let request_file = flag_value(flags, "--request-file").ok_or(
                "atomic swap quote response validation requires --request-file for exact request binding",
            )?;
            let request = read_request_file(request_file).map_err(|error| {
                format!("atomic swap quote request read failed at {request_file}: {error}")
            })?;
            let quote = decode_atomic_swap_fee_quote_summary(&response, &request)
                .map_err(|error| format!("response validation failed at {input}: {error}"))?;
            validate_response_domain(
                domain_context.as_ref(),
                &quote.unsigned_transaction.chain_id,
                &quote.unsigned_transaction.genesis_hash,
                quote.unsigned_transaction.protocol_version,
            )?;
        } else if expected_kind == RpcResponseKind::MempoolSubmitSignedAtomicSwapTransaction {
            let request_file = flag_value(flags, "--request-file").ok_or(
                "atomic swap submit response validation requires --request-file for exact request binding",
            )?;
            let request = read_request_file(request_file).map_err(|error| {
                format!("atomic swap submit request read failed at {request_file}: {error}")
            })?;
            let entry = decode_atomic_swap_mempool_submit_entry(&response, &request)
                .map_err(|error| format!("response validation failed at {input}: {error}"))?;
            validate_response_domain(
                domain_context.as_ref(),
                &entry.transaction.unsigned.chain_id,
                &entry.transaction.unsigned.genesis_hash,
                entry.transaction.unsigned.protocol_version,
            )?;
        } else if expected_kind == RpcResponseKind::MempoolSubmitSignedAtomicSwapTransactionFinality
        {
            let request_file = flag_value(flags, "--request-file").ok_or(
                "atomic swap finality response validation requires --request-file for exact request binding",
            )?;
            let request = read_request_file(request_file).map_err(|error| {
                format!("atomic swap finality request read failed at {request_file}: {error}")
            })?;
            let finality = decode_atomic_swap_finality_summary(&response, &request)
                .map_err(|error| format!("response validation failed at {input}: {error}"))?;
            validate_response_domain(
                domain_context.as_ref(),
                &finality.chain_id,
                &finality.genesis_hash,
                finality.protocol_version,
            )?;
        } else {
            validate_response_kind_with_context(&response, expected_kind, archive_context.as_ref())
                .map_err(|error| format!("response validation failed at {input}: {error}"))?;
        }
    }
    println!("rpc_response=ok id={} ok={}", response.id, response.ok);
    Ok(())
}

fn write_wallet_backup(flags: &[String]) -> Result<(), String> {
    let chain_id = flag_value(flags, "--chain-id").ok_or("missing --chain-id")?;
    let master_seed_hex = required_secret_flag(
        flags,
        "--master-seed-hex",
        "--master-seed-hex-file",
        "wallet master seed",
    )?;
    let account_index = optional_u32_flag(flags, "--account-index")?.unwrap_or(0);
    let output = flag_value(flags, "--output").ok_or("missing --output")?;
    let backup = wallet_backup_from_master_seed(chain_id, master_seed_hex.as_str(), account_index)
        .map_err(|error| format!("wallet backup creation failed: {error}"))?;
    write_private_json_output(output, &backup)
}

fn write_wallet_identity(flags: &[String]) -> Result<(), String> {
    let backup_file = flag_value(flags, "--backup-file").ok_or("missing --backup-file")?;
    let output = flag_value(flags, "--output").ok_or("missing --output")?;
    let backup = read_wallet_backup_file(backup_file)?;
    let identity = wallet_identity_from_backup(&backup)
        .map_err(|error| format!("wallet identity restore failed: {error}"))?;
    write_json_output(output, &identity)
}

fn write_wallet_signed_quote(flags: &[String]) -> Result<(), String> {
    let backup_file = flag_value(flags, "--backup-file").ok_or("missing --backup-file")?;
    let quote_response_file =
        flag_value(flags, "--quote-response").ok_or("missing --quote-response")?;
    let output = flag_value(flags, "--output").ok_or("missing --output")?;
    let backup = read_wallet_backup_file(backup_file)?;
    let quote_response = read_response_file(quote_response_file)
        .map_err(|error| format!("quote response read failed at {quote_response_file}: {error}"))?;
    let quote = decode_transfer_fee_quote_summary(&quote_response)
        .map_err(|error| format!("quote response validation failed: {error}"))?;
    let signed = wallet_sign_transfer_from_quote(&backup, &quote)
        .map_err(|error| format!("wallet quote signing failed: {error}"))?;
    write_json_output(output, &signed)
}

fn write_wallet_signed_payment_v2(flags: &[String]) -> Result<(), String> {
    let backup_file = flag_value(flags, "--backup-file").ok_or("missing --backup-file")?;
    let output = flag_value(flags, "--output").ok_or("missing --output")?;
    let chain_id = flag_value(flags, "--chain-id").ok_or("missing --chain-id")?;
    let genesis_hash = flag_value(flags, "--genesis-hash").ok_or("missing --genesis-hash")?;
    let protocol_version = u32_flag(flags, "--protocol-version")?;
    let to = flag_value(flags, "--to").ok_or("missing --to")?;
    let amount = u64_flag(flags, "--amount")?;
    let fee = u64_flag(flags, "--fee")?;
    let sequence = u64_flag(flags, "--sequence")?;
    let backup = read_wallet_backup_file(backup_file)?;
    let memos = payment_v2_memos_from_flags(flags)?;
    let signed = wallet_sign_payment_v2_from_fields(
        &backup,
        WalletSignPaymentV2Fields {
            chain_id: chain_id.to_string(),
            genesis_hash: genesis_hash.to_string(),
            protocol_version,
            to: to.to_string(),
            amount,
            fee,
            sequence,
            memos,
        },
    )
    .map_err(|error| format!("wallet payment v2 signing failed: {error}"))?;
    write_json_output(output, &signed)
}

fn write_wallet_signed_asset_transaction(flags: &[String]) -> Result<(), String> {
    let backup_file = flag_value(flags, "--backup-file").ok_or("missing --backup-file")?;
    let quote_response_file =
        flag_value(flags, "--quote-response").ok_or("missing --quote-response")?;
    let output = flag_value(flags, "--output").ok_or("missing --output")?;
    let backup = read_wallet_backup_file(backup_file)?;
    let quote_response = read_response_file(quote_response_file).map_err(|error| {
        format!("asset quote response read failed at {quote_response_file}: {error}")
    })?;
    let signed = wallet_sign_asset_transaction_from_quote(&backup, &quote_response)
        .map_err(|error| format!("wallet asset transaction signing failed: {error}"))?;
    write_json_output(output, &signed)
}

fn write_wallet_signed_escrow_transaction(flags: &[String]) -> Result<(), String> {
    let backup_file = flag_value(flags, "--backup-file").ok_or("missing --backup-file")?;
    let quote_response_file =
        flag_value(flags, "--quote-response").ok_or("missing --quote-response")?;
    let output = flag_value(flags, "--output").ok_or("missing --output")?;
    let backup = read_wallet_backup_file(backup_file)?;
    let quote_response = read_response_file(quote_response_file).map_err(|error| {
        format!("escrow quote response read failed at {quote_response_file}: {error}")
    })?;
    let signed = wallet_sign_escrow_transaction_from_quote(&backup, &quote_response)
        .map_err(|error| format!("wallet escrow transaction signing failed: {error}"))?;
    write_json_output(output, &signed)
}

fn write_wallet_signed_nft_transaction(flags: &[String]) -> Result<(), String> {
    let backup_file = flag_value(flags, "--backup-file").ok_or("missing --backup-file")?;
    let quote_response_file =
        flag_value(flags, "--quote-response").ok_or("missing --quote-response")?;
    let output = flag_value(flags, "--output").ok_or("missing --output")?;
    let backup = read_wallet_backup_file(backup_file)?;
    let quote_response = read_response_file(quote_response_file).map_err(|error| {
        format!("nft quote response read failed at {quote_response_file}: {error}")
    })?;
    let signed = wallet_sign_nft_transaction_from_quote(&backup, &quote_response)
        .map_err(|error| format!("wallet nft transaction signing failed: {error}"))?;
    write_json_output(output, &signed)
}

fn write_wallet_signed_offer_transaction(flags: &[String]) -> Result<(), String> {
    let backup_file = flag_value(flags, "--backup-file").ok_or("missing --backup-file")?;
    let quote_response_file =
        flag_value(flags, "--quote-response").ok_or("missing --quote-response")?;
    let output = flag_value(flags, "--output").ok_or("missing --output")?;
    let backup = read_wallet_backup_file(backup_file)?;
    let quote_response = read_response_file(quote_response_file).map_err(|error| {
        format!("offer quote response read failed at {quote_response_file}: {error}")
    })?;
    let signed = wallet_sign_offer_transaction_from_quote(&backup, &quote_response)
        .map_err(|error| format!("wallet offer transaction signing failed: {error}"))?;
    write_json_output(output, &signed)
}

fn write_wallet_signed_atomic_swap(flags: &[String]) -> Result<(), String> {
    let owner_0_backup_file =
        flag_value(flags, "--owner-0-backup-file").ok_or("missing --owner-0-backup-file")?;
    let owner_1_backup_file =
        flag_value(flags, "--owner-1-backup-file").ok_or("missing --owner-1-backup-file")?;
    let quote_request_file =
        flag_value(flags, "--quote-request").ok_or("missing --quote-request")?;
    let quote_response_file =
        flag_value(flags, "--quote-response").ok_or("missing --quote-response")?;
    let output = flag_value(flags, "--output").ok_or("missing --output")?;
    let owner_0_backup = read_wallet_backup_file(owner_0_backup_file)?;
    let owner_1_backup = read_wallet_backup_file(owner_1_backup_file)?;
    let quote_request = read_request_file(quote_request_file).map_err(|error| {
        format!("atomic swap quote request read failed at {quote_request_file}: {error}")
    })?;
    let quote_response = read_response_file(quote_response_file).map_err(|error| {
        format!("atomic swap quote response read failed at {quote_response_file}: {error}")
    })?;
    let quote = decode_atomic_swap_fee_quote_summary(&quote_response, &quote_request)
        .map_err(|error| format!("atomic swap quote binding failed: {error}"))?;
    let signed = wallet_sign_atomic_swap_from_quote(
        &owner_0_backup,
        &owner_1_backup,
        &quote_request,
        &quote,
    )
    .map_err(|error| format!("wallet atomic swap signing failed: {error}"))?;
    write_private_json_output(output, &signed)
}

fn write_wallet_signed_fastswap_intent(flags: &[String]) -> Result<(), String> {
    let owner_0_backup_file =
        flag_value(flags, "--owner-0-backup-file").ok_or("missing --owner-0-backup-file")?;
    let owner_1_backup_file =
        flag_value(flags, "--owner-1-backup-file").ok_or("missing --owner-1-backup-file")?;
    let intent_file = flag_value(flags, "--intent-file").ok_or("missing --intent-file")?;
    let output = flag_value(flags, "--output").ok_or("missing --output")?;
    let owner_0_backup = read_wallet_backup_file(owner_0_backup_file)?;
    let owner_1_backup = read_wallet_backup_file(owner_1_backup_file)?;
    let intent = serde_json::from_slice::<postfiat_types::FastSwapIntentV1>(
        &fs::read(intent_file)
            .map_err(|error| format!("FastSwap intent read failed at {intent_file}: {error}"))?,
    )
    .map_err(|error| format!("FastSwap intent parse failed at {intent_file}: {error}"))?;
    let signed = wallet_dual_sign_fastswap_intent(&owner_0_backup, &owner_1_backup, intent)
        .map_err(|error| format!("wallet FastSwap signing failed: {error}"))?;
    write_private_json_output(output, &signed)
}

fn write_wallet_signed_fastlane_deposit(flags: &[String]) -> Result<(), String> {
    let backup_file = flag_value(flags, "--backup-file").ok_or("missing --backup-file")?;
    let deposit_file = flag_value(flags, "--deposit-file").ok_or("missing --deposit-file")?;
    let output = flag_value(flags, "--output").ok_or("missing --output")?;
    let backup = read_wallet_backup_file(backup_file)?;
    let deposit = serde_json::from_slice::<postfiat_types::FastLaneDepositV1>(
        &fs::read(deposit_file)
            .map_err(|error| format!("FastLane deposit read failed at {deposit_file}: {error}"))?,
    )
    .map_err(|error| format!("FastLane deposit parse failed at {deposit_file}: {error}"))?;
    let signed = wallet_sign_fastlane_deposit(&backup, deposit)
        .map_err(|error| format!("wallet FastLane deposit signing failed: {error}"))?;
    write_private_json_output(output, &signed)
}

fn write_wallet_signed_owned_deposit(flags: &[String]) -> Result<(), String> {
    let backup_file = flag_value(flags, "--backup-file").ok_or("missing --backup-file")?;
    let deposit_file = flag_value(flags, "--deposit-file").ok_or("missing --deposit-file")?;
    let output = flag_value(flags, "--output").ok_or("missing --output")?;
    let backup = read_wallet_backup_file(backup_file)?;
    let deposit = serde_json::from_slice::<postfiat_types::OwnedDepositV1>(
        &fs::read(deposit_file)
            .map_err(|error| format!("owned deposit read failed at {deposit_file}: {error}"))?,
    )
    .map_err(|error| format!("owned deposit parse failed at {deposit_file}: {error}"))?;
    let signed = wallet_sign_owned_deposit(&backup, deposit)
        .map_err(|error| format!("wallet owned deposit signing failed: {error}"))?;
    write_private_json_output(output, &signed)
}

fn write_wallet_signed_fast_asset_control(flags: &[String]) -> Result<(), String> {
    let backup_file = flag_value(flags, "--backup-file").ok_or("missing --backup-file")?;
    let command_file = flag_value(flags, "--command-file").ok_or("missing --command-file")?;
    let output = flag_value(flags, "--output").ok_or("missing --output")?;
    let backup = read_wallet_backup_file(backup_file)?;
    let command = serde_json::from_slice::<postfiat_types::FastAssetControlCommandV1>(
        &fs::read(command_file).map_err(|error| {
            format!("FastLane asset-control command read failed at {command_file}: {error}")
        })?,
    )
    .map_err(|error| {
        format!("FastLane asset-control command parse failed at {command_file}: {error}")
    })?;
    let signed = wallet_sign_fast_asset_control_command(&backup, command)
        .map_err(|error| format!("wallet FastLane asset-control signing failed: {error}"))?;
    write_private_json_output(output, &signed)
}

fn write_atomic_swap_tx_id(flags: &[String]) -> Result<(), String> {
    let signed_json = required_json_input(
        flags,
        "--signed-atomic-swap-transaction-json",
        "--signed-atomic-swap-transaction-json-file",
        "signed atomic swap transaction",
    )?;
    let signed = serde_json::from_str::<postfiat_types::SignedAtomicSwapTransaction>(&signed_json)
        .map_err(|error| format!("signed atomic swap transaction parse failed: {error}"))?;
    signed
        .validate()
        .map_err(|error| format!("signed atomic swap transaction validation failed: {error}"))?;
    let output = flag_value(flags, "--output").unwrap_or("-");
    write_json_output(
        output,
        &serde_json::json!({
            "schema": "postfiat-atomic-swap-tx-id-v1",
            "tx_id": atomic_swap_transaction_tx_id(&signed),
        }),
    )
}

fn write_wallet_signed_owned_transfer(flags: &[String]) -> Result<(), String> {
    let backup_file = flag_value(flags, "--backup-file").ok_or("missing --backup-file")?;
    let order_file = flag_value(flags, "--order-file").ok_or("missing --order-file")?;
    let output = flag_value(flags, "--output").ok_or("missing --output")?;
    let backup = read_wallet_backup_file(backup_file)?;
    let order_raw = fs::read_to_string(order_file)
        .map_err(|error| format!("owned-transfer order read failed at {order_file}: {error}"))?;
    let order = serde_json::from_str::<postfiat_types::OwnedTransferOrder>(&order_raw)
        .map_err(|error| format!("owned-transfer order parse failed at {order_file}: {error}"))?;
    let signed = wallet_sign_owned_transfer_order(&backup, order)
        .map_err(|error| format!("wallet owned-transfer signing failed: {error}"))?;
    write_json_output(output, &signed)
}

fn write_wallet_signed_owned_unwrap(flags: &[String]) -> Result<(), String> {
    let backup_file = flag_value(flags, "--backup-file").ok_or("missing --backup-file")?;
    let order_file = flag_value(flags, "--order-file").ok_or("missing --order-file")?;
    let output = flag_value(flags, "--output").ok_or("missing --output")?;
    let backup = read_wallet_backup_file(backup_file)?;
    let order_raw = fs::read_to_string(order_file)
        .map_err(|error| format!("owned-unwrap order read failed at {order_file}: {error}"))?;
    let order = serde_json::from_str::<postfiat_types::OwnedUnwrapOrder>(&order_raw)
        .map_err(|error| format!("owned-unwrap order parse failed at {order_file}: {error}"))?;
    let signed = wallet_sign_owned_unwrap_order(&backup, order)
        .map_err(|error| format!("wallet owned-unwrap signing failed: {error}"))?;
    write_json_output(output, &signed)
}

fn write_wallet_signed_owned_transfer_v3(flags: &[String]) -> Result<(), String> {
    let backup_file = flag_value(flags, "--backup-file").ok_or("missing --backup-file")?;
    let order_file = flag_value(flags, "--order-file").ok_or("missing --order-file")?;
    let capabilities_file =
        flag_value(flags, "--capabilities-file").ok_or("missing --capabilities-file")?;
    let output = flag_value(flags, "--output").ok_or("missing --output")?;
    let backup = read_wallet_backup_file(backup_file)?;
    let mut order = serde_json::from_slice::<postfiat_types::OwnedTransferOrderV3>(
        &fs::read(order_file)
            .map_err(|error| format!("FastPay v3 transfer read failed at {order_file}: {error}"))?,
    )
    .map_err(|error| format!("FastPay v3 transfer parse failed at {order_file}: {error}"))?;
    let capabilities = serde_json::from_slice::<postfiat_types::FastPayRecoveryCapabilitiesV1>(
        &fs::read(capabilities_file).map_err(|error| {
            format!("FastPay capabilities read failed at {capabilities_file}: {error}")
        })?,
    )
    .map_err(|error| {
        format!("FastPay capabilities parse failed at {capabilities_file}: {error}")
    })?;
    order.recovery.lock_id = wallet_fastpay_transfer_lock_id_v1(&order);
    let signed = wallet_sign_owned_transfer_order_v3(&backup, order, &capabilities)
        .map_err(|error| format!("wallet FastPay v3 transfer signing failed: {error}"))?;
    write_private_json_output(output, &signed)
}

fn write_wallet_signed_owned_unwrap_v3(flags: &[String]) -> Result<(), String> {
    let backup_file = flag_value(flags, "--backup-file").ok_or("missing --backup-file")?;
    let order_file = flag_value(flags, "--order-file").ok_or("missing --order-file")?;
    let capabilities_file =
        flag_value(flags, "--capabilities-file").ok_or("missing --capabilities-file")?;
    let output = flag_value(flags, "--output").ok_or("missing --output")?;
    let backup = read_wallet_backup_file(backup_file)?;
    let mut order = serde_json::from_slice::<postfiat_types::OwnedUnwrapOrderV3>(
        &fs::read(order_file)
            .map_err(|error| format!("FastPay v3 unwrap read failed at {order_file}: {error}"))?,
    )
    .map_err(|error| format!("FastPay v3 unwrap parse failed at {order_file}: {error}"))?;
    let capabilities = serde_json::from_slice::<postfiat_types::FastPayRecoveryCapabilitiesV1>(
        &fs::read(capabilities_file).map_err(|error| {
            format!("FastPay capabilities read failed at {capabilities_file}: {error}")
        })?,
    )
    .map_err(|error| {
        format!("FastPay capabilities parse failed at {capabilities_file}: {error}")
    })?;
    order.recovery.lock_id = wallet_fastpay_unwrap_lock_id_v1(&order);
    let signed = wallet_sign_owned_unwrap_order_v3(&backup, order, &capabilities)
        .map_err(|error| format!("wallet FastPay v3 unwrap signing failed: {error}"))?;
    write_private_json_output(output, &signed)
}

fn write_verified_fastpay_apply_v3(flags: &[String]) -> Result<(), String> {
    let operation = flag_value(flags, "--operation").ok_or("missing --operation")?;
    let certificate_file =
        flag_value(flags, "--certificate-file").ok_or("missing --certificate-file")?;
    let apply_response_file =
        flag_value(flags, "--apply-response-file").ok_or("missing --apply-response-file")?;
    let capabilities_file =
        flag_value(flags, "--capabilities-file").ok_or("missing --capabilities-file")?;
    let validators_file =
        flag_value(flags, "--validators-file").ok_or("missing --validators-file")?;
    let output = flag_value(flags, "--output").ok_or("missing --output")?;
    let capabilities = serde_json::from_slice::<postfiat_types::FastPayRecoveryCapabilitiesV1>(
        &fs::read(capabilities_file).map_err(|error| {
            format!("FastPay capabilities read failed at {capabilities_file}: {error}")
        })?,
    )
    .map_err(|error| {
        format!("FastPay capabilities parse failed at {capabilities_file}: {error}")
    })?;
    capabilities
        .validate()
        .map_err(|error| format!("FastPay capabilities invalid: {error}"))?;
    let certificate_raw = fs::read(certificate_file).map_err(|error| {
        format!("FastPay certificate read failed at {certificate_file}: {error}")
    })?;
    let (domain, committee_epoch, lock_id, certificate_digest) = match operation {
        "transfer" => {
            let certificate = serde_json::from_slice::<postfiat_types::OwnedTransferCertificateV3>(
                &certificate_raw,
            )
            .map_err(|error| format!("FastPay transfer certificate parse failed: {error}"))?;
            let digest = wallet_fastpay_transfer_certificate_digest_v3(&certificate)
                .map_err(|error| format!("FastPay transfer certificate invalid: {error}"))?;
            (
                certificate.order.domain,
                certificate.order.recovery.committee_epoch,
                certificate.order.recovery.lock_id,
                digest,
            )
        }
        "unwrap" => {
            let certificate = serde_json::from_slice::<postfiat_types::OwnedUnwrapCertificateV3>(
                &certificate_raw,
            )
            .map_err(|error| format!("FastPay unwrap certificate parse failed: {error}"))?;
            let digest = wallet_fastpay_unwrap_certificate_digest_v3(&certificate)
                .map_err(|error| format!("FastPay unwrap certificate invalid: {error}"))?;
            (
                certificate.order.domain,
                certificate.order.recovery.committee_epoch,
                certificate.order.recovery.lock_id,
                digest,
            )
        }
        _ => return Err("--operation must be transfer or unwrap".to_string()),
    };
    if domain != capabilities.domain || committee_epoch != capabilities.committee_epoch {
        return Err("FastPay certificate does not match the live governed capability".to_string());
    }

    let validators_value: serde_json::Value =
        serde_json::from_slice(&fs::read(validators_file).map_err(|error| {
            format!("FastPay validators read failed at {validators_file}: {error}")
        })?)
        .map_err(|error| format!("FastPay validators parse failed: {error}"))?;
    let validator_rows = validators_value
        .get("validators")
        .and_then(serde_json::Value::as_array)
        .or_else(|| validators_value.as_array())
        .ok_or("FastPay validators response omitted validators")?;
    let mut validator_keys = BTreeMap::new();
    for row in validator_rows {
        let validator_id = row
            .get("node_id")
            .or_else(|| row.get("validator_id"))
            .or_else(|| row.get("id"))
            .and_then(serde_json::Value::as_str)
            .ok_or("FastPay validator row omitted ID")?;
        let public_key_hex = row
            .get("public_key_hex")
            .and_then(serde_json::Value::as_str)
            .ok_or("FastPay validator row omitted public key")?;
        if validator_keys
            .insert(validator_id.to_string(), public_key_hex.to_string())
            .is_some()
        {
            return Err("FastPay validator response contains a duplicate ID".to_string());
        }
    }
    if validator_keys.len() != capabilities.validator_count {
        return Err("FastPay validator response does not match the governed count".to_string());
    }

    let apply_response: serde_json::Value =
        serde_json::from_slice(&fs::read(apply_response_file).map_err(|error| {
            format!("FastPay apply response read failed at {apply_response_file}: {error}")
        })?)
        .map_err(|error| format!("FastPay apply response parse failed: {error}"))?;
    if apply_response
        .get("schema")
        .and_then(serde_json::Value::as_str)
        == Some("postfiat-fastpay-certificate-finality-v1")
    {
        if apply_response
            .get("certificate_final")
            .and_then(serde_json::Value::as_bool)
            != Some(true)
        {
            return Err("compact FastPay response is not certificate-final".to_string());
        }
        let expected_method = match operation {
            "transfer" => "owned_apply_v3",
            "unwrap" => "owned_unwrap_apply_v3",
            _ => unreachable!("operation checked while parsing the certificate"),
        };
        if apply_response
            .get("method")
            .and_then(serde_json::Value::as_str)
            != Some(expected_method)
        {
            return Err("compact FastPay response method does not match the operation".to_string());
        }
        let validator_pks = validator_keys
            .iter()
            .map(|(validator_id, public_key_hex)| (validator_id.clone(), public_key_hex.clone()))
            .collect::<Vec<_>>();
        let verified_votes = match operation {
            "transfer" => {
                let certificate = serde_json::from_slice::<
                    postfiat_types::OwnedTransferCertificateV3,
                >(&certificate_raw)
                .map_err(|error| format!("FastPay transfer certificate parse failed: {error}"))?;
                postfiat_execution::verify_owned_transfer_certificate_v3(
                    &certificate,
                    &validator_pks,
                    &capabilities.domain,
                    capabilities.committee_epoch,
                    &capabilities.policy,
                    capabilities.current_height,
                    capabilities.quorum,
                )
            }
            "unwrap" => {
                let certificate =
                    serde_json::from_slice::<postfiat_types::OwnedUnwrapCertificateV3>(
                        &certificate_raw,
                    )
                    .map_err(|error| format!("FastPay unwrap certificate parse failed: {error}"))?;
                postfiat_execution::verify_owned_unwrap_certificate_v3(
                    &certificate,
                    &validator_pks,
                    &capabilities.domain,
                    capabilities.committee_epoch,
                    &capabilities.policy,
                    capabilities.current_height,
                    capabilities.quorum,
                )
            }
            _ => unreachable!("operation checked while parsing the certificate"),
        }
        .map_err(|error| format!("compact FastPay certificate verification failed: {error:?}"))?;
        let response_quorum = apply_response
            .get("certificate_quorum")
            .and_then(serde_json::Value::as_u64)
            .and_then(|value| usize::try_from(value).ok());
        let response_vote_count = apply_response
            .get("certificate_vote_count")
            .and_then(serde_json::Value::as_u64)
            .and_then(|value| usize::try_from(value).ok());
        let response_fleet_count = apply_response
            .get("fleet_count")
            .and_then(serde_json::Value::as_u64)
            .and_then(|value| usize::try_from(value).ok());
        if response_quorum != Some(capabilities.quorum)
            || response_vote_count != Some(verified_votes)
            || response_fleet_count != Some(capabilities.validator_count)
        {
            return Err(
                "compact FastPay response counts do not match the verified certificate".to_string(),
            );
        }
        let acknowledgement_values = apply_response
            .get("apply_acknowledgements")
            .and_then(serde_json::Value::as_array)
            .ok_or("compact FastPay finality omitted the signed apply acknowledgement quorum")?;
        let mut acknowledgements = Vec::new();
        let mut seen = BTreeSet::new();
        let mut terminal_state_digest = None;
        let mut order_digest = None;
        for value in acknowledgement_values {
            let Ok(acknowledgement) =
                serde_json::from_value::<postfiat_types::FastPayApplyAckV1>(value.clone())
            else {
                continue;
            };
            if !seen.insert(acknowledgement.validator_id.clone()) {
                continue;
            }
            let Some(public_key_hex) = validator_keys.get(&acknowledgement.validator_id) else {
                continue;
            };
            if acknowledgement.domain != domain
                || acknowledgement.committee_epoch != committee_epoch
                || acknowledgement.lock_id != lock_id
                || acknowledgement.certificate_digest != certificate_digest
                || terminal_state_digest
                    .as_ref()
                    .is_some_and(|value| value != &acknowledgement.terminal_state_digest)
                || order_digest
                    .as_ref()
                    .is_some_and(|value| value != &acknowledgement.order_digest)
                || wallet_verify_fastpay_apply_ack_v1(&acknowledgement, public_key_hex).is_err()
            {
                continue;
            }
            terminal_state_digest = Some(acknowledgement.terminal_state_digest.clone());
            order_digest = Some(acknowledgement.order_digest.clone());
            acknowledgements.push(acknowledgement);
        }
        acknowledgements.sort_by(|left, right| left.validator_id.cmp(&right.validator_id));
        if acknowledgements.len() < capabilities.quorum {
            return Err(format!(
                "compact FastPay finality has {}/{} authenticated durable acknowledgements",
                acknowledgements.len(),
                capabilities.quorum
            ));
        }
        return write_json_output(
            output,
            &serde_json::json!({
                "schema": "postfiat-fastpay-apply-verification-v1",
                "operation": operation,
                "lock_id": lock_id,
                "certificate_digest": certificate_digest,
                "quorum": capabilities.quorum,
                "certificate_votes_verified": verified_votes,
                "authenticated_acknowledgements": acknowledgements,
            }),
        );
    }
    let rows = apply_response
        .get("validators")
        .and_then(serde_json::Value::as_array)
        .ok_or("FastPay apply response omitted validator results")?;
    let mut accepted = Vec::new();
    let mut seen = BTreeSet::new();
    let mut terminal_state_digest = None;
    let mut order_digest = None;
    for row in rows {
        if row.get("ok").and_then(serde_json::Value::as_bool) != Some(true) {
            continue;
        }
        let Some(validator_id) = row.get("validator_id").and_then(serde_json::Value::as_str) else {
            continue;
        };
        if !seen.insert(validator_id.to_string()) {
            continue;
        }
        let Some(public_key_hex) = validator_keys.get(validator_id) else {
            continue;
        };
        let Some(result) = row.get("result") else {
            continue;
        };
        let Ok(acknowledgement) =
            serde_json::from_value::<postfiat_types::FastPayApplyAckV1>(result.clone())
        else {
            continue;
        };
        if acknowledgement.validator_id != validator_id
            || acknowledgement.domain != domain
            || acknowledgement.committee_epoch != committee_epoch
            || acknowledgement.lock_id != lock_id
            || acknowledgement.certificate_digest != certificate_digest
            || terminal_state_digest
                .as_ref()
                .is_some_and(|value| value != &acknowledgement.terminal_state_digest)
            || order_digest
                .as_ref()
                .is_some_and(|value| value != &acknowledgement.order_digest)
            || wallet_verify_fastpay_apply_ack_v1(&acknowledgement, public_key_hex).is_err()
        {
            continue;
        }
        terminal_state_digest = Some(acknowledgement.terminal_state_digest.clone());
        order_digest = Some(acknowledgement.order_digest.clone());
        accepted.push(acknowledgement);
    }
    accepted.sort_by(|left, right| left.validator_id.cmp(&right.validator_id));
    if accepted.len() < capabilities.quorum {
        return Err(format!(
            "FastPay apply has {}/{} authenticated durable acknowledgements",
            accepted.len(),
            capabilities.quorum
        ));
    }
    write_json_output(
        output,
        &serde_json::json!({
            "schema": "postfiat-fastpay-apply-verification-v1",
            "operation": operation,
            "lock_id": lock_id,
            "certificate_digest": certificate_digest,
            "quorum": capabilities.quorum,
            "authenticated_acknowledgements": accepted,
        }),
    )
}

fn payment_v2_memos_from_flags(flags: &[String]) -> Result<Vec<PaymentMemo>, String> {
    let memo_type = flag_value(flags, "--memo-type").unwrap_or("").to_string();
    let memo_format = flag_value(flags, "--memo-format").unwrap_or("").to_string();
    let memo_data = flag_value(flags, "--memo-data").unwrap_or("").to_string();
    if memo_type.is_empty() && memo_format.is_empty() && memo_data.is_empty() {
        return Ok(Vec::new());
    }
    let memo = PaymentMemo {
        memo_type,
        memo_format,
        memo_data,
    };
    memo.validate()
        .map_err(|error| format!("payment memo invalid: {error}"))?;
    Ok(vec![memo])
}

fn read_wallet_backup_file(path: &str) -> Result<WalletBackupFile, String> {
    validate_secret_file_permissions(path, "wallet backup")?;
    let raw = fs::read_to_string(path)
        .map_err(|error| format!("wallet backup read failed at {path}: {error}"))?;
    let backup = serde_json::from_str::<WalletBackupFile>(&raw)
        .map_err(|error| format!("wallet backup parse failed at {path}: {error}"))?;
    wallet_identity_from_backup(&backup)
        .map_err(|error| format!("wallet backup validation failed at {path}: {error}"))?;
    Ok(backup)
}

fn write_json_output<T: serde::Serialize>(output: &str, value: &T) -> Result<(), String> {
    let json = serde_json::to_string_pretty(value)
        .map_err(|error| format!("JSON serialization failed: {error}"))?;
    if output == "-" {
        println!("{json}");
        return Ok(());
    }
    fs::write(output, format!("{json}\n"))
        .map_err(|error| format!("JSON file write failed at {output}: {error}"))
}

fn write_private_json_output<T: serde::Serialize>(output: &str, value: &T) -> Result<(), String> {
    let json = serde_json::to_string_pretty(value)
        .map_err(|error| format!("JSON serialization failed: {error}"))?;
    if output == "-" {
        println!("{json}");
        return Ok(());
    }
    write_private_file(output, format!("{json}\n").as_bytes())
}

#[cfg(unix)]
fn write_private_file(path: &str, payload: &[u8]) -> Result<(), String> {
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

    let mut file = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)
        .map_err(|error| format!("private JSON file open failed at {path}: {error}"))?;
    file.write_all(payload)
        .map_err(|error| format!("private JSON file write failed at {path}: {error}"))?;
    let mut permissions = file
        .metadata()
        .map_err(|error| format!("private JSON file metadata failed at {path}: {error}"))?
        .permissions();
    permissions.set_mode(0o600);
    file.set_permissions(permissions)
        .map_err(|error| format!("private JSON file permissions failed at {path}: {error}"))
}

#[cfg(not(unix))]
fn write_private_file(path: &str, payload: &[u8]) -> Result<(), String> {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(path)
        .map_err(|error| format!("private JSON file open failed at {path}: {error}"))?;
    file.write_all(payload)
        .map_err(|error| format!("private JSON file write failed at {path}: {error}"))
}

fn request_kind(flags: &[String]) -> Result<Option<RpcRequestKind>, String> {
    let Some(kind) = flag_value(flags, "--expect-kind") else {
        return Ok(None);
    };
    match kind {
        METHOD_STATUS => Ok(Some(RpcRequestKind::Status)),
        METHOD_SERVER_INFO => Ok(Some(RpcRequestKind::ServerInfo)),
        METHOD_METRICS => Ok(Some(RpcRequestKind::Metrics)),
        METHOD_LEDGER => Ok(Some(RpcRequestKind::Ledger)),
        METHOD_VERIFY_STATE => Ok(Some(RpcRequestKind::VerifyState)),
        METHOD_VALIDATE_LOCAL_KEYS => {
            let validators = flag_value(flags, "--validators")
                .map(|value| {
                    value
                        .parse::<u32>()
                        .map_err(|_| "--validators must be a u32".to_string())
                })
                .transpose()?;
            Ok(Some(RpcRequestKind::ValidateLocalKeys { validators }))
        }
        METHOD_ACCOUNT => Ok(Some(RpcRequestKind::Account)),
        METHOD_ACCOUNT_TX => Ok(Some(RpcRequestKind::AccountTx)),
        METHOD_FEE => Ok(Some(RpcRequestKind::Fee)),
        METHOD_TRANSFER_FEE_QUOTE => Ok(Some(RpcRequestKind::TransferFeeQuote)),
        METHOD_ATOMIC_SWAP_FEE_QUOTE => Ok(Some(RpcRequestKind::AtomicSwapFeeQuote)),
        METHOD_ESCROW_FEE_QUOTE => Ok(Some(RpcRequestKind::EscrowFeeQuote)),
        METHOD_OFFER_FEE_QUOTE => Ok(Some(RpcRequestKind::OfferFeeQuote)),
        METHOD_ATOMIC_SETTLEMENT_TEMPLATE => Ok(Some(RpcRequestKind::AtomicSettlementTemplate)),
        METHOD_OFFER_INFO => Ok(Some(RpcRequestKind::OfferInfo)),
        METHOD_ACCOUNT_OFFERS => Ok(Some(RpcRequestKind::AccountOffers)),
        METHOD_BOOK_OFFERS => Ok(Some(RpcRequestKind::BookOffers)),
        METHOD_ESCROW_INFO => Ok(Some(RpcRequestKind::EscrowInfo)),
        METHOD_ACCOUNT_ESCROWS => Ok(Some(RpcRequestKind::AccountEscrows)),
        METHOD_NFT_INFO => Ok(Some(RpcRequestKind::NftInfo)),
        METHOD_ACCOUNT_NFTS => Ok(Some(RpcRequestKind::AccountNfts)),
        METHOD_ISSUER_NFTS => Ok(Some(RpcRequestKind::IssuerNfts)),
        METHOD_RECEIPTS => Ok(Some(RpcRequestKind::Receipts)),
        METHOD_TX => Ok(Some(RpcRequestKind::Tx)),
        METHOD_BLOCKS => Ok(Some(RpcRequestKind::Blocks)),
        METHOD_VALIDATORS => Ok(Some(RpcRequestKind::Validators)),
        METHOD_MANIFESTS => Ok(Some(RpcRequestKind::Manifests)),
        METHOD_BATCH_ARCHIVE => Ok(Some(RpcRequestKind::BatchArchive)),
        METHOD_ARCHIVE_WINDOW => Ok(Some(RpcRequestKind::ArchiveWindow)),
        METHOD_MEMPOOL_SUBMIT_TRANSFER => Ok(Some(RpcRequestKind::MempoolSubmitTransfer)),
        METHOD_MEMPOOL_SUBMIT_SIGNED_TRANSFER => {
            Ok(Some(RpcRequestKind::MempoolSubmitSignedTransfer))
        }
        METHOD_MEMPOOL_SUBMIT_SIGNED_PAYMENT_V2 => {
            Ok(Some(RpcRequestKind::MempoolSubmitSignedPaymentV2))
        }
        METHOD_MEMPOOL_SUBMIT_SIGNED_ATOMIC_SWAP_TRANSACTION => Ok(Some(
            RpcRequestKind::MempoolSubmitSignedAtomicSwapTransaction,
        )),
        METHOD_MEMPOOL_SUBMIT_SIGNED_ATOMIC_SWAP_TRANSACTION_FINALITY => Ok(Some(
            RpcRequestKind::MempoolSubmitSignedAtomicSwapTransactionFinality,
        )),
        METHOD_MEMPOOL_SUBMIT_SIGNED_ESCROW_TRANSACTION => {
            Ok(Some(RpcRequestKind::MempoolSubmitSignedEscrowTransaction))
        }
        METHOD_MEMPOOL_SUBMIT_SIGNED_OFFER_TRANSACTION => {
            Ok(Some(RpcRequestKind::MempoolSubmitSignedOfferTransaction))
        }
        METHOD_MEMPOOL_STATUS => Ok(Some(RpcRequestKind::MempoolStatus)),
        METHOD_MEMPOOL_BATCH => Ok(Some(RpcRequestKind::MempoolBatch)),
        METHOD_APPLY_BATCH => Ok(Some(RpcRequestKind::ApplyBatch)),
        METHOD_SHIELD_BATCH_MINT => Ok(Some(RpcRequestKind::ShieldBatchMint)),
        METHOD_SHIELD_BATCH_SPEND => Ok(Some(RpcRequestKind::ShieldBatchSpend)),
        METHOD_SHIELD_BATCH_MIGRATE => Ok(Some(RpcRequestKind::ShieldBatchMigrate)),
        METHOD_SHIELD_BATCH_ORCHARD => Ok(Some(RpcRequestKind::ShieldBatchOrchard)),
        METHOD_SHIELD_BATCH_ORCHARD_DEPOSIT => {
            Ok(Some(RpcRequestKind::ShieldBatchOrchardDeposit))
        }
        METHOD_SHIELD_BATCH_ORCHARD_WITHDRAW => {
            Ok(Some(RpcRequestKind::ShieldBatchOrchardWithdraw))
        }
        METHOD_SHIELD_BATCH_SWAP => Ok(Some(RpcRequestKind::ShieldBatchSwap)),
        METHOD_APPLY_SHIELD_BATCH => Ok(Some(RpcRequestKind::ApplyShieldBatch)),
        METHOD_SHIELD_SCAN => Ok(Some(RpcRequestKind::ShieldScan)),
        METHOD_SHIELD_DISCLOSE => Ok(Some(RpcRequestKind::ShieldDisclose)),
        METHOD_SHIELD_TURNSTILE => Ok(Some(RpcRequestKind::ShieldTurnstile)),
        METHOD_BRIDGE_STATUS => Ok(Some(RpcRequestKind::BridgeStatus)),
        METHOD_NAVCOIN_BRIDGE_ROUTES => Ok(Some(RpcRequestKind::NavcoinBridgeRoutes)),
        METHOD_NAVCOIN_BRIDGE_PACKET => Ok(Some(RpcRequestKind::NavcoinBridgePacket)),
        METHOD_NAVCOIN_BRIDGE_CLAIMS => Ok(Some(RpcRequestKind::NavcoinBridgeClaims)),
        METHOD_NAVCOIN_BRIDGE_SUPPLY_STATUS => {
            Ok(Some(RpcRequestKind::NavcoinBridgeSupplyStatus))
        }
        METHOD_NAVCOIN_BRIDGE_RECEIPT_REPLAY => {
            Ok(Some(RpcRequestKind::NavcoinBridgeReceiptReplay))
        }
        METHOD_NAVCOIN_BRIDGE_PACKET_PREFLIGHT => {
            Ok(Some(RpcRequestKind::NavcoinBridgePacketPreflight))
        }
        METHOD_BRIDGE_BATCH_DOMAIN => Ok(Some(RpcRequestKind::BridgeBatchDomain)),
        METHOD_BRIDGE_BATCH_TRANSFER => Ok(Some(RpcRequestKind::BridgeBatchTransfer)),
        METHOD_BRIDGE_BATCH_PAUSE => Ok(Some(RpcRequestKind::BridgeBatchPause)),
        METHOD_BRIDGE_BATCH_RESUME => Ok(Some(RpcRequestKind::BridgeBatchResume)),
        METHOD_APPLY_BRIDGE_BATCH => Ok(Some(RpcRequestKind::ApplyBridgeBatch)),
        METHOD_MEMPOOL_SUBMIT_FASTLANE_PRIMARY => {
            Ok(Some(RpcRequestKind::MempoolSubmitFastLanePrimary))
        }
        METHOD_FASTSWAP_CAPABILITIES => Ok(Some(RpcRequestKind::FastSwapCapabilities)),
        METHOD_FASTSWAP_PREVIEW => Ok(Some(RpcRequestKind::FastSwapPreview)),
        METHOD_FASTSWAP_PREPARE => Ok(Some(RpcRequestKind::FastSwapPrepare)),
        METHOD_FASTSWAP_COMMIT => Ok(Some(RpcRequestKind::FastSwapCommit)),
        METHOD_FASTSWAP_APPLY => Ok(Some(RpcRequestKind::FastSwapApply)),
        METHOD_FASTSWAP_CATCH_UP => Ok(Some(RpcRequestKind::FastSwapCatchUp)),
        METHOD_FASTSWAP_STATUS => Ok(Some(RpcRequestKind::FastSwapStatus)),
        METHOD_FASTSWAP_EFFECTS => Ok(Some(RpcRequestKind::FastSwapEffects)),
        METHOD_FASTSWAP_VOTES => Ok(Some(RpcRequestKind::FastSwapVotes)),
        METHOD_FASTSWAP_NEW_ROUND_VOTE => Ok(Some(RpcRequestKind::FastSwapNewRoundVote)),
        METHOD_FASTSWAP_PROPOSE_ROUND => Ok(Some(RpcRequestKind::FastSwapProposeRound)),
        METHOD_FASTSWAP_PRECOMMIT => Ok(Some(RpcRequestKind::FastSwapPrecommit)),
        METHOD_FASTSWAP_COMMIT_ROUND => Ok(Some(RpcRequestKind::FastSwapCommitRound)),
        METHOD_FASTSWAP_CANCEL_APPLY => Ok(Some(RpcRequestKind::FastSwapCancelApply)),
        METHOD_FASTLANE_EXIT => Ok(Some(RpcRequestKind::FastLaneExit)),
        METHOD_FASTSWAP_CHECKPOINT_STATUS => Ok(Some(RpcRequestKind::FastSwapCheckpointStatus)),
        METHOD_FASTSWAP_OBJECTS => Ok(Some(RpcRequestKind::FastSwapObjects)),
        METHOD_FASTSWAP_POLICY => Ok(Some(RpcRequestKind::FastSwapPolicy)),
        METHOD_FASTLANE_ASSET_CONTROL_PREVIEW => {
            Ok(Some(RpcRequestKind::FastLaneAssetControlPreview))
        }
        METHOD_FASTLANE_ASSET_CONTROL_PREPARE => {
            Ok(Some(RpcRequestKind::FastLaneAssetControlPrepare))
        }
        METHOD_FASTLANE_ASSET_CONTROL_APPLY => {
            Ok(Some(RpcRequestKind::FastLaneAssetControlApply))
        }
        METHOD_FASTLANE_ASSET_CONTROL_CATCH_UP => {
            Ok(Some(RpcRequestKind::FastLaneAssetControlCatchUp))
        }
        other => Err(format!(
            "unsupported request kind `{other}`; supported: {METHOD_STATUS}, {METHOD_SERVER_INFO}, {METHOD_METRICS}, {METHOD_LEDGER}, {METHOD_VERIFY_STATE}, {METHOD_VALIDATE_LOCAL_KEYS}, {METHOD_ACCOUNT}, {METHOD_ACCOUNT_TX}, {METHOD_FEE}, {METHOD_TRANSFER_FEE_QUOTE}, {METHOD_ESCROW_INFO}, {METHOD_ACCOUNT_ESCROWS}, {METHOD_NFT_INFO}, {METHOD_ACCOUNT_NFTS}, {METHOD_ISSUER_NFTS}, {METHOD_RECEIPTS}, {METHOD_TX}, {METHOD_BLOCKS}, {METHOD_VALIDATORS}, {METHOD_MANIFESTS}, {METHOD_BATCH_ARCHIVE}, {METHOD_ARCHIVE_WINDOW}, {METHOD_MEMPOOL_SUBMIT_TRANSFER}, {METHOD_MEMPOOL_SUBMIT_SIGNED_TRANSFER}, {METHOD_MEMPOOL_STATUS}, {METHOD_MEMPOOL_BATCH}, {METHOD_APPLY_BATCH}, {METHOD_SHIELD_BATCH_MINT}, {METHOD_SHIELD_BATCH_SPEND}, {METHOD_SHIELD_BATCH_MIGRATE}, {METHOD_SHIELD_BATCH_ORCHARD}, {METHOD_SHIELD_BATCH_ORCHARD_DEPOSIT}, {METHOD_SHIELD_BATCH_ORCHARD_WITHDRAW}, {METHOD_SHIELD_BATCH_SWAP}, {METHOD_APPLY_SHIELD_BATCH}, {METHOD_SHIELD_SCAN}, {METHOD_SHIELD_DISCLOSE}, {METHOD_SHIELD_TURNSTILE}, {METHOD_BRIDGE_STATUS}, {METHOD_NAVCOIN_BRIDGE_ROUTES}, {METHOD_NAVCOIN_BRIDGE_PACKET}, {METHOD_NAVCOIN_BRIDGE_CLAIMS}, {METHOD_NAVCOIN_BRIDGE_SUPPLY_STATUS}, {METHOD_NAVCOIN_BRIDGE_RECEIPT_REPLAY}, {METHOD_NAVCOIN_BRIDGE_PACKET_PREFLIGHT}, {METHOD_BRIDGE_BATCH_DOMAIN}, {METHOD_BRIDGE_BATCH_TRANSFER}, {METHOD_BRIDGE_BATCH_PAUSE}, {METHOD_BRIDGE_BATCH_RESUME}, {METHOD_APPLY_BRIDGE_BATCH}"
        )),
    }
}

fn response_kind(flags: &[String]) -> Result<Option<RpcResponseKind>, String> {
    let Some(kind) = flag_value(flags, "--expect-kind") else {
        return Ok(None);
    };
    match kind {
        METHOD_STATUS => Ok(Some(RpcResponseKind::Status)),
        METHOD_SERVER_INFO => Ok(Some(RpcResponseKind::ServerInfo)),
        METHOD_METRICS => Ok(Some(RpcResponseKind::Metrics)),
        METHOD_LEDGER => Ok(Some(RpcResponseKind::Ledger)),
        METHOD_VERIFY_STATE => Ok(Some(RpcResponseKind::VerifyState)),
        METHOD_VALIDATE_LOCAL_KEYS => {
            let validators = flag_value(flags, "--validators")
                .map(|value| {
                    value
                        .parse::<u32>()
                        .map_err(|_| "--validators must be a u32".to_string())
                })
                .transpose()?;
            Ok(Some(RpcResponseKind::ValidateLocalKeys { validators }))
        }
        METHOD_ACCOUNT => Ok(Some(RpcResponseKind::Account)),
        METHOD_ACCOUNT_TX => Ok(Some(RpcResponseKind::AccountTx)),
        METHOD_FEE => Ok(Some(RpcResponseKind::Fee)),
        METHOD_TRANSFER_FEE_QUOTE => Ok(Some(RpcResponseKind::TransferFeeQuote)),
        METHOD_ATOMIC_SWAP_FEE_QUOTE => Ok(Some(RpcResponseKind::AtomicSwapFeeQuote)),
        METHOD_ESCROW_FEE_QUOTE => Ok(Some(RpcResponseKind::EscrowFeeQuote)),
        METHOD_OFFER_FEE_QUOTE => Ok(Some(RpcResponseKind::OfferFeeQuote)),
        METHOD_ATOMIC_SETTLEMENT_TEMPLATE => Ok(Some(RpcResponseKind::AtomicSettlementTemplate)),
        METHOD_OFFER_INFO => Ok(Some(RpcResponseKind::OfferInfo)),
        METHOD_ACCOUNT_OFFERS => Ok(Some(RpcResponseKind::AccountOffers)),
        METHOD_BOOK_OFFERS => Ok(Some(RpcResponseKind::BookOffers)),
        METHOD_ESCROW_INFO => Ok(Some(RpcResponseKind::EscrowInfo)),
        METHOD_ACCOUNT_ESCROWS => Ok(Some(RpcResponseKind::AccountEscrows)),
        METHOD_NFT_INFO => Ok(Some(RpcResponseKind::NftInfo)),
        METHOD_ACCOUNT_NFTS => Ok(Some(RpcResponseKind::AccountNfts)),
        METHOD_ISSUER_NFTS => Ok(Some(RpcResponseKind::IssuerNfts)),
        METHOD_RECEIPTS => Ok(Some(RpcResponseKind::Receipts)),
        METHOD_TX => Ok(Some(RpcResponseKind::Tx)),
        METHOD_BLOCKS => Ok(Some(RpcResponseKind::Blocks)),
        METHOD_VALIDATORS => Ok(Some(RpcResponseKind::Validators)),
        METHOD_MANIFESTS => Ok(Some(RpcResponseKind::Manifests)),
        METHOD_BATCH_ARCHIVE => Ok(Some(RpcResponseKind::BatchArchive)),
        METHOD_ARCHIVE_WINDOW => Ok(Some(RpcResponseKind::ArchiveWindow)),
        METHOD_MEMPOOL_SUBMIT_TRANSFER => Ok(Some(RpcResponseKind::MempoolSubmitTransfer)),
        METHOD_MEMPOOL_SUBMIT_SIGNED_TRANSFER => {
            Ok(Some(RpcResponseKind::MempoolSubmitSignedTransfer))
        }
        METHOD_MEMPOOL_SUBMIT_SIGNED_PAYMENT_V2 => {
            Ok(Some(RpcResponseKind::MempoolSubmitSignedPaymentV2))
        }
        METHOD_MEMPOOL_SUBMIT_SIGNED_ATOMIC_SWAP_TRANSACTION => Ok(Some(
            RpcResponseKind::MempoolSubmitSignedAtomicSwapTransaction,
        )),
        METHOD_MEMPOOL_SUBMIT_SIGNED_ATOMIC_SWAP_TRANSACTION_FINALITY => Ok(Some(
            RpcResponseKind::MempoolSubmitSignedAtomicSwapTransactionFinality,
        )),
        METHOD_MEMPOOL_SUBMIT_SIGNED_ESCROW_TRANSACTION => {
            Ok(Some(RpcResponseKind::MempoolSubmitSignedEscrowTransaction))
        }
        METHOD_MEMPOOL_SUBMIT_SIGNED_OFFER_TRANSACTION => {
            Ok(Some(RpcResponseKind::MempoolSubmitSignedOfferTransaction))
        }
        METHOD_MEMPOOL_STATUS => Ok(Some(RpcResponseKind::MempoolStatus)),
        METHOD_MEMPOOL_BATCH => Ok(Some(RpcResponseKind::MempoolBatch)),
        METHOD_APPLY_BATCH => Ok(Some(RpcResponseKind::ApplyBatch)),
        METHOD_SHIELD_BATCH_MINT => Ok(Some(RpcResponseKind::ShieldBatchMint)),
        METHOD_SHIELD_BATCH_SPEND => Ok(Some(RpcResponseKind::ShieldBatchSpend)),
        METHOD_SHIELD_BATCH_MIGRATE => Ok(Some(RpcResponseKind::ShieldBatchMigrate)),
        METHOD_SHIELD_BATCH_ORCHARD => Ok(Some(RpcResponseKind::ShieldBatchOrchard)),
        METHOD_SHIELD_BATCH_ORCHARD_DEPOSIT => {
            Ok(Some(RpcResponseKind::ShieldBatchOrchardDeposit))
        }
        METHOD_SHIELD_BATCH_ORCHARD_WITHDRAW => {
            Ok(Some(RpcResponseKind::ShieldBatchOrchardWithdraw))
        }
        METHOD_SHIELD_BATCH_SWAP => Ok(Some(RpcResponseKind::ShieldBatchSwap)),
        METHOD_APPLY_SHIELD_BATCH => Ok(Some(RpcResponseKind::ApplyShieldBatch)),
        METHOD_SHIELD_SCAN => Ok(Some(RpcResponseKind::ShieldScan)),
        METHOD_SHIELD_DISCLOSE => Ok(Some(RpcResponseKind::ShieldDisclose)),
        METHOD_SHIELD_TURNSTILE => Ok(Some(RpcResponseKind::ShieldTurnstile)),
        METHOD_BRIDGE_STATUS => Ok(Some(RpcResponseKind::BridgeStatus)),
        METHOD_NAVCOIN_BRIDGE_ROUTES => Ok(Some(RpcResponseKind::NavcoinBridgeRoutes)),
        METHOD_NAVCOIN_BRIDGE_PACKET => Ok(Some(RpcResponseKind::NavcoinBridgePacket)),
        METHOD_NAVCOIN_BRIDGE_CLAIMS => Ok(Some(RpcResponseKind::NavcoinBridgeClaims)),
        METHOD_NAVCOIN_BRIDGE_SUPPLY_STATUS => {
            Ok(Some(RpcResponseKind::NavcoinBridgeSupplyStatus))
        }
        METHOD_NAVCOIN_BRIDGE_RECEIPT_REPLAY => {
            Ok(Some(RpcResponseKind::NavcoinBridgeReceiptReplay))
        }
        METHOD_NAVCOIN_BRIDGE_PACKET_PREFLIGHT => {
            Ok(Some(RpcResponseKind::NavcoinBridgePacketPreflight))
        }
        METHOD_BRIDGE_BATCH_DOMAIN => Ok(Some(RpcResponseKind::BridgeBatchDomain)),
        METHOD_BRIDGE_BATCH_TRANSFER => Ok(Some(RpcResponseKind::BridgeBatchTransfer)),
        METHOD_BRIDGE_BATCH_PAUSE => Ok(Some(RpcResponseKind::BridgeBatchPause)),
        METHOD_BRIDGE_BATCH_RESUME => Ok(Some(RpcResponseKind::BridgeBatchResume)),
        METHOD_APPLY_BRIDGE_BATCH => Ok(Some(RpcResponseKind::ApplyBridgeBatch)),
        METHOD_MEMPOOL_SUBMIT_FASTLANE_PRIMARY => {
            Ok(Some(RpcResponseKind::MempoolSubmitFastLanePrimary))
        }
        METHOD_FASTSWAP_CAPABILITIES => Ok(Some(RpcResponseKind::FastSwapCapabilities)),
        METHOD_FASTSWAP_PREVIEW => Ok(Some(RpcResponseKind::FastSwapPreview)),
        METHOD_FASTSWAP_PREPARE
        | METHOD_FASTSWAP_COMMIT
        | METHOD_FASTSWAP_APPLY
        | METHOD_FASTSWAP_CATCH_UP
        | METHOD_FASTSWAP_PROPOSE_ROUND
        | METHOD_FASTSWAP_PRECOMMIT
        | METHOD_FASTSWAP_COMMIT_ROUND
        | METHOD_FASTSWAP_CANCEL_APPLY
        | METHOD_FASTLANE_ASSET_CONTROL_PREPARE
        | METHOD_FASTLANE_ASSET_CONTROL_APPLY
        | METHOD_FASTLANE_ASSET_CONTROL_CATCH_UP => Ok(Some(RpcResponseKind::FastSwapVote)),
        METHOD_FASTSWAP_STATUS => Ok(Some(RpcResponseKind::FastSwapStatus)),
        METHOD_FASTSWAP_EFFECTS => Ok(Some(RpcResponseKind::FastSwapEffects)),
        METHOD_FASTSWAP_VOTES => Ok(Some(RpcResponseKind::FastSwapVoteEvidence)),
        METHOD_FASTSWAP_NEW_ROUND_VOTE => Ok(Some(RpcResponseKind::FastSwapNewRoundVote)),
        METHOD_FASTLANE_EXIT => Ok(Some(RpcResponseKind::FastLaneExitVote)),
        METHOD_FASTSWAP_CHECKPOINT_STATUS => Ok(Some(RpcResponseKind::FastSwapCheckpointStatus)),
        METHOD_FASTSWAP_OBJECTS => Ok(Some(RpcResponseKind::FastSwapObjects)),
        METHOD_FASTSWAP_POLICY => Ok(Some(RpcResponseKind::FastSwapPolicy)),
        METHOD_FASTLANE_ASSET_CONTROL_PREVIEW => {
            Ok(Some(RpcResponseKind::FastLaneAssetControlPreview))
        }
        other => Err(format!(
            "unsupported response kind `{other}`; supported: {METHOD_STATUS}, {METHOD_SERVER_INFO}, {METHOD_METRICS}, {METHOD_LEDGER}, {METHOD_VERIFY_STATE}, {METHOD_VALIDATE_LOCAL_KEYS}, {METHOD_ACCOUNT}, {METHOD_ACCOUNT_TX}, {METHOD_FEE}, {METHOD_TRANSFER_FEE_QUOTE}, {METHOD_ESCROW_INFO}, {METHOD_ACCOUNT_ESCROWS}, {METHOD_NFT_INFO}, {METHOD_ACCOUNT_NFTS}, {METHOD_ISSUER_NFTS}, {METHOD_RECEIPTS}, {METHOD_TX}, {METHOD_BLOCKS}, {METHOD_VALIDATORS}, {METHOD_MANIFESTS}, {METHOD_BATCH_ARCHIVE}, {METHOD_ARCHIVE_WINDOW}, {METHOD_MEMPOOL_SUBMIT_TRANSFER}, {METHOD_MEMPOOL_SUBMIT_SIGNED_TRANSFER}, {METHOD_MEMPOOL_STATUS}, {METHOD_MEMPOOL_BATCH}, {METHOD_APPLY_BATCH}, {METHOD_SHIELD_BATCH_MINT}, {METHOD_SHIELD_BATCH_SPEND}, {METHOD_SHIELD_BATCH_MIGRATE}, {METHOD_SHIELD_BATCH_ORCHARD}, {METHOD_SHIELD_BATCH_ORCHARD_DEPOSIT}, {METHOD_SHIELD_BATCH_ORCHARD_WITHDRAW}, {METHOD_SHIELD_BATCH_SWAP}, {METHOD_APPLY_SHIELD_BATCH}, {METHOD_SHIELD_SCAN}, {METHOD_SHIELD_DISCLOSE}, {METHOD_SHIELD_TURNSTILE}, {METHOD_BRIDGE_STATUS}, {METHOD_NAVCOIN_BRIDGE_ROUTES}, {METHOD_NAVCOIN_BRIDGE_PACKET}, {METHOD_NAVCOIN_BRIDGE_CLAIMS}, {METHOD_NAVCOIN_BRIDGE_SUPPLY_STATUS}, {METHOD_NAVCOIN_BRIDGE_RECEIPT_REPLAY}, {METHOD_NAVCOIN_BRIDGE_PACKET_PREFLIGHT}, {METHOD_BRIDGE_BATCH_DOMAIN}, {METHOD_BRIDGE_BATCH_TRANSFER}, {METHOD_BRIDGE_BATCH_PAUSE}, {METHOD_BRIDGE_BATCH_RESUME}, {METHOD_APPLY_BRIDGE_BATCH}"
        )),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ResponseDomainContext {
    chain_id: String,
    genesis_hash: String,
    protocol_version: u32,
}

fn response_domain_context(flags: &[String]) -> Result<Option<ResponseDomainContext>, String> {
    let chain_id = flag_value(flags, "--chain-id");
    let genesis_hash = flag_value(flags, "--genesis-hash");
    let protocol_version = flag_value(flags, "--protocol-version");
    if chain_id.is_none() && genesis_hash.is_none() && protocol_version.is_none() {
        return Ok(None);
    }
    let chain_id = chain_id.ok_or("missing --chain-id")?;
    if chain_id.trim().is_empty() {
        return Err("--chain-id must be nonempty".to_string());
    }
    let genesis_hash = genesis_hash.ok_or("missing --genesis-hash")?;
    if !is_lower_hex_len(genesis_hash, 96) {
        return Err("--genesis-hash must be 96 lowercase hex characters".to_string());
    }
    let protocol_version = protocol_version
        .ok_or("missing --protocol-version")?
        .parse::<u32>()
        .map_err(|_| "--protocol-version must be a u32".to_string())?;
    Ok(Some(ResponseDomainContext {
        chain_id: chain_id.to_string(),
        genesis_hash: genesis_hash.to_string(),
        protocol_version,
    }))
}

fn batch_archive_context(
    expected_kind: Option<RpcResponseKind>,
    domain_context: Option<&ResponseDomainContext>,
) -> Result<Option<BatchArchiveValidationContext>, String> {
    let Some(domain_context) = domain_context else {
        return Ok(None);
    };
    if expected_kind == Some(RpcResponseKind::BatchArchive) {
        return Ok(Some(BatchArchiveValidationContext {
            chain_id: domain_context.chain_id.clone(),
            genesis_hash: domain_context.genesis_hash.clone(),
            protocol_version: domain_context.protocol_version,
        }));
    }
    if matches!(
        expected_kind,
        Some(
            RpcResponseKind::AtomicSwapFeeQuote
                | RpcResponseKind::MempoolSubmitSignedAtomicSwapTransaction
                | RpcResponseKind::MempoolSubmitSignedAtomicSwapTransactionFinality
        )
    ) {
        return Ok(None);
    }
    Err(
        "response domain context flags require an atomic swap response or --expect-kind batch_archive"
            .to_string(),
    )
}

fn validate_response_domain(
    expected: Option<&ResponseDomainContext>,
    chain_id: &str,
    genesis_hash: &str,
    protocol_version: u32,
) -> Result<(), String> {
    let Some(expected) = expected else {
        return Ok(());
    };
    if chain_id != expected.chain_id
        || genesis_hash != expected.genesis_hash
        || protocol_version != expected.protocol_version
    {
        return Err(
            "atomic swap response domain does not match --chain-id/--genesis-hash/--protocol-version"
                .to_string(),
        );
    }
    Ok(())
}

fn flag_value<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    args.windows(2)
        .find(|window| window[0] == flag)
        .map(|window| window[1].as_str())
}

fn has_flag(args: &[String], flag: &str) -> bool {
    args.iter().any(|arg| arg == flag)
}

fn required_json_input(
    args: &[String],
    inline_flag: &str,
    file_flag: &str,
    label: &str,
) -> Result<String, String> {
    match (flag_value(args, inline_flag), flag_value(args, file_flag)) {
        (Some(_), Some(_)) => Err(format!(
            "{label} must use either {inline_flag} or {file_flag}, not both"
        )),
        (Some(value), None) if !value.trim().is_empty() => Ok(value.to_string()),
        (Some(_), None) => Err(format!("{inline_flag} must be nonempty")),
        (None, Some(path)) => {
            let value = fs::read_to_string(path)
                .map_err(|error| format!("failed to read {label} file `{path}`: {error}"))?;
            if value.trim().is_empty() {
                return Err(format!("{label} file `{path}` is empty"));
            }
            Ok(value)
        }
        (None, None) => Err(format!("missing {inline_flag} or {file_flag}")),
    }
}

fn required_secret_flag(
    args: &[String],
    inline_flag: &str,
    file_flag: &str,
    label: &str,
) -> Result<Zeroizing<String>, String> {
    match (flag_value(args, inline_flag), flag_value(args, file_flag)) {
        (Some(_), Some(_)) => Err(format!(
            "{label} must use either {inline_flag} or {file_flag}, not both"
        )),
        (Some(value), None) => Ok(Zeroizing::new(value.to_string())),
        (None, Some(path)) => read_secret_file(path, label),
        (None, None) => Err(format!("missing {inline_flag} or {file_flag}")),
    }
}

fn read_secret_file(path: &str, label: &str) -> Result<Zeroizing<String>, String> {
    validate_secret_file_permissions(path, label)?;
    let raw = fs::read_to_string(path)
        .map_err(|error| format!("failed to read {label} file `{path}`: {error}"))?;
    let value = raw.trim().to_string();
    if value.is_empty() {
        return Err(format!("{label} file `{path}` is empty"));
    }
    Ok(Zeroizing::new(value))
}

#[cfg(unix)]
fn validate_secret_file_permissions(path: &str, label: &str) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;

    let metadata = fs::metadata(path)
        .map_err(|error| format!("failed to inspect {label} file `{path}`: {error}"))?;
    if !metadata.is_file() {
        return Err(format!("{label} file `{path}` is not a regular file"));
    }
    let mode = metadata.permissions().mode();
    if mode & 0o077 != 0 {
        return Err(format!(
            "{label} file `{path}` must not be group/world readable or writable"
        ));
    }
    Ok(())
}

#[cfg(not(unix))]
fn validate_secret_file_permissions(path: &str, label: &str) -> Result<(), String> {
    let metadata = fs::metadata(path)
        .map_err(|error| format!("failed to inspect {label} file `{path}`: {error}"))?;
    if !metadata.is_file() {
        return Err(format!("{label} file `{path}` is not a regular file"));
    }
    Ok(())
}

fn optional_usize_flag(args: &[String], flag: &str) -> Result<Option<usize>, String> {
    flag_value(args, flag)
        .map(|value| {
            value
                .parse::<usize>()
                .map_err(|_| format!("{flag} must be a usize"))
        })
        .transpose()
}

fn optional_u32_flag(args: &[String], flag: &str) -> Result<Option<u32>, String> {
    flag_value(args, flag)
        .map(|value| {
            value
                .parse::<u32>()
                .map_err(|_| format!("{flag} must be a u32"))
        })
        .transpose()
}

fn optional_u64_flag(args: &[String], flag: &str) -> Result<Option<u64>, String> {
    flag_value(args, flag)
        .map(|value| {
            value
                .parse::<u64>()
                .map_err(|_| format!("{flag} must be a u64"))
        })
        .transpose()
}

fn optional_string(args: &[String], flag: &str) -> Option<String> {
    flag_value(args, flag).map(str::to_string)
}

fn is_lower_hex_len(value: &str, expected_len: usize) -> bool {
    value.len() == expected_len
        && value
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
}

fn u64_flag(args: &[String], flag: &str) -> Result<u64, String> {
    flag_value(args, flag)
        .ok_or_else(|| format!("missing {flag}"))?
        .parse::<u64>()
        .map_err(|_| format!("{flag} must be a u64"))
}

fn u32_flag(args: &[String], flag: &str) -> Result<u32, String> {
    let value = u64_flag(args, flag)?;
    u32::try_from(value).map_err(|_| format!("{flag} must be a u32"))
}

fn print_usage() {
    eprintln!(
        r#"usage:
  postfiat-rpc-sdk request --method METHOD --id ID --output PATH [method flags]
  postfiat-rpc-sdk validate-request --input PATH [--expect-id ID] [--expect-kind KIND] [--validators N]
  postfiat-rpc-sdk validate-response --input PATH [--expect-id ID] [--require-ok] [--expect-kind KIND] [--request-file PATH] [--validators N] [--chain-id ID --genesis-hash HEX --protocol-version N]
  postfiat-rpc-sdk wallet-backup --chain-id ID (--master-seed-hex HEX | --master-seed-hex-file PATH) [--account-index N] --output PATH
  postfiat-rpc-sdk wallet-identity --backup-file PATH --output PATH
  postfiat-rpc-sdk wallet-sign-quote --backup-file PATH --quote-response PATH --output PATH
  postfiat-rpc-sdk wallet-sign-payment-v2 --backup-file PATH --chain-id ID --genesis-hash HASH --protocol-version N --to ADDRESS --amount AMOUNT --fee FEE --sequence N --output PATH [--memo-type TEXT] [--memo-format TEXT] [--memo-data TEXT]
  postfiat-rpc-sdk wallet-sign-asset-transaction --backup-file PATH --quote-response PATH --output PATH
  postfiat-rpc-sdk wallet-sign-escrow-transaction --backup-file PATH --quote-response PATH --output PATH
  postfiat-rpc-sdk wallet-sign-nft-transaction --backup-file PATH --quote-response PATH --output PATH
  postfiat-rpc-sdk wallet-sign-offer-transaction --backup-file PATH --quote-response PATH --output PATH
  postfiat-rpc-sdk wallet-sign-atomic-swap --owner-0-backup-file PATH --owner-1-backup-file PATH --quote-request PATH --quote-response PATH --output PATH
  postfiat-rpc-sdk wallet-sign-fastswap-intent --owner-0-backup-file PATH --owner-1-backup-file PATH --intent-file PATH --output PATH
  postfiat-rpc-sdk wallet-sign-fastlane-deposit --backup-file PATH --deposit-file PATH --output PATH
  postfiat-rpc-sdk wallet-sign-owned-deposit --backup-file PATH --deposit-file PATH --output PATH
  postfiat-rpc-sdk wallet-sign-fastlane-asset-control --backup-file PATH --command-file PATH --output PATH
  postfiat-rpc-sdk atomic-swap-tx-id (--signed-atomic-swap-transaction-json JSON | --signed-atomic-swap-transaction-json-file PATH) [--output PATH]
  postfiat-rpc-sdk wallet-sign-owned-transfer --backup-file PATH --order-file PATH --output PATH
  postfiat-rpc-sdk wallet-sign-owned-unwrap --backup-file PATH --order-file PATH --output PATH
  postfiat-rpc-sdk wallet-sign-owned-transfer-v3 --backup-file PATH --order-file PATH --capabilities-file PATH --output PATH
  postfiat-rpc-sdk wallet-sign-owned-unwrap-v3 --backup-file PATH --order-file PATH --capabilities-file PATH --output PATH
  postfiat-rpc-sdk wallet-verify-fastpay-apply-v3 --operation transfer|unwrap --certificate-file PATH --apply-response-file PATH --capabilities-file PATH --validators-file PATH --output PATH

Supported request methods: status, server_info, metrics, ledger, verify_state, validate_local_keys, account, account_tx, fee, transfer_fee_quote, atomic_swap_fee_quote, escrow_fee_quote, offer_fee_quote, atomic_settlement_template, offer_info, account_offers, book_offers, escrow_info, account_escrows, nft_info, account_nfts, issuer_nfts, receipts, tx, blocks, validators, manifests, batch_archive, archive_window, mempool_submit_transfer, mempool_submit_signed_transfer, mempool_submit_signed_payment_v2, mempool_submit_signed_atomic_swap_transaction, mempool_submit_signed_atomic_swap_transaction_finality, mempool_submit_signed_escrow_transaction, mempool_submit_signed_offer_transaction, mempool_status, mempool_batch, apply_batch, shield_batch_mint, shield_batch_spend, shield_batch_migrate, shield_batch_orchard, shield_batch_orchard_deposit, shield_batch_orchard_withdraw, shield_batch_swap, apply_shield_batch, shield_scan, shield_disclose, shield_turnstile, bridge_status, navcoin_bridge_routes, navcoin_bridge_packet, navcoin_bridge_claims, navcoin_bridge_supply_status, navcoin_bridge_receipt_replay, navcoin_bridge_packet_preflight, bridge_batch_domain, bridge_batch_transfer, bridge_batch_pause, bridge_batch_resume, apply_bridge_batch.
FastSwap methods: mempool_submit_fastlane_primary, mempool_submit_fastlane_primary_finality, fastswap_capabilities, fastswap_preview, fastswap_prepare, fastswap_commit, fastswap_apply, fastswap_catch_up, fastswap_status, fastswap_effects, fastswap_new_round_vote, fastswap_propose_round, fastswap_precommit, fastswap_commit_round, fastswap_cancel_apply, fastlane_exit, fastswap_checkpoint_status, fastswap_objects, fastswap_policy, fastlane_asset_control_preview, fastlane_asset_control_prepare, fastlane_asset_control_apply, fastlane_asset_control_catch_up.
FastLane primary submit requires exactly one of --fastlane-primary-json or --fastlane-primary-json-file.
Account_tx request supports --address, --from-height, --to-height, and --limit.
Escrow_fee_quote request supports --source, --operation-json, and --sequence.
Offer_fee_quote request supports --source, --operation-json, and --sequence.
Atomic_settlement_template request supports --left-owner, --left-recipient, --left-asset-id, --left-amount, --right-owner, --right-recipient, --right-asset-id, --right-amount, --condition, --finish-after, --cancel-after, --left-sequence, and --right-sequence.
Atomic_swap_fee_quote supports --rfq-hash, --market-envelope-hash, --nav-epoch, --expires-at-height, --swap-nonce, and --leg-N-owner/recipient/issuer/asset-id/amount for N=0,1.
Atomic swap submit supports exactly one of --signed-atomic-swap-transaction-json or --signed-atomic-swap-transaction-json-file. Finality submit additionally requires --proxy-required-current-height, --proxy-required-state-root, and --proxy-required-parent-hash, with optional --proxy-readiness-timeout-ms.
Validating atomic swap quote, raw-submit, or finality responses requires --request-file so the response is bound to the exact request and signed body; --chain-id, --genesis-hash, and --protocol-version may be supplied together to assert its chain domain.
Offer_info request supports --offer-id. Account_offers request supports --account, --state, and --limit. Book_offers request supports --taker-gets-asset-id, --taker-pays-asset-id, and --limit.
Escrow_info request supports --escrow-id. Account_escrows request supports --account, --role, --state, and --limit.
Nft_info request supports --nft-id. Account_nfts request supports --account, --include-burned, and --limit. Issuer_nfts request supports --issuer, --collection-id, --include-burned, and --limit.
Blocks request supports --from-height and --limit.
Tx request supports --audit-block-log for full replay verification.
Supported response kinds: status, server_info, metrics, ledger, verify_state, validate_local_keys, account, account_tx, fee, transfer_fee_quote, atomic_swap_fee_quote, escrow_fee_quote, offer_fee_quote, atomic_settlement_template, offer_info, account_offers, book_offers, escrow_info, account_escrows, nft_info, account_nfts, issuer_nfts, receipts, tx, blocks, validators, manifests, batch_archive, archive_window, mempool_submit_transfer, mempool_submit_signed_transfer, mempool_submit_signed_payment_v2, mempool_submit_signed_atomic_swap_transaction, mempool_submit_signed_atomic_swap_transaction_finality, mempool_submit_signed_escrow_transaction, mempool_submit_signed_offer_transaction, mempool_status, mempool_batch, apply_batch, shield_batch_mint, shield_batch_spend, shield_batch_migrate, shield_batch_orchard, shield_batch_orchard_deposit, shield_batch_orchard_withdraw, shield_batch_swap, apply_shield_batch, shield_scan, shield_disclose, shield_turnstile, bridge_status, navcoin_bridge_routes, navcoin_bridge_packet, navcoin_bridge_claims, navcoin_bridge_supply_status, navcoin_bridge_receipt_replay, navcoin_bridge_packet_preflight, bridge_batch_domain, bridge_batch_transfer, bridge_batch_pause, bridge_batch_resume, apply_bridge_batch.
Batch archive response validation can bind payload hashes with --chain-id, --genesis-hash, and --protocol-version.
Use --output - to print request, wallet identity, wallet backup, or signed transaction JSON to stdout."#
    );
}

#[cfg(test)]
mod atomic_swap_cli_tests {
    use super::*;

    fn test_atomic_swap() -> postfiat_types::SignedAtomicSwapTransaction {
        let owner_0 = format!("pf{}", "01".repeat(20));
        let owner_1 = format!("pf{}", "02".repeat(20));
        postfiat_types::SignedAtomicSwapTransaction {
            unsigned: postfiat_types::UnsignedAtomicSwapTransaction {
                chain_id: "postfiat-local".to_string(),
                genesis_hash: "aa".repeat(48),
                protocol_version: 1,
                address_namespace: postfiat_types::ADDRESS_NAMESPACE.to_string(),
                signature_algorithm_id: postfiat_crypto_provider::ML_DSA_65_ALGORITHM.to_string(),
                rfq_hash: "bb".repeat(48),
                market_envelope_hash: "cc".repeat(48),
                nav_epoch: 7,
                expires_at_height: 99,
                swap_nonce: "dd".repeat(48),
                leg_0: postfiat_types::AtomicSwapLeg {
                    owner: owner_0.clone(),
                    recipient: owner_1.clone(),
                    issuer: format!("pf{}", "03".repeat(20)),
                    asset_id: "10".repeat(48),
                    amount: 20_000,
                    sequence: 3,
                    fee: 22,
                },
                leg_1: postfiat_types::AtomicSwapLeg {
                    owner: owner_1.clone(),
                    recipient: owner_0.clone(),
                    issuer: format!("pf{}", "04".repeat(20)),
                    asset_id: "20".repeat(48),
                    amount: 164_020,
                    sequence: 5,
                    fee: 22,
                },
            },
            authorization_0: postfiat_types::AtomicSwapAuthorization {
                owner: owner_0,
                algorithm_id: postfiat_crypto_provider::ML_DSA_65_ALGORITHM.to_string(),
                public_key_hex: "aa".to_string(),
                signature_hex: "bb".to_string(),
            },
            authorization_1: postfiat_types::AtomicSwapAuthorization {
                owner: owner_1,
                algorithm_id: postfiat_crypto_provider::ML_DSA_65_ALGORITHM.to_string(),
                public_key_hex: "cc".to_string(),
                signature_hex: "dd".to_string(),
            },
        }
    }

    #[test]
    fn atomic_swap_cli_kinds_and_request_paths_are_wired() {
        for (method, expected_request_kind, expected_response_kind) in [
            (
                METHOD_MEMPOOL_SUBMIT_FASTLANE_PRIMARY,
                RpcRequestKind::MempoolSubmitFastLanePrimary,
                RpcResponseKind::MempoolSubmitFastLanePrimary,
            ),
            (
                METHOD_ATOMIC_SWAP_FEE_QUOTE,
                RpcRequestKind::AtomicSwapFeeQuote,
                RpcResponseKind::AtomicSwapFeeQuote,
            ),
            (
                METHOD_MEMPOOL_SUBMIT_SIGNED_ATOMIC_SWAP_TRANSACTION,
                RpcRequestKind::MempoolSubmitSignedAtomicSwapTransaction,
                RpcResponseKind::MempoolSubmitSignedAtomicSwapTransaction,
            ),
            (
                METHOD_MEMPOOL_SUBMIT_SIGNED_ATOMIC_SWAP_TRANSACTION_FINALITY,
                RpcRequestKind::MempoolSubmitSignedAtomicSwapTransactionFinality,
                RpcResponseKind::MempoolSubmitSignedAtomicSwapTransactionFinality,
            ),
        ] {
            let flags = vec!["--expect-kind".to_string(), method.to_string()];
            assert_eq!(
                request_kind(&flags).expect("request kind"),
                Some(expected_request_kind)
            );
            assert_eq!(
                response_kind(&flags).expect("response kind"),
                Some(expected_response_kind)
            );
        }

        let swap = test_atomic_swap();
        let path = env::temp_dir().join(format!(
            "postfiat-rpc-sdk-atomic-cli-{}.json",
            process::id()
        ));
        let args = [
            "--method".to_string(),
            METHOD_MEMPOOL_SUBMIT_SIGNED_ATOMIC_SWAP_TRANSACTION_FINALITY.to_string(),
            "--id".to_string(),
            "atomic-cli-finality".to_string(),
            "--output".to_string(),
            path.display().to_string(),
            "--signed-atomic-swap-transaction-json".to_string(),
            serde_json::to_string(&swap).expect("signed swap JSON"),
            "--proxy-required-current-height".to_string(),
            "7".to_string(),
            "--proxy-required-state-root".to_string(),
            "11".repeat(48),
            "--proxy-required-parent-hash".to_string(),
            "22".repeat(48),
        ];
        write_request(&args).expect("write atomic finality request from CLI flags");
        let request = read_request_file(&path).expect("read atomic finality CLI request");
        postfiat_rpc_sdk::validate_request(
            &request,
            Some("atomic-cli-finality"),
            Some(RpcRequestKind::MempoolSubmitSignedAtomicSwapTransactionFinality),
        )
        .expect("validate atomic finality CLI request");

        fs::remove_file(path).expect("remove atomic CLI request");

        let primary_path = env::temp_dir().join(format!(
            "postfiat-rpc-sdk-fastlane-primary-cli-{}.json",
            process::id()
        ));
        let primary_json = r#"{"operation":{"anchor_checkpoint":{"certificate":{}}}}"#;
        let primary_args = [
            "--method".to_string(),
            METHOD_MEMPOOL_SUBMIT_FASTLANE_PRIMARY.to_string(),
            "--id".to_string(),
            "fastlane-primary-cli".to_string(),
            "--output".to_string(),
            primary_path.display().to_string(),
            "--fastlane-primary-json".to_string(),
            primary_json.to_string(),
        ];
        write_request(&primary_args).expect("write FastLane primary request from CLI flags");
        let primary_request =
            read_request_file(&primary_path).expect("read FastLane primary CLI request");
        postfiat_rpc_sdk::validate_request(
            &primary_request,
            Some("fastlane-primary-cli"),
            Some(RpcRequestKind::MempoolSubmitFastLanePrimary),
        )
        .expect("validate FastLane primary CLI request");
        assert_eq!(
            primary_request
                .params
                .get("fastlane_primary_json")
                .and_then(serde_json::Value::as_str),
            Some(primary_json)
        );
        fs::remove_file(primary_path).expect("remove FastLane primary CLI request");

        assert!(write_wallet_signed_atomic_swap(&[])
            .expect_err("missing dual-wallet flags")
            .contains("--owner-0-backup-file"));
    }

    #[test]
    fn wallet_sign_atomic_swap_cli_file_path_succeeds() {
        let root = env::temp_dir().join(format!(
            "postfiat-rpc-sdk-wallet-sign-atomic-swap-{}",
            process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("create CLI test directory");
        let owner_0_backup_path = root.join("owner-0.wallet.json");
        let owner_1_backup_path = root.join("owner-1.wallet.json");
        let quote_request_path = root.join("quote.request.json");
        let quote_response_path = root.join("quote.response.json");
        let signed_path = root.join("signed-atomic-swap.json");

        let backup_0 = wallet_backup_from_master_seed("postfiat-local", "11".repeat(32), 0)
            .expect("owner 0 backup");
        let backup_1 = wallet_backup_from_master_seed("postfiat-local", "22".repeat(32), 0)
            .expect("owner 1 backup");
        let owner_0 = wallet_identity_from_backup(&backup_0).expect("owner 0 identity");
        let owner_1 = wallet_identity_from_backup(&backup_1).expect("owner 1 identity");
        write_private_json_output(
            owner_0_backup_path.to_str().expect("owner 0 backup path"),
            &backup_0,
        )
        .expect("write owner 0 backup");
        write_private_json_output(
            owner_1_backup_path.to_str().expect("owner 1 backup path"),
            &backup_1,
        )
        .expect("write owner 1 backup");

        let unsigned = postfiat_types::UnsignedAtomicSwapTransaction {
            chain_id: "postfiat-local".to_string(),
            genesis_hash: "aa".repeat(48),
            protocol_version: 1,
            address_namespace: postfiat_types::ADDRESS_NAMESPACE.to_string(),
            signature_algorithm_id: postfiat_crypto_provider::ML_DSA_65_ALGORITHM.to_string(),
            rfq_hash: "bb".repeat(48),
            market_envelope_hash: "cc".repeat(48),
            nav_epoch: 7,
            expires_at_height: 99,
            swap_nonce: "dd".repeat(48),
            leg_0: postfiat_types::AtomicSwapLeg {
                owner: owner_0.address.clone(),
                recipient: owner_1.address.clone(),
                issuer: format!("pf{}", "33".repeat(20)),
                asset_id: "10".repeat(48),
                amount: 20_000,
                sequence: 3,
                fee: 22,
            },
            leg_1: postfiat_types::AtomicSwapLeg {
                owner: owner_1.address.clone(),
                recipient: owner_0.address.clone(),
                issuer: format!("pf{}", "44".repeat(20)),
                asset_id: "20".repeat(48),
                amount: 164_020,
                sequence: 5,
                fee: 22,
            },
        };
        let request = atomic_swap_fee_quote_request(
            "atomic-cli-quote",
            unsigned.rfq_hash.clone(),
            unsigned.market_envelope_hash.clone(),
            unsigned.nav_epoch,
            unsigned.expires_at_height,
            unsigned.swap_nonce.clone(),
            unsigned.leg_0.owner.clone(),
            unsigned.leg_0.recipient.clone(),
            unsigned.leg_0.issuer.clone(),
            unsigned.leg_0.asset_id.clone(),
            unsigned.leg_0.amount,
            unsigned.leg_1.owner.clone(),
            unsigned.leg_1.recipient.clone(),
            unsigned.leg_1.issuer.clone(),
            unsigned.leg_1.asset_id.clone(),
            unsigned.leg_1.amount,
        );
        let response = postfiat_rpc_sdk::success_response(
            &request.id,
            &serde_json::json!({
                "schema": postfiat_rpc_sdk::ATOMIC_SWAP_FEE_QUOTE_SCHEMA,
                "transaction_kind": postfiat_types::ATOMIC_SWAP_TRANSACTION_KIND,
                "parent_height": 7,
                "parent_hash": "01".repeat(48),
                "parent_state_root": "02".repeat(48),
                "quote_height": 8,
                "account_reserve": 10,
                "transfer_fee_byte_quantum": 512,
                "transfer_fee_per_quantum": 1,
                "atomic_swap_weight_bytes": 4096,
                "leg_0": {
                    "owner": owner_0.address,
                    "sender_balance": 1_000,
                    "sender_sequence": 2,
                    "sequence": 3,
                    "mempool_pending_for_owner": 0,
                    "base_atomic_swap_fee": 20,
                    "state_expansion_fee": 2,
                    "minimum_fee": 22,
                    "sender_balance_after_fee": 978,
                    "sender_meets_reserve_after_fee": true
                },
                "leg_1": {
                    "owner": owner_1.address,
                    "sender_balance": 1_000,
                    "sender_sequence": 4,
                    "sequence": 5,
                    "mempool_pending_for_owner": 0,
                    "base_atomic_swap_fee": 20,
                    "state_expansion_fee": 2,
                    "minimum_fee": 22,
                    "sender_balance_after_fee": 978,
                    "sender_meets_reserve_after_fee": true
                },
                "unsigned_transaction": unsigned
            }),
            Vec::new(),
        )
        .expect("build quote response");
        write_request_file(&quote_request_path, &request).expect("write quote request");
        postfiat_rpc_sdk::write_response_file(&quote_response_path, &response)
            .expect("write quote response");
        let quote_validation_flags = vec![
            "--input".to_string(),
            quote_response_path.display().to_string(),
            "--expect-id".to_string(),
            request.id.clone(),
            "--require-ok".to_string(),
            "--expect-kind".to_string(),
            METHOD_ATOMIC_SWAP_FEE_QUOTE.to_string(),
            "--request-file".to_string(),
            quote_request_path.display().to_string(),
            "--chain-id".to_string(),
            "postfiat-local".to_string(),
            "--genesis-hash".to_string(),
            "aa".repeat(48),
            "--protocol-version".to_string(),
            "1".to_string(),
        ];
        validate_response(&quote_validation_flags)
            .expect("validate request-bound atomic quote in the invoked chain domain");
        let mut wrong_domain_flags = quote_validation_flags;
        let genesis_index = wrong_domain_flags
            .iter()
            .position(|flag| flag == "--genesis-hash")
            .expect("genesis flag")
            + 1;
        wrong_domain_flags[genesis_index] = "ff".repeat(48);
        assert!(validate_response(&wrong_domain_flags)
            .expect_err("wrong-domain atomic quote reached signing")
            .contains("response domain"));

        write_wallet_signed_atomic_swap(&[
            "--owner-0-backup-file".to_string(),
            owner_0_backup_path.display().to_string(),
            "--owner-1-backup-file".to_string(),
            owner_1_backup_path.display().to_string(),
            "--quote-request".to_string(),
            quote_request_path.display().to_string(),
            "--quote-response".to_string(),
            quote_response_path.display().to_string(),
            "--output".to_string(),
            signed_path.display().to_string(),
        ])
        .expect("shipping wallet-sign-atomic-swap file path");

        let signed: postfiat_types::SignedAtomicSwapTransaction = serde_json::from_str(
            &fs::read_to_string(&signed_path).expect("read signed atomic swap"),
        )
        .expect("parse signed atomic swap");
        assert_eq!(signed.unsigned, unsigned);
        assert_eq!(signed.authorization_0.owner, signed.unsigned.leg_0.owner);
        assert_eq!(signed.authorization_1.owner, signed.unsigned.leg_1.owner);
        signed.validate().expect("validate signed atomic swap");
        let tx_id_path = root.join("atomic-swap-tx-id.json");
        write_atomic_swap_tx_id(&[
            "--signed-atomic-swap-transaction-json-file".to_string(),
            signed_path.display().to_string(),
            "--output".to_string(),
            tx_id_path.display().to_string(),
        ])
        .expect("derive atomic swap tx id");
        let tx_id: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&tx_id_path).expect("read tx-id output"))
                .expect("parse tx-id output");
        assert_eq!(tx_id["tx_id"], atomic_swap_transaction_tx_id(&signed));
        let serialized = serde_json::to_string(&signed).expect("serialize signed atomic swap");
        for forbidden in ["trustline", "trust_set", "line_create"] {
            assert!(!serialized.contains(forbidden), "found `{forbidden}`");
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&signed_path)
                    .expect("signed output metadata")
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn fastpay_v3_cli_signs_lock_bound_order_and_verifies_distinct_ack_quorum() {
        let root =
            env::temp_dir().join(format!("postfiat-rpc-sdk-fastpay-v3-cli-{}", process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("create FastPay v3 CLI test directory");
        let backup_path = root.join("wallet.json");
        let order_path = root.join("order.json");
        let capabilities_path = root.join("capabilities.json");
        let signed_path = root.join("signed.json");
        let certificate_path = root.join("certificate.json");
        let validators_path = root.join("validators.json");
        let apply_path = root.join("apply.json");
        let verification_path = root.join("verification.json");

        let backup = wallet_backup_from_master_seed("postfiat-fastpay-v3", "71".repeat(32), 0)
            .expect("FastPay v3 wallet backup");
        let identity = wallet_identity_from_backup(&backup).expect("FastPay v3 wallet identity");
        write_private_json_output(backup_path.to_str().expect("backup path"), &backup)
            .expect("write FastPay v3 backup");
        let domain = postfiat_types::OwnedCertificateDomain {
            schema: postfiat_types::OWNED_CERTIFICATE_DOMAIN_SCHEMA_V3.to_string(),
            chain_id: backup.chain_id.clone(),
            genesis_hash: "72".repeat(48),
            protocol_version: 3,
            registry_id: "73".repeat(48),
        };
        let capabilities = postfiat_types::FastPayRecoveryCapabilitiesV1 {
            schema: postfiat_types::FASTPAY_RECOVERY_CAPABILITIES_SCHEMA_V1.to_string(),
            domain: domain.clone(),
            committee_epoch: 7,
            current_height: 100,
            validator_count: 4,
            quorum: 3,
            policy: postfiat_types::FastPayRecoveryPolicyV1 {
                schema: postfiat_types::FASTPAY_RECOVERY_POLICY_SCHEMA_V1.to_string(),
                activation_height: 90,
                max_validity_blocks: 10,
                max_recovery_blocks: 10,
            },
        };
        let order = postfiat_types::OwnedTransferOrderV3 {
            domain: domain.clone(),
            recovery: postfiat_types::FastPayOrderRecoveryV1 {
                schema: postfiat_types::FASTPAY_ORDER_RECOVERY_SCHEMA_V1.to_string(),
                committee_epoch: 7,
                lock_id: "00".repeat(48),
                valid_from_height: 100,
                expires_at_height: 110,
                recovery_closes_at_height: 120,
            },
            inputs: vec![postfiat_types::OwnedObjectRef {
                id: "74".repeat(32),
                version: 2,
            }],
            outputs: vec![postfiat_types::OwnedOutputSpec {
                owner_pubkey_hex: identity.public_key_hex.clone(),
                value: 9,
                asset: "PFT".to_string(),
            }],
            fee: 1,
            nonce: 8,
            memos: Vec::new(),
        };
        write_json_output(order_path.to_str().expect("order path"), &order)
            .expect("write FastPay v3 order");
        write_json_output(
            capabilities_path.to_str().expect("capabilities path"),
            &capabilities,
        )
        .expect("write FastPay capabilities");
        write_wallet_signed_owned_transfer_v3(&[
            "--backup-file".to_string(),
            backup_path.display().to_string(),
            "--order-file".to_string(),
            order_path.display().to_string(),
            "--capabilities-file".to_string(),
            capabilities_path.display().to_string(),
            "--output".to_string(),
            signed_path.display().to_string(),
        ])
        .expect("sign FastPay v3 transfer through CLI");
        let signed: postfiat_types::SignedOwnedTransferOrderV3 = serde_json::from_slice(
            &fs::read(&signed_path).expect("read signed FastPay v3 transfer"),
        )
        .expect("parse signed FastPay v3 transfer");
        assert_eq!(
            signed.order.recovery.lock_id,
            postfiat_types::fastpay_transfer_lock_id_v1(&signed.order)
        );
        assert_ne!(signed.order.recovery.lock_id, "00".repeat(48));

        let validator_key_pairs = (0..4_u8)
            .map(|index| {
                (
                    format!("validator-{index}"),
                    postfiat_crypto_provider::ml_dsa_65_keygen_from_seed(&[index + 1; 32]),
                )
            })
            .collect::<Vec<_>>();
        let transfer_signing_bytes =
            postfiat_execution::owned_transfer_v3_signing_bytes(&signed.order);
        let certificate = postfiat_types::OwnedTransferCertificateV3 {
            order: signed.order.clone(),
            owner_pubkey_hex: signed.owner_pubkey_hex,
            owner_signature_hex: signed.owner_signature_hex,
            votes: validator_key_pairs
                .iter()
                .take(3)
                .map(
                    |(validator_id, key_pair)| postfiat_types::OwnedTransferVote {
                        validator_id: validator_id.clone(),
                        signature_hex: postfiat_crypto_provider::bytes_to_hex(
                            &postfiat_crypto_provider::ml_dsa_65_sign_with_context(
                                &key_pair.private_key,
                                &transfer_signing_bytes,
                                postfiat_execution::OWNED_TRANSFER_CONTEXT_V3,
                            )
                            .expect("sign FastPay certificate vote"),
                        ),
                    },
                )
                .collect(),
        };
        let certificate_digest = wallet_fastpay_transfer_certificate_digest_v3(&certificate)
            .expect("FastPay v3 certificate digest");
        write_json_output(
            certificate_path.to_str().expect("certificate path"),
            &certificate,
        )
        .expect("write FastPay certificate");

        let mut validator_rows = Vec::new();
        let mut apply_rows = Vec::new();
        for (index, (validator_id, key_pair)) in validator_key_pairs.iter().enumerate() {
            let public_key_hex = postfiat_crypto_provider::bytes_to_hex(&key_pair.public_key);
            validator_rows.push(serde_json::json!({
                "node_id": validator_id,
                "public_key_hex": public_key_hex,
            }));
            if index >= 3 {
                continue;
            }
            let mut acknowledgement = postfiat_types::FastPayApplyAckV1 {
                schema: postfiat_types::FASTPAY_APPLY_ACK_SCHEMA_V1.to_string(),
                domain: domain.clone(),
                committee_epoch: 7,
                lock_id: certificate.order.recovery.lock_id.clone(),
                order_digest: "75".repeat(48),
                certificate_digest: certificate_digest.clone(),
                terminal_state_digest: "76".repeat(48),
                validator_id: validator_id.clone(),
                signature_hex: String::new(),
            };
            let signing_bytes =
                postfiat_execution::fastpay_apply_ack_signing_bytes_v1(&acknowledgement)
                    .expect("FastPay ack signing bytes");
            acknowledgement.signature_hex = postfiat_crypto_provider::bytes_to_hex(
                &postfiat_crypto_provider::ml_dsa_65_sign_with_context(
                    &key_pair.private_key,
                    &signing_bytes,
                    postfiat_execution::FASTPAY_APPLY_ACK_CONTEXT_V1,
                )
                .expect("sign FastPay apply ack"),
            );
            apply_rows.push(serde_json::json!({
                "validator_id": validator_id,
                "ok": true,
                "result": acknowledgement,
            }));
        }
        write_json_output(
            validators_path.to_str().expect("validators path"),
            &serde_json::json!({"validators": validator_rows}),
        )
        .expect("write FastPay validators");
        write_json_output(
            apply_path.to_str().expect("apply path"),
            &serde_json::json!({"validators": apply_rows}),
        )
        .expect("write FastPay apply response");
        let verification_flags = vec![
            "--operation".to_string(),
            "transfer".to_string(),
            "--certificate-file".to_string(),
            certificate_path.display().to_string(),
            "--apply-response-file".to_string(),
            apply_path.display().to_string(),
            "--capabilities-file".to_string(),
            capabilities_path.display().to_string(),
            "--validators-file".to_string(),
            validators_path.display().to_string(),
            "--output".to_string(),
            verification_path.display().to_string(),
        ];
        write_verified_fastpay_apply_v3(&verification_flags)
            .expect("verify FastPay v3 distinct ack quorum");
        let verification: serde_json::Value = serde_json::from_slice(
            &fs::read(&verification_path).expect("read FastPay apply verification"),
        )
        .expect("parse FastPay apply verification");
        assert_eq!(verification["quorum"], 3);
        assert_eq!(
            verification["authenticated_acknowledgements"]
                .as_array()
                .expect("authenticated acknowledgements")
                .len(),
            3
        );

        let compact_acks = apply_rows
            .iter()
            .take(3)
            .map(|row| row["result"].clone())
            .collect::<Vec<_>>();
        write_json_output(
            apply_path.to_str().expect("compact apply path"),
            &serde_json::json!({
                "schema": "postfiat-fastpay-certificate-finality-v1",
                "method": "owned_apply_v3",
                "certificate_final": true,
                "certificate_quorum": 3,
                "certificate_vote_count": 3,
                "apply_acknowledgements": compact_acks,
                "fleet_count": 4,
            }),
        )
        .expect("write compact FastPay finality response");
        write_verified_fastpay_apply_v3(&verification_flags)
            .expect("verify compact FastPay finality with signed ack");
        let compact_verification: serde_json::Value = serde_json::from_slice(
            &fs::read(&verification_path).expect("read compact FastPay verification"),
        )
        .expect("parse compact FastPay verification");
        assert_eq!(compact_verification["certificate_votes_verified"], 3);
        assert_eq!(
            compact_verification["authenticated_acknowledgements"]
                .as_array()
                .expect("compact authenticated acknowledgements")
                .len(),
            3
        );

        write_json_output(
            apply_path.to_str().expect("missing ack apply path"),
            &serde_json::json!({
                "schema": "postfiat-fastpay-certificate-finality-v1",
                "method": "owned_apply_v3",
                "certificate_final": true,
                "certificate_quorum": 3,
                "certificate_vote_count": 3,
                "fleet_count": 4,
            }),
        )
        .expect("write compact response without signed ack");
        assert!(write_verified_fastpay_apply_v3(&verification_flags)
            .expect_err("compact finality without a signed ack must fail closed")
            .contains("signed apply acknowledgement"));

        let mut tampered: serde_json::Value = serde_json::from_slice(
            &serde_json::to_vec(&serde_json::json!({"validators": apply_rows}))
                .expect("serialize FastPay apply response for tamper"),
        )
        .expect("parse FastPay apply response for tamper");
        tampered["validators"][0]["result"]["terminal_state_digest"] =
            serde_json::Value::String("77".repeat(48));
        write_json_output(apply_path.to_str().expect("apply path"), &tampered)
            .expect("write tampered FastPay response");
        assert!(write_verified_fastpay_apply_v3(&verification_flags)
            .expect_err("tampered ack must drop below quorum")
            .contains("2/3 authenticated"));

        let _ = fs::remove_dir_all(root);
    }
}
