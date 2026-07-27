// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.24;

import {IERC20BridgeTokenV2, IPFTLFinalityVerifierV1} from "../src/ERC20BridgeVaultV2.sol";
import {ERC20BridgeVaultL1} from "../src/ERC20BridgeVaultL1.sol";

interface Vm {
    function load(address target, bytes32 slot) external view returns (bytes32);
}

contract L1MockToken is IERC20BridgeTokenV2 {
    mapping(address => uint256) public balanceOf;
    mapping(address => mapping(address => uint256)) public allowance;

    function mint(address to, uint256 amount) external {
        balanceOf[to] += amount;
    }

    function approve(address spender, uint256 amount) external returns (bool) {
        allowance[msg.sender][spender] = amount;
        return true;
    }

    function transfer(address to, uint256 amount) external returns (bool) {
        balanceOf[msg.sender] -= amount;
        balanceOf[to] += amount;
        return true;
    }

    function transferFrom(address from, address to, uint256 amount) external returns (bool) {
        allowance[from][msg.sender] -= amount;
        balanceOf[from] -= amount;
        balanceOf[to] += amount;
        return true;
    }
}

/// Deterministic finality verifier double returning pinned commitments so the
/// vault's replay-protection path can be exercised without a live SP1 proof.
contract L1FixedFinality is IPFTLFinalityVerifierV1 {
    address private immutable recipient;
    uint256 private immutable amount;
    bytes32 private immutable withdrawalCommitment;
    bytes32 private immutable burnCommitment;
    bytes32 private immutable packetDigest;

    constructor(address recipient_, uint256 amount_) {
        recipient = recipient_;
        amount = amount_;
        withdrawalCommitment = keccak256("l1-withdrawal");
        burnCommitment = keccak256("l1-burn");
        packetDigest = keccak256("l1-packet");
    }

    function verifyAndConsume(bytes calldata, bytes calldata)
        external
        view
        returns (address, uint256, bytes32, bytes32, bytes32)
    {
        return (recipient, amount, withdrawalCommitment, burnCommitment, packetDigest);
    }
}

