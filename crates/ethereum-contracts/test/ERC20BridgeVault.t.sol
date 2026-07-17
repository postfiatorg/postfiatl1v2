// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.24;

import {IPFTLWithdrawalVerifier, IERC20BridgeToken, ERC20BridgeVault} from "../src/ERC20BridgeVault.sol";

interface BridgeVaultVm {
    function warp(uint256 timestamp) external;
    function prank(address sender) external;
}

contract ERC20BridgeVaultTest {
    BridgeVaultVm private constant vm = BridgeVaultVm(address(uint160(uint256(keccak256("hevm cheat code")))));

    MockERC20 private token;
    MockWithdrawalVerifier private verifier;
    ERC20BridgeVault private vault;

    uint64 private constant PFTL_CHAIN_ID = 65_100;
    uint64 private constant CHALLENGE_DELAY = 100;
    uint64 private constant EXECUTION_WINDOW = 1_000;
    bytes private constant PFTL_WITHDRAWAL_HASH =
        hex"0102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f202122232425262728292a2b2c2d2e2f30";
    address private constant RECIPIENT = address(0xBEEF);

    function setUp() public {
        token = new MockERC20();
        verifier = new MockWithdrawalVerifier();
        vault = new ERC20BridgeVault(
            IERC20BridgeToken(address(token)),
            IPFTLWithdrawalVerifier(address(verifier)),
            address(this),
            PFTL_CHAIN_ID,
            _assetId(),
            CHALLENGE_DELAY,
            EXECUTION_WINDOW
        );
    }

    function testDepositV2TransfersTokenAndCommitsRecipientAndRoute() public {
        token.mint(address(this), 1_000_000);
        token.approve(address(vault), 1_000_000);

        bytes32 recipient_hash = keccak256(bytes("bridge-recipient-000000000000000000000000"));
        bytes32 route_binding = keccak256("governed-route-profile-1");
        bytes32 expected_deposit_id =
            vault.depositIdV2(address(this), 1_000_000, recipient_hash, bytes32(uint256(0x1234)), route_binding);

        bytes32 deposit_id = vault.depositV2(
            1_000_000,
            "bridge-recipient-000000000000000000000000",
            bytes32(uint256(0x1234)),
            route_binding
        );

        _assertEq(deposit_id, expected_deposit_id, "deposit id");
        _assertTrue(vault.deposit_seen(deposit_id), "deposit seen");
        _assertEq(token.balanceOf(address(vault)), 1_000_000, "vault balance");
        _assertEq(token.balanceOf(address(this)), 0, "depositor balance");
        _expectDepositV2Revert(
            1_000_000,
            "bridge-recipient-000000000000000000000000",
            bytes32(uint256(0x1234)),
            route_binding
        );
    }

    function testLegacyAndUnboundDepositsFailBeforeTokenMutation() public {
        token.mint(address(this), 2_000_000);
        token.approve(address(vault), 2_000_000);

        _expectDepositRevert(1_000_000, "bridge-recipient", bytes32(uint256(0x1234)));
        _expectDepositV2Revert(1_000_000, "bridge-recipient", bytes32(uint256(0x1234)), bytes32(0));

        _assertEq(token.balanceOf(address(vault)), 0, "unbound deposit must not fund vault");
        _assertEq(token.balanceOf(address(this)), 2_000_000, "unbound deposit must not debit user");
    }

    function testWithdrawalFinalizesAndPaysRecipientDirectly() public {
        token.mint(address(vault), 2_000_000);
        vm.warp(1_000);
        ERC20BridgeVault.WithdrawalPacket memory packet = _packet(1_500_000, RECIPIENT, bytes32(uint256(0x101)));

        bytes32 pending_id = _submitApproved(packet, PFTL_WITHDRAWAL_HASH);
        _assertEq(
            uint256(vault.getWithdrawalStatus(pending_id)),
            uint256(ERC20BridgeVault.WithdrawalStatus.Pending),
            "pending"
        );
        _expectFinalizeRevert(pending_id);

        vm.warp(1_100);
        vault.finalizeWithdrawal(pending_id);
        _assertTrue(vault.isWithdrawalClaimable(pending_id), "claimable");

        vault.claimWithdrawal(pending_id);
        _assertEq(token.balanceOf(RECIPIENT), 1_500_000, "recipient paid");
        _assertEq(token.balanceOf(address(vault)), 500_000, "vault remainder");
        _assertEq(
            uint256(vault.getWithdrawalStatus(pending_id)),
            uint256(ERC20BridgeVault.WithdrawalStatus.Claimed),
            "claimed"
        );
        _expectClaimRevert(pending_id);
    }

    function testChallengedWithdrawalFreezesAndCannotPay() public {
        token.mint(address(vault), 1_000_000);
        vm.warp(1_000);
        bytes32 pending_id =
            _submitApproved(_packet(1_000_000, RECIPIENT, bytes32(uint256(0x102))), PFTL_WITHDRAWAL_HASH);

        vault.challengeWithdrawal(pending_id, ERC20BridgeVault.ChallengeFault.HashMismatch);
        _assertEq(
            uint256(vault.getWithdrawalStatus(pending_id)),
            uint256(ERC20BridgeVault.WithdrawalStatus.Challenged),
            "challenged"
        );

        vm.warp(1_100);
        vault.finalizeWithdrawal(pending_id);
        _assertEq(
            uint256(vault.getWithdrawalStatus(pending_id)), uint256(ERC20BridgeVault.WithdrawalStatus.Frozen), "frozen"
        );
        _expectClaimRevert(pending_id);
        _assertEq(token.balanceOf(RECIPIENT), 0, "recipient unpaid");
        _assertEq(token.balanceOf(address(vault)), 1_000_000, "vault retained funds");
    }

    function testUnauthorizedChallengeCannotFreezeValidWithdrawal() public {
        token.mint(address(vault), 1_000_000);
        vm.warp(1_000);
        bytes32 pending_id =
            _submitApproved(_packet(1_000_000, RECIPIENT, bytes32(uint256(0x112))), PFTL_WITHDRAWAL_HASH);

        vm.prank(address(0xBAD));
        _expectChallengeRevert(pending_id);

        vm.warp(1_100);
        vault.finalizeWithdrawal(pending_id);
        vault.claimWithdrawal(pending_id);
        _assertEq(token.balanceOf(RECIPIENT), 1_000_000, "recipient paid despite grief attempt");
    }

    function testBurnTxReplayRejected() public {
        vm.warp(1_000);
        ERC20BridgeVault.WithdrawalPacket memory packet = _packet(1_000_000, RECIPIENT, bytes32(uint256(0x103)));
        bytes32 pending_id = _submitApproved(packet, PFTL_WITHDRAWAL_HASH);
        _assertTrue(pending_id != bytes32(0), "pending id");

        _expectSubmitRevert(packet, PFTL_WITHDRAWAL_HASH);
    }

    function testInsufficientVaultLiquidityFailsClosedUntilFunded() public {
        vm.warp(1_000);
        bytes32 pending_id =
            _submitApproved(_packet(1_000_000, RECIPIENT, bytes32(uint256(0x104))), PFTL_WITHDRAWAL_HASH);
        vm.warp(1_100);
        vault.finalizeWithdrawal(pending_id);

        _expectClaimRevert(pending_id);
        token.mint(address(vault), 1_000_000);
        vault.claimWithdrawal(pending_id);
        _assertEq(token.balanceOf(RECIPIENT), 1_000_000, "recipient paid after funding");
    }

    function testAcceptedWithdrawalCanBeClaimedAfterExecutionWindow() public {
        token.mint(address(vault), 1_000_000);
        vm.warp(1_000);
        bytes32 pending_id =
            _submitApproved(_packet(1_000_000, RECIPIENT, bytes32(uint256(0x105))), PFTL_WITHDRAWAL_HASH);
        vm.warp(1_100);
        vault.finalizeWithdrawal(pending_id);
        vm.warp(2_101);
        _assertTrue(vault.isWithdrawalClaimable(pending_id), "claimable after expiry");
        vault.claimWithdrawal(pending_id);
        _assertEq(token.balanceOf(RECIPIENT), 1_000_000, "recipient paid after expiry");
    }

    function testBadPacketRejected() public {
        vm.warp(1_000);
        ERC20BridgeVault.WithdrawalPacket memory wrong_chain = _packet(1_000_000, RECIPIENT, bytes32(uint256(0x106)));
        wrong_chain.pftl_chain_id = 7;
        _expectSubmitRevert(wrong_chain, PFTL_WITHDRAWAL_HASH);

        ERC20BridgeVault.WithdrawalPacket memory bad_asset = _packet(1_000_000, RECIPIENT, bytes32(uint256(0x107)));
        bad_asset.vault_bridge_asset_id = hex"1234";
        _expectSubmitRevert(bad_asset, PFTL_WITHDRAWAL_HASH);

        _expectSubmitRevert(_packet(1_000_000, RECIPIENT, bytes32(uint256(0x108))), hex"1234");
    }

    function testUnverifiedWithdrawalRejectedBeforePending() public {
        vm.warp(1_000);
        ERC20BridgeVault.WithdrawalPacket memory packet = _packet(1_000_000, RECIPIENT, bytes32(uint256(0x109)));
        _expectSubmitRevert(packet, PFTL_WITHDRAWAL_HASH);
    }

    function testWithdrawalCannotReplayAcrossVaults() public {
        ERC20BridgeVault second_vault = new ERC20BridgeVault(
            IERC20BridgeToken(address(token)),
            IPFTLWithdrawalVerifier(address(verifier)),
            address(this),
            PFTL_CHAIN_ID,
            _assetId(),
            CHALLENGE_DELAY,
            EXECUTION_WINDOW
        );
        token.mint(address(vault), 1_000_000);
        token.mint(address(second_vault), 1_000_000);
        vm.warp(1_000);

        ERC20BridgeVault.WithdrawalPacket memory first_packet = _packet(1_000_000, RECIPIENT, bytes32(uint256(0x10a)));
        _approve(first_packet, PFTL_WITHDRAWAL_HASH);
        bytes32 first_pending_id = vault.submitWithdrawal(first_packet, PFTL_WITHDRAWAL_HASH);
        _assertTrue(first_pending_id != bytes32(0), "first vault accepted");

        _expectSubmitRevertTo(second_vault, first_packet, PFTL_WITHDRAWAL_HASH);

        ERC20BridgeVault.WithdrawalPacket memory second_packet =
            _packetFor(address(second_vault), 1_000_000, RECIPIENT, bytes32(uint256(0x10a)));
        _expectSubmitRevertTo(second_vault, second_packet, PFTL_WITHDRAWAL_HASH);
    }

    function testWithdrawalPacketDigestMatchesPFTLVector() public view {
        ERC20BridgeVault.WithdrawalPacket memory packet;
        packet.pftl_chain_id = PFTL_CHAIN_ID;
        packet.source_chain_id = 42_161;
        packet.vault_address = address(0x1111111111111111111111111111111111111111);
        packet.token_address = address(0x3333333333333333333333333333333333333333);
        packet.vault_bridge_asset_id =
        hex"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        packet.burn_tx_id =
        hex"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
        packet.withdrawal_id =
        hex"cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";
        packet.recipient = address(0x2222222222222222222222222222222222222222);
        packet.amount = 1_000_000;
        packet.source_bucket_id =
        hex"dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd";
        packet.destination_hash =
        hex"eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee";
        packet.finalized_height = 77;
        packet.evidence_root =
        hex"111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111";

        _assertEq(
            vault.withdrawalPacketDigest(packet),
            0xfaf77ea9f7590b08fdaa1ce11263a0d952781118a867e0dbfe99c34e31c8e0c3,
            "packet digest"
        );
    }

    function testWithdrawalPlanVectorMatchesVaultPendingId() public view {
        ERC20BridgeVault.WithdrawalPacket memory packet;
        packet.pftl_chain_id = 4_660_518_586_501_272_219;
        packet.source_chain_id = 42_161;
        packet.vault_address = address(0x1111111111111111111111111111111111111111);
        packet.token_address = address(0x3333333333333333333333333333333333333333);
        packet.vault_bridge_asset_id =
        hex"c14c0838675d8a9914b8b961fc100fc1aeb882fbb0516ab254f48da8786a631ff04c75fc714996db3d77636dc01ac893";
        packet.burn_tx_id =
        hex"999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999999";
        packet.withdrawal_id =
        hex"bb0c61c90f14cbba4c7365d3291cca6fa20fd415a0485bfdd0bb6e6d4535da38dbad1010183f37c24e163d7bec74cf34";
        packet.recipient = address(0x2222222222222222222222222222222222222222);
        packet.amount = 1_000_000;
        packet.source_bucket_id =
        hex"14af56c0841fcf2a6195a2d49a21a16b52bfda24ecff5f654fa10a5d7638faf584eb657c750a216c550fadd1d5dd956b";
        packet.destination_hash =
        hex"cd0a3af345d4f3daede40fe3b1c6ebd2022697adacddc3088d1baaff52c1d39540d75c12d2cd192feb7f57e3553e1786";
        packet.finalized_height = 14;
        packet.evidence_root =
        hex"b481a7eb7e8def24dc1d797d16cf177ac1f90919ca3f02fd4e54871291b925fb81122e5f1305ede94792a57ca150d3fb";

        bytes32 hash_commitment = 0xc04691b95e3006772b9bafe7cddbcfbaf7fb9585fb61afb1d84102be2e169e22;
        _assertEq(
            vault.withdrawalPacketDigest(packet),
            0xf19528b6109bc310b93371e0c35f7c03cafae30dfd57acaa8b0e9d82aa6e425e,
            "packet digest"
        );
        _assertEq(
            vault.withdrawalPendingId(packet, hash_commitment),
            0x4296224e52cf5ae729ef33f2e177d7a246ab66826fd9d2c965cc437b7d2ccbed,
            "pending withdrawal id"
        );
    }

    function _packet(uint256 amount, address recipient, bytes32 burn_seed)
        private
        view
        returns (ERC20BridgeVault.WithdrawalPacket memory packet)
    {
        packet = _packetFor(address(vault), amount, recipient, burn_seed);
    }

    function _packetFor(address vault_address, uint256 amount, address recipient, bytes32 burn_seed)
        private
        view
        returns (ERC20BridgeVault.WithdrawalPacket memory packet)
    {
        packet.pftl_chain_id = PFTL_CHAIN_ID;
        packet.source_chain_id = block.chainid;
        packet.vault_address = vault_address;
        packet.token_address = address(token);
        packet.vault_bridge_asset_id = _assetId();
        packet.burn_tx_id = _pftlHash(burn_seed);
        packet.withdrawal_id = _pftlHash(keccak256(abi.encode("withdrawal", burn_seed)));
        packet.recipient = recipient;
        packet.amount = amount;
        packet.source_bucket_id = _pftlHash(bytes32(uint256(0xbbbb)));
        packet.destination_hash = _pftlHash(keccak256(abi.encode(recipient)));
        packet.finalized_height = 77;
        packet.evidence_root = _pftlHash(bytes32(uint256(0xeeee)));
    }

    function _assetId() private pure returns (bytes memory) {
        return _pftlHash(bytes32(uint256(0xaaaa)));
    }

    function _pftlHash(bytes32 seed) private pure returns (bytes memory out) {
        out = new bytes(48);
        bytes32 left = keccak256(abi.encode("left", seed));
        bytes32 right = keccak256(abi.encode("right", seed));
        for (uint256 i = 0; i < 32; i++) {
            out[i] = left[i];
        }
        for (uint256 i = 0; i < 16; i++) {
            out[32 + i] = right[i];
        }
    }

    function _approve(ERC20BridgeVault.WithdrawalPacket memory packet, bytes memory pftl_hash) private {
        verifier.approve(vault.withdrawalPacketDigest(packet), keccak256(pftl_hash));
    }

    function _submitApproved(ERC20BridgeVault.WithdrawalPacket memory packet, bytes memory pftl_hash)
        private
        returns (bytes32 pending_id)
    {
        _approve(packet, pftl_hash);
        pending_id = vault.submitWithdrawal(packet, pftl_hash);
    }

    function _expectDepositRevert(uint256 amount, string memory recipient, bytes32 nonce) private view {
        try vault.deposit(amount, recipient, nonce) returns (bytes32) {
            revert("expected deposit revert");
        } catch {}
    }

    function _expectDepositV2Revert(uint256 amount, string memory recipient, bytes32 nonce, bytes32 route_binding)
        private
    {
        try vault.depositV2(amount, recipient, nonce, route_binding) returns (bytes32) {
            revert("expected depositV2 revert");
        } catch {}
    }

    function _expectSubmitRevert(ERC20BridgeVault.WithdrawalPacket memory packet, bytes memory pftl_hash) private {
        try vault.submitWithdrawal(packet, pftl_hash) returns (bytes32) {
            revert("expected submitWithdrawal revert");
        } catch {}
    }

    function _expectSubmitRevertTo(
        ERC20BridgeVault target,
        ERC20BridgeVault.WithdrawalPacket memory packet,
        bytes memory pftl_hash
    ) private {
        try target.submitWithdrawal(packet, pftl_hash) returns (bytes32) {
            revert("expected submitWithdrawal revert");
        } catch {}
    }

    function _expectChallengeRevert(bytes32 pending_id) private {
        try vault.challengeWithdrawal(pending_id, ERC20BridgeVault.ChallengeFault.HashMismatch) {
            revert("expected challengeWithdrawal revert");
        } catch {}
    }

    function _expectFinalizeRevert(bytes32 pending_id) private {
        try vault.finalizeWithdrawal(pending_id) {
            revert("expected finalizeWithdrawal revert");
        } catch {}
    }

    function _expectClaimRevert(bytes32 pending_id) private {
        try vault.claimWithdrawal(pending_id) {
            revert("expected claimWithdrawal revert");
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

contract MockERC20 {
    mapping(address => uint256) public balanceOf;
    mapping(address => mapping(address => uint256)) public allowance;
    uint256 public totalSupply;

    function mint(address to, uint256 amount) external {
        balanceOf[to] += amount;
        totalSupply += amount;
    }

    function approve(address spender, uint256 amount) external returns (bool) {
        allowance[msg.sender][spender] = amount;
        return true;
    }

    function transfer(address to, uint256 amount) external returns (bool) {
        if (balanceOf[msg.sender] < amount) {
            return false;
        }
        balanceOf[msg.sender] -= amount;
        balanceOf[to] += amount;
        return true;
    }

    function transferFrom(address from, address to, uint256 amount) external returns (bool) {
        if (balanceOf[from] < amount || allowance[from][msg.sender] < amount) {
            return false;
        }
        allowance[from][msg.sender] -= amount;
        balanceOf[from] -= amount;
        balanceOf[to] += amount;
        return true;
    }
}

contract MockWithdrawalVerifier is IPFTLWithdrawalVerifier {
    mapping(bytes32 => bool) public accepted;

    function approve(bytes32 packet_digest, bytes32 pftl_withdrawal_hash_commitment) external {
        accepted[_key(packet_digest, pftl_withdrawal_hash_commitment)] = true;
    }

    function isWithdrawalAccepted(bytes32 packet_digest, bytes32 pftl_withdrawal_hash_commitment)
        external
        view
        returns (bool)
    {
        return accepted[_key(packet_digest, pftl_withdrawal_hash_commitment)];
    }

    function _key(bytes32 packet_digest, bytes32 pftl_withdrawal_hash_commitment) private pure returns (bytes32) {
        return keccak256(abi.encode(packet_digest, pftl_withdrawal_hash_commitment));
    }
}
