// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.24;

import {MarketOpsEnvelope} from "../src/MarketOpsEnvelope.sol";
import {
    IMintBridgeAdapter,
    IMintableEscrowToken,
    IMintSettlementVerifier,
    MintController
} from "../src/MintController.sol";
import {ThresholdMintSettlementVerifier} from "../src/ThresholdMintSettlementVerifier.sol";

/// @dev Test-environment token. The controller and settlement verifier below
///      are the production contracts; only the asset and accepted-envelope
///      source are isolated fixtures.
contract P0SupplyDeploymentToken is IMintableEscrowToken {
    mapping(address => uint256) public balanceOf;

    function mint(address to, uint256 amount) external {
        balanceOf[to] += amount;
    }

    function transfer(address to, uint256 amount) external returns (bool) {
        uint256 balance = balanceOf[msg.sender];
        if (balance < amount) {
            return false;
        }
        balanceOf[msg.sender] = balance - amount;
        balanceOf[to] += amount;
        return true;
    }
}

/// @dev Exact immutable accepted-envelope fixture for the isolated deployment.
contract P0SupplyDeploymentAdapter is IMintBridgeAdapter {
    address private immutable owner;
    bytes32 public pending_id;
    bytes32 public envelope_digest;
    uint256 public cap_atoms;

    constructor() {
        owner = msg.sender;
    }

    function configure(bytes32 pending_id_, bytes32 envelope_digest_, uint256 cap_atoms_) external {
        require(msg.sender == owner, "not owner");
        require(pending_id == bytes32(0), "already configured");
        require(pending_id_ != bytes32(0), "zero pending id");
        require(envelope_digest_ != bytes32(0), "zero envelope digest");
        require(cap_atoms_ != 0, "zero cap");
        pending_id = pending_id_;
        envelope_digest = envelope_digest_;
        cap_atoms = cap_atoms_;
    }

    function accepted_envelope_by_asset_epoch(bytes32) external view returns (bytes32) {
        return pending_id;
    }

    function assetEpochKey(bytes32 asset_id, uint64 epoch) external pure returns (bytes32) {
        return keccak256(abi.encode(asset_id, epoch));
    }

    function getEvmEnvelopeDigest(bytes32 candidate) external view returns (bytes32) {
        return candidate == pending_id ? envelope_digest : bytes32(0);
    }

    function isEnvelopeExecutable(bytes32 candidate) external view returns (bool) {
        return candidate == pending_id;
    }

    function mintCapAtoms(bytes32 candidate) external view returns (uint256) {
        return candidate == pending_id ? cap_atoms : 0;
    }
}

