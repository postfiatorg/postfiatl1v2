// Transaction builder - orchestrates quote -> sign -> submit -> poll.

export function assertTransferQuoteMatchesIntent(quote, fromAddress, toAddress, amount) {
  if (
    !quote
    || typeof quote !== 'object'
    || quote.from !== fromAddress
    || quote.to !== toAddress
    || quote.amount !== amount
  ) {
    throw new Error('Transfer fee quote does not match the reviewed sender, recipient, and amount');
  }
}

export class TxBuilder {
  constructor(rpcClient, wasmModule) {
    this.rpc = rpcClient;
    this.wasm = wasmModule;
  }

  async sendTransfer(backupJson, fromAddress, toAddress, amount) {
    // 1. Get fee quote
    const quoteResp = await this.rpc.transferFeeQuote(fromAddress, toAddress, amount);
    if (!quoteResp.ok) throw new Error('Fee quote failed: ' + quoteResp.error?.message);
    const quote = quoteResp.result;

    // 2. Bind the untrusted RPC quote to the reviewed transfer intent.
    assertTransferQuoteMatchesIntent(quote, fromAddress, toAddress, amount);
    if (quote.sender_meets_reserve_after_transfer === false) {
      throw new Error('Insufficient balance after transfer. Balance after: ' + quote.sender_balance_after_amount_and_fee);
    }

    // 3. Sign with WASM
    const signed = this.wasm.wallet_sign_transfer(
      backupJson,
      JSON.stringify(quote),
      JSON.stringify({ from: fromAddress, to: toAddress, amount }),
    );

    // 4. Submit to mempool
    const signedJson = JSON.stringify(signed);
    const submitResp = await this.rpc.submitSignedTransfer(signedJson);
    if (!submitResp.ok) throw new Error('Submit failed: ' + submitResp.error?.message);
    const txId = submitResp.result.tx_id;

    // 5. Poll for receipt
    const receipt = await this.pollReceipt(txId, 30000);

    return { txId, receipt, quote, signed };
  }

  async pollReceipt(txId, timeoutMs) {
    const deadline = Date.now() + timeoutMs;
    while (Date.now() < deadline) {
      await new Promise(r => setTimeout(r, 2000));
      const resp = await this.rpc.receipts(txId);
      if (resp.ok && resp.result && resp.result.length > 0) {
        return resp.result[0];
      }
    }
    return { accepted: null, message: 'timeout' };
  }
}
