// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.24;

import {MarketOpsEnvelope} from "../src/MarketOpsEnvelope.sol";
import {
    IMintBridgeAdapter,
    IMintableEscrowToken,
    IMintSettlementVerifier,
    MintController
} from "../src/MintController.sol";
import {PFTLBridgeAdapter} from "../src/PFTLBridgeAdapter.sol";
import {PolicyRegistry} from "../src/PolicyRegistry.sol";

interface MintVm {
    function warp(uint256 timestamp) external;
}

contract MintControllerTest {
    MintVm private constant vm = MintVm(address(uint160(uint256(keccak256("hevm cheat code")))));

    MintMockToken private asset;
    PolicyRegistry private registry;
    PFTLBridgeAdapter private adapter;
    MintController private controller;
    MintMockSettlementVerifier private settlement_verifier;

    uint64 private constant CHAIN_ID = 65_100;
    uint64 private constant CHALLENGE_DELAY = 100;
    uint64 private constant EXECUTION_WINDOW = 1_000;
    uint64 private constant MAX_STALENESS = 75;

    address private constant VAULT = address(0x1212);

    bytes32 private constant ASSET_ID = bytes32(uint256(0xaaaaaaaa));
    bytes32 private constant PROGRAM_ID = bytes32(uint256(0x31));
    bytes32 private constant POLICY_HASH = bytes32(uint256(0x32));
    bytes32 private constant PARAMETER_HASH = bytes32(uint256(0x33));
    bytes32 private constant VENUE_ID = bytes32(uint256(0x37));
    bytes32 private constant POOL_CONFIG_HASH = bytes32(uint256(0x38));
    bytes32 private constant HOOK_CODE_HASH = bytes32(uint256(0x39));

    bytes private constant ENVELOPE_HASH =
        hex"0102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f202122232425262728292a2b2c2d2e2f30";

    function setUp() public {
        asset = new MintMockToken();
        registry = new PolicyRegistry(address(this));
        registry.registerPolicy(
            PROGRAM_ID, POLICY_HASH, PARAMETER_HASH, VENUE_ID, POOL_CONFIG_HASH, HOOK_CODE_HASH, 1, 0
        );
        controller = new MintController(IMintableEscrowToken(address(asset)), address(this), 1);
        settlement_verifier = new MintMockSettlementVerifier();
        adapter = new PFTLBridgeAdapter(
            registry,
            address(this),
            CHAIN_ID,
            VAULT,
            address(controller),
            CHALLENGE_DELAY,
            EXECUTION_WINDOW,
            MAX_STALENESS
        );
        controller.setBridgeAdapter(IMintBridgeAdapter(address(adapter)));
        controller.setSettlementVerifier(
            IMintSettlementVerifier(address(settlement_verifier)), address(settlement_verifier).codehash
        );
    }

    function testRequestMintMintsIntoEscrow() public {
        (MarketOpsEnvelope memory envelope, bytes32 pending_id) = _acceptedEnvelope(1, 8_300);

        bytes32 escrow_id = controller.requestMint(envelope, 100);
        MintController.MintEscrow memory escrow = controller.getEscrow(escrow_id);

        _assertEq(escrow.pending_id, pending_id, "pending id");
        _assertEq(escrow.amount_atoms, 100, "escrow amount");
        _assertEq(asset.balanceOf(address(controller)), 100, "controller balance");
        _assertEq(asset.balanceOf(address(this)), 0, "beneficiary balance");
        _assertEq(controller.requested_mint_atoms_by_pending_id(pending_id), 100, "requested");
        _assertEq(controller.escrowed_atoms_by_asset(ASSET_ID), 100, "escrowed");
        _assertEq(controller.released_mint_atoms_by_pending_id(pending_id), 0, "released");
    }

    function testRequestMintRespectsCap() public {
        (MarketOpsEnvelope memory envelope,) = _acceptedEnvelope(1, 50);

        _expectRequestRevert(envelope, 51);

        _assertEq(asset.balanceOf(address(controller)), 0, "controller balance");
        _assertEq(controller.escrowed_atoms_by_asset(ASSET_ID), 0, "escrowed");
    }

    function testReleaseMintOnSettlement() public {
        (MarketOpsEnvelope memory envelope, bytes32 pending_id) = _acceptedEnvelope(1, 8_300);
        bytes32 escrow_id = controller.requestMint(envelope, 100);
        _attest(pending_id, escrow_id, bytes32(uint256(0x5555)), 500e8, 0);

        controller.releaseMint(envelope, escrow_id, _settledProof(500e8));

        MintController.MintEscrow memory escrow = controller.getEscrow(escrow_id);
        _assertTrue(escrow.released, "released");
        _assertEq(asset.balanceOf(address(controller)), 0, "controller balance");
        _assertEq(asset.balanceOf(address(this)), 100, "beneficiary balance");
        _assertEq(controller.escrowed_atoms_by_asset(ASSET_ID), 0, "escrowed");
        _assertEq(controller.released_atoms_by_asset(ASSET_ID), 100, "released asset");
        _assertEq(controller.released_mint_atoms_by_pending_id(pending_id), 100, "released pending");
        _assertEq(controller.settled_value_usd_e8_by_pending_id(pending_id), 500e8, "settled value");
    }

    function testFabricatedBeneficiarySettlementIsRejectedWithoutVerifierRecord() public {
        (MarketOpsEnvelope memory envelope, bytes32 pending_id) = _acceptedEnvelope(1, 8_300);
        bytes32 escrow_id = controller.requestMint(envelope, 100);

        MintController.SettlementProof memory fabricated = MintController.SettlementProof({
            recipient: address(this),
            settled_proceeds_usd_e8: type(uint128).max,
            locked_liquidity_usd_e8: 0,
            proceeds_settled: true,
            liquidity_locked: false,
            proof_hash: keccak256("beneficiary-authored-no-settlement-evidence")
        });
        _expectReleaseRevert(envelope, escrow_id, fabricated);

        _assertEq(asset.balanceOf(address(this)), 0, "fabricated proof did not release mint");
        _assertEq(controller.released_mint_atoms_by_pending_id(pending_id), 0, "fabricated release not recorded");
    }

    function testCallerCannotAlterVerifierAuthorizedSettlementValue() public {
        (MarketOpsEnvelope memory envelope, bytes32 pending_id) = _acceptedEnvelope(1, 8_300);
        bytes32 escrow_id = controller.requestMint(envelope, 100);
        _attest(pending_id, escrow_id, bytes32(uint256(0x5555)), 500e8, 0);

        _expectReleaseRevert(envelope, escrow_id, _settledProof(501e8));

        _assertEq(asset.balanceOf(address(this)), 0, "mismatched value did not release mint");
        _assertEq(controller.released_mint_atoms_by_pending_id(pending_id), 0, "mismatched release not recorded");
    }

    function testSettlementRecordIsBoundToEscrowAndConsumedOnce() public {
        (MarketOpsEnvelope memory envelope, bytes32 pending_id) = _acceptedEnvelope(1, 8_300);
        bytes32 first_escrow_id = controller.requestMint(envelope, 100);
        bytes32 second_escrow_id = controller.requestMint(envelope, 100);
        _attest(pending_id, first_escrow_id, bytes32(uint256(0x5555)), 500e8, 0);

        _expectReleaseRevert(envelope, second_escrow_id, _settledProof(500e8));
        controller.releaseMint(envelope, first_escrow_id, _settledProof(500e8));
        _expectReleaseRevert(envelope, second_escrow_id, _settledProof(500e8));

        _assertEq(asset.balanceOf(address(this)), 100, "only authorized escrow released");
        _assertEq(controller.released_mint_atoms_by_pending_id(pending_id), 100, "proof consumed once");
    }

    function testSettlementVerifierCannotBeReplacedWithoutTimelockAndDrain() public {
        MintMockSettlementVerifier replacement = new MintMockSettlementVerifier();
        try controller.setSettlementVerifier(
            IMintSettlementVerifier(address(replacement)), address(replacement).codehash
        ) {
            revert("expected verifier replacement revert");
        } catch {}
    }

    function testVerifierRotationRequiresExactCodeHashTimelockAndNoUnresolvedEscrow() public {
        MintMockSettlementVerifier replacement = new MintMockSettlementVerifier();

        try controller.scheduleSettlementVerifierRotation(
            IMintSettlementVerifier(address(replacement)), bytes32(uint256(1))
        ) {
            revert("expected bad code hash rejection");
        } catch {}

        (MarketOpsEnvelope memory envelope, bytes32 pending_id) = _acceptedEnvelope(1, 8_300);
        bytes32 escrow_id = controller.requestMint(envelope, 100);
        try controller.scheduleSettlementVerifierRotation(
            IMintSettlementVerifier(address(replacement)), address(replacement).codehash
        ) {
            revert("expected unresolved escrow rejection");
        } catch {}

        _attest(pending_id, escrow_id, bytes32(uint256(0x5555)), 500e8, 0);
        controller.releaseMint(envelope, escrow_id, _settledProof(500e8));
        controller.scheduleSettlementVerifierRotation(
            IMintSettlementVerifier(address(replacement)), address(replacement).codehash
        );
        try controller.activateSettlementVerifierRotation() {
            revert("expected timelock rejection");
        } catch {}

        vm.warp(block.timestamp + controller.SETTLEMENT_VERIFIER_ROTATION_DELAY_SECONDS());
        controller.activateSettlementVerifierRotation();
        _assertEq(address(controller.settlement_verifier()), address(replacement), "rotated verifier");
        _assertEq(controller.settlement_verifier_code_hash(), address(replacement).codehash, "rotated code hash");
    }

    function testReleaseMintRevertsWithoutSettlement() public {
        (MarketOpsEnvelope memory envelope,) = _acceptedEnvelope(1, 8_300);
        bytes32 escrow_id = controller.requestMint(envelope, 100);

        _expectReleaseRevert(
            envelope,
            escrow_id,
            MintController.SettlementProof({
                recipient: address(this),
                settled_proceeds_usd_e8: 0,
                locked_liquidity_usd_e8: 0,
                proceeds_settled: false,
                liquidity_locked: false,
                proof_hash: bytes32(uint256(0x5555))
            })
        );

        MintController.MintEscrow memory escrow = controller.getEscrow(escrow_id);
        _assertTrue(!escrow.released, "still escrowed");
        _assertEq(asset.balanceOf(address(controller)), 100, "controller balance");
        _assertEq(asset.balanceOf(address(this)), 0, "beneficiary balance");
    }

    function testPostMintBackingEnforced() public {
        (MarketOpsEnvelope memory envelope, bytes32 pending_id) = _acceptedEnvelope(1, 8_300);
        bytes32 escrow_id = controller.requestMint(envelope, 100);

        _attest(pending_id, escrow_id, bytes32(uint256(0x5555)), 499e8, 0);
        _expectReleaseRevert(envelope, escrow_id, _settledProof(499e8));

        MintController.MintEscrow memory escrow = controller.getEscrow(escrow_id);
        _assertTrue(!escrow.released, "still escrowed after failed backing");
        _attest(pending_id, escrow_id, bytes32(uint256(0x5555)), 500e8, 0);
        controller.releaseMint(envelope, escrow_id, _settledProof(500e8));
        _assertEq(asset.balanceOf(address(this)), 100, "released after sufficient backing");
    }

    function testReleaseMintOnLockedLiquidity() public {
        (MarketOpsEnvelope memory envelope, bytes32 pending_id) = _acceptedEnvelope(1, 8_300);
        bytes32 escrow_id = controller.requestMint(envelope, 100);
        _attest(pending_id, escrow_id, bytes32(uint256(0x5555)), 0, 500e8);

        controller.releaseMint(
            envelope,
            escrow_id,
            MintController.SettlementProof({
                recipient: address(this),
                settled_proceeds_usd_e8: 0,
                locked_liquidity_usd_e8: 500e8,
                proceeds_settled: false,
                liquidity_locked: true,
                proof_hash: bytes32(uint256(0x5555))
            })
        );

        _assertEq(asset.balanceOf(address(this)), 100, "released against locked liquidity");
    }

    function _acceptedEnvelope(uint64 epoch, uint256 max_mint_atoms)
        private
        returns (MarketOpsEnvelope memory envelope, bytes32 pending_id)
    {
        vm.warp(1_000);
        envelope = _envelope(epoch, 1_100, 1_600, 950, max_mint_atoms);
        pending_id = adapter.submitEnvelope(envelope, ENVELOPE_HASH);
        vm.warp(1_100);
        adapter.finalizeEnvelope(pending_id);
    }

    function _envelope(
        uint64 epoch,
        uint64 valid_after,
        uint64 expires_at,
        uint64 data_window_end,
        uint256 max_mint_atoms
    ) private view returns (MarketOpsEnvelope memory envelope) {
        envelope.encoding_version = 1;
        envelope.chain_id = CHAIN_ID;
        envelope.adapter_address = address(adapter);
        envelope.vault_address = VAULT;
        envelope.mint_controller_address = address(controller);
        envelope.asset_id = ASSET_ID;
        envelope.epoch = epoch;
        envelope.program_id = PROGRAM_ID;
        envelope.policy_hash = POLICY_HASH;
        envelope.parameter_hash = PARAMETER_HASH;
        envelope.reserve_packet_hash = bytes32(uint256(0xabab));
        envelope.supply_packet_hash = bytes32(uint256(0xcdcd));
        envelope.evidence_root = bytes32(uint256(0xefef));
        envelope.previous_market_state_hash = bytes32(0);
        envelope.venue_id = VENUE_ID;
        envelope.pool_config_hash = POOL_CONFIG_HASH;
        envelope.hook_code_hash = HOOK_CODE_HASH;
        envelope.nav_floor_usd_e8 = 5e8;
        envelope.valid_global_supply_atoms = 1_000_000;
        envelope.verified_net_assets_usd_e8 = 5_000_000e8;
        envelope.funded_alignment_reserve_usd_e8 = 150_000e8;
        envelope.required_alignment_reserve_usd_e8 = 135_000e8;
        envelope.max_reserve_deploy_usd_e8 = 0;
        envelope.max_mint_atoms = max_mint_atoms;
        envelope.discount_trigger_bps = 300;
        envelope.premium_trigger_bps = 1_000;
        envelope.data_window_start = data_window_end - 100;
        envelope.data_window_end = data_window_end;
        envelope.valid_after = valid_after;
        envelope.expires_at = expires_at;
        envelope.cooldown_seconds = 600;
        envelope.nonce = bytes32(uint256(0x55));
    }

    function _settledProof(uint256 settled_proceeds_usd_e8)
        private
        view
        returns (MintController.SettlementProof memory)
    {
        return MintController.SettlementProof({
            recipient: address(this),
            settled_proceeds_usd_e8: settled_proceeds_usd_e8,
            locked_liquidity_usd_e8: 0,
            proceeds_settled: true,
            liquidity_locked: false,
            proof_hash: bytes32(uint256(0x5555))
        });
    }

    function _attest(
        bytes32 pending_id,
        bytes32 escrow_id,
        bytes32 proof_hash,
        uint256 settled_proceeds_usd_e8,
        uint256 locked_liquidity_usd_e8
    ) private {
        settlement_verifier.attest(
            pending_id, escrow_id, address(this), 100, proof_hash, settled_proceeds_usd_e8, locked_liquidity_usd_e8
        );
    }

    function _expectReleaseRevert(
        MarketOpsEnvelope memory envelope,
        bytes32 escrow_id,
        MintController.SettlementProof memory proof
    ) private {
        try controller.releaseMint(envelope, escrow_id, proof) {
            revert("expected releaseMint revert");
        } catch {}
    }

    function _expectRequestRevert(MarketOpsEnvelope memory envelope, uint256 amount_atoms) private {
        try controller.requestMint(envelope, amount_atoms) returns (bytes32) {
            revert("expected requestMint revert");
        } catch {}
    }

    function _assertTrue(bool value, string memory message) private pure {
        if (!value) {
            revert(message);
        }
    }

    function _assertEq(bytes32 actual, bytes32 expected, string memory message) private pure {
        if (actual != expected) {
            revert(message);
        }
    }

    function _assertEq(address actual, address expected, string memory message) private pure {
        if (actual != expected) {
            revert(message);
        }
    }

    function _assertEq(uint256 actual, uint256 expected, string memory message) private pure {
        if (actual != expected) {
            revert(message);
        }
    }
}

