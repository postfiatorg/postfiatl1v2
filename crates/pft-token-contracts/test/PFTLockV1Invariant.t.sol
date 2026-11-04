// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.24;

import {PFTToken} from "../src/PFTToken.sol";
import {IPFTLockV1, PFTLockV1} from "../src/PFTLockV1.sol";
import {MockFinalityVerifier, VmLog} from "./PFTLockV1.t.sol";

interface ILockInvariantVm {
    function addr(uint256 privateKey) external pure returns (address);
    function sign(uint256 privateKey, bytes32 digest) external pure returns (uint8 v, bytes32 r, bytes32 s);
    function prank(address sender) external;
    function recordLogs() external;
    function getRecordedLogs() external returns (VmLog[] memory);
}

/// Random locks, permit locks (some front-run), and plain transfers to the lock
/// by five holders; the Phase 2 regime, where nothing is released.
contract PFTLockV1Handler {
    ILockInvariantVm internal constant VM = ILockInvariantVm(address(uint160(uint256(keccak256("hevm cheat code")))));
    bytes32 private constant PERMIT_TYPEHASH =
        keccak256("Permit(address owner,address spender,uint256 value,uint256 nonce,uint256 deadline)");
    uint64 public constant MAX_LOCK = 20_000_000_000;
    /// The PFTL account reserve, the deployed minimum (`ACCOUNT_RESERVE`).
    uint64 public constant MIN_LOCK = 10;

    PFTToken public immutable token;
    MockFinalityVerifier public immutable verifier;
    PFTLockV1 public immutable lockC;
    uint256[5] private keys = [uint256(0xB1), 0xB2, 0xB3, 0xB4, 0xB5];
    address[5] internal actors;
    uint256 public lockedEvents;
    uint256 public plainTransferred;
    uint256 public calls;

    /// Counts the `Locked` events the lock emitted during the call.
    modifier counted() {
        VM.recordLogs();
        _;
        VmLog[] memory logs = VM.getRecordedLogs();
        for (uint256 i = 0; i < logs.length; i++) {
            if (logs[i].emitter == address(lockC) && logs[i].topics[0] == IPFTLockV1.Locked.selector) {
                lockedEvents++;
            }
        }
        calls++;
    }

    constructor(uint256 supply) {
        for (uint256 i = 0; i < keys.length; i++) {
            actors[i] = VM.addr(keys[i]);
        }
        token = new PFTToken("Post Fiat Token", "PFT", supply, actors[0]);
        verifier = new MockFinalityVerifier();
        lockC = new PFTLockV1(address(token), address(verifier), address(token).codehash, MAX_LOCK, MIN_LOCK);
        for (uint256 i = 1; i < actors.length; i++) {
            VM.prank(actors[0]);
            require(token.transfer(actors[i], supply / 10), "fund");
        }
    }

    function lock(uint256 actorSeed, uint256 amount, uint256 recipientLength) external counted {
        address holder = actors[actorSeed % actors.length];
        uint64 bounded = _bound(holder, amount);
        if (bounded == 0) return;
        VM.prank(holder);
        require(token.approve(address(lockC), bounded), "approve");
        VM.prank(holder);
        lockC.lock(bounded, _recipient(recipientLength));
    }

    function lockWithPermit(uint256 actorSeed, uint256 amount, uint256 recipientLength, bool frontRun)
        external
        counted
    {
        uint256 i = actorSeed % actors.length;
        address holder = actors[i];
        uint64 bounded = _bound(holder, amount);
        if (bounded == 0) return;
        uint256 deadline = block.timestamp + 1;
        bytes32 digest = keccak256(
            abi.encodePacked(
                "\x19\x01",
                token.DOMAIN_SEPARATOR(),
                keccak256(abi.encode(PERMIT_TYPEHASH, holder, address(lockC), bounded, token.nonces(holder), deadline))
            )
        );
        (uint8 v, bytes32 r, bytes32 s) = VM.sign(keys[i], digest);
        if (frontRun) token.permit(holder, address(lockC), bounded, deadline, v, r, s);
        VM.prank(holder);
        lockC.lockWithPermit(bounded, _recipient(recipientLength), deadline, v, r, s);
    }

    function plainTransfer(uint256 actorSeed, uint256 amount) external counted {
        address holder = actors[actorSeed % actors.length];
        amount = amount % (token.balanceOf(holder) + 1);
        VM.prank(holder);
        require(token.transfer(address(lockC), amount), "transfer");
        plainTransferred += amount;
    }

    function _bound(address holder, uint256 amount) private view returns (uint64) {
        uint256 limit = token.balanceOf(holder);
        if (limit > MAX_LOCK) limit = MAX_LOCK;
        if (limit < MIN_LOCK) return 0;
        return uint64(MIN_LOCK + amount % (limit - MIN_LOCK + 1));
    }

    /// A Lock below the minimum must revert and change nothing.
    function lockBelowMinimum(uint256 actorSeed, uint256 amount) external counted {
        address holder = actors[actorSeed % actors.length];
        uint64 small = uint64(1 + amount % (MIN_LOCK - 1));
        VM.prank(holder);
        require(token.approve(address(lockC), small), "approve");
        uint64 before = lockC.nextNonce();
        VM.prank(holder);
        try lockC.lock(small, "r") {
            revert("a Lock below the minimum was accepted");
        } catch (bytes memory reason) {
            require(
                keccak256(reason)
                    == keccak256(abi.encodeWithSelector(PFTLockV1.LockBelowMinimum.selector, small, MIN_LOCK)),
                "wrong refusal"
            );
        }
        require(lockC.nextNonce() == before, "nonce moved");
    }

    function _recipient(uint256 length) private pure returns (bytes memory recipient) {
        recipient = new bytes(1 + length % 256);
        recipient[0] = "p";
    }
}

