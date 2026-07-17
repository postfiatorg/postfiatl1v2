// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.24;

interface IERC20BridgeTokenV2 {
    function balanceOf(address account) external view returns (uint256);
    function transfer(address to, uint256 amount) external returns (bool);
    function transferFrom(address from, address to, uint256 amount) external returns (bool);
}

interface IPFTLFinalityVerifierV1 {
    function verifyAndConsume(bytes calldata publicValues, bytes calldata proofBytes)
        external
        returns (
            address recipient,
            uint256 amount,
            bytes32 withdrawalIdCommitment,
            bytes32 burnTxIdCommitment,
            bytes32 packetDigest
        );
}

/// @notice Proof-native pfUSDC vault. No signer committee or challenge window
///         participates in a Tier-4 withdrawal.
contract ERC20BridgeVaultV2 {
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
        if (amount == 0) revert InvalidAmount();
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
        depositSeen[depositId] = true;
        uint256 beforeBalance = token.balanceOf(address(this));
        if (!token.transferFrom(msg.sender, address(this), amount)) revert TokenTransferFromFailed();
        uint256 received = token.balanceOf(address(this)) - beforeBalance;
        if (received != amount) revert UnexpectedTokenBalanceDelta(amount, received);
        _emitDeposit(depositId, recipientHash, pftlRecipient, amount, nonce, routeBinding);
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
        if (consumedWithdrawalIdCommitment[withdrawalCommitment]) {
            revert WithdrawalAlreadyConsumed(withdrawalCommitment);
        }
        if (consumedBurnTxIdCommitment[burnCommitment]) revert BurnAlreadyConsumed(burnCommitment);
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
}
