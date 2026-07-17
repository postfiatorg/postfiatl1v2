// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.24;

import {MarketOpsEnvelope} from "../src/MarketOpsEnvelope.sol";
import {IBurnableToken, IERC20Minimal, IPFTLBridgeAdapter, MarketOpsVault} from "../src/MarketOpsVault.sol";
import {
    IMintBridgeAdapter,
    IMintableEscrowToken,
    IMintSettlementVerifier,
    MintController
} from "../src/MintController.sol";
import {PFTLBridgeAdapter} from "../src/PFTLBridgeAdapter.sol";
import {PolicyRegistry} from "../src/PolicyRegistry.sol";

interface AdvVm {
    function warp(uint256 timestamp) external;
}

contract MarketOpsAdversarialTest {
    AdvVm private constant vm = AdvVm(address(uint160(uint256(keccak256("hevm cheat code")))));

    AdvMockToken private reserve_token;
    AdvMockToken private asset;
    AdvMockVenue private venue;
    PolicyRegistry private registry;
    PFTLBridgeAdapter private adapter;
    MarketOpsVault private vault;
    MintController private controller;
    AdvMockSettlementVerifier private settlement_verifier;

    uint64 private constant CHAIN_ID = 65_100;
    uint64 private constant CHALLENGE_DELAY = 100;
    uint64 private constant EXECUTION_WINDOW = 1_000;
    uint64 private constant MAX_STALENESS = 75;
    uint256 private constant SUPPORT_DISCOUNT_BPS = 500;

    bytes32 private constant ASSET_ID = bytes32(uint256(0xaaaaaaaa));
    bytes32 private constant PROGRAM_ID = bytes32(uint256(0x31));
    bytes32 private constant POLICY_HASH = bytes32(uint256(0x32));
    bytes32 private constant PARAMETER_HASH = bytes32(uint256(0x33));
    bytes32 private constant VENUE_ID = bytes32(uint256(0x37));
    bytes32 private constant POOL_CONFIG_HASH = bytes32(uint256(0x38));
    bytes32 private constant HOOK_CODE_HASH = bytes32(uint256(0x39));

    bytes private constant ENVELOPE_HASH =
        hex"0102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f202122232425262728292a2b2c2d2e2f30";
    bytes private constant OTHER_ENVELOPE_HASH =
        hex"3102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f202122232425262728292a2b2c2d2e2f30";

    function setUp() public {
        reserve_token = new AdvMockToken();
        asset = new AdvMockToken();
        venue = new AdvMockVenue(asset);
        registry = new PolicyRegistry(address(this));
        registry.registerPolicy(
            PROGRAM_ID, POLICY_HASH, PARAMETER_HASH, VENUE_ID, POOL_CONFIG_HASH, HOOK_CODE_HASH, 1, 0
        );
        vault = new MarketOpsVault(
            IERC20Minimal(address(reserve_token)),
            IBurnableToken(address(asset)),
            address(this),
            SUPPORT_DISCOUNT_BPS,
            true
        );
        controller = new MintController(IMintableEscrowToken(address(asset)), address(this), 1);
        settlement_verifier = new AdvMockSettlementVerifier();
        adapter = new PFTLBridgeAdapter(
            registry,
            address(this),
            CHAIN_ID,
            address(vault),
            address(controller),
            CHALLENGE_DELAY,
            EXECUTION_WINDOW,
            MAX_STALENESS
        );
        vault.setBridgeAdapter(IPFTLBridgeAdapter(address(adapter)));
        vault.setVenue(VENUE_ID, address(venue));
        controller.setBridgeAdapter(IMintBridgeAdapter(address(adapter)));
        controller.setSettlementVerifier(
            IMintSettlementVerifier(address(settlement_verifier)), address(settlement_verifier).codehash
        );
    }

    function testFrontRunUpwardVaultRevertsAbovePriceLimit() public {
        (MarketOpsEnvelope memory envelope, bytes32 pending_id) = _acceptedEnvelope(1, 100_000e8, 0, 600);
        _fund(48_000_000);
        venue.setPriceUsdE8(480_000_000);

        _expectExecuteRevert(envelope, _route(pending_id), 48_000_000, 10, 1_200);

        _assertEq(vault.deployed_usd_e8_by_pending_id(pending_id), 0, "deployed remains zero");
        _assertEq(asset.totalSupply(), 0, "no acquired supply");
    }

    function testFrontRunDownwardVaultBuysDiscountedUnitsWithinCap() public {
        (MarketOpsEnvelope memory envelope, bytes32 pending_id) = _acceptedEnvelope(1, 100_000e8, 0, 600);
        _fund(45_000_000);
        venue.setPriceUsdE8(450_000_000);

        uint256 received = vault.executeBuy(envelope, _route(pending_id), 45_000_000, 10, 1_200);

        _assertEq(received, 10, "received");
        _assertEq(vault.deployed_usd_e8_by_pending_id(pending_id), 45e8, "deployed within cap");
        _assertEq(vault.burned_atoms_by_asset(ASSET_ID), 10, "burned acquired units");
    }

    function testPremiumMintEscrowCannotLeaveWithoutSettlement() public {
        (MarketOpsEnvelope memory envelope,) = _acceptedEnvelope(1, 0, 100, 0);
        bytes32 escrow_id = controller.requestMint(envelope, 100);

        _assertEq(asset.balanceOf(address(controller)), 100, "escrow balance");
        _assertEq(asset.balanceOf(address(this)), 0, "beneficiary balance");
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
        _assertEq(asset.balanceOf(address(controller)), 100, "still escrowed");
    }

    function testPostMintBackingReconcilesAfterSettlement() public {
        (MarketOpsEnvelope memory envelope, bytes32 pending_id) = _acceptedEnvelope(1, 0, 100, 0);
        bytes32 escrow_id = controller.requestMint(envelope, 100);
        settlement_verifier.attest(pending_id, escrow_id, address(this), 100, bytes32(uint256(0x5555)), 500e8, 0);

        controller.releaseMint(
            envelope,
            escrow_id,
            MintController.SettlementProof({
                recipient: address(this),
                settled_proceeds_usd_e8: 500e8,
                locked_liquidity_usd_e8: 0,
                proceeds_settled: true,
                liquidity_locked: false,
                proof_hash: bytes32(uint256(0x5555))
            })
        );

        _assertEq(asset.balanceOf(address(this)), 100, "released");
        _assertEq(controller.escrowed_atoms_by_asset(ASSET_ID), 0, "escrow cleared");
        _assertEq(controller.released_mint_atoms_by_pending_id(pending_id), 100, "released by pending");
        _assertEq(controller.settled_value_usd_e8_by_pending_id(pending_id), 500e8, "settled value");
    }

    function testSelfReserveAccountingBurnsTreasuryA651ToZeroSupply() public {
        (MarketOpsEnvelope memory envelope, bytes32 pending_id) = _acceptedEnvelope(1, 100_000e8, 0, 600);
        _fund(47_000_000);
        venue.setPriceUsdE8(470_000_000);

        vault.executeBuy(envelope, _route(pending_id), 47_000_000, 10, 1_200);

        _assertEq(asset.balanceOf(address(vault)), 0, "vault asset balance");
        _assertEq(asset.totalSupply(), 0, "treasury inventory has no circulating supply");
        _assertEq(vault.burned_atoms_by_asset(ASSET_ID), 10, "burned accounting");
        _assertEq(vault.locked_treasury_inventory_atoms_by_asset(ASSET_ID), 0, "no reserve-valued inventory");
    }

    function testBridgeChallengeWrongPacketCannotExecute() public {
        vm.warp(1_000);
        MarketOpsEnvelope memory envelope = _envelope(1, 1_100, 1_600, 950, 100_000e8, 100, 0);
        bytes32 pending_id = adapter.submitEnvelope(envelope, ENVELOPE_HASH);

        adapter.challengeEnvelope(pending_id, PFTLBridgeAdapter.ChallengeFault.HashMismatch);
        vm.warp(1_100);
        adapter.finalizeEnvelope(pending_id);

        _assertEq(
            uint256(adapter.getEnvelopeStatus(pending_id)), uint256(PFTLBridgeAdapter.EnvelopeStatus.Frozen), "frozen"
        );
        _assertTrue(!adapter.isEnvelopeExecutable(pending_id), "not executable");
        _assertEq(adapter.reserveDeployCapUsdE8(pending_id), 0, "reserve cap zero");
        _assertEq(adapter.mintCapAtoms(pending_id), 0, "mint cap zero");
    }

    function testStalePacketExpiredEnvelopeRejectedByExecution() public {
        (MarketOpsEnvelope memory envelope, bytes32 pending_id) = _acceptedEnvelope(1, 100_000e8, 100, 600);
        _fund(47_000_000);
        venue.setPriceUsdE8(470_000_000);

        vm.warp(1_601);

        _assertTrue(!adapter.isEnvelopeExecutable(pending_id), "expired");
        _assertEq(adapter.reserveDeployCapUsdE8(pending_id), 0, "reserve cap zero");
        _assertEq(adapter.mintCapAtoms(pending_id), 0, "mint cap zero");
        _expectExecuteRevert(envelope, _route(pending_id), 47_000_000, 10, 1_700);
    }

    function testPFTLHaltPausesEnvelopeOperations() public {
        (MarketOpsEnvelope memory envelope, bytes32 pending_id) = _acceptedEnvelope(1, 100_000e8, 100, 600);
        _fund(47_000_000);
        venue.setPriceUsdE8(470_000_000);

        adapter.setPaused(true);

        _assertTrue(!adapter.isEnvelopeExecutable(pending_id), "paused");
        _assertEq(adapter.reserveDeployCapUsdE8(pending_id), 0, "reserve cap zero");
        _assertEq(adapter.mintCapAtoms(pending_id), 0, "mint cap zero");
        _expectExecuteRevert(envelope, _route(pending_id), 47_000_000, 10, 1_200);
        _expectRequestRevert(envelope, 1);
    }

    function testPFTLEquivocationPausesAdapterAndZerosCaps() public {
        vm.warp(1_000);
        MarketOpsEnvelope memory envelope = _envelope(1, 1_100, 1_600, 950, 100_000e8, 100, 0);
        bytes32 first_pending_id = adapter.submitEnvelope(envelope, ENVELOPE_HASH);
        envelope.nonce = bytes32(uint256(0x56));

        bytes32 second_pending_id = adapter.submitEnvelope(envelope, OTHER_ENVELOPE_HASH);

        _assertEq(second_pending_id, bytes32(0), "equivocation returns zero");
        _assertTrue(adapter.paused(), "adapter paused");
        _assertEq(adapter.reserveDeployCapUsdE8(first_pending_id), 0, "reserve cap zero");
        _assertEq(adapter.mintCapAtoms(first_pending_id), 0, "mint cap zero");
    }

    function testOperatorNonFundingKeepsExecutionAtZero() public {
        (MarketOpsEnvelope memory envelope, bytes32 pending_id) = _acceptedEnvelope(1, 0, 0, 600);
        venue.setPriceUsdE8(470_000_000);

        _assertEq(adapter.reserveDeployCapUsdE8(pending_id), 0, "adapter cap zero");
        _assertEq(vault.venue_reserve_balance(VENUE_ID), 0, "vault reserve zero");
        _expectExecuteRevert(envelope, _route(pending_id), 47_000_000, 10, 1_200);
        _assertEq(vault.deployed_usd_e8_by_pending_id(pending_id), 0, "nothing deployed");
    }

    function _acceptedEnvelope(
        uint64 epoch,
        uint256 reserve_cap_usd_e8,
        uint256 mint_cap_atoms,
        uint64 cooldown_seconds
    ) private returns (MarketOpsEnvelope memory envelope, bytes32 pending_id) {
        vm.warp(1_000);
        envelope = _envelope(epoch, 1_100, 1_600, 950, reserve_cap_usd_e8, mint_cap_atoms, cooldown_seconds);
        pending_id = adapter.submitEnvelope(envelope, ENVELOPE_HASH);
        vm.warp(1_100);
        adapter.finalizeEnvelope(pending_id);
    }

    function _envelope(
        uint64 epoch,
        uint64 valid_after,
        uint64 expires_at,
        uint64 data_window_end,
        uint256 reserve_cap_usd_e8,
        uint256 mint_cap_atoms,
        uint64 cooldown_seconds
    ) private view returns (MarketOpsEnvelope memory envelope) {
        envelope.encoding_version = 1;
        envelope.chain_id = CHAIN_ID;
        envelope.adapter_address = address(adapter);
        envelope.vault_address = address(vault);
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
        envelope.max_reserve_deploy_usd_e8 = reserve_cap_usd_e8;
        envelope.max_mint_atoms = mint_cap_atoms;
        envelope.discount_trigger_bps = 300;
        envelope.premium_trigger_bps = 1_000;
        envelope.data_window_start = data_window_end - 100;
        envelope.data_window_end = data_window_end;
        envelope.valid_after = valid_after;
        envelope.expires_at = expires_at;
        envelope.cooldown_seconds = cooldown_seconds;
        envelope.nonce = bytes32(uint256(0x55));
    }

    function _route(bytes32 pending_id) private view returns (MarketOpsVault.BuyRoute memory) {
        return MarketOpsVault.BuyRoute({pending_id: pending_id, venue: address(venue), data: ""});
    }

    function _fund(uint256 amount) private {
        reserve_token.mint(address(this), amount);
        reserve_token.approve(address(vault), amount);
        vault.fundVenueReserve(VENUE_ID, amount);
    }

    function _expectExecuteRevert(
        MarketOpsEnvelope memory envelope,
        MarketOpsVault.BuyRoute memory route,
        uint256 amount,
        uint256 min_received,
        uint256 deadline
    ) private {
        try vault.executeBuy(envelope, route, amount, min_received, deadline) returns (uint256) {
            revert("expected executeBuy revert");
        } catch {}
    }

    function _expectRequestRevert(MarketOpsEnvelope memory envelope, uint256 amount) private {
        try controller.requestMint(envelope, amount) returns (bytes32) {
            revert("expected requestMint revert");
        } catch {}
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

    function _assertEq(uint256 actual, uint256 expected, string memory message) private pure {
        if (actual != expected) {
            revert(message);
        }
    }
}

contract AdvMockSettlementVerifier is IMintSettlementVerifier {
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
        records[proof_hash] = Record(
            pending_id, escrow_id, recipient, amount_atoms, settled_proceeds_usd_e8, locked_liquidity_usd_e8
        );
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

contract AdvMockVenue {
    AdvMockToken private immutable asset;
    uint256 public price_usd_e8;

    constructor(AdvMockToken asset_) {
        asset = asset_;
    }

    function setPriceUsdE8(uint256 price_usd_e8_) external {
        price_usd_e8 = price_usd_e8_;
    }

    function executeBuy(address, address, uint256, uint256 min_received, address recipient, bytes calldata)
        external
        returns (uint256 reported_received)
    {
        asset.mint(recipient, min_received);
        return min_received;
    }
}

contract AdvMockToken {
    mapping(address => uint256) public balanceOf;
    mapping(address => mapping(address => uint256)) public allowance;
    uint256 public totalSupply;

    function approve(address spender, uint256 amount) external returns (bool) {
        allowance[msg.sender][spender] = amount;
        return true;
    }

    function transfer(address to, uint256 amount) external returns (bool) {
        _transfer(msg.sender, to, amount);
        return true;
    }

    function transferFrom(address from, address to, uint256 amount) external returns (bool) {
        uint256 current_allowance = allowance[from][msg.sender];
        if (current_allowance < amount) {
            return false;
        }
        allowance[from][msg.sender] = current_allowance - amount;
        _transfer(from, to, amount);
        return true;
    }

    function mint(address to, uint256 amount) external {
        balanceOf[to] += amount;
        totalSupply += amount;
    }

    function burn(uint256 amount) external {
        uint256 balance = balanceOf[msg.sender];
        if (balance < amount) {
            revert("insufficient balance");
        }
        balanceOf[msg.sender] = balance - amount;
        totalSupply -= amount;
    }

    function _transfer(address from, address to, uint256 amount) private {
        uint256 balance = balanceOf[from];
        if (balance < amount) {
            revert("insufficient balance");
        }
        balanceOf[from] = balance - amount;
        balanceOf[to] += amount;
    }
}