/// The Phase 3 regime: the same actions plus releases the mock verifier accepts.
contract PFTLockV1ReleaseHandler is PFTLockV1Handler {
    uint256 public released;
    uint256 private releases;

    constructor(uint256 supply) PFTLockV1Handler(supply) {}

    function release(uint256 recipientSeed, uint256 amount) external counted {
        uint256 available = lockC.totalLocked() - lockC.totalReleased();
        if (available == 0) return;
        amount = 1 + amount % available;
        releases++;
        bytes memory values = abi.encode(
            actors[recipientSeed % actors.length],
            amount,
            keccak256(abi.encode("withdrawal", releases)),
            keccak256(abi.encode("burn", releases)),
            bytes32(releases)
        );
        lockC.release(values, "proof");
        released += amount;
    }
}

abstract contract PFTLockV1InvariantBase {
    uint256 internal constant SUPPLY = 1_000_000_000_000;

    PFTLockV1Handler internal handler;
    PFTLockV1 internal lockC;
    PFTToken internal token;

    /// Only the handler is called; forge reads this like forge-std's targetContract.
    function targetContracts() public view returns (address[] memory targets) {
        targets = new address[](1);
        targets[0] = address(handler);
    }

    /// Invariant 1: the outstanding Locks are backed, and the balance exceeds
    /// them only by plain transfers.
    function invariant_OutstandingIsBackedByBalance() public view {
        uint256 outstanding = lockC.totalLocked() - lockC.totalReleased();
        uint256 balance = token.balanceOf(address(lockC));
        _assertTrue(outstanding <= balance, "totalLocked - totalReleased > balance");
        _assertEq(balance, outstanding + handler.plainTransferred(), "balance = outstanding + plain transfers");
    }

    /// Invariant 2: one `Locked` event and one record per nonce.
    function invariant_NextNonceEqualsLockedEvents() public view {
        uint64 next = lockC.nextNonce();
        _assertEq(next, handler.lockedEvents(), "nextNonce != Locked events");
        uint256 sum = 0;
        for (uint64 nonce = 0; nonce < next; nonce++) {
            (address holder, uint64 amount,) = lockC.lockRecord(nonce);
            _assertTrue(holder != address(0) && amount != 0, "nonce without a record");
            _assertTrue(amount >= lockC.minimumLockAmount(), "a record below the minimum");
            sum += amount;
        }
        (address none,,) = lockC.lockRecord(next);
        _assertTrue(none == address(0), "record past nextNonce");
        _assertEq(sum, lockC.totalLocked(), "records sum to totalLocked");
    }

    /// With fail_on_revert every handler call must succeed; this proves calls were made.
    function afterInvariant() public view {
        _assertTrue(handler.calls() > 0, "the handler made no calls");
    }

    function _assertEq(uint256 actual, uint256 expected, string memory message) internal pure {
        if (actual != expected) revert(message);
    }

    function _assertTrue(bool value, string memory message) internal pure {
        if (!value) revert(message);
    }
}

contract PFTLockV1InvariantTest is PFTLockV1InvariantBase {
    function setUp() public {
        handler = new PFTLockV1Handler(SUPPLY);
        lockC = handler.lockC();
        token = handler.token();
    }

    /// Invariant 4, Phase 2: nothing is released, so totalLocked stays within the supply.
    function invariant_TotalLockedWithinSupply() public view {
        _assertTrue(lockC.totalLocked() <= token.totalSupply(), "totalLocked > totalSupply");
        _assertEq(lockC.totalReleased(), 0, "a Phase 2 release");
    }
}

contract PFTLockV1ReleaseInvariantTest is PFTLockV1InvariantBase {
    PFTLockV1ReleaseHandler private releaseHandler;

    function setUp() public {
        releaseHandler = new PFTLockV1ReleaseHandler(SUPPLY);
        handler = releaseHandler;
        lockC = handler.lockC();
        token = handler.token();
    }

    /// Invariant 4 with releases: released tokens can be locked again, so the
    /// cumulative totalLocked may pass the supply; the outstanding amount cannot.
    function invariant_OutstandingWithinSupply() public view {
        _assertTrue(lockC.totalReleased() <= lockC.totalLocked(), "released more than locked");
        _assertTrue(lockC.totalLocked() - lockC.totalReleased() <= token.totalSupply(), "outstanding > totalSupply");
        _assertEq(lockC.totalReleased(), releaseHandler.released(), "totalReleased = sum of releases");
    }
}
