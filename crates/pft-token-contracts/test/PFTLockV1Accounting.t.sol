// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.24;

import {ERC20Permit} from "@openzeppelin/contracts/token/ERC20/extensions/ERC20Permit.sol";
import {PFTToken} from "../src/PFTToken.sol";
import {PFTLockV1} from "../src/PFTLockV1.sol";
import {MockFinalityVerifier} from "./PFTLockV1.t.sol";

interface IAccountingVm {
    function addr(uint256 privateKey) external pure returns (address);
    function sign(uint256 privateKey, bytes32 digest) external pure returns (uint8 v, bytes32 r, bytes32 s);
    function prank(address sender) external;
    function warp(uint256 timestamp) external;
    function load(address target, bytes32 slot) external view returns (bytes32);
    function expectRevert(bytes calldata revertData) external;
}

/// A verifier that refuses every proof by reverting.
contract RevertingFinalityVerifier {
    error VerifierDown();

    function verifyAndConsume(bytes calldata, bytes calldata)
        external
        pure
        returns (address, uint256, bytes32, bytes32, bytes32)
    {
        revert VerifierDown();
    }
}

/// A verifier whose answer is four words, one short of the five the lock decodes.
contract ShortAnswerFinalityVerifier {
    fallback() external {
        address payee = address(0xFEE);
        bytes32 withdrawal = keccak256("w1");
        bytes32 burn = keccak256("b1");
        assembly ("memory-safe") {
            let ptr := mload(0x40)
            mstore(ptr, payee)
            mstore(add(ptr, 0x20), 100)
            mstore(add(ptr, 0x40), withdrawal)
            mstore(add(ptr, 0x60), burn)
            return(ptr, 0x80)
        }
    }
}

