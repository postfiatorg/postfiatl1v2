function sameAddress(a, b) {
  return String(a || '').trim().toLowerCase() === String(b || '').trim().toLowerCase();
}

function legOperationKind(leg) {
  return leg?.transaction_kind
    || leg?.operation?.transaction_kind
    || leg?.operation?.operation
    || leg?.operation?.kind
    || null;
}

function legSequence(leg) {
  const value = leg?.sequence ?? leg?.operation?.sequence ?? null;
  if (value === null || value === undefined || value === '') return null;
  const sequence = Number(value);
  if (!Number.isSafeInteger(sequence) || sequence <= 0) {
    throw new Error('atomic escrow leg sequence is invalid');
  }
  return sequence;
}

export function findAtomicWalletCreateLeg(template, walletAddress) {
  if (!template || typeof template !== 'object' || !walletAddress) return null;
  for (const side of ['left', 'right']) {
    const leg = template[side];
    if (!leg || typeof leg !== 'object') continue;
    const operation = leg.operation;
    if (!operation || typeof operation !== 'object') continue;
    const owner = operation.owner || leg.owner;
    if (!sameAddress(owner, walletAddress)) continue;
    if (leg.owner && operation.owner && !sameAddress(leg.owner, operation.owner)) {
      throw new Error(`${side} escrow leg owner does not match its operation`);
    }
    if (legOperationKind(leg) !== 'escrow_create') {
      throw new Error(`${side} wallet-owned atomic leg is not an escrow_create`);
    }
    return {
      side,
      leg,
      operation,
      sequence: legSequence(leg),
      escrowId: leg.escrow_id || null,
    };
  }
  return null;
}

export function findAtomicWalletFinishLeg(template, walletAddress, fulfillment) {
  if (!template || typeof template !== 'object' || !walletAddress) return null;
  for (const side of ['left', 'right']) {
    const leg = template[side];
    if (!leg || typeof leg !== 'object') continue;
    if (!sameAddress(leg.recipient, walletAddress)) continue;
    if (sameAddress(leg.owner, walletAddress)) continue;
    if (!leg.escrow_id) throw new Error(`${side} incoming escrow leg is missing escrow_id`);
    if (legOperationKind(leg) !== 'escrow_create') {
      throw new Error(`${side} incoming atomic leg is not an escrow_create`);
    }
    return {
      side,
      leg,
      escrowId: leg.escrow_id,
      operation: {
        operation: 'escrow_finish',
        escrow_id: leg.escrow_id,
        owner: leg.owner,
        recipient: walletAddress,
        fulfillment,
      },
    };
  }
  return null;
}

export function findAtomicWalletCancelLeg(template, walletAddress) {
  const createLeg = findAtomicWalletCreateLeg(template, walletAddress);
  if (!createLeg) return null;
  if (!createLeg.escrowId) {
    throw new Error(`${createLeg.side} wallet-owned escrow leg is missing escrow_id`);
  }
  return {
    side: createLeg.side,
    leg: createLeg.leg,
    escrowId: createLeg.escrowId,
    operation: {
      operation: 'escrow_cancel',
      escrow_id: createLeg.escrowId,
      owner: walletAddress,
    },
  };
}