contract ERC20BridgeVaultL1Test {
    address private constant WITHDRAW_RECIPIENT = address(0xBEEF);
    uint256 private constant WITHDRAW_AMOUNT = 400_000;
    Vm private constant VM = Vm(address(uint160(uint256(keccak256("hevm cheat code")))));

    L1MockToken private token;
    L1FixedFinality private finality;
    ERC20BridgeVaultL1 private vault;

    function setUp() public {
        token = new L1MockToken();
        finality = new L1FixedFinality(WITHDRAW_RECIPIENT, WITHDRAW_AMOUNT);
        vault = new ERC20BridgeVaultL1(
            IERC20BridgeTokenV2(address(token)),
            IPFTLFinalityVerifierV1(address(finality)),
            address(token).codehash,
            address(this)
        );
    }

    function testConstructorReadbackPinsTokenVerifierCodehashOwner() public view {
        require(address(vault.token()) == address(token), "token readback drift");
        require(address(vault.finalityVerifier()) == address(finality), "verifier readback drift");
        require(vault.tokenRuntimeCodeHash() == address(token).codehash, "codehash readback drift");
        require(vault.owner() == address(this), "owner readback drift");
        require(!vault.paused(), "vault must start unpaused");
    }

    function testRuntimeCodeHashReadbackIsNonzero() public view {
        require(address(vault).codehash != bytes32(0), "runtime code readback failed");
    }

    function testConstructorRejectsTokenCodehashMismatch() public {
        try new ERC20BridgeVaultL1(
            IERC20BridgeTokenV2(address(token)),
            IPFTLFinalityVerifierV1(address(finality)),
            bytes32(uint256(1)),
            address(this)
        ) {
            revert("constructor accepted mismatched token codehash");
        } catch {}
    }

    function testDepositTransfersExactAtomsAndEmitsEvidence() public {
        token.mint(address(this), 1_000_000);
        token.approve(address(vault), 1_000_000);
        bytes32 routeBinding = keccak256("ethereum-sepolia-usdc-v1-route-binding");
        bytes32 depositId = vault.depositV2(1_000_000, "pf1lanecrecipient", bytes32(uint256(0x1234)), routeBinding);
        require(depositId != bytes32(0), "deposit id missing");
        require(token.balanceOf(address(vault)) == 1_000_000, "vault balance delta mismatch");
        require(token.balanceOf(address(this)) == 0, "depositor balance delta mismatch");
        require(vault.depositSeen(depositId), "deposit not recorded");
        require(vault.totalObligations() == 1_000_000, "obligations delta mismatch");
    }

    function testDepositStorageMatchesFrozenGuestDecode() public {
        uint256 amount = 25_000_000;
        string memory recipient = "pf1mainnetcampaignrecipient";
        bytes32 nonce = bytes32(uint256(0x1234));
        bytes32 routeBinding = keccak256("ethereum-mainnet-usdc-v1-epoch4-route-binding");
        token.mint(address(this), amount);
        token.approve(address(vault), amount);

        bytes32 depositId = vault.depositV2(amount, recipient, nonce, routeBinding);
        bytes32 base = keccak256(abi.encode(depositId, uint256(1)));
        bytes32 packed = VM.load(address(vault), base);

        require(uint256(VM.load(address(vault), bytes32(uint256(0)))) == amount, "guest obligations slot drift");
        require(address(uint160(uint256(packed))) == address(this), "guest depositor packing drift");
        require(uint256(packed) >> 160 == amount, "guest amount packing drift");
        require(
            VM.load(address(vault), bytes32(uint256(base) + 1)) == keccak256(bytes(recipient)),
            "guest recipient-hash slot drift"
        );
        require(VM.load(address(vault), bytes32(uint256(base) + 2)) == routeBinding, "guest route-binding slot drift");
        require(VM.load(address(vault), bytes32(uint256(base) + 3)) == nonce, "guest nonce slot drift");
    }

    function testDepositRejectsAmountOutsideGuestU64Domain() public {
        uint256 amount = uint256(type(uint64).max) + 1;
        token.mint(address(this), amount);
        token.approve(address(vault), amount);
        try vault.depositV2(
            amount,
            "pf1mainnetcampaignrecipient",
            bytes32(uint256(0x1234)),
            keccak256("ethereum-mainnet-usdc-v1-epoch4-route-binding")
        ) {
            revert("deposit exceeded frozen guest u64 domain");
        } catch {}
    }

    function testDepositReplayReverts() public {
        token.mint(address(this), 2_000_000);
        token.approve(address(vault), 2_000_000);
        bytes32 routeBinding = keccak256("ethereum-sepolia-usdc-v1-route-binding");
        vault.depositV2(1_000_000, "pf1lanecrecipient", bytes32(uint256(0x1234)), routeBinding);
        try vault.depositV2(1_000_000, "pf1lanecrecipient", bytes32(uint256(0x1234)), routeBinding) {
            revert("duplicate deposit accepted");
        } catch {}
    }

    function testProofNativeWithdrawalPaysDifferentRecipientExactly() public {
        _fundVaultThroughDeposit(1_000_000);
        uint256 recipientBefore = token.balanceOf(WITHDRAW_RECIPIENT);
        vault.withdrawWithProof(bytes(""), bytes(""));
        require(token.balanceOf(WITHDRAW_RECIPIENT) - recipientBefore == WITHDRAW_AMOUNT, "recipient delta mismatch");
        require(token.balanceOf(address(vault)) == 1_000_000 - WITHDRAW_AMOUNT, "vault delta mismatch");
        require(vault.totalObligations() == 1_000_000 - WITHDRAW_AMOUNT, "withdrawal obligations mismatch");
        require(WITHDRAW_RECIPIENT != address(this), "withdrawal recipient must differ from depositor");
    }

    function testWithdrawalReplayRevertsNegativeTest() public {
        _fundVaultThroughDeposit(1_000_000);
        vault.withdrawWithProof(bytes(""), bytes(""));
        try vault.withdrawWithProof(bytes(""), bytes("")) {
            revert("withdrawal replay accepted");
        } catch {}
    }

    function _fundVaultThroughDeposit(uint256 amount) private {
        token.mint(address(this), amount);
        token.approve(address(vault), amount);
        vault.depositV2(
            amount,
            "pf1withdrawalbacking",
            bytes32(uint256(0xBEEF)),
            keccak256("ethereum-mainnet-usdc-v1-withdrawal-backing")
        );
    }
}