/// @notice Isolated Anvil orchestrator for a real production-controller and
///         production-threshold-verifier deployment. It deliberately exposes a
///         narrow ABI so the host test can bind a real accepted PFTL receipt,
///         obtain independent signatures, and audit every value transition.
contract P0SupplyControlledDeployment {
    P0SupplyDeploymentToken public immutable token;
    P0SupplyDeploymentAdapter public immutable adapter;
    MintController public immutable controller;
    ThresholdMintSettlementVerifier public immutable verifier;

    bytes32 public immutable pending_id;
    uint256 public immutable amount_atoms;
    uint64 public immutable pftl_finalized_height;
    bytes32 public immutable accepted_receipt_code;

    MarketOpsEnvelope private envelope;
    bytes private pftl_finalized_state_root;
    bytes private pftl_receipt_hash;
    bytes private route_config_digest;

    constructor(
        bytes32 pftl_chain_id_hash,
        bytes32 pftl_genesis_hash_commitment,
        uint32 pftl_protocol_version,
        uint64 authority_epoch,
        uint256 amount_atoms_,
        uint64 pftl_finalized_height_,
        bytes memory pftl_finalized_state_root_,
        bytes memory pftl_receipt_hash_,
        bytes memory route_config_digest_,
        address[] memory sorted_signers
    ) {
        require(amount_atoms_ != 0, "zero amount");
        require(pftl_finalized_height_ != 0, "zero PFTL height");
        require(pftl_finalized_state_root_.length == 48, "bad PFTL state root");
        require(pftl_receipt_hash_.length == 48, "bad PFTL receipt hash");
        require(route_config_digest_.length == 48, "bad route digest");

        token = new P0SupplyDeploymentToken();
        controller = new MintController(IMintableEscrowToken(address(token)), address(this), 1);

        amount_atoms = amount_atoms_;
        pftl_finalized_height = pftl_finalized_height_;
        pftl_finalized_state_root = pftl_finalized_state_root_;
        pftl_receipt_hash = pftl_receipt_hash_;
        route_config_digest = route_config_digest_;
        accepted_receipt_code = keccak256("accepted");
        pending_id = keccak256(
            abi.encode(
                "postfiat.p0.supply.controlled-deployment.v1",
                pftl_chain_id_hash,
                pftl_genesis_hash_commitment,
                pftl_finalized_height_,
                keccak256(pftl_receipt_hash_),
                amount_atoms_
            )
        );

        envelope.encoding_version = 1;
        envelope.chain_id = 65_100;
        envelope.asset_id = keccak256("postfiat-p0-supply-controlled-asset");
        envelope.epoch = authority_epoch;
        envelope.mint_controller_address = address(controller);
        envelope.nav_floor_usd_e8 = 1;
        envelope.valid_global_supply_atoms = 0;
        envelope.verified_net_assets_usd_e8 = 0;
        envelope.max_mint_atoms = amount_atoms_;

        adapter = new P0SupplyDeploymentAdapter();
        envelope.adapter_address = address(adapter);
        adapter.configure(pending_id, keccak256(abi.encode(envelope)), amount_atoms_);

        verifier = new ThresholdMintSettlementVerifier(
            pftl_chain_id_hash,
            pftl_genesis_hash_commitment,
            pftl_protocol_version,
            authority_epoch,
            address(controller),
            address(token),
            sorted_signers,
            sorted_signers.length - ((sorted_signers.length - 1) / 3)
        );
        controller.setBridgeAdapter(IMintBridgeAdapter(address(adapter)));
        controller.setSettlementVerifier(
            IMintSettlementVerifier(address(verifier)), address(verifier).codehash
        );
    }

    function requestMint() external returns (bytes32) {
        return controller.requestMint(envelope, amount_atoms);
    }

    function certificateDigest(bytes32 escrow_id) external view returns (bytes32) {
        return verifier.certificateDigest(_certificate(escrow_id));
    }

    function settlementId(bytes32 escrow_id) external view returns (bytes32) {
        return verifier.settlementId(_certificate(escrow_id));
    }

    function submitCertificate(bytes32 escrow_id, bytes[] calldata signatures)
        external
        returns (bytes32)
    {
        return verifier.submitSettlementCertificate(_certificate(escrow_id), signatures);
    }

    function releaseMint(bytes32 escrow_id, bytes32 proof_hash) external {
        controller.releaseMint(
            envelope,
            escrow_id,
            MintController.SettlementProof({
                recipient: address(this),
                settled_proceeds_usd_e8: amount_atoms,
                locked_liquidity_usd_e8: 0,
                proceeds_settled: true,
                liquidity_locked: false,
                proof_hash: proof_hash
            })
        );
    }

    function conservationAudit()
        external
        view
        returns (
            uint256 certified_backing_atoms,
            uint256 released_supply_atoms,
            uint256 escrow_atoms,
            uint256 beneficiary_atoms,
            uint256 unresolved_escrows,
            bool conserved
        )
    {
        certified_backing_atoms = controller.settled_value_usd_e8_by_pending_id(pending_id);
        released_supply_atoms = controller.released_mint_atoms_by_pending_id(pending_id);
        escrow_atoms = token.balanceOf(address(controller));
        beneficiary_atoms = token.balanceOf(address(this));
        unresolved_escrows = controller.unresolved_mint_escrow_count();
        conserved = certified_backing_atoms == released_supply_atoms
            && released_supply_atoms == beneficiary_atoms
            && escrow_atoms == 0
            && unresolved_escrows == 0;
    }

    function _certificate(bytes32 escrow_id)
        private
        view
        returns (ThresholdMintSettlementVerifier.SettlementCertificate memory)
    {
        return ThresholdMintSettlementVerifier.SettlementCertificate({
            pending_id: pending_id,
            escrow_id: escrow_id,
            recipient: address(this),
            amount_atoms: amount_atoms,
            settled_proceeds_usd_e8: amount_atoms,
            locked_liquidity_usd_e8: 0,
            pftl_finalized_height: pftl_finalized_height,
            pftl_finalized_state_root: pftl_finalized_state_root,
            pftl_receipt_hash: pftl_receipt_hash,
            route_config_digest: route_config_digest,
            receipt_code: accepted_receipt_code
        });
    }
}
