// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity 0.8.24;

import {IERC20} from "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import {IERC20Permit} from "@openzeppelin/contracts/token/ERC20/extensions/IERC20Permit.sol";
import {SafeERC20} from "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";
import {ReentrancyGuard} from "@openzeppelin/contracts/utils/ReentrancyGuard.sol";

/// @notice The PFTL finality verifier the dormant release path calls; the same
///         interface as `crates/ethereum-contracts/src/ERC20BridgeVaultV2.sol:10-20`.
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

/// @notice The lock contract of section 3 of
///         `docs/specs/pft-erc20-base-bridge-phase2-design-20261026.md`, plus the
///         dormant Phase 3 release function of its section 6.
interface IPFTLockV1 {
    event Locked(
        uint64 indexed nonce,
        address indexed holder,
        bytes32 indexed pftlRecipientHash,
        bytes pftlRecipient,
        uint64 amount
    );
    event Released(
        bytes32 indexed withdrawalIdCommitment,
        bytes32 indexed burnTxIdCommitment,
        bytes32 indexed packetDigest,
        address recipient,
        uint256 amount
    );

    function token() external view returns (address);
    function finalityVerifier() external view returns (address);
    function maxLockAmount() external view returns (uint64);
    function minimumLockAmount() external view returns (uint64);
    function nextNonce() external view returns (uint64);
    function totalLocked() external view returns (uint256);
    function totalReleased() external view returns (uint256);
    function lockRecord(uint64 nonce) external view returns (address holder, uint64 amount, bytes32 pftlRecipientHash);
    function lock(uint64 amount, bytes calldata pftlRecipient) external returns (uint64 nonce);
    function lockWithPermit(
        uint64 amount,
        bytes calldata pftlRecipient,
        uint256 deadline,
        uint8 v,
        bytes32 r,
        bytes32 s
    ) external returns (uint64 nonce);
    function release(bytes calldata publicValues, bytes calldata proofBytes)
        external
        returns (bytes32 withdrawalIdCommitment);
}

