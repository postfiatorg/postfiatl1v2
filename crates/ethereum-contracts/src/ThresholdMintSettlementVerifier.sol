// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.24;

import {IMintSettlementVerifier} from "./MintController.sol";

/// @notice BFT certificate verifier for settlement-backed mint release.
/// @dev One deployment is immutable for one PFTL authority epoch and one mint
///      controller/token pair. Any relayer may submit a certificate, but only an
///      exact sorted BFT quorum of governed committee signatures can create a
///      settlement record.
contract ThresholdMintSettlementVerifier is IMintSettlementVerifier {
    struct SettlementCertificate {
        bytes32 pending_id;
        bytes32 escrow_id;
        address recipient;
        uint256 amount_atoms;
        uint256 settled_proceeds_usd_e8;
        uint256 locked_liquidity_usd_e8;
        uint64 pftl_finalized_height;
        bytes pftl_finalized_state_root;
        bytes pftl_receipt_hash;
        bytes route_config_digest;
        bytes32 receipt_code;
    }

    struct SettlementRecord {
        bytes32 pending_id;
        bytes32 escrow_id;
        address recipient;
        uint256 amount_atoms;
        uint256 settled_proceeds_usd_e8;
        uint256 locked_liquidity_usd_e8;
    }

    error InvalidCommitteeSize(uint256 count);
    error InvalidThreshold(uint256 actual, uint256 required);
    error ZeroAddress(bytes32 field);
    error ZeroDigest(bytes32 field);
    error ZeroAmount(bytes32 field);
    error InvalidPftlBytes(bytes32 field, uint256 actual_length, uint256 expected_length);
    error InvalidSignatureCount(uint256 actual, uint256 required);
    error DuplicateOrUnsortedSigner(address signer);
    error UnauthorizedSigner(address signer);
    error BadSignatureLength(uint256 length);
    error BadSignature();
    error ReceiptCodeNotAccepted(bytes32 receipt_code);
    error SettlementAlreadyCertified(bytes32 proof_hash);

    event SettlementCertified(
        bytes32 indexed proof_hash,
        bytes32 indexed pending_id,
        bytes32 indexed escrow_id,
        uint64 pftl_finalized_height,
        bytes32 certificate_digest
    );

    uint256 private constant SECP256K1N_HALF = 0x7fffffffffffffffffffffffffffffff5d576e7357a4501ddfe92f46681b20a0;
    uint256 private constant MAX_COMMITTEE_SIZE = 64;

    bytes32 public constant ACCEPTED_RECEIPT_CODE = keccak256("accepted");

    bytes32 public immutable pftl_chain_id_hash;
    bytes32 public immutable pftl_genesis_hash_commitment;
    uint32 public immutable pftl_protocol_version;
    uint64 public immutable authority_epoch;
    bytes32 public immutable committee_root;
    address public immutable mint_controller;
    address public immutable asset_token;
    uint256 public immutable signer_count;
    uint256 public immutable threshold;

    mapping(address => bool) public is_signer;
    mapping(bytes32 => SettlementRecord) private settlements;

    constructor(
        bytes32 pftl_chain_id_hash_,
        bytes32 pftl_genesis_hash_commitment_,
        uint32 pftl_protocol_version_,
        uint64 authority_epoch_,
        address mint_controller_,
        address asset_token_,
        address[] memory sorted_signers,
        uint256 threshold_
    ) {
        if (pftl_chain_id_hash_ == bytes32(0)) {
            revert ZeroDigest("pftl_chain_id_hash");
        }
        if (pftl_genesis_hash_commitment_ == bytes32(0)) {
            revert ZeroDigest("pftl_genesis_hash_commitment");
        }
        if (pftl_protocol_version_ == 0) {
            revert ZeroAmount("pftl_protocol_version");
        }
        if (authority_epoch_ == 0) {
            revert ZeroAmount("authority_epoch");
        }
        if (mint_controller_ == address(0)) {
            revert ZeroAddress("mint_controller");
        }
        if (asset_token_ == address(0)) {
            revert ZeroAddress("asset_token");
        }
        uint256 count = sorted_signers.length;
        if (count == 0 || count > MAX_COMMITTEE_SIZE) {
            revert InvalidCommitteeSize(count);
        }
        uint256 required = count - ((count - 1) / 3);
        if (threshold_ != required) {
            revert InvalidThreshold(threshold_, required);
        }
        address previous = address(0);
        for (uint256 i = 0; i < count; i++) {
            address signer = sorted_signers[i];
            if (signer == address(0)) {
                revert ZeroAddress("signer");
            }
            if (signer <= previous) {
                revert DuplicateOrUnsortedSigner(signer);
            }
            is_signer[signer] = true;
            previous = signer;
        }

        pftl_chain_id_hash = pftl_chain_id_hash_;
        pftl_genesis_hash_commitment = pftl_genesis_hash_commitment_;
        pftl_protocol_version = pftl_protocol_version_;
        authority_epoch = authority_epoch_;
        mint_controller = mint_controller_;
        asset_token = asset_token_;
        signer_count = count;
        threshold = threshold_;
        committee_root = keccak256(
            abi.encode(
                "postfiat.mint_settlement.committee.v1",
                pftl_chain_id_hash_,
                pftl_genesis_hash_commitment_,
                pftl_protocol_version_,
                authority_epoch_,
                mint_controller_,
                asset_token_,
                sorted_signers,
                threshold_
            )
        );
    }

    function submitSettlementCertificate(SettlementCertificate calldata certificate, bytes[] calldata signatures)
        external
        returns (bytes32 proof_hash)
    {
        _validateCertificate(certificate);
        proof_hash = settlementId(certificate);
        if (settlements[proof_hash].amount_atoms != 0) {
            revert SettlementAlreadyCertified(proof_hash);
        }
        bytes32 digest = certificateDigest(certificate);
        _validateSignatures(digest, signatures);
        settlements[proof_hash] = SettlementRecord({
            pending_id: certificate.pending_id,
            escrow_id: certificate.escrow_id,
            recipient: certificate.recipient,
            amount_atoms: certificate.amount_atoms,
            settled_proceeds_usd_e8: certificate.settled_proceeds_usd_e8,
            locked_liquidity_usd_e8: certificate.locked_liquidity_usd_e8
        });
        emit SettlementCertified(
            proof_hash, certificate.pending_id, certificate.escrow_id, certificate.pftl_finalized_height, digest
        );
    }

    function verifiedSettlement(
        bytes32 pending_id,
        bytes32 escrow_id,
        address recipient,
        uint256 amount_atoms,
        bytes32 proof_hash
    ) external view returns (uint256 settled_proceeds_usd_e8, uint256 locked_liquidity_usd_e8) {
        SettlementRecord storage record = settlements[proof_hash];
        if (
            record.pending_id != pending_id || record.escrow_id != escrow_id || record.recipient != recipient
                || record.amount_atoms != amount_atoms
        ) {
            return (0, 0);
        }
        return (record.settled_proceeds_usd_e8, record.locked_liquidity_usd_e8);
    }

    function settlementId(SettlementCertificate calldata certificate) public view returns (bytes32) {
        _validateCertificate(certificate);
        bytes32 value_commitment = keccak256(
            abi.encode(
                "postfiat.mint_settlement.value.v1",
                certificate.pending_id,
                certificate.escrow_id,
                certificate.recipient,
                certificate.amount_atoms,
                certificate.settled_proceeds_usd_e8,
                certificate.locked_liquidity_usd_e8
            )
        );
        bytes32 finality_commitment = keccak256(
            abi.encode(
                "postfiat.mint_settlement.finality.v1",
                certificate.pftl_finalized_height,
                keccak256(certificate.pftl_finalized_state_root),
                keccak256(certificate.pftl_receipt_hash),
                keccak256(certificate.route_config_digest),
                certificate.receipt_code
            )
        );
        return keccak256(
            abi.encode(
                "postfiat.mint_settlement.id.v1",
                block.chainid,
                address(this),
                mint_controller,
                asset_token,
                value_commitment,
                finality_commitment
            )
        );
    }

    function certificateDigest(SettlementCertificate calldata certificate) public view returns (bytes32) {
        bytes32 proof_hash = settlementId(certificate);
        return keccak256(
            abi.encode(
                "postfiat.mint_settlement.certificate.v1",
                block.chainid,
                address(this),
                pftl_chain_id_hash,
                pftl_genesis_hash_commitment,
                pftl_protocol_version,
                authority_epoch,
                committee_root,
                proof_hash
            )
        );
    }

    function _validateCertificate(SettlementCertificate calldata certificate) private pure {
        if (certificate.pending_id == bytes32(0)) {
            revert ZeroDigest("pending_id");
        }
        if (certificate.escrow_id == bytes32(0)) {
            revert ZeroDigest("escrow_id");
        }
        if (certificate.recipient == address(0)) {
            revert ZeroAddress("recipient");
        }
        if (certificate.amount_atoms == 0) {
            revert ZeroAmount("amount_atoms");
        }
        if (certificate.settled_proceeds_usd_e8 == 0 && certificate.locked_liquidity_usd_e8 == 0) {
            revert ZeroAmount("settlement_value");
        }
        if (certificate.pftl_finalized_height == 0) {
            revert ZeroAmount("pftl_finalized_height");
        }
        _requirePftlBytes(certificate.pftl_finalized_state_root, "pftl_finalized_state_root");
        _requirePftlBytes(certificate.pftl_receipt_hash, "pftl_receipt_hash");
        _requirePftlBytes(certificate.route_config_digest, "route_config_digest");
        if (certificate.receipt_code != ACCEPTED_RECEIPT_CODE) {
            revert ReceiptCodeNotAccepted(certificate.receipt_code);
        }
    }

    function _validateSignatures(bytes32 digest, bytes[] calldata signatures) private view {
        if (signatures.length != threshold) {
            revert InvalidSignatureCount(signatures.length, threshold);
        }
        address previous = address(0);
        for (uint256 i = 0; i < signatures.length; i++) {
            address signer = _recover(digest, signatures[i]);
            if (!is_signer[signer]) {
                revert UnauthorizedSigner(signer);
            }
            if (signer <= previous) {
                revert DuplicateOrUnsortedSigner(signer);
            }
            previous = signer;
        }
    }

    function _recover(bytes32 digest, bytes calldata signature) private pure returns (address signer) {
        if (signature.length != 65) {
            revert BadSignatureLength(signature.length);
        }
        bytes32 r;
        bytes32 s;
        uint8 v;
        assembly {
            r := calldataload(signature.offset)
            s := calldataload(add(signature.offset, 32))
            v := byte(0, calldataload(add(signature.offset, 64)))
        }
        if (v != 27 && v != 28) {
            revert BadSignature();
        }
        if (uint256(s) > SECP256K1N_HALF) {
            revert BadSignature();
        }
        signer = ecrecover(digest, v, r, s);
        if (signer == address(0)) {
            revert BadSignature();
        }
    }

    function _requirePftlBytes(bytes calldata value, bytes32 field) private pure {
        if (value.length != 48) {
            revert InvalidPftlBytes(field, value.length, 48);
        }
        for (uint256 i = 0; i < value.length; i++) {
            if (value[i] != 0) {
                return;
            }
        }
        revert ZeroDigest(field);
    }
}
