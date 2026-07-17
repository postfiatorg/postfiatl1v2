// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.24;

import {MarketOpsEnvelope} from "./MarketOpsEnvelope.sol";

interface IMintableEscrowToken {
    function balanceOf(address account) external view returns (uint256);
    function mint(address to, uint256 amount) external;
    function transfer(address to, uint256 amount) external returns (bool);
}

interface IMintBridgeAdapter {
    function accepted_envelope_by_asset_epoch(bytes32 asset_epoch_key) external view returns (bytes32);
    function assetEpochKey(bytes32 asset_id, uint64 epoch) external pure returns (bytes32);
    function getEvmEnvelopeDigest(bytes32 pending_id) external view returns (bytes32);
    function isEnvelopeExecutable(bytes32 pending_id) external view returns (bool);
    function mintCapAtoms(bytes32 pending_id) external view returns (uint256);
}

interface IMintSettlementVerifier {
    function verifiedSettlement(
        bytes32 pending_id,
        bytes32 escrow_id,
        address recipient,
        uint256 amount_atoms,
        bytes32 proof_hash
    ) external view returns (uint256 settled_proceeds_usd_e8, uint256 locked_liquidity_usd_e8);
}

/// @notice Escrows above-NAV mints until settlement or locked-liquidity proof satisfies backing.
contract MintController {
    struct MintEscrow {
        bytes32 pending_id;
        bytes32 asset_id;
        uint64 epoch;
        address beneficiary;
        uint256 amount_atoms;
        bool released;
    }

    struct SettlementProof {
        address recipient;
        uint256 settled_proceeds_usd_e8;
        uint256 locked_liquidity_usd_e8;
        bool proceeds_settled;
        bool liquidity_locked;
        bytes32 proof_hash;
    }

    struct VerifiedSettlement {
        uint256 settled_proceeds_usd_e8;
        uint256 locked_liquidity_usd_e8;
    }

    error NotOwner();
    error ZeroOwner();
    error ZeroAddress(bytes32 field);
    error AdapterAlreadySet(address adapter);
    error AdapterUnset();
    error SettlementVerifierAlreadySet(address verifier);
    error SettlementVerifierUnset();
    error SettlementVerifierCodeHashMismatch(bytes32 expected, bytes32 actual);
    error SettlementVerifierRotationAlreadyPending(address verifier);
    error SettlementVerifierRotationNotPending();
    error SettlementVerifierRotationNotReady(uint64 now_timestamp, uint64 activates_at);
    error UnresolvedMintEscrows(uint256 count);
    error TimestampOverflow();
    error InvalidAmount();
    error MissingAcceptedEnvelope(bytes32 asset_id, uint64 epoch);
    error EnvelopeDigestMismatch(bytes32 expected, bytes32 actual);
    error EnvelopeNotExecutable(bytes32 pending_id);
    error MintCapExceeded(bytes32 pending_id, uint256 attempted_atoms, uint256 cap_atoms);
    error UnknownEscrow(bytes32 escrow_id);
    error EscrowAlreadyReleased(bytes32 escrow_id);
    error EscrowEnvelopeMismatch(bytes32 escrow_id, bytes32 expected_pending_id, bytes32 actual_pending_id);
    error MissingSettlementProof();
    error SettlementProofMismatch();
    error SettlementProofAlreadyUsed(bytes32 proof_hash);
    error RecipientMismatch(address expected, address actual);
    error PostMintBackingViolation(
        uint256 verified_net_assets_after_usd_e8, uint256 valid_global_supply_after_atoms, uint256 nav_floor_usd_e8
    );
    error EscrowTransferFailed();

    event OwnershipTransferred(address indexed previous_owner, address indexed new_owner);
    event BridgeAdapterSet(address indexed adapter);
    event SettlementVerifierSet(address indexed verifier, bytes32 runtime_code_hash);
    event SettlementVerifierRotationScheduled(
        address indexed current_verifier,
        address indexed pending_verifier,
        bytes32 pending_runtime_code_hash,
        uint64 activates_at
    );
    event SettlementVerifierRotationCancelled(address indexed pending_verifier);
    event MintEscrowRequested(
        bytes32 indexed escrow_id,
        bytes32 indexed pending_id,
        bytes32 indexed asset_id,
        uint64 epoch,
        address beneficiary,
        uint256 amount_atoms
    );
    event MintEscrowReleased(
        bytes32 indexed escrow_id,
        bytes32 indexed pending_id,
        bytes32 indexed asset_id,
        uint64 epoch,
        address recipient,
        uint256 amount_atoms,
        uint256 settled_proceeds_usd_e8,
        uint256 locked_liquidity_usd_e8,
        bytes32 proof_hash
    );

    IMintableEscrowToken public immutable asset_token;
    uint256 public immutable unit_scale;
    address public owner;
    IMintBridgeAdapter public bridge_adapter;
    IMintSettlementVerifier public settlement_verifier;
    bytes32 public settlement_verifier_code_hash;
    IMintSettlementVerifier public pending_settlement_verifier;
    bytes32 public pending_settlement_verifier_code_hash;
    uint64 public pending_settlement_verifier_activates_at;
    uint256 public unresolved_mint_escrow_count;

    mapping(bytes32 => MintEscrow) private escrows;
    mapping(bytes32 => uint256) public requested_mint_atoms_by_pending_id;
    mapping(bytes32 => uint256) public released_mint_atoms_by_pending_id;
    mapping(bytes32 => uint256) public settled_value_usd_e8_by_pending_id;
    mapping(bytes32 => uint256) public escrowed_atoms_by_asset;
    mapping(bytes32 => uint256) public released_atoms_by_asset;
    mapping(bytes32 => bool) public used_settlement_proof_hash;

    uint256 private next_escrow_nonce = 1;
    uint256 private reentrancy_lock;

    uint64 public constant SETTLEMENT_VERIFIER_ROTATION_DELAY_SECONDS = 2 days;

    modifier onlyOwner() {
        if (msg.sender != owner) {
            revert NotOwner();
        }
        _;
    }

    modifier nonReentrant() {
        if (reentrancy_lock != 0) {
            revert("reentrant");
        }
        reentrancy_lock = 1;
        _;
        reentrancy_lock = 0;
    }

    constructor(IMintableEscrowToken asset_token_, address initial_owner, uint256 unit_scale_) {
        if (address(asset_token_) == address(0)) {
            revert ZeroAddress("asset_token");
        }
        if (initial_owner == address(0)) {
            revert ZeroOwner();
        }
        if (unit_scale_ == 0) {
            revert InvalidAmount();
        }

        asset_token = asset_token_;
        owner = initial_owner;
        unit_scale = unit_scale_;

        emit OwnershipTransferred(address(0), initial_owner);
    }

    function transferOwnership(address new_owner) external onlyOwner {
        if (new_owner == address(0)) {
            revert ZeroOwner();
        }
        emit OwnershipTransferred(owner, new_owner);
        owner = new_owner;
    }

    function setBridgeAdapter(IMintBridgeAdapter adapter) external onlyOwner {
        if (address(adapter) == address(0)) {
            revert ZeroAddress("bridge_adapter");
        }
        if (address(bridge_adapter) != address(0)) {
            revert AdapterAlreadySet(address(bridge_adapter));
        }
        bridge_adapter = adapter;
        emit BridgeAdapterSet(address(adapter));
    }

    function setSettlementVerifier(IMintSettlementVerifier verifier, bytes32 expected_runtime_code_hash)
        external
        onlyOwner
    {
        if (address(verifier) == address(0)) {
            revert ZeroAddress("settlement_verifier");
        }
        if (address(settlement_verifier) != address(0)) {
            revert SettlementVerifierAlreadySet(address(settlement_verifier));
        }
        bytes32 runtime_code_hash = _requireVerifierCodeHash(verifier, expected_runtime_code_hash);
        settlement_verifier = verifier;
        settlement_verifier_code_hash = runtime_code_hash;
        emit SettlementVerifierSet(address(verifier), runtime_code_hash);
    }

    function scheduleSettlementVerifierRotation(IMintSettlementVerifier verifier, bytes32 expected_runtime_code_hash)
        external
        onlyOwner
    {
        if (address(settlement_verifier) == address(0)) {
            revert SettlementVerifierUnset();
        }
        if (address(verifier) == address(0)) {
            revert ZeroAddress("settlement_verifier");
        }
        if (address(pending_settlement_verifier) != address(0)) {
            revert SettlementVerifierRotationAlreadyPending(address(pending_settlement_verifier));
        }
        if (unresolved_mint_escrow_count != 0) {
            revert UnresolvedMintEscrows(unresolved_mint_escrow_count);
        }
        bytes32 runtime_code_hash = _requireVerifierCodeHash(verifier, expected_runtime_code_hash);
        uint256 activates_at = block.timestamp + SETTLEMENT_VERIFIER_ROTATION_DELAY_SECONDS;
        if (activates_at > type(uint64).max) {
            revert TimestampOverflow();
        }
        pending_settlement_verifier = verifier;
        pending_settlement_verifier_code_hash = runtime_code_hash;
        pending_settlement_verifier_activates_at = uint64(activates_at);
        emit SettlementVerifierRotationScheduled(
            address(settlement_verifier), address(verifier), runtime_code_hash, uint64(activates_at)
        );
    }

    function cancelSettlementVerifierRotation() external onlyOwner {
        IMintSettlementVerifier verifier = pending_settlement_verifier;
        if (address(verifier) == address(0)) {
            revert SettlementVerifierRotationNotPending();
        }
        delete pending_settlement_verifier;
        delete pending_settlement_verifier_code_hash;
        delete pending_settlement_verifier_activates_at;
        emit SettlementVerifierRotationCancelled(address(verifier));
    }

    function activateSettlementVerifierRotation() external {
        IMintSettlementVerifier verifier = pending_settlement_verifier;
        if (address(verifier) == address(0)) {
            revert SettlementVerifierRotationNotPending();
        }
        uint64 activates_at = pending_settlement_verifier_activates_at;
        if (block.timestamp > type(uint64).max) {
            revert TimestampOverflow();
        }
        if (block.timestamp < activates_at) {
            revert SettlementVerifierRotationNotReady(uint64(block.timestamp), activates_at);
        }
        if (unresolved_mint_escrow_count != 0) {
            revert UnresolvedMintEscrows(unresolved_mint_escrow_count);
        }
        bytes32 runtime_code_hash = _requireVerifierCodeHash(verifier, pending_settlement_verifier_code_hash);
        settlement_verifier = verifier;
        settlement_verifier_code_hash = runtime_code_hash;
        delete pending_settlement_verifier;
        delete pending_settlement_verifier_code_hash;
        delete pending_settlement_verifier_activates_at;
        emit SettlementVerifierSet(address(verifier), runtime_code_hash);
    }

    function requestMint(MarketOpsEnvelope calldata envelope, uint256 amount_atoms)
        external
        nonReentrant
        returns (bytes32 escrow_id)
    {
        if (address(bridge_adapter) == address(0)) {
            revert AdapterUnset();
        }
        if (amount_atoms == 0) {
            revert InvalidAmount();
        }

        bytes32 pending_id = _acceptedPendingId(envelope);
        _requireEnvelope(envelope, pending_id);

        uint256 new_requested_atoms = requested_mint_atoms_by_pending_id[pending_id] + amount_atoms;
        uint256 cap_atoms = bridge_adapter.mintCapAtoms(pending_id);
        if (new_requested_atoms > cap_atoms) {
            revert MintCapExceeded(pending_id, new_requested_atoms, cap_atoms);
        }

        requested_mint_atoms_by_pending_id[pending_id] = new_requested_atoms;
        escrowed_atoms_by_asset[envelope.asset_id] += amount_atoms;
        unresolved_mint_escrow_count += 1;
        escrow_id = keccak256(abi.encode(pending_id, msg.sender, amount_atoms, next_escrow_nonce++));
        escrows[escrow_id] = MintEscrow({
            pending_id: pending_id,
            asset_id: envelope.asset_id,
            epoch: envelope.epoch,
            beneficiary: msg.sender,
            amount_atoms: amount_atoms,
            released: false
        });

        asset_token.mint(address(this), amount_atoms);
        emit MintEscrowRequested(escrow_id, pending_id, envelope.asset_id, envelope.epoch, msg.sender, amount_atoms);
    }

    function releaseMint(MarketOpsEnvelope calldata envelope, bytes32 escrow_id, SettlementProof calldata proof)
        external
        nonReentrant
    {
        MintEscrow storage escrow = escrows[escrow_id];
        if (escrow.amount_atoms == 0) {
            revert UnknownEscrow(escrow_id);
        }
        if (escrow.released) {
            revert EscrowAlreadyReleased(escrow_id);
        }

        bytes32 pending_id = _acceptedPendingId(envelope);
        _requireEnvelope(envelope, pending_id);
        if (pending_id != escrow.pending_id || envelope.asset_id != escrow.asset_id || envelope.epoch != escrow.epoch) {
            revert EscrowEnvelopeMismatch(escrow_id, escrow.pending_id, pending_id);
        }
        if (proof.recipient != escrow.beneficiary) {
            revert RecipientMismatch(escrow.beneficiary, proof.recipient);
        }
        if (proof.proof_hash == bytes32(0)) {
            revert MissingSettlementProof();
        }
        if (used_settlement_proof_hash[proof.proof_hash]) {
            revert SettlementProofAlreadyUsed(proof.proof_hash);
        }

        VerifiedSettlement memory verified = _verifiedSettlementValue(pending_id, escrow_id, escrow, proof);
        uint256 proof_value_usd_e8 = verified.settled_proceeds_usd_e8 + verified.locked_liquidity_usd_e8;
        uint256 new_settled_value_usd_e8 = settled_value_usd_e8_by_pending_id[pending_id] + proof_value_usd_e8;
        uint256 new_released_atoms = released_mint_atoms_by_pending_id[pending_id] + escrow.amount_atoms;
        _enforcePostMintBacking(envelope, new_settled_value_usd_e8, new_released_atoms);

        escrow.released = true;
        used_settlement_proof_hash[proof.proof_hash] = true;
        settled_value_usd_e8_by_pending_id[pending_id] = new_settled_value_usd_e8;
        released_mint_atoms_by_pending_id[pending_id] = new_released_atoms;
        escrowed_atoms_by_asset[escrow.asset_id] -= escrow.amount_atoms;
        released_atoms_by_asset[escrow.asset_id] += escrow.amount_atoms;
        unresolved_mint_escrow_count -= 1;

        bool ok = asset_token.transfer(proof.recipient, escrow.amount_atoms);
        if (!ok) {
            revert EscrowTransferFailed();
        }

        _emitMintEscrowReleased(escrow_id, pending_id, escrow, proof.proof_hash, verified);
    }

    function getEscrow(bytes32 escrow_id) external view returns (MintEscrow memory) {
        return escrows[escrow_id];
    }

    function _acceptedPendingId(MarketOpsEnvelope calldata envelope) private view returns (bytes32) {
        bytes32 asset_epoch_key = bridge_adapter.assetEpochKey(envelope.asset_id, envelope.epoch);
        bytes32 pending_id = bridge_adapter.accepted_envelope_by_asset_epoch(asset_epoch_key);
        if (pending_id == bytes32(0)) {
            revert MissingAcceptedEnvelope(envelope.asset_id, envelope.epoch);
        }
        return pending_id;
    }

    function _emitMintEscrowReleased(
        bytes32 escrow_id,
        bytes32 pending_id,
        MintEscrow storage escrow,
        bytes32 proof_hash,
        VerifiedSettlement memory verified
    ) private {
        emit MintEscrowReleased(
            escrow_id,
            pending_id,
            escrow.asset_id,
            escrow.epoch,
            escrow.beneficiary,
            escrow.amount_atoms,
            verified.settled_proceeds_usd_e8,
            verified.locked_liquidity_usd_e8,
            proof_hash
        );
    }

    function _requireEnvelope(MarketOpsEnvelope calldata envelope, bytes32 pending_id) private view {
        bytes32 expected_digest = bridge_adapter.getEvmEnvelopeDigest(pending_id);
        bytes32 actual_digest = keccak256(abi.encode(envelope));
        if (expected_digest != actual_digest) {
            revert EnvelopeDigestMismatch(expected_digest, actual_digest);
        }
        if (!bridge_adapter.isEnvelopeExecutable(pending_id)) {
            revert EnvelopeNotExecutable(pending_id);
        }
    }

    function _verifiedSettlementValue(
        bytes32 pending_id,
        bytes32 escrow_id,
        MintEscrow storage escrow,
        SettlementProof calldata proof
    ) private view returns (VerifiedSettlement memory verified) {
        IMintSettlementVerifier verifier = settlement_verifier;
        if (address(verifier) == address(0)) {
            revert SettlementVerifierUnset();
        }
        _requireVerifierCodeHash(verifier, settlement_verifier_code_hash);
        (verified.settled_proceeds_usd_e8, verified.locked_liquidity_usd_e8) = verifier.verifiedSettlement(
            pending_id, escrow_id, escrow.beneficiary, escrow.amount_atoms, proof.proof_hash
        );
        if (verified.settled_proceeds_usd_e8 == 0 && verified.locked_liquidity_usd_e8 == 0) {
            revert MissingSettlementProof();
        }
        if (
            proof.settled_proceeds_usd_e8 != verified.settled_proceeds_usd_e8
                || proof.locked_liquidity_usd_e8 != verified.locked_liquidity_usd_e8
                || proof.proceeds_settled != (verified.settled_proceeds_usd_e8 != 0)
                || proof.liquidity_locked != (verified.locked_liquidity_usd_e8 != 0)
        ) {
            revert SettlementProofMismatch();
        }
    }

    function _requireVerifierCodeHash(IMintSettlementVerifier verifier, bytes32 expected)
        private
        view
        returns (bytes32 actual)
    {
        actual = address(verifier).codehash;
        if (expected == bytes32(0) || actual != expected) {
            revert SettlementVerifierCodeHashMismatch(expected, actual);
        }
    }

    function _enforcePostMintBacking(
        MarketOpsEnvelope calldata envelope,
        uint256 settled_value_usd_e8,
        uint256 released_atoms
    ) private view {
        uint256 verified_net_assets_after_usd_e8 =
            envelope.verified_net_assets_usd_e8 + settled_value_usd_e8;
        uint256 valid_global_supply_after_atoms = envelope.valid_global_supply_atoms + released_atoms;
        if (verified_net_assets_after_usd_e8 * unit_scale < valid_global_supply_after_atoms * envelope.nav_floor_usd_e8)
        {
            revert PostMintBackingViolation(
                verified_net_assets_after_usd_e8, valid_global_supply_after_atoms, envelope.nav_floor_usd_e8
            );
        }
    }
}