/// @title PFTLockV1
/// @notice Locks ERC-20 PFT on Base for native PFT on PFTL (Phase 2). A Lock pulls
///         the tokens with `transferFrom`, takes the next nonce, writes a permanent
///         record, emits one `Locked` event and keeps the tokens.
///
///         No privileged role: there is no owner, admin, pause, setter, sweep or
///         upgrade path. The token, its runtime code hash, the finality verifier,
///         `maxLockAmount` and `minimumLockAmount` are fixed at construction.
///
///         `minimumLockAmount` is at least `PFTL_ACCOUNT_RESERVE`, the PFTL
///         account reserve: PFTL refuses to create an account below it
///         (`below_account_reserve`), so a smaller Lock to a new PFTL account
///         could never be released there.
///
///         Invariants (design note, section 3):
///         1. `totalLocked - totalReleased` is the sum of `Locked` amounts minus
///            releases, and `token.balanceOf(this)` is at least that; the gap is
///            only tokens sent by plain transfer, which no Lock records and
///            nothing can release.
///         2. Nonces run from 0 to `nextNonce - 1`, each with exactly one record
///            and one `Locked` event.
///         3. No function lowers this contract's token balance except `release`.
///         4. `totalLocked` cannot exceed the token's fixed supply while nothing
///            is released (Phase 2); with releases, `totalLocked - totalReleased`
///            cannot, so releases on PFTL cannot exceed the lane.
///
///         The release path is dormant until Phase 3: it pays out only what the
///         immutable `finalityVerifier` accepts, once per withdrawal and burn
///         commitment, and never more than `totalLocked - totalReleased`.
/// @dev Storage layout, for a later storage proof of the records:
///      slot 0 `totalLocked`, slot 1 `totalReleased`, slot 2 `nextNonce`,
///      slot 3 `mapping(uint64 nonce => LockRecord)`: two slots at
///      keccak256(nonce . 3), the first packing holder (low 160 bits) and
///      amount (next 64 bits), the second the recipient hash;
///      slots 4 and 5 the consumed withdrawal and burn commitments.
///      OpenZeppelin's ReentrancyGuard keeps its flag in an ERC-7201 slot.
contract PFTLockV1 is IPFTLockV1, ReentrancyGuard {
    using SafeERC20 for IERC20;

    error ZeroAddress(bytes32 field);
    error NoCode(bytes32 field);
    error TokenCodeHashMismatch(bytes32 expected, bytes32 actual);
    error ZeroMaxLockAmount();
    error MinimumBelowAccountReserve(uint64 minimum, uint64 reserve);
    error MinimumAboveMaxLock(uint64 minimumLockAmount, uint64 maxLockAmount);
    error ZeroAmount();
    error LockBelowMinimum(uint256 amount, uint256 minimum);
    error AmountAboveMaxLock(uint256 amount, uint256 maxLockAmount);
    error RecipientEmpty();
    error RecipientTooLong(uint256 length);
    error PermitFailed(uint256 allowance, uint256 amount);
    error UnexpectedTokenBalanceDelta(uint256 expected, uint256 actual);
    error WithdrawalAlreadyConsumed(bytes32 withdrawalIdCommitment);
    error BurnAlreadyConsumed(bytes32 burnTxIdCommitment);
    error ReleaseAboveLocked(uint256 available, uint256 requested);

    struct LockRecord {
        address holder;
        uint64 amount;
        bytes32 pftlRecipientHash;
    }

    /// The PFTL account reserve in six-decimal units (`ACCOUNT_RESERVE`,
    /// `crates/execution/src/entrypoints.rs`); the floor of `minimumLockAmount`.
    uint64 public constant PFTL_ACCOUNT_RESERVE = 10;

    /// The bound of the vault (`ERC20BridgeVaultL1.sol:55`).
    uint256 private constant MAX_PFTL_RECIPIENT_BYTES = 256;

    uint256 public totalLocked;
    uint256 public totalReleased;
    uint64 public nextNonce;
    mapping(uint64 => LockRecord) private _lockRecords;
    mapping(bytes32 => bool) private _consumedWithdrawalIdCommitment;
    mapping(bytes32 => bool) private _consumedBurnTxIdCommitment;

    address public immutable token;
    address public immutable finalityVerifier;
    uint64 public immutable maxLockAmount;
    uint64 public immutable minimumLockAmount;
    bytes32 private immutable _tokenRuntimeCodeHash;

    /// @param token_ The PFTToken on this chain.
    /// @param finalityVerifier_ The PFTL finality verifier, deployed before this contract.
    /// @param tokenRuntimeCodeHash_ The token's runtime code hash; must equal its code on chain.
    /// @param maxLockAmount_ The genesis per-window release cap (design note, section 5.1).
    /// @param minimumLockAmount_ The smallest Lock: at least `PFTL_ACCOUNT_RESERVE`, at most `maxLockAmount_`.
    constructor(
        address token_,
        address finalityVerifier_,
        bytes32 tokenRuntimeCodeHash_,
        uint64 maxLockAmount_,
        uint64 minimumLockAmount_
    ) {
        if (token_ == address(0)) revert ZeroAddress("token");
        if (finalityVerifier_ == address(0)) revert ZeroAddress("finality_verifier");
        if (token_.code.length == 0) revert NoCode("token");
        if (finalityVerifier_.code.length == 0) revert NoCode("finality_verifier");
        if (token_.codehash != tokenRuntimeCodeHash_) {
            revert TokenCodeHashMismatch(tokenRuntimeCodeHash_, token_.codehash);
        }
        if (maxLockAmount_ == 0) revert ZeroMaxLockAmount();
        if (minimumLockAmount_ < PFTL_ACCOUNT_RESERVE) {
            revert MinimumBelowAccountReserve(minimumLockAmount_, PFTL_ACCOUNT_RESERVE);
        }
        if (minimumLockAmount_ > maxLockAmount_) revert MinimumAboveMaxLock(minimumLockAmount_, maxLockAmount_);
        token = token_;
        finalityVerifier = finalityVerifier_;
        _tokenRuntimeCodeHash = tokenRuntimeCodeHash_;
        maxLockAmount = maxLockAmount_;
        minimumLockAmount = minimumLockAmount_;
    }

    function lockRecord(uint64 nonce) external view returns (address holder, uint64 amount, bytes32 pftlRecipientHash) {
        LockRecord storage record = _lockRecords[nonce];
        return (record.holder, record.amount, record.pftlRecipientHash);
    }

    /// @notice Locks `amount` units, approved beforehand, for `pftlRecipient` on PFTL.
    ///         The contract checks only the recipient's length (1 to 256 bytes);
    ///         a Lock to a wrong PFTL address cannot be undone.
    function lock(uint64 amount, bytes calldata pftlRecipient) external nonReentrant returns (uint64 nonce) {
        bytes32 recipientHash = _checkLock(amount, pftlRecipient);
        return _pullAndRecord(amount, pftlRecipient, recipientHash);
    }

    /// @notice `lock` with an EIP-2612 permit of `amount` to this contract. A permit
    ///         that fails, for example because someone submitted it first, is
    ///         ignored when the allowance already covers `amount` (OpenZeppelin
    ///         `IERC20Permit`, Security Considerations).
    function lockWithPermit(
        uint64 amount,
        bytes calldata pftlRecipient,
        uint256 deadline,
        uint8 v,
        bytes32 r,
        bytes32 s
    ) external nonReentrant returns (uint64 nonce) {
        bytes32 recipientHash = _checkLock(amount, pftlRecipient);
        try IERC20Permit(token).permit(msg.sender, address(this), amount, deadline, v, r, s) {}
        catch {
            uint256 allowed = IERC20(token).allowance(msg.sender, address(this));
            if (allowed < amount) revert PermitFailed(allowed, amount);
        }
        return _pullAndRecord(amount, pftlRecipient, recipientHash);
    }

    /// @notice The dormant Phase 3 release: pays out ERC-20 PFT exported from PFTL,
    ///         as proven to the immutable finality verifier.
    function release(bytes calldata publicValues, bytes calldata proofBytes)
        external
        nonReentrant
        returns (bytes32 withdrawalIdCommitment)
    {
        _checkTokenCode();
        (
            address recipient,
            uint256 amount,
            bytes32 withdrawalCommitment,
            bytes32 burnCommitment,
            bytes32 packetDigest
        ) = IPFTLFinalityVerifierV1(finalityVerifier).verifyAndConsume(publicValues, proofBytes);
        if (amount == 0) revert ZeroAmount();
        if (recipient == address(0)) revert ZeroAddress("recipient");
        if (_consumedWithdrawalIdCommitment[withdrawalCommitment]) {
            revert WithdrawalAlreadyConsumed(withdrawalCommitment);
        }
        if (_consumedBurnTxIdCommitment[burnCommitment]) revert BurnAlreadyConsumed(burnCommitment);
        uint256 available = totalLocked - totalReleased;
        if (amount > available) revert ReleaseAboveLocked(available, amount);
        _consumedWithdrawalIdCommitment[withdrawalCommitment] = true;
        _consumedBurnTxIdCommitment[burnCommitment] = true;
        totalReleased += amount;
        // The only earlier call is to the immutable verifier, under nonReentrant.
        // forge-lint: disable-next-line(reentrancy-events)
        emit Released(withdrawalCommitment, burnCommitment, packetDigest, recipient, amount);

        // The event comes before the transfer; a failed balance check reverts both.
        IERC20 erc20 = IERC20(token);
        uint256 lockBefore = erc20.balanceOf(address(this));
        uint256 recipientBefore = erc20.balanceOf(recipient);
        erc20.safeTransfer(recipient, amount);
        uint256 sent = lockBefore - erc20.balanceOf(address(this));
        uint256 received = erc20.balanceOf(recipient) - recipientBefore;
        if (sent != amount) revert UnexpectedTokenBalanceDelta(amount, sent);
        if (received != amount) revert UnexpectedTokenBalanceDelta(amount, received);
        return withdrawalCommitment;
    }

    function _checkLock(uint64 amount, bytes calldata pftlRecipient) private view returns (bytes32) {
        if (amount == 0) revert ZeroAmount();
        if (amount < minimumLockAmount) revert LockBelowMinimum(amount, minimumLockAmount);
        if (amount > maxLockAmount) revert AmountAboveMaxLock(amount, maxLockAmount);
        if (pftlRecipient.length == 0) revert RecipientEmpty();
        if (pftlRecipient.length > MAX_PFTL_RECIPIENT_BYTES) revert RecipientTooLong(pftlRecipient.length);
        _checkTokenCode();
        return keccak256(pftlRecipient);
    }

    function _checkTokenCode() private view {
        if (token.codehash != _tokenRuntimeCodeHash) {
            revert TokenCodeHashMismatch(_tokenRuntimeCodeHash, token.codehash);
        }
    }

    function _pullAndRecord(uint64 amount, bytes calldata pftlRecipient, bytes32 recipientHash)
        private
        returns (uint64 nonce)
    {
        nonce = nextNonce;
        _lockRecords[nonce] = LockRecord({holder: msg.sender, amount: amount, pftlRecipientHash: recipientHash});
        nextNonce = nonce + 1;
        totalLocked += amount;
        // The only earlier call is lockWithPermit's permit, under nonReentrant.
        // forge-lint: disable-next-line(reentrancy-events)
        emit Locked(nonce, msg.sender, recipientHash, pftlRecipient, amount);

        // Effects and event first, then the pull; a wrong balance delta reverts all of it.
        IERC20 erc20 = IERC20(token);
        uint256 before = erc20.balanceOf(address(this));
        erc20.safeTransferFrom(msg.sender, address(this), amount);
        uint256 received = erc20.balanceOf(address(this)) - before;
        if (received != amount) revert UnexpectedTokenBalanceDelta(amount, received);
    }
}