contract MintMockToken {
    mapping(address => uint256) public balanceOf;
    uint256 public totalSupply;

    function transfer(address to, uint256 amount) external returns (bool) {
        uint256 balance = balanceOf[msg.sender];
        if (balance < amount) {
            return false;
        }
        balanceOf[msg.sender] = balance - amount;
        balanceOf[to] += amount;
        return true;
    }

    function mint(address to, uint256 amount) external {
        balanceOf[to] += amount;
        totalSupply += amount;
    }
}

contract MintMockSettlementVerifier is IMintSettlementVerifier {
    struct Record {
        bytes32 pending_id;
        bytes32 escrow_id;
        address recipient;
        uint256 amount_atoms;
        uint256 settled_proceeds_usd_e8;
        uint256 locked_liquidity_usd_e8;
    }

    mapping(bytes32 => Record) private records;

    function attest(
        bytes32 pending_id,
        bytes32 escrow_id,
        address recipient,
        uint256 amount_atoms,
        bytes32 proof_hash,
        uint256 settled_proceeds_usd_e8,
        uint256 locked_liquidity_usd_e8
    ) external {
        records[proof_hash] = Record({
            pending_id: pending_id,
            escrow_id: escrow_id,
            recipient: recipient,
            amount_atoms: amount_atoms,
            settled_proceeds_usd_e8: settled_proceeds_usd_e8,
            locked_liquidity_usd_e8: locked_liquidity_usd_e8
        });
    }

    function verifiedSettlement(
        bytes32 pending_id,
        bytes32 escrow_id,
        address recipient,
        uint256 amount_atoms,
        bytes32 proof_hash
    ) external view returns (uint256 settled_proceeds_usd_e8, uint256 locked_liquidity_usd_e8) {
        Record storage record = records[proof_hash];
        if (
            record.pending_id != pending_id || record.escrow_id != escrow_id || record.recipient != recipient
                || record.amount_atoms != amount_atoms
        ) {
            return (0, 0);
        }
        return (record.settled_proceeds_usd_e8, record.locked_liquidity_usd_e8);
    }
}
