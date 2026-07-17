/* tslint:disable */
/* eslint-disable */

/**
 *
 * method: RPC method name
 * params_json: params as JSON object string
 * Returns: complete RPC request JSON string ready to send
 */
export function make_rpc_request(method: string, params_json: string): string;

/**
 * Parse and validate an RPC response.
 *
 * response_json: raw response JSON string from the server
 * Returns: JS object { ok, result, error }
 */
export function parse_rpc_response(response_json: string): any;

/**
 * Generate a random 32-byte master seed (64 hex chars).
 *
 * Uses getrandom with the `js` feature for browser-compatible randomness.
 */
export function random_master_seed(): string;

/**
 * Derive only the address from a master seed (no full keygen output).
 */
export function wallet_address_from_seed(chain_id: string, master_seed_hex: string, account_index: number): string;

export function wallet_fastpay_transfer_certificate_digest(certificate_json: string): string;

export function wallet_fastpay_transfer_lock_id(order_json: string): string;

export function wallet_fastpay_unwrap_certificate_digest(certificate_json: string): string;

export function wallet_fastpay_unwrap_lock_id(order_json: string): string;

/**
 * Generate a wallet backup and identity from a master seed.
 *
 * Returns a JS object: { address, public_key_hex, backup_json }
 */
export function wallet_keygen(chain_id: string, master_seed_hex: string, account_index: number): any;

/**
 * Sign an asset transaction using a fee quote from the RPC server.
 *
 * backup_json: WalletBackupFile as JSON string
 * quote_json: raw RPC response JSON string containing AssetFeeQuoteSummary
 * Returns: SignedAssetTransaction as JS object
 */
export function wallet_sign_asset_transaction(backup_json: string, quote_json: string): any;

/**
 * Sign an asset transaction from explicit fields (no quote needed).
 *
 * backup_json: WalletBackupFile as JSON string
 * fields_json: WalletSignAssetTransactionFields as JSON string
 * Returns: SignedAssetTransaction as JS object
 */
export function wallet_sign_asset_transaction_fields(backup_json: string, fields_json: string): any;

/**
 * Sign an escrow transaction using a fee quote from the RPC server.
 *
 * backup_json: WalletBackupFile as JSON string
 * quote_json: raw RPC response JSON string containing EscrowFeeQuoteSummary
 * Returns: SignedEscrowTransaction as JS object
 */
export function wallet_sign_escrow_transaction(backup_json: string, quote_json: string): any;

/**
 * Sign an escrow transaction from explicit fields (no quote needed).
 *
 * backup_json: WalletBackupFile as JSON string
 * fields_json: WalletSignEscrowTransactionFields as JSON string
 * Returns: SignedEscrowTransaction as JS object
 */
export function wallet_sign_escrow_transaction_fields(backup_json: string, fields_json: string): any;

/**
 * Sign an offer transaction using a fee quote from the RPC server.
 *
 * backup_json: WalletBackupFile as JSON string
 * quote_json: raw RPC response JSON string containing OfferFeeQuoteSummary
 * Returns: SignedOfferTransaction as JS object
 */
export function wallet_sign_offer_transaction(backup_json: string, quote_json: string): any;

/**
 * Sign an offer transaction from explicit fields (no quote needed).
 *
 * backup_json: WalletBackupFile as JSON string
 * fields_json: WalletSignOfferTransactionFields as JSON string
 * Returns: SignedOfferTransaction as JS object
 */
export function wallet_sign_offer_transaction_fields(backup_json: string, fields_json: string): any;

/**
 * Sign an account-to-FastPay deposit locally and return the consensus primary
 * transaction. The wallet backup never crosses the browser boundary.
 */
export function wallet_sign_owned_deposit(backup_json: string, deposit_json: string): any;

/**
 * Sign a FastPay owned-transfer order with the wallet owner's key.
 *
 * backup_json: WalletBackupFile as JSON string
 * order_json: OwnedTransferOrder as JSON string
 * Returns: JS object { owner_pubkey_hex, owner_signature_hex, order }
 */
export function wallet_sign_owned_transfer(backup_json: string, order_json: string): any;

/**
 * Sign a recovery-safe FastPay v3 transfer against the exact live capability
 * returned by `owned_recovery_capabilities`.
 */
export function wallet_sign_owned_transfer_v3(backup_json: string, order_json: string, capabilities_json: string): any;

/**
 * Sign a FastPay owned-unwrap order with the wallet owner's key.
 *
 * backup_json: WalletBackupFile as JSON string
 * order_json: OwnedUnwrapOrder as JSON string
 * Returns: JS object { owner_pubkey_hex, owner_signature_hex, order }
 */
