// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.24;

import {ERC20} from "@openzeppelin/contracts/token/ERC20/ERC20.sol";
import {ReentrancyGuard} from "@openzeppelin/contracts/utils/ReentrancyGuard.sol";
import {PFTToken} from "../src/PFTToken.sol";
import {IPFTLockV1, PFTLockV1} from "../src/PFTLockV1.sol";

struct VmLog {
    bytes32[] topics;
    bytes data;
    address emitter;
}

interface ILockVm {
    function addr(uint256 privateKey) external pure returns (address);
    function sign(uint256 privateKey, bytes32 digest) external pure returns (uint8 v, bytes32 r, bytes32 s);
    function prank(address sender) external;
    function warp(uint256 timestamp) external;
    function etch(address target, bytes calldata code) external;
    function expectRevert(bytes calldata revertData) external;
    function expectEmit(bool checkTopic1, bool checkTopic2, bool checkTopic3, bool checkData, address emitter) external;
    function recordLogs() external;
    function getRecordedLogs() external returns (VmLog[] memory);
    function isContext(uint8 context) external view returns (bool);
    function exists(string calldata path) external view returns (bool);
    function readFile(string calldata path) external view returns (string memory);
    function parseJsonKeys(string calldata json, string calldata key) external pure returns (string[] memory);
}

/// A stand-in for the PFTL finality verifier: it accepts the proof bytes "proof"
/// and returns what `publicValues` encodes. Unlike the real one it consumes
/// nothing, so the lock's own replay sets are what the tests exercise.
contract MockFinalityVerifier {
    error InvalidProof();

    uint256 public calls;
    bytes32 public lastInput;

    function verifyAndConsume(bytes calldata publicValues, bytes calldata proofBytes)
        external
        returns (address, uint256, bytes32, bytes32, bytes32)
    {
        if (keccak256(proofBytes) != keccak256("proof")) revert InvalidProof();
        calls++;
        lastInput = keccak256(abi.encode(publicValues, proofBytes));
        return abi.decode(publicValues, (address, uint256, bytes32, bytes32, bytes32));
    }
}

/// A token that misbehaves on demand, for the balance-delta and reentrancy checks.
contract MockQuirkyToken is ERC20 {
    uint8 public constant NORMAL = 0;
    uint8 public constant SHORT_PULL = 1;
    uint8 public constant SHORT_PAYOUT = 2;
    uint8 public constant OVERDRAWN_PAYOUT = 3;
    uint8 public constant REENTER = 4;

    uint8 public mode;
    address private target;
    bytes private reentry;

    constructor(uint256 supply) ERC20("Quirky", "QRK") {
        _mint(msg.sender, supply);
    }

    function setMode(uint8 mode_, address target_, bytes calldata reentry_) external {
        (mode, target, reentry) = (mode_, target_, reentry_);
    }

    function transferFrom(address from, address to, uint256 value) public override returns (bool) {
        _maybeReenter();
        if (mode == SHORT_PULL) {
            _spendAllowance(from, msg.sender, value);
            _transfer(from, to, value - 1);
            return true;
        }
        return super.transferFrom(from, to, value);
    }

    function transfer(address to, uint256 value) public override returns (bool) {
        _maybeReenter();
        if (mode == SHORT_PAYOUT) {
            _transfer(msg.sender, to, value - 1);
            _burn(msg.sender, 1);
            return true;
        }
        if (mode == OVERDRAWN_PAYOUT) {
            _transfer(msg.sender, address(0xdead), 1);
        }
        return super.transfer(to, value);
    }

    function _maybeReenter() private {
        if (mode != REENTER) return;
        (bool ok, bytes memory reason) = target.call(reentry);
        if (!ok) {
            assembly ("memory-safe") {
                revert(add(reason, 32), mload(reason))
            }
        }
    }
}

