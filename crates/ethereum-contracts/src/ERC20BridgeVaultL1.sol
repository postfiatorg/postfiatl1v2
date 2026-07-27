// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.24;

import {IERC20BridgeTokenV2, IPFTLFinalityVerifierV1} from "./ERC20BridgeVaultV2.sol";

/// @notice Proof-native pfUSDC vault for a direct Ethereum L1 deployment
///         (Ethereum Sepolia chain id 11155111 and, later, Ethereum mainnet).
/// @dev Layout reference: ERC20BridgeVaultV2. The L2 variant commits each
///      deposit through ArbSys.sendTxToL1 into an L1 ingress anchor; on a
///      direct L1 deployment that hop does not exist, so the on-chain
///      `ERC20BridgeDepositedV2` event (with block/tx/log coordinates) is the
///      sole ingress evidence consumed by the SP1 pfusdc-eth-ingress program.
///      Withdrawals stay proof-native: PFTLFinalityVerifierV1 consumes one
///      batch-exit Merkle root per PFTL finality proof and rejects replay of
///      withdrawal-id and burn-tx commitments. No signer committee, observer
///      quorum, or challenge window participates in either direction.
contract ERC20BridgeVaultL1 {
    error NotOwner();
    error ZeroAddress(bytes32 field);
    error VaultPaused();
    error InvalidAmount();
    error RecipientTextEmpty();
    error RecipientTextTooLong(uint256 length);
    error RouteBindingRequired();
    error DuplicateDeposit(bytes32 depositId);
    error WithdrawalAlreadyConsumed(bytes32 withdrawalIdCommitment);
    error BurnAlreadyConsumed(bytes32 burnTxIdCommitment);
    error TokenTransferFailed();
    error TokenTransferFromFailed();
    error UnexpectedTokenBalanceDelta(uint256 expected, uint256 actual);
    error InsufficientObligations(uint256 available, uint256 requested);

    event OwnershipTransferred(address indexed previousOwner, address indexed newOwner);
    event PausedSet(bool paused);
    event ERC20BridgeDepositedV2(
        bytes32 indexed depositId,
        address indexed depositor,
        bytes32 indexed pftlRecipientHash,
        string pftlRecipient,
        uint256 amount,
        bytes32 nonce,
        bytes32 routeBinding,
        uint256 sourceChainId,
        address vault,
        address token
    );
    event ProofNativeWithdrawal(
        bytes32 indexed withdrawalIdCommitment,
        bytes32 indexed burnTxIdCommitment,
        bytes32 indexed packetDigest,
        address recipient,
        uint256 amount
    );

    uint256 public constant MAX_PFTL_RECIPIENT_BYTES = 256;
    uint256 public constant MAX_DEPOSIT_AMOUNT = type(uint64).max;

    /// @dev Frozen ingress guest layout:
    ///      slot 0: totalObligations
    ///      slot 1: mapping(bytes32 depositId => DepositRecord)
    ///      The struct occupies four consecutive slots at keccak256(depositId || slot 1).
    ///      Solidity packs depositor into the low 160 bits and amount into the high 96 bits.
    struct DepositRecord {
        address depositor;
        uint96 amount;
        bytes32 recipientHash;
        bytes32 routeBinding;
        bytes32 nonce;
    }

    uint256 public totalObligations;
    mapping(bytes32 => DepositRecord) public depositRecords;

    IERC20BridgeTokenV2 public immutable token;
    IPFTLFinalityVerifierV1 public finalityVerifier;
    bytes32 public immutable tokenRuntimeCodeHash;
    address public owner;
    bool public paused;
    uint256 private reentrancyLock;

    mapping(bytes32 => bool) public depositSeen;
    mapping(bytes32 => bool) public consumedWithdrawalIdCommitment;
    mapping(bytes32 => bool) public consumedBurnTxIdCommitment;

    modifier onlyOwner() {
        if (msg.sender != owner) revert NotOwner();
        _;
    }

    modifier nonReentrant() {
        if (reentrancyLock != 0) revert("reentrant");
        reentrancyLock = 1;
        _;
        reentrancyLock = 0;
    }

    constructor(
        IERC20BridgeTokenV2 token_,
        IPFTLFinalityVerifierV1 finalityVerifier_,
        bytes32 tokenRuntimeCodeHash_,
        address initialOwner
    ) {
        if (address(token_) == address(0)) revert ZeroAddress("token");
        if (address(finalityVerifier_) == address(0)) revert ZeroAddress("finality_verifier");
        if (initialOwner == address(0)) revert ZeroAddress("owner");
        if (tokenRuntimeCodeHash_ == bytes32(0) || address(token_).codehash != tokenRuntimeCodeHash_) {
            revert ZeroAddress("token_code_hash");
        }
        token = token_;
        finalityVerifier = finalityVerifier_;
        tokenRuntimeCodeHash = tokenRuntimeCodeHash_;
        owner = initialOwner;
        emit OwnershipTransferred(address(0), initialOwner);
    }

    function transferOwnership(address newOwner) external onlyOwner {
        if (newOwner == address(0)) revert ZeroAddress("owner");
        emit OwnershipTransferred(owner, newOwner);
        owner = newOwner;
    }

    function setPaused(bool paused_) external onlyOwner {
        paused = paused_;
        emit PausedSet(paused_);
    }

    function depositV2(uint256 amount, string calldata pftlRecipient, bytes32 nonce, bytes32 routeBinding)
        external
        nonReentrant
        returns (bytes32 depositId)
    {
        if (paused) revert VaultPaused();
        if (amount == 0 || amount > MAX_DEPOSIT_AMOUNT) revert InvalidAmount();
        bytes calldata recipientBytes = bytes(pftlRecipient);
        if (recipientBytes.length == 0) revert RecipientTextEmpty();
        if (recipientBytes.length > MAX_PFTL_RECIPIENT_BYTES) revert RecipientTextTooLong(recipientBytes.length);
        if (routeBinding == bytes32(0)) revert RouteBindingRequired();
        bytes32 recipientHash = keccak256(recipientBytes);
        depositId = keccak256(
            abi.encode(
                "postfiat.erc20_bridge.deposit.v2",
                block.chainid,
                address(this),
                address(token),
                msg.sender,
                amount,
                recipientHash,
                nonce,
                routeBinding
            )
        );
        if (depositSeen[depositId]) revert DuplicateDeposit(depositId);
        uint256 beforeBalance = token.balanceOf(address(this));
        if (!token.transferFrom(msg.sender, address(this), amount)) revert TokenTransferFromFailed();
        uint256 received = token.balanceOf(address(this)) - beforeBalance;
        if (received != amount) revert UnexpectedTokenBalanceDelta(amount, received);
        depositSeen[depositId] = true;
        totalObligations += amount;
        depositRecords[depositId] = DepositRecord({
            depositor: msg.sender,
            amount: uint96(amount),
            recipientHash: recipientHash,
            routeBinding: routeBinding,
            nonce: nonce
        });
        _emitDeposit(depositId, recipientHash, pftlRecipient, amount, nonce, routeBinding);
    }

    function _emitDeposit(
        bytes32 depositId,
        bytes32 recipientHash,
        string calldata pftlRecipient,
        uint256 amount,
        bytes32 nonce,
        bytes32 routeBinding
    ) private {
        emit ERC20BridgeDepositedV2(
            depositId,
            msg.sender,
            recipientHash,
            pftlRecipient,
            amount,
            nonce,
            routeBinding,
            block.chainid,
            address(this),
            address(token)
        );
    }

    function withdrawWithProof(bytes calldata publicValues, bytes calldata proofBytes)
        external
        nonReentrant
        returns (bytes32 withdrawalIdCommitment)
    {
        if (paused) revert VaultPaused();
        (
            address recipient,
            uint256 amount,
            bytes32 withdrawalCommitment,
            bytes32 burnCommitment,
            bytes32 packetDigest
        ) = finalityVerifier.verifyAndConsume(publicValues, proofBytes);
        if (amount == 0 || recipient == address(0)) revert InvalidAmount();
        if (amount > totalObligations) revert InsufficientObligations(totalObligations, amount);
        if (consumedWithdrawalIdCommitment[withdrawalCommitment]) {
            revert WithdrawalAlreadyConsumed(withdrawalCommitment);
        }
        if (consumedBurnTxIdCommitment[burnCommitment]) revert BurnAlreadyConsumed(burnCommitment);
        totalObligations -= amount;
        consumedWithdrawalIdCommitment[withdrawalCommitment] = true;
        consumedBurnTxIdCommitment[burnCommitment] = true;

        uint256 vaultBefore = token.balanceOf(address(this));
        uint256 recipientBefore = token.balanceOf(recipient);
        if (!token.transfer(recipient, amount)) revert TokenTransferFailed();
        uint256 vaultDelta = vaultBefore - token.balanceOf(address(this));
        uint256 recipientDelta = token.balanceOf(recipient) - recipientBefore;
        if (vaultDelta != amount) revert UnexpectedTokenBalanceDelta(amount, vaultDelta);
        if (recipientDelta != amount) revert UnexpectedTokenBalanceDelta(amount, recipientDelta);
        emit ProofNativeWithdrawal(withdrawalCommitment, burnCommitment, packetDigest, recipient, amount);
        return withdrawalCommitment;
    }
}