export function wallet_sign_owned_unwrap(backup_json: string, order_json: string): any;

/**
 * Sign a recovery-safe FastPay v3 unwrap against the exact live capability.
 */
export function wallet_sign_owned_unwrap_v3(backup_json: string, order_json: string, capabilities_json: string): any;

/**
 * Sign a payment v2 (transfer with memos).
 *
 * backup_json: WalletBackupFile as JSON string
 * fields_json: WalletSignPaymentV2Fields as JSON string
 * Returns: SignedPaymentV2 as JS object
 */
export function wallet_sign_payment_v2(backup_json: string, fields_json: string): any;

/**
 * Sign a transfer using a fee quote from the RPC server.
 *
 * backup_json: WalletBackupFile as JSON string
 * quote_json: TransferFeeQuoteSummary as JSON string
 * Returns: SignedTransfer as JS object
 */
export function wallet_sign_transfer(backup_json: string, quote_json: string): any;

/**
 * Sign a transfer from explicit fields (no quote needed).
 *
 * backup_json: WalletBackupFile as JSON string
 * fields_json: WalletSignTransferFields as JSON string
 * Returns: SignedTransfer as JS object
 */
export function wallet_sign_transfer_fields(backup_json: string, fields_json: string): any;

export function wallet_verify_fastpay_apply_ack(acknowledgement_json: string, validator_public_key_hex: string): boolean;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly make_rpc_request: (a: number, b: number, c: number, d: number) => [number, number, number, number];
    readonly parse_rpc_response: (a: number, b: number) => [number, number, number];
    readonly random_master_seed: () => [number, number, number, number];
    readonly wallet_address_from_seed: (a: number, b: number, c: number, d: number, e: number) => [number, number, number, number];
    readonly wallet_fastpay_transfer_certificate_digest: (a: number, b: number) => [number, number, number, number];
    readonly wallet_fastpay_transfer_lock_id: (a: number, b: number) => [number, number, number, number];
    readonly wallet_fastpay_unwrap_certificate_digest: (a: number, b: number) => [number, number, number, number];
    readonly wallet_fastpay_unwrap_lock_id: (a: number, b: number) => [number, number, number, number];
    readonly wallet_keygen: (a: number, b: number, c: number, d: number, e: number) => [number, number, number];
    readonly wallet_sign_asset_transaction: (a: number, b: number, c: number, d: number) => [number, number, number];
    readonly wallet_sign_asset_transaction_fields: (a: number, b: number, c: number, d: number) => [number, number, number];
    readonly wallet_sign_escrow_transaction: (a: number, b: number, c: number, d: number) => [number, number, number];
    readonly wallet_sign_escrow_transaction_fields: (a: number, b: number, c: number, d: number) => [number, number, number];
    readonly wallet_sign_offer_transaction: (a: number, b: number, c: number, d: number) => [number, number, number];
    readonly wallet_sign_offer_transaction_fields: (a: number, b: number, c: number, d: number) => [number, number, number];
    readonly wallet_sign_owned_deposit: (a: number, b: number, c: number, d: number) => [number, number, number];
    readonly wallet_sign_owned_transfer: (a: number, b: number, c: number, d: number) => [number, number, number];
    readonly wallet_sign_owned_transfer_v3: (a: number, b: number, c: number, d: number, e: number, f: number) => [number, number, number];
    readonly wallet_sign_owned_unwrap: (a: number, b: number, c: number, d: number) => [number, number, number];
    readonly wallet_sign_owned_unwrap_v3: (a: number, b: number, c: number, d: number, e: number, f: number) => [number, number, number];
    readonly wallet_sign_payment_v2: (a: number, b: number, c: number, d: number) => [number, number, number];
    readonly wallet_sign_transfer: (a: number, b: number, c: number, d: number) => [number, number, number];
    readonly wallet_sign_transfer_fields: (a: number, b: number, c: number, d: number) => [number, number, number];
    readonly wallet_verify_fastpay_apply_ack: (a: number, b: number, c: number, d: number) => [number, number, number];
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
    readonly __wbindgen_exn_store: (a: number) => void;
    readonly __externref_table_alloc: () => number;
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __externref_table_dealloc: (a: number) => void;
    readonly __wbindgen_free: (a: number, b: number, c: number) => void;
    readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
 * Instantiates the given `module`, which can either be bytes or
 * a precompiled `WebAssembly.Module`.
 *
 * @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
 *
 * @returns {InitOutput}
 */
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
 * If `module_or_path` is {RequestInfo} or {URL}, makes a request and
 * for everything else, calls `WebAssembly.instantiate` directly.
 *
 * @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
 *
 * @returns {Promise<InitOutput>}
 */
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
