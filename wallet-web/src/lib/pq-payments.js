export function checkPublicTerms(payment, terms, hash) {
  if (payment.mode !== terms.mode || payment.asset !== terms.asset || payment.fee !== terms.fee
    || payment.payer !== terms.payer || payment.sequence !== terms.sequence || payment.action_hash !== hash
    || payment.mint !== (terms.mode === 'Shield' ? terms.amount : '0')
    || payment.exit !== (terms.mode === 'Exit' ? terms.amount : '0')
    || payment.recipient !== (terms.mode === 'Exit' ? terms.recipient : null)) {
    throw Error('The signed payment differs from the reviewed terms.');
  }
}
