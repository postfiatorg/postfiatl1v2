import { isValidAddress } from './utils.js';
import { parseAccountResult } from './rpc-client.js';

export function looksLikePublicKeyHex(value) {
  const text = String(value || '').trim();
  return text.length >= 128 && text.length % 2 === 0 && /^[0-9a-fA-F]+$/.test(text);
}

export async function resolveFastpayRecipientPublicKey({
  rpc,
  recipient,
  ownAddress,
  ownPublicKeyHex,
}) {
  const value = String(recipient || '').trim();
  if (!value) {
    throw new Error('Enter a FastPay recipient');
  }

  if (looksLikePublicKeyHex(value)) {
    return value.toLowerCase();
  }

  if (!isValidAddress(value)) {
    throw new Error('FastPay recipient must be a pf address or a public key hex');
  }

  if (ownAddress && value.toLowerCase() === ownAddress.toLowerCase()) {
    if (!ownPublicKeyHex) {
      throw new Error('Your wallet public key is missing');
    }
    return ownPublicKeyHex;
  }

  if (!rpc) {
    throw new Error('Wallet RPC is not connected');
  }

  const resp = await rpc.account(value);
  const account = parseAccountResult(resp);
  if (!account.public_key_hex) {
    // The recipient has never submitted an Account-lane transfer or payment,
    // so the L1 ledger has not recorded their public key (see
    // crates/execution/src/lib_parts/entrypoints.rs). FastPay cannot address
    // them until they publish it. The recipient must publish their own key —
    // no other wallet can do it for them.
    throw new Error(
      'This recipient has not published a public key yet. FastPay needs the ' +
      'recipient’s public key to construct an owned-transfer order. Ask the ' +
      'recipient to open their PostFiat wallet and tap “Publish public key” ' +
      '(Wallet tab) — it costs only the network fee. Alternatively, paste ' +
      'their full public key hex directly instead of their pf address.'
    );
  }
  return account.public_key_hex;
}