contract PFTLockV1Test {
    ILockVm private constant VM = ILockVm(address(uint160(uint256(keccak256("hevm cheat code")))));

    /// `ForgeContext.Coverage` in forge-std's Vm.
    uint8 private constant COVERAGE = 2;
    string private constant ARTIFACT = "out/PFTLockV1.sol/PFTLockV1.json";
    bytes32 private constant PERMIT_TYPEHASH =
        keccak256("Permit(address owner,address spender,uint256 value,uint256 nonce,uint256 deadline)");

    // Test amounts only; the product owner has not decided the real supply or cap.
    uint256 private constant SUPPLY = 1_000_000_000_000;
    uint64 private constant MAX_LOCK = 20_000_000_000;
    /// The general tests use the smallest minimum the constructor admits; the
    /// reserve minimum has its own tests below.
    uint64 private constant MIN_LOCK = 1;
    uint256 private constant HOLDING = 100_000_000_000;
    address private constant TREASURY = address(0x7EA5);
    address private constant BOB = address(0xB0B);
    address private constant PAYEE = address(0xFEE);
    // Foundry test key for permit signatures; it never holds value outside the test EVM.
    uint256 private constant HOLDER_KEY = 0x10C4;
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
        VM.prank(TREASURY);
        _assertTrue(token.transfer(holder, HOLDING), "fund holder");
        VM.prank(TREASURY);
        _assertTrue(token.transfer(BOB, HOLDING), "fund bob");
        VM.warp(1_000_000);
    }

    // ------------------------------------------------------------ section 8

    function test_lock_pulls_tokens_and_records_nonce() public {
        uint64 amount = 1_500_000;
        _approve(holder, amount);
        VM.expectEmit(true, true, true, true, address(lockC));
        emit IPFTLockV1.Locked(0, holder, keccak256(RECIPIENT), RECIPIENT, amount);
        VM.prank(holder);
        uint64 nonce = lockC.lock(amount, RECIPIENT);

        _assertEq(nonce, 0, "first nonce");
        _assertEq(lockC.nextNonce(), 1, "next nonce");
        _assertEq(lockC.totalLocked(), amount, "total locked");
        _assertEq(lockC.totalReleased(), 0, "total released");
        _assertEq(token.balanceOf(address(lockC)), amount, "lock balance");
        _assertEq(token.balanceOf(holder), HOLDING - amount, "holder balance");
        _assertEq(token.allowance(holder, address(lockC)), 0, "allowance spent");
        _assertRecord(0, holder, amount, RECIPIENT);
    }

    function test_lock_nonces_are_sequential() public {
        _lock(holder, 10, RECIPIENT);
        _lock(BOB, 20, "bob-on-pftl");
        _lock(holder, 30, RECIPIENT);
        _assertEq(lockC.nextNonce(), 3, "next nonce");
        _assertRecord(0, holder, 10, RECIPIENT);
        _assertRecord(1, BOB, 20, "bob-on-pftl");
        _assertRecord(2, holder, 30, RECIPIENT);
        (address none, uint64 nothing, bytes32 noHash) = lockC.lockRecord(3);
        _assertTrue(none == address(0) && nothing == 0 && noHash == bytes32(0), "no record past nextNonce");
    }

    function test_lock_rejects_zero_amount() public {
        _approve(holder, 1);
        VM.expectRevert(abi.encodeWithSelector(PFTLockV1.ZeroAmount.selector));
        VM.prank(holder);
        lockC.lock(0, RECIPIENT);
        _assertUnchanged();
    }

    /// The ABI takes a uint64, so a larger word is refused by the decoder; the
    /// largest uint64 itself goes through when the cap allows it.
    function test_lock_rejects_amount_above_uint64() public {
        uint256 tooLarge = uint256(type(uint64).max) + 1;
        _approve(holder, tooLarge);
        VM.prank(holder);
        (bool ok, bytes memory reason) =
            address(lockC).call(abi.encodeWithSelector(lockC.lock.selector, tooLarge, RECIPIENT));
        _assertTrue(!ok && reason.length == 0, "lock above uint64 decoded");
        VM.prank(holder);
        (ok, reason) =
            address(lockC).call(abi.encodeWithSelector(lockC.lockWithPermit.selector, tooLarge, RECIPIENT, 0, 0, 0, 0));
        _assertTrue(!ok && reason.length == 0, "lockWithPermit above uint64 decoded");
        _assertUnchanged();

        PFTToken big = new PFTToken("Post Fiat Token", "PFT", tooLarge, TREASURY);
        PFTLockV1 wide =
            new PFTLockV1(address(big), address(verifier), address(big).codehash, type(uint64).max, MIN_LOCK);
        VM.prank(TREASURY);
        _assertTrue(big.approve(address(wide), type(uint64).max), "approve");
        VM.prank(TREASURY);
        wide.lock(type(uint64).max, RECIPIENT);
        _assertEq(wide.totalLocked(), type(uint64).max, "uint64 max locked");
    }

    function test_lock_rejects_empty_or_oversized_recipient() public {
        _approve(holder, 100);
        VM.expectRevert(abi.encodeWithSelector(PFTLockV1.RecipientEmpty.selector));
        VM.prank(holder);
        lockC.lock(1, "");
        VM.expectRevert(abi.encodeWithSelector(PFTLockV1.RecipientTooLong.selector, 257));
        VM.prank(holder);
        lockC.lock(1, new bytes(257));
        _assertUnchanged();

        VM.prank(holder);
        lockC.lock(1, new bytes(256));
        VM.prank(holder);
        lockC.lock(1, "r");
        _assertRecord(0, holder, 1, new bytes(256));
        _assertRecord(1, holder, 1, "r");
    }

    function test_lock_with_permit_matches_lock() public {
        uint64 amount = 2_000_000;
        _lock(BOB, amount, RECIPIENT);

        (uint8 v, bytes32 r, bytes32 s) = _signPermit(amount, block.timestamp + 1 hours);
        VM.expectEmit(true, true, true, true, address(lockC));
        emit IPFTLockV1.Locked(1, holder, keccak256(RECIPIENT), RECIPIENT, amount);
        VM.prank(holder);
        uint64 nonce = lockC.lockWithPermit(amount, RECIPIENT, block.timestamp + 1 hours, v, r, s);

        _assertEq(nonce, 1, "permit lock nonce");
        _assertEq(token.nonces(holder), 1, "permit consumed");
        _assertEq(token.allowance(holder, address(lockC)), 0, "allowance spent");
        _assertEq(token.balanceOf(holder), HOLDING - amount, "holder balance");
        _assertEq(token.balanceOf(BOB), HOLDING - amount, "same pull as lock");
        _assertEq(lockC.totalLocked(), 2 * uint256(amount), "total locked");
        _assertRecord(1, holder, amount, RECIPIENT);
    }

    function test_total_locked_equals_sum_of_lock_events() public {
        VM.recordLogs();
        _lock(holder, 7, RECIPIENT);
        _lock(BOB, 11_000_000, "b");
        _lock(holder, MAX_LOCK, RECIPIENT);
        VM.prank(TREASURY);
        _assertTrue(token.transfer(address(lockC), 5), "plain transfer");
        VmLog[] memory logs = VM.getRecordedLogs();

        uint256 sum = 0;
        uint256 count = 0;
        for (uint256 i = 0; i < logs.length; i++) {
            if (logs[i].emitter != address(lockC) || logs[i].topics[0] != IPFTLockV1.Locked.selector) continue;
            _assertEq(uint256(logs[i].topics[1]), count, "event nonce");
            (, uint64 amount) = abi.decode(logs[i].data, (bytes, uint64));
            sum += amount;
            count++;
        }
        _assertEq(count, lockC.nextNonce(), "one event per nonce");
        _assertEq(sum, lockC.totalLocked(), "sum of Locked amounts");
        _assertEq(token.balanceOf(address(lockC)), sum + 5, "balance holds the plain transfer too");
    }

    function test_plain_transfer_is_not_a_lock() public {
        VM.recordLogs();
        VM.prank(holder);
        _assertTrue(token.transfer(address(lockC), 1_000), "plain transfer");
        VmLog[] memory logs = VM.getRecordedLogs();
        for (uint256 i = 0; i < logs.length; i++) {
            _assertTrue(logs[i].emitter != address(lockC), "the lock emitted an event");
        }
        _assertEq(token.balanceOf(address(lockC)), 1_000, "tokens arrived");
        _assertEq(lockC.nextNonce(), 0, "no nonce");
        _assertEq(lockC.totalLocked(), 0, "nothing locked");

        // Nothing can release it: there is no Lock behind it.
        VM.expectRevert(abi.encodeWithSelector(PFTLockV1.ReleaseAboveLocked.selector, 0, 1));
        lockC.release(_values(holder, 1, "w", "b"), "proof");
    }

    function test_release_is_bound_to_immutable_finality_verifier() public {
        _assertTrue(lockC.finalityVerifier() == address(verifier), "verifier");
        _assertTrue(lockC.token() == address(token), "token");
        _assertEq(lockC.maxLockAmount(), MAX_LOCK, "max lock");

        _lock(holder, 5_000, RECIPIENT);
        bytes memory values = _values(PAYEE, 3_000, "w1", "b1");

        // A proof the bound verifier refuses releases nothing.
        VM.expectRevert(abi.encodeWithSelector(MockFinalityVerifier.InvalidProof.selector));
        lockC.release(values, "forged");
        _assertEq(lockC.totalReleased(), 0, "nothing released");

        // Another verifier's acceptance is irrelevant: release only asks the bound one.
        MockFinalityVerifier other = new MockFinalityVerifier();
        other.verifyAndConsume(values, "proof");
        _assertEq(token.balanceOf(PAYEE), 0, "other verifier moved nothing");

        VM.expectEmit(true, true, true, true, address(lockC));
        emit IPFTLockV1.Released(keccak256("w1"), keccak256("b1"), keccak256("digest"), PAYEE, 3_000);
        bytes32 withdrawal = lockC.release(values, "proof");
        _assertTrue(withdrawal == keccak256("w1"), "withdrawal commitment");
        _assertEq(verifier.calls(), 1, "bound verifier asked once");
        _assertTrue(verifier.lastInput() == keccak256(abi.encode(values, bytes("proof"))), "inputs passed through");
        _assertEq(token.balanceOf(PAYEE), 3_000, "recipient paid");
        _assertEq(token.balanceOf(address(lockC)), 2_000, "lock balance");
        _assertEq(lockC.totalReleased(), 3_000, "total released");
        _assertEq(lockC.totalLocked(), 5_000, "total locked unchanged");

        // The constructor refuses a missing verifier; no setter exists (see the ABI test).
        VM.expectRevert(abi.encodeWithSelector(PFTLockV1.ZeroAddress.selector, bytes32("finality_verifier")));
        new PFTLockV1(address(token), address(0), address(token).codehash, MAX_LOCK, MIN_LOCK);
    }

    function test_lock_rejects_amount_above_max_lock_amount() public {
        _approve(holder, uint256(MAX_LOCK) + 1);
        VM.expectRevert(abi.encodeWithSelector(PFTLockV1.AmountAboveMaxLock.selector, uint256(MAX_LOCK) + 1, MAX_LOCK));
        VM.prank(holder);
        lockC.lock(MAX_LOCK + 1, RECIPIENT);
        _assertUnchanged();
        VM.prank(holder);
        lockC.lock(MAX_LOCK, RECIPIENT);
        _assertEq(lockC.totalLocked(), MAX_LOCK, "the cap itself is accepted");
    }

    /// The deployed dispatcher answers exactly the eleven functions of IPFTLockV1;
    /// typical admin entry points revert and move nothing.
    function test_contract_has_no_owner_pause_or_withdraw() public {
        bytes4[11] memory expected = [
            IPFTLockV1.token.selector,
            IPFTLockV1.finalityVerifier.selector,
            IPFTLockV1.maxLockAmount.selector,
            IPFTLockV1.minimumLockAmount.selector,
            IPFTLockV1.nextNonce.selector,
            IPFTLockV1.totalLocked.selector,
            IPFTLockV1.totalReleased.selector,
            IPFTLockV1.lockRecord.selector,
            IPFTLockV1.lock.selector,
            IPFTLockV1.lockWithPermit.selector,
            IPFTLockV1.release.selector
        ];
        bytes4 xor;
        for (uint256 i = 0; i < expected.length; i++) {
            xor ^= expected[i];
        }
        _assertTrue(xor == type(IPFTLockV1).interfaceId, "expected list matches the interface");
        (bytes4[] memory found, uint256 count) = _dispatchedSelectors(address(lockC).code);
        _assertEq(count, expected.length, "number of dispatched selectors");
        for (uint256 i = 0; i < expected.length; i++) {
            bool seen = false;
            for (uint256 j = 0; j < count; j++) {
                seen = seen || found[j] == expected[i];
            }
            _assertTrue(seen, "an interface function is not dispatched");
        }

        _lock(holder, 1_000, RECIPIENT);
        string[12] memory admin = [
            string("owner()"),
            "renounceOwnership()",
            "transferOwnership(address)",
            "paused()",
            "pause()",
            "unpause()",
            "withdraw(uint256)",
            "withdraw(address,uint256)",
            "sweep(address)",
            "rescueTokens(address,address,uint256)",
            "setFinalityVerifier(address)",
            "upgradeToAndCall(address,bytes)"
        ];
        for (uint256 i = 0; i < admin.length; i++) {
            (bool ok,) = address(lockC).call(abi.encodeWithSignature(admin[i], BOB, BOB, 1_000));
            _assertTrue(!ok, string.concat("admin entry point answered: ", admin[i]));
        }
        (bool paid,) = address(lockC).call{value: 0}("");
        _assertTrue(!paid, "fallback answered");
        _assertEq(token.balanceOf(address(lockC)), 1_000, "balance untouched");

        // The compiler's own list; coverage builds do not refresh this artifact.
        if (VM.isContext(COVERAGE)) return;
        _assertTrue(VM.exists(ARTIFACT), "artifact out/PFTLockV1.sol/PFTLockV1.json missing");
        string[] memory signatures = VM.parseJsonKeys(VM.readFile(ARTIFACT), ".methodIdentifiers");
        _assertEq(signatures.length, expected.length, "number of methodIdentifiers");
        for (uint256 i = 0; i < signatures.length; i++) {
            bytes4 selector = bytes4(keccak256(bytes(signatures[i])));
            bool allowed = false;
            for (uint256 j = 0; j < expected.length; j++) {
                allowed = allowed || selector == expected[j];
            }
            _assertTrue(allowed, string.concat("method not in the interface: ", signatures[i]));
        }
    }

    // ------------------------------------------------------- release path

    function test_release_rejects_replayed_commitments() public {
        _lock(holder, 10_000, RECIPIENT);
        lockC.release(_values(PAYEE, 100, "w1", "b1"), "proof");

        VM.expectRevert(abi.encodeWithSelector(PFTLockV1.WithdrawalAlreadyConsumed.selector, keccak256("w1")));
        lockC.release(_values(PAYEE, 100, "w1", "b2"), "proof");
        VM.expectRevert(abi.encodeWithSelector(PFTLockV1.BurnAlreadyConsumed.selector, keccak256("b1")));
        lockC.release(_values(PAYEE, 100, "w2", "b1"), "proof");
        _assertEq(lockC.totalReleased(), 100, "one release only");
        _assertEq(token.balanceOf(PAYEE), 100, "paid once");

        lockC.release(_values(PAYEE, 100, "w2", "b2"), "proof");
        _assertEq(lockC.totalReleased(), 200, "fresh commitments release");
    }

    function test_release_rejects_amount_above_locked_minus_released() public {
        _lock(holder, 1_000, RECIPIENT);
        VM.prank(TREASURY);
        _assertTrue(token.transfer(address(lockC), 500), "plain transfer");

        VM.expectRevert(abi.encodeWithSelector(PFTLockV1.ReleaseAboveLocked.selector, 1_000, 1_001));
        lockC.release(_values(PAYEE, 1_001, "w1", "b1"), "proof");
        lockC.release(_values(PAYEE, 600, "w1", "b1"), "proof");
        VM.expectRevert(abi.encodeWithSelector(PFTLockV1.ReleaseAboveLocked.selector, 400, 401));
        lockC.release(_values(PAYEE, 401, "w2", "b2"), "proof");
        lockC.release(_values(PAYEE, 400, "w2", "b2"), "proof");
        _assertEq(lockC.totalReleased(), lockC.totalLocked(), "everything locked released");
        _assertEq(token.balanceOf(address(lockC)), 500, "the plain transfer stays");
    }

    function test_release_rejects_zero_amount_or_recipient() public {
        _lock(holder, 1_000, RECIPIENT);
        VM.expectRevert(abi.encodeWithSelector(PFTLockV1.ZeroAmount.selector));
        lockC.release(_values(PAYEE, 0, "w1", "b1"), "proof");
        VM.expectRevert(abi.encodeWithSelector(PFTLockV1.ZeroAddress.selector, bytes32("recipient")));
        lockC.release(_values(address(0), 1, "w1", "b1"), "proof");
        _assertEq(lockC.totalReleased(), 0, "nothing released");
    }

    // ------------------------------------------- construction and the token

    // One refused creation per test: forge coverage counts only the first in a test.

    function test_constructor_rejects_zero_token() public {
        VM.expectRevert(abi.encodeWithSelector(PFTLockV1.ZeroAddress.selector, bytes32("token")));
        new PFTLockV1(address(0), address(verifier), address(token).codehash, MAX_LOCK, MIN_LOCK);
    }

    function test_constructor_rejects_token_without_code() public {
        VM.expectRevert(abi.encodeWithSelector(PFTLockV1.NoCode.selector, bytes32("token")));
        new PFTLockV1(BOB, address(verifier), keccak256(""), MAX_LOCK, MIN_LOCK);
    }

    function test_constructor_rejects_verifier_without_code() public {
        VM.expectRevert(abi.encodeWithSelector(PFTLockV1.NoCode.selector, bytes32("finality_verifier")));
        new PFTLockV1(address(token), BOB, address(token).codehash, MAX_LOCK, MIN_LOCK);
    }

    function test_constructor_rejects_wrong_token_code_hash() public {
        bytes32 hash = address(token).codehash;
        VM.expectRevert(abi.encodeWithSelector(PFTLockV1.TokenCodeHashMismatch.selector, bytes32(uint256(1)), hash));
        new PFTLockV1(address(token), address(verifier), bytes32(uint256(1)), MAX_LOCK, MIN_LOCK);
    }

    function test_constructor_rejects_zero_max_lock_amount() public {
        VM.expectRevert(abi.encodeWithSelector(PFTLockV1.ZeroMaxLockAmount.selector));
        new PFTLockV1(address(token), address(verifier), address(token).codehash, 0, MIN_LOCK);
    }

    function test_lock_and_release_refuse_changed_token_code() public {
        _lock(holder, 1_000, RECIPIENT);
        _approve(holder, 1);
        bytes32 hash = address(token).codehash;
        bytes memory original = address(token).code;
        VM.etch(address(token), address(new MockQuirkyToken(1)).code);
        bytes32 changed = address(token).codehash;

        VM.expectRevert(abi.encodeWithSelector(PFTLockV1.TokenCodeHashMismatch.selector, hash, changed));
        VM.prank(holder);
        lockC.lock(1, RECIPIENT);
        VM.expectRevert(abi.encodeWithSelector(PFTLockV1.TokenCodeHashMismatch.selector, hash, changed));
        lockC.release(_values(PAYEE, 1, "w1", "b1"), "proof");

        VM.etch(address(token), original);
        VM.prank(holder);
        lockC.lock(1, RECIPIENT);
        _assertEq(lockC.nextNonce(), 2, "restored code locks again");
    }

    function test_lock_and_release_check_balance_deltas() public {
        MockQuirkyToken quirky = new MockQuirkyToken(1_000_000);
        PFTLockV1 qlock =
            new PFTLockV1(address(quirky), address(verifier), address(quirky).codehash, MAX_LOCK, MIN_LOCK);
        _assertTrue(quirky.approve(address(qlock), type(uint256).max), "approve");
        qlock.lock(1_000, RECIPIENT);

        quirky.setMode(quirky.SHORT_PULL(), address(0), "");
        VM.expectRevert(abi.encodeWithSelector(PFTLockV1.UnexpectedTokenBalanceDelta.selector, 100, 99));
        qlock.lock(100, RECIPIENT);

        quirky.setMode(quirky.SHORT_PAYOUT(), address(0), "");
        VM.expectRevert(abi.encodeWithSelector(PFTLockV1.UnexpectedTokenBalanceDelta.selector, 100, 99));
        qlock.release(_values(PAYEE, 100, "w1", "b1"), "proof");

        quirky.setMode(quirky.OVERDRAWN_PAYOUT(), address(0), "");
        VM.expectRevert(abi.encodeWithSelector(PFTLockV1.UnexpectedTokenBalanceDelta.selector, 100, 101));
        qlock.release(_values(PAYEE, 100, "w1", "b1"), "proof");

        _assertEq(qlock.nextNonce(), 1, "refused lock left no nonce");
        _assertEq(qlock.totalLocked(), 1_000, "refused lock left the total");
        _assertEq(qlock.totalReleased(), 0, "refused releases left the total");
        _assertEq(quirky.balanceOf(address(qlock)), 1_000, "lock balance");
    }

    function test_lock_and_release_are_non_reentrant() public {
        MockQuirkyToken quirky = new MockQuirkyToken(1_000_000);
        PFTLockV1 qlock =
            new PFTLockV1(address(quirky), address(verifier), address(quirky).codehash, MAX_LOCK, MIN_LOCK);
        _assertTrue(quirky.approve(address(qlock), type(uint256).max), "approve");
        qlock.lock(1_000, RECIPIENT);

        bytes memory reenterLock = abi.encodeWithSelector(qlock.lock.selector, uint64(1), RECIPIENT);
        quirky.setMode(quirky.REENTER(), address(qlock), reenterLock);
        VM.expectRevert(abi.encodeWithSelector(ReentrancyGuard.ReentrancyGuardReentrantCall.selector));
        qlock.lock(100, RECIPIENT);
        VM.expectRevert(abi.encodeWithSelector(ReentrancyGuard.ReentrancyGuardReentrantCall.selector));
        qlock.release(_values(PAYEE, 100, "w1", "b1"), "proof");
        _assertEq(qlock.nextNonce(), 1, "no reentrant lock");
    }

    /// Someone submits the holder's permit first: the lock still goes through on
    /// the allowance it set. Without an allowance a failed permit is refused.
    function test_lock_with_permit_survives_front_run_permit() public {
        uint64 amount = 3_000_000;
        uint256 deadline = block.timestamp + 1 hours;
        (uint8 v, bytes32 r, bytes32 s) = _signPermit(amount, deadline);
        VM.prank(BOB);
        token.permit(holder, address(lockC), amount, deadline, v, r, s);

        VM.prank(holder);
        uint64 nonce = lockC.lockWithPermit(amount, RECIPIENT, deadline, v, r, s);
        _assertEq(nonce, 0, "nonce");
        _assertRecord(0, holder, amount, RECIPIENT);
        _assertEq(token.allowance(holder, address(lockC)), 0, "allowance spent");

        VM.expectRevert(abi.encodeWithSelector(PFTLockV1.PermitFailed.selector, 0, amount));
        VM.prank(holder);
        lockC.lockWithPermit(amount, RECIPIENT, deadline, v, r, s);
        _assertEq(lockC.nextNonce(), 1, "no second lock");
    }

    // --------------------------------------------------------------- fuzz

    function testFuzz_lock_records_any_valid_amount_and_recipient(uint64 amount, bytes calldata recipient) public {
        amount = uint64(1 + amount % MAX_LOCK);
        _approve(holder, amount);
        if (recipient.length == 0 || recipient.length > 256) {
            VM.expectRevert(
                recipient.length == 0
                    ? abi.encodeWithSelector(PFTLockV1.RecipientEmpty.selector)
                    : abi.encodeWithSelector(PFTLockV1.RecipientTooLong.selector, recipient.length)
            );
            VM.prank(holder);
            lockC.lock(amount, recipient);
            _assertUnchanged();
            return;
        }
        VM.expectEmit(true, true, true, true, address(lockC));
        emit IPFTLockV1.Locked(0, holder, keccak256(recipient), recipient, amount);
        VM.prank(holder);
        lockC.lock(amount, recipient);
        _assertRecord(0, holder, amount, recipient);
        _assertEq(lockC.totalLocked(), amount, "total locked");
        _assertEq(token.balanceOf(address(lockC)), amount, "lock balance");
        _assertEq(token.balanceOf(holder), HOLDING - amount, "holder balance");
    }

    // ------------------------------------------------------------ helpers

    function _approve(address from, uint256 amount) private {
        VM.prank(from);
        _assertTrue(token.approve(address(lockC), amount), "approve");
    }

    // ------------------------------------------------ the minimum lock amount

    /// The PFTL account reserve (`ACCOUNT_RESERVE`, `crates/execution/src/entrypoints.rs`):
    /// a smaller Lock to a new PFTL account could never be released there.
    uint64 private constant RESERVE_MIN = 10;

    function _reserveLock() private returns (PFTLockV1) {
        return new PFTLockV1(address(token), address(verifier), address(token).codehash, MAX_LOCK, RESERVE_MIN);
    }

    function test_minimum_lock_amount_is_fixed_at_construction() public {
        PFTLockV1 minLock = _reserveLock();
        _assertEq(minLock.minimumLockAmount(), RESERVE_MIN, "minimum getter");
        _assertEq(lockC.minimumLockAmount(), MIN_LOCK, "fixture minimum");
    }

    function test_constructor_rejects_zero_minimum_lock_amount() public {
        VM.expectRevert(abi.encodeWithSelector(PFTLockV1.ZeroMinimumLockAmount.selector));
        new PFTLockV1(address(token), address(verifier), address(token).codehash, MAX_LOCK, 0);
    }

    function test_constructor_rejects_minimum_above_max_lock_amount() public {
        VM.expectRevert(abi.encodeWithSelector(PFTLockV1.MinimumAboveMaxLock.selector, MAX_LOCK + 1, MAX_LOCK));
        new PFTLockV1(address(token), address(verifier), address(token).codehash, MAX_LOCK, MAX_LOCK + 1);
    }

    function test_lock_refuses_amount_below_minimum() public {
        PFTLockV1 minLock = _reserveLock();
        VM.prank(holder);
        _assertTrue(token.approve(address(minLock), RESERVE_MIN), "approve");
        VM.expectRevert(abi.encodeWithSelector(PFTLockV1.LockBelowMinimum.selector, RESERVE_MIN - 1, RESERVE_MIN));
        VM.prank(holder);
        minLock.lock(RESERVE_MIN - 1, RECIPIENT);
        _assertUnchangedLock(minLock);
    }

    function test_lock_with_permit_refuses_amount_below_minimum() public {
        PFTLockV1 minLock = _reserveLock();
        uint256 deadline = block.timestamp + 1 hours;
        (uint8 v, bytes32 r, bytes32 s) = _signPermitFor(address(minLock), RESERVE_MIN - 1, deadline);
        VM.expectRevert(abi.encodeWithSelector(PFTLockV1.LockBelowMinimum.selector, RESERVE_MIN - 1, RESERVE_MIN));
        VM.prank(holder);
        minLock.lockWithPermit(RESERVE_MIN - 1, RECIPIENT, deadline, v, r, s);
        _assertEq(token.nonces(holder), 0, "permit not consumed");
        _assertUnchangedLock(minLock);
    }

    function test_lock_accepts_exactly_the_minimum() public {
        PFTLockV1 minLock = _reserveLock();
        VM.prank(holder);
        _assertTrue(token.approve(address(minLock), RESERVE_MIN), "approve");
        VM.expectEmit(true, true, true, true, address(minLock));
        emit IPFTLockV1.Locked(0, holder, keccak256(RECIPIENT), RECIPIENT, RESERVE_MIN);
        VM.prank(holder);
        _assertEq(minLock.lock(RESERVE_MIN, RECIPIENT), 0, "nonce");
        _assertEq(minLock.totalLocked(), RESERVE_MIN, "total locked");
        _assertEq(token.balanceOf(address(minLock)), RESERVE_MIN, "lock balance");
    }

    function test_lock_with_permit_accepts_exactly_the_minimum() public {
        PFTLockV1 minLock = _reserveLock();
        uint256 deadline = block.timestamp + 1 hours;
        (uint8 v, bytes32 r, bytes32 s) = _signPermitFor(address(minLock), RESERVE_MIN, deadline);
        VM.expectEmit(true, true, true, true, address(minLock));
        emit IPFTLockV1.Locked(0, holder, keccak256(RECIPIENT), RECIPIENT, RESERVE_MIN);
        VM.prank(holder);
        _assertEq(minLock.lockWithPermit(RESERVE_MIN, RECIPIENT, deadline, v, r, s), 0, "nonce");
        _assertEq(token.nonces(holder), 1, "permit consumed");
        _assertEq(minLock.totalLocked(), RESERVE_MIN, "total locked");
    }

    /// Every amount in 1..MAX_LOCK is refused below the minimum and recorded at or above it.
    function testFuzz_minimum_decides_every_amount(uint64 seed) public {
        PFTLockV1 minLock = _reserveLock();
        // Half the runs near the boundary, half across the whole range.
        uint64 amount = seed % 2 == 0 ? 1 + (seed / 2) % (2 * RESERVE_MIN) : 1 + (seed / 2) % MAX_LOCK;
        VM.prank(holder);
        _assertTrue(token.approve(address(minLock), amount), "approve");
        if (amount < RESERVE_MIN) {
            VM.expectRevert(abi.encodeWithSelector(PFTLockV1.LockBelowMinimum.selector, amount, RESERVE_MIN));
            VM.prank(holder);
            minLock.lock(amount, RECIPIENT);
            _assertUnchangedLock(minLock);
        } else {
            VM.prank(holder);
            _assertEq(minLock.lock(amount, RECIPIENT), 0, "nonce");
            (, uint64 recorded,) = minLock.lockRecord(0);
            _assertEq(recorded, amount, "recorded amount");
            _assertEq(minLock.totalLocked(), amount, "total locked");
        }
    }

    function _assertUnchangedLock(PFTLockV1 minLock) private view {
        _assertEq(minLock.nextNonce(), 0, "nonce moved");
        _assertEq(minLock.totalLocked(), 0, "total moved");
        _assertEq(token.balanceOf(address(minLock)), 0, "balance moved");
        _assertEq(token.balanceOf(holder), HOLDING, "holder balance moved");
    }

    function _lock(address from, uint64 amount, bytes memory recipient) private {
        _approve(from, amount);
        VM.prank(from);
        lockC.lock(amount, recipient);
    }

    function _values(address recipient, uint256 amount, bytes memory withdrawal, bytes memory burn)
        private
        pure
        returns (bytes memory)
    {
        return abi.encode(recipient, amount, keccak256(withdrawal), keccak256(burn), keccak256("digest"));
    }

    function _signPermit(uint256 value, uint256 deadline) private view returns (uint8, bytes32, bytes32) {
        return _signPermitFor(address(lockC), value, deadline);
    }

    function _signPermitFor(address spender, uint256 value, uint256 deadline)
        private
        view
        returns (uint8, bytes32, bytes32)
    {
        bytes32 structHash =
            keccak256(abi.encode(PERMIT_TYPEHASH, holder, spender, value, token.nonces(holder), deadline));
        return VM.sign(HOLDER_KEY, keccak256(abi.encodePacked("\x19\x01", token.DOMAIN_SEPARATOR(), structHash)));
    }

    function _assertRecord(uint64 nonce, address expectedHolder, uint64 amount, bytes memory recipient) private view {
        (address recordHolder, uint64 recordAmount, bytes32 recipientHash) = lockC.lockRecord(nonce);
        _assertTrue(recordHolder == expectedHolder, "record holder");
        _assertEq(recordAmount, amount, "record amount");
        _assertTrue(recipientHash == keccak256(recipient), "record recipient hash");
    }

    function _assertUnchanged() private view {
        _assertEq(lockC.nextNonce(), 0, "nonce moved");
        _assertEq(lockC.totalLocked(), 0, "total moved");
        _assertEq(token.balanceOf(address(lockC)), 0, "balance moved");
        _assertEq(token.balanceOf(holder), HOLDING, "holder balance moved");
    }

    /// `PUSHn <selector> EQ` (n = 1..4) in the runtime code without its CBOR
    /// metadata, as in `PFTTokenBoundary.t.sol`.
    function _dispatchedSelectors(bytes memory code) private pure returns (bytes4[] memory found, uint256 count) {
        uint256 end = code.length;
        uint256 metadata = (uint256(uint8(code[end - 2])) << 8) | uint8(code[end - 1]);
        if (metadata + 2 <= end) {
            end -= metadata + 2;
        }
        found = new bytes4[](64);
        for (uint256 i = 0; i < end; i++) {
            uint8 op = uint8(code[i]);
            if (op >= 0x60 && op <= 0x63) {
                uint256 n = op - 0x5f;
                if (i + n + 1 < end && uint8(code[i + n + 1]) == 0x14) {
                    uint32 value = 0;
                    for (uint256 k = 1; k <= n; k++) {
                        value = (value << 8) | uint8(code[i + k]);
                    }
                    found[count++] = bytes4(value);
                }
            }
            if (op >= 0x60 && op <= 0x7f) {
                i += op - 0x5f;
            }
        }
    }

    function _assertTrue(bool value, string memory message) private pure {
        if (!value) revert(message);
    }

    function _assertEq(uint256 actual, uint256 expected, string memory message) private pure {
        if (actual != expected) revert(message);
    }
}