/// Deterministic accounting, release and permit cases asked for by the
/// correctness review of `src/PFTLockV1.sol`; the mock verifier and the amounts
/// are those of `PFTLockV1.t.sol`.
contract PFTLockV1AccountingTest {
    IAccountingVm private constant VM = IAccountingVm(address(uint160(uint256(keccak256("hevm cheat code")))));

    bytes32 private constant PERMIT_TYPEHASH =
        keccak256("Permit(address owner,address spender,uint256 value,uint256 nonce,uint256 deadline)");
    /// Storage slots of the consumed withdrawal and burn commitments (the storage layout in PFTLockV1's natspec).
    uint256 private constant WITHDRAWAL_SLOT = 4;
    uint256 private constant BURN_SLOT = 5;

    // Test amounts only; the product owner has not decided the real supply or cap.
    uint256 private constant SUPPLY = 1_000_000_000_000;
    uint64 private constant MAX_LOCK = 20_000_000_000;
    /// The PFTL account reserve, the smallest minimum the constructor admits.
    uint64 private constant MIN_LOCK = 10;
    uint256 private constant HOLDING = 100_000_000_000;
    uint64 private constant AMOUNT = 3_000_000;
    address private constant TREASURY = address(0x7EA5);
    address private constant BOB = address(0xB0B);
    address private constant PAYEE = address(0xFEE);
    // Foundry test keys for permit signatures; they never hold value outside the test EVM.
    uint256 private constant HOLDER_KEY = 0x10C4;
    uint256 private constant OTHER_KEY = 0x0BAD;
    bytes private constant RECIPIENT = "pftl-recipient-0001";

    PFTToken private token;
    MockFinalityVerifier private verifier;
    PFTLockV1 private lockC;
    address private holder;

    function setUp() public {
        token = new PFTToken("Post Fiat Token", "PFT", SUPPLY, TREASURY);
        verifier = new MockFinalityVerifier();
        lockC = new PFTLockV1(address(token), address(verifier), address(token).codehash, MAX_LOCK, MIN_LOCK);
        holder = VM.addr(HOLDER_KEY);
        VM.warp(1_000_000);
    }

    // ------------------------------------------------ accounting (section 3)

    /// Lock the whole supply, release all of it, lock it again: the cumulative
    /// total passes the supply, the outstanding amount never does.
    function test_release_and_relock_round_trip_stays_within_supply() public {
        uint256 locks = SUPPLY / MAX_LOCK;
        _assertEq(locks * MAX_LOCK, SUPPLY, "the fixture supply is whole locks");
        _approve(TREASURY, lockC, type(uint256).max);

        for (uint256 i = 0; i < locks; i++) {
            VM.prank(TREASURY);
            lockC.lock(MAX_LOCK, RECIPIENT);
            _assertWithinSupply();
        }
        _assertEq(lockC.totalLocked(), SUPPLY, "whole supply locked");
        _assertEq(token.balanceOf(TREASURY), 0, "treasury emptied");

        for (uint256 i = 0; i < locks; i++) {
            lockC.release(_answer(TREASURY, MAX_LOCK, _id("w", i), _id("b", i)), "proof");
            _assertWithinSupply();
        }
        _assertEq(lockC.totalReleased(), SUPPLY, "whole supply released");
        _assertEq(token.balanceOf(address(lockC)), 0, "lock emptied");
        _assertEq(token.balanceOf(TREASURY), SUPPLY, "treasury repaid");

        for (uint256 i = 0; i < locks; i++) {
            VM.prank(TREASURY);
            lockC.lock(MAX_LOCK, RECIPIENT);
            _assertWithinSupply();
        }
        _assertEq(lockC.totalLocked(), 2 * SUPPLY, "total locked is twice the supply");
        _assertEq(lockC.totalReleased(), SUPPLY, "total released is the supply");
        _assertEq(token.balanceOf(address(lockC)), SUPPLY, "lock balance is the supply");
        _assertEq(lockC.nextNonce(), 2 * locks, "one nonce per lock");
    }

    /// `amount` is a uint64 per Lock; the totals are uint256 and add up past it.
    function test_locks_aggregate_beyond_uint64() public {
        uint64 max = type(uint64).max;
        uint256 count = 3;
        PFTToken big = new PFTToken("Post Fiat Token", "PFT", count * max, TREASURY);
        PFTLockV1 wide = new PFTLockV1(address(big), address(verifier), address(big).codehash, max, MIN_LOCK);
        VM.prank(TREASURY);
        _assertTrue(big.approve(address(wide), type(uint256).max), "approve");

        for (uint256 i = 0; i < count; i++) {
            VM.prank(TREASURY);
            _assertEq(wide.lock(max, RECIPIENT), i, "nonce");
            (address recordHolder, uint64 recordAmount, bytes32 recipientHash) = wide.lockRecord(uint64(i));
            _assertTrue(recordHolder == TREASURY, "record holder");
            _assertEq(recordAmount, max, "record amount");
            _assertTrue(recipientHash == keccak256(RECIPIENT), "record recipient hash");
        }
        _assertTrue(wide.totalLocked() > max, "total locked passes uint64");
        _assertEq(wide.totalLocked(), count * max, "total locked");
        _assertEq(big.balanceOf(address(wide)), count * max, "lock balance");
        _assertEq(wide.nextNonce(), count, "next nonce");

        // A release amount is uint256 and may itself exceed uint64.
        uint256 amount = 2 * uint256(max);
        wide.release(_answer(PAYEE, amount, _id("w", 0), _id("b", 0)), "proof");
        _assertEq(wide.totalReleased(), amount, "total released passes uint64");
        _assertEq(wide.totalLocked() - wide.totalReleased(), max, "outstanding");
        _assertEq(big.balanceOf(PAYEE), amount, "recipient paid");
    }

    // ---------------------------------------------------------- release path

    function test_oversized_release_consumes_no_commitment() public {
        _lockFrom(TREASURY, 1_000);
        lockC.release(_answer(PAYEE, 400, _id("w", 1), _id("b", 1)), "proof");

        bytes32 w = _id("w", 2);
        bytes32 b = _id("b", 2);
        VM.expectRevert(abi.encodeWithSelector(PFTLockV1.ReleaseAboveLocked.selector, 600, 601));
        lockC.release(_answer(PAYEE, 601, w, b), "proof");
        _assertNotConsumed(lockC, w, b);
        _assertEq(lockC.totalReleased(), 400, "total released unchanged");

        lockC.release(_answer(PAYEE, 600, w, b), "proof");
        _assertTrue(_consumed(lockC, WITHDRAWAL_SLOT, w) && _consumed(lockC, BURN_SLOT, b), "consumed now");
        _assertEq(lockC.totalReleased(), 1_000, "everything released");
        _assertEq(token.balanceOf(PAYEE), 1_000, "recipient paid");
    }

    /// Current behaviour, pinned so that changing it is a conscious decision: a
    /// zero withdrawal or burn commitment from the verifier is accepted once and
    /// bytes32(0) is then consumed in that set.
    function test_zero_commitments_are_accepted_once() public {
        _lockFrom(TREASURY, 1_000);

        bytes32 returned = lockC.release(_answer(PAYEE, 100, bytes32(0), _id("b", 1)), "proof");
        _assertTrue(returned == bytes32(0), "zero withdrawal commitment returned");
        _assertTrue(_consumed(lockC, WITHDRAWAL_SLOT, bytes32(0)), "zero withdrawal consumed");
        VM.expectRevert(abi.encodeWithSelector(PFTLockV1.WithdrawalAlreadyConsumed.selector, bytes32(0)));
        lockC.release(_answer(PAYEE, 100, bytes32(0), _id("b", 2)), "proof");

        lockC.release(_answer(PAYEE, 100, _id("w", 2), bytes32(0)), "proof");
        _assertTrue(_consumed(lockC, BURN_SLOT, bytes32(0)), "zero burn consumed");
        VM.expectRevert(abi.encodeWithSelector(PFTLockV1.BurnAlreadyConsumed.selector, bytes32(0)));
        lockC.release(_answer(PAYEE, 100, _id("w", 3), bytes32(0)), "proof");

        _assertEq(lockC.totalReleased(), 200, "two releases");
        _assertEq(token.balanceOf(PAYEE), 200, "paid twice");
    }

    function test_release_with_reverting_verifier_changes_nothing() public {
        PFTLockV1 broken = _lockWith(address(new RevertingFinalityVerifier()));
        bytes32 before = _snapshot(broken);
        VM.expectRevert(abi.encodeWithSelector(RevertingFinalityVerifier.VerifierDown.selector));
        broken.release(_answer(PAYEE, 100, keccak256("w1"), keccak256("b1")), "proof");
        _assertTrue(_snapshot(broken) == before, "state changed");
        _assertNotConsumed(broken, keccak256("w1"), keccak256("b1"));
    }

    function test_release_with_short_verifier_answer_changes_nothing() public {
        PFTLockV1 broken = _lockWith(address(new ShortAnswerFinalityVerifier()));
        bytes32 before = _snapshot(broken);
        // The ABI decoder refuses the four-word answer with empty revert data.
        (bool ok, bytes memory reason) = address(broken)
            .call(abi.encodeCall(PFTLockV1.release, (_answer(PAYEE, 100, keccak256("w1"), keccak256("b1")), "proof")));
        _assertTrue(!ok && reason.length == 0, "short answer decoded");
        _assertTrue(_snapshot(broken) == before, "state changed");
        _assertNotConsumed(broken, keccak256("w1"), keccak256("b1"));
    }

    /// A proof naming the lock itself as recipient moves nothing, so the balance
    /// check refuses it and no commitment is consumed. The proof fixes the
    /// recipient: resubmitting it fails the same way, never as a replay, so such
    /// an export can never be paid on Base (design note, section 6).
    function test_release_to_the_lock_itself_is_refused() public {
        _lockFrom(TREASURY, 1_000);
        bytes memory values = _answer(address(lockC), 100, keccak256("w1"), keccak256("b1"));

        for (uint256 attempt = 0; attempt < 2; attempt++) {
            VM.expectRevert(abi.encodeWithSelector(PFTLockV1.UnexpectedTokenBalanceDelta.selector, 100, 0));
            lockC.release(values, "proof");
        }
        _assertNotConsumed(lockC, keccak256("w1"), keccak256("b1"));
        _assertEq(lockC.totalReleased(), 0, "nothing released");
        _assertEq(token.balanceOf(address(lockC)), 1_000, "lock balance");
    }

    // ------------------------------------------------------------- permits

    /// The deadline bounds the permit, not the lock: an expired permit falls
    /// back to the allowance already given.
    function test_lock_with_expired_permit_uses_sufficient_allowance() public {
        _fund(holder);
        uint256 deadline = block.timestamp - 1;
        (uint8 v, bytes32 r, bytes32 s) = _signPermit(HOLDER_KEY, address(lockC), deadline);
        _approve(holder, lockC, AMOUNT);

        VM.prank(holder);
        _assertEq(lockC.lockWithPermit(AMOUNT, RECIPIENT, deadline, v, r, s), 0, "nonce");
        _assertEq(token.nonces(holder), 0, "permit not used");
        _assertEq(token.allowance(holder, address(lockC)), 0, "allowance spent");
        _assertEq(lockC.totalLocked(), AMOUNT, "total locked");
        (address recordHolder, uint64 recordAmount,) = lockC.lockRecord(0);
        _assertTrue(recordHolder == holder && recordAmount == AMOUNT, "record");
    }

    function test_lock_with_expired_permit_and_insufficient_allowance_reverts() public {
        _fund(holder);
        uint256 deadline = block.timestamp - 1;
        (uint8 v, bytes32 r, bytes32 s) = _signPermit(HOLDER_KEY, address(lockC), deadline);
        _approve(holder, lockC, AMOUNT - 1);

        VM.expectRevert(abi.encodeWithSelector(PFTLockV1.PermitFailed.selector, AMOUNT - 1, AMOUNT));
        VM.prank(holder);
        lockC.lockWithPermit(AMOUNT, RECIPIENT, deadline, v, r, s);
        _assertEq(token.allowance(holder, address(lockC)), AMOUNT - 1, "allowance untouched");
        _assertNothingLocked();
    }

    function test_lock_with_permit_of_another_signer_reverts() public {
        _fund(holder);
        uint256 deadline = block.timestamp + 1 hours;
        (uint8 v, bytes32 r, bytes32 s) = _signPermit(OTHER_KEY, address(lockC), deadline);

        VM.expectRevert(abi.encodeWithSelector(ERC20Permit.ERC2612InvalidSigner.selector, VM.addr(OTHER_KEY), holder));
        token.permit(holder, address(lockC), AMOUNT, deadline, v, r, s);
        _assertPermitRefusedByLock(deadline, v, r, s);
    }

    function test_lock_with_permit_of_another_signer_uses_sufficient_allowance() public {
        _fund(holder);
        uint256 deadline = block.timestamp + 1 hours;
        (uint8 v, bytes32 r, bytes32 s) = _signPermit(OTHER_KEY, address(lockC), deadline);
        _assertLockedWithAllowance(deadline, v, r, s);
    }

    function test_lock_with_permit_for_another_spender_reverts() public {
        _fund(holder);
        uint256 deadline = block.timestamp + 1 hours;
        (uint8 v, bytes32 r, bytes32 s) = _signPermit(HOLDER_KEY, BOB, deadline);

        address recovered = ecrecover(_permitDigest(address(lockC), deadline), v, r, s);
        VM.expectRevert(abi.encodeWithSelector(ERC20Permit.ERC2612InvalidSigner.selector, recovered, holder));
        token.permit(holder, address(lockC), AMOUNT, deadline, v, r, s);
        _assertPermitRefusedByLock(deadline, v, r, s);
    }

    function test_lock_with_permit_for_another_spender_uses_sufficient_allowance() public {
        _fund(holder);
        uint256 deadline = block.timestamp + 1 hours;
        (uint8 v, bytes32 r, bytes32 s) = _signPermit(HOLDER_KEY, BOB, deadline);
        _assertLockedWithAllowance(deadline, v, r, s);
    }

    function test_lock_with_expired_permit_and_no_allowance_reverts() public {
        _fund(holder);
        uint256 deadline = block.timestamp - 1;
        (uint8 v, bytes32 r, bytes32 s) = _signPermit(HOLDER_KEY, address(lockC), deadline);

        VM.expectRevert(abi.encodeWithSelector(ERC20Permit.ERC2612ExpiredSignature.selector, deadline));
        token.permit(holder, address(lockC), AMOUNT, deadline, v, r, s);
        _assertPermitRefusedByLock(deadline, v, r, s);
    }

    // ------------------------------------------------------------ helpers

    function _fund(address to) private {
        VM.prank(TREASURY);
        _assertTrue(token.transfer(to, HOLDING), "fund");
    }

    function _approve(address from, PFTLockV1 spender, uint256 amount) private {
        VM.prank(from);
        _assertTrue(token.approve(address(spender), amount), "approve");
    }

    function _lockFrom(address from, uint64 amount) private {
        _approve(from, lockC, amount);
        VM.prank(from);
        lockC.lock(amount, RECIPIENT);
    }

    /// A lock bound to `verifier_`, holding 1_000 locked units.
    function _lockWith(address verifier_) private returns (PFTLockV1 bound) {
        bound = new PFTLockV1(address(token), verifier_, address(token).codehash, MAX_LOCK, MIN_LOCK);
        _approve(TREASURY, bound, 1_000);
        VM.prank(TREASURY);
        bound.lock(1_000, RECIPIENT);
    }

    function _answer(address recipient, uint256 amount, bytes32 withdrawal, bytes32 burn)
        private
        pure
        returns (bytes memory)
    {
        return abi.encode(recipient, amount, withdrawal, burn, keccak256("digest"));
    }

    function _id(string memory kind, uint256 i) private pure returns (bytes32) {
        return keccak256(abi.encodePacked(kind, i));
    }

    function _permitDigest(address spender, uint256 deadline) private view returns (bytes32) {
        bytes32 structHash =
            keccak256(abi.encode(PERMIT_TYPEHASH, holder, spender, uint256(AMOUNT), token.nonces(holder), deadline));
        return keccak256(abi.encodePacked("\x19\x01", token.DOMAIN_SEPARATOR(), structHash));
    }

    function _signPermit(uint256 key, address spender, uint256 deadline)
        private
        view
        returns (uint8, bytes32, bytes32)
    {
        return VM.sign(key, _permitDigest(spender, deadline));
    }

    function _consumed(PFTLockV1 target, uint256 slot, bytes32 commitment) private view returns (bool) {
        return VM.load(address(target), keccak256(abi.encode(commitment, slot))) != bytes32(0);
    }

    function _snapshot(PFTLockV1 target) private view returns (bytes32) {
        return keccak256(
            abi.encode(
                target.totalLocked(),
                target.totalReleased(),
                target.nextNonce(),
                token.balanceOf(address(target)),
                token.balanceOf(PAYEE)
            )
        );
    }

    function _assertWithinSupply() private view {
        uint256 outstanding = lockC.totalLocked() - lockC.totalReleased();
        _assertTrue(outstanding <= token.totalSupply(), "outstanding above the supply");
        _assertTrue(outstanding <= token.balanceOf(address(lockC)), "outstanding above the balance");
    }

    function _assertNotConsumed(PFTLockV1 target, bytes32 withdrawal, bytes32 burn) private view {
        _assertTrue(!_consumed(target, WITHDRAWAL_SLOT, withdrawal), "withdrawal commitment consumed");
        _assertTrue(!_consumed(target, BURN_SLOT, burn), "burn commitment consumed");
    }

    /// The token refused the permit; without an allowance the lock refuses too.
    function _assertPermitRefusedByLock(uint256 deadline, uint8 v, bytes32 r, bytes32 s) private {
        VM.expectRevert(abi.encodeWithSelector(PFTLockV1.PermitFailed.selector, 0, AMOUNT));
        VM.prank(holder);
        lockC.lockWithPermit(AMOUNT, RECIPIENT, deadline, v, r, s);
        _assertEq(token.nonces(holder), 0, "permit nonce moved");
        _assertNothingLocked();
    }

    /// The token refuses the permit; the lock falls back to the allowance already
    /// given and spends exactly the amount of it.
    function _assertLockedWithAllowance(uint256 deadline, uint8 v, bytes32 r, bytes32 s) private {
        _approve(holder, lockC, 2 * uint256(AMOUNT));

        VM.prank(holder);
        _assertEq(lockC.lockWithPermit(AMOUNT, RECIPIENT, deadline, v, r, s), 0, "nonce");
        _assertEq(token.nonces(holder), 0, "permit nonce moved");
        _assertEq(token.allowance(holder, address(lockC)), AMOUNT, "allowance spent by the amount");
        _assertEq(lockC.nextNonce(), 1, "next nonce");
        _assertEq(lockC.totalLocked(), AMOUNT, "total locked");
        _assertEq(lockC.totalReleased(), 0, "total released");
        _assertEq(token.balanceOf(address(lockC)), AMOUNT, "lock balance");
        _assertEq(token.balanceOf(holder), HOLDING - AMOUNT, "holder balance");
        (address recordHolder, uint64 recordAmount,) = lockC.lockRecord(0);
        _assertTrue(recordHolder == holder && recordAmount == AMOUNT, "record");
    }

    function _assertNothingLocked() private view {
        _assertEq(lockC.nextNonce(), 0, "nonce moved");
        _assertEq(lockC.totalLocked(), 0, "total moved");
        _assertEq(token.balanceOf(address(lockC)), 0, "balance moved");
        _assertEq(token.balanceOf(holder), HOLDING, "holder balance moved");
    }

    function _assertTrue(bool value, string memory message) private pure {
        if (!value) revert(message);
    }

    function _assertEq(uint256 actual, uint256 expected, string memory message) private pure {
        if (actual != expected) revert(message);
    }
}
