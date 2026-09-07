// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.24;

import {ExitExecutorV1} from "../src/ExitExecutorV1.sol";

interface Vm {
    struct SignedDelegation {
        uint8 v;
        bytes32 r;
        bytes32 s;
        uint64 nonce;
        address implementation;
    }

    function addr(uint256 privateKey) external pure returns (address);
    function sign(uint256 privateKey, bytes32 digest) external pure returns (uint8 v, bytes32 r, bytes32 s);
    function signAndAttachDelegation(address implementation, uint256 privateKey)
        external
        returns (SignedDelegation memory);
    function deal(address who, uint256 newBalance) external;
    function prank(address msgSender) external;
    function warp(uint256 newTimestamp) external;
    function expectRevert(bytes calldata revertData) external;
    function expectRevert(bytes4 revertData) external;
    function expectPartialRevert(bytes4 revertData) external;
    function load(address target, bytes32 slot) external view returns (bytes32);
}

/// Minimal WETH9 semantics: deposit/withdraw/transfer.
contract MockWETH {
    mapping(address => uint256) public balanceOf;

    function deposit() external payable {
        balanceOf[msg.sender] += msg.value;
    }

    function withdraw(uint256 wad) external {
        balanceOf[msg.sender] -= wad;
        (bool ok,) = msg.sender.call{value: wad}("");
        require(ok, "weth: send");
    }

    function transfer(address to, uint256 wad) external returns (bool) {
        balanceOf[msg.sender] -= wad;
        balanceOf[to] += wad;
        return true;
    }
}

/// Stand-in for a venue L1 deposit contract that takes native ETH.
contract MockVenue {
    mapping(address => uint256) public credited;

    function depositETH(address account) external payable {
        credited[account] += msg.value;
    }
}

contract Reverter {
    error Nope(uint256 code);

    function fail(uint256 code) external pure {
        revert Nope(code);
    }
}

contract ExitExecutorV1Test {
    Vm constant vm = Vm(address(uint160(uint256(keccak256("hevm cheat code")))));

    uint256 constant PK = 0xA11CE;
    uint256 constant OTHER_PK = 0xB0B;

    ExitExecutorV1 impl;
    MockWETH weth;
    MockVenue venue;
    address eoa;
    address relayer = address(0xBEEF);

    function setUp() public {
        impl = new ExitExecutorV1();
        weth = new MockWETH();
        venue = new MockVenue();
        eoa = vm.addr(PK);
        vm.signAndAttachDelegation(address(impl), PK);
        // Vault release: EOA receives WETH, holds zero ETH.
        vm.deal(address(this), 100 ether);
        weth.deposit{value: 10 ether}();
        weth.transfer(eoa, 10 ether);
        require(eoa.balance == 0, "eoa must start with zero ETH");
    }

    function _exec(address account) internal pure returns (ExitExecutorV1) {
        return ExitExecutorV1(payable(account));
    }

    function _lighterBatch(uint256 amount) internal view returns (ExitExecutorV1.Call[] memory calls) {
        calls = new ExitExecutorV1.Call[](2);
        calls[0] = ExitExecutorV1.Call({to: address(weth), value: 0, data: abi.encodeCall(MockWETH.withdraw, (amount))});
        calls[1] =
            ExitExecutorV1.Call({to: address(venue), value: amount, data: abi.encodeCall(MockVenue.depositETH, (eoa))});
    }

    function _sign(uint256 pk, ExitExecutorV1.Call[] memory calls, uint256 nonce, uint256 deadline)
        internal
        view
        returns (bytes memory)
    {
        bytes32 d = _exec(eoa).digest(calls, nonce, deadline);
        (uint8 v, bytes32 r, bytes32 s) = vm.sign(pk, d);
        return abi.encodePacked(r, s, v);
    }

    function test_delegation_attached() public view {
        require(eoa.code.length == 23, "7702 designator expected");
        require(_exec(eoa).implementation() == address(impl), "impl mismatch");
        require(_exec(eoa).nonce() == 0, "nonce 0");
    }

    function test_sponsored_unwrap_and_deposit_from_zero_gas_eoa() public {
        ExitExecutorV1.Call[] memory calls = _lighterBatch(10 ether);
        bytes memory sig = _sign(PK, calls, 0, block.timestamp + 1 hours);

        vm.prank(relayer);
        _exec(eoa).execute(calls, 0, block.timestamp + 1 hours, sig);

        require(weth.balanceOf(eoa) == 0, "weth not unwrapped");
        require(venue.credited(eoa) == 10 ether, "venue not credited");
        require(eoa.balance == 0, "eoa should hold no residual ETH");
        require(_exec(eoa).nonce() == 1, "nonce not advanced");
    }

    function test_replay_rejected() public {
        ExitExecutorV1.Call[] memory calls = _lighterBatch(1 ether);
        uint256 deadline = block.timestamp + 1 hours;
        bytes memory sig = _sign(PK, calls, 0, deadline);
        vm.prank(relayer);
        _exec(eoa).execute(calls, 0, deadline, sig);

        vm.expectRevert(abi.encodeWithSelector(ExitExecutorV1.BadNonce.selector, 1, 0));
        vm.prank(relayer);
        _exec(eoa).execute(calls, 0, deadline, sig);
    }

    function test_wrong_signer_rejected() public {
        ExitExecutorV1.Call[] memory calls = _lighterBatch(1 ether);
        uint256 deadline = block.timestamp + 1 hours;
        bytes memory sig = _sign(OTHER_PK, calls, 0, deadline);
        vm.expectRevert(abi.encodeWithSelector(ExitExecutorV1.BadSigner.selector, vm.addr(OTHER_PK)));
        vm.prank(relayer);
        _exec(eoa).execute(calls, 0, deadline, sig);
    }

    function test_tampered_batch_rejected() public {
        ExitExecutorV1.Call[] memory calls = _lighterBatch(1 ether);
        uint256 deadline = block.timestamp + 1 hours;
        bytes memory sig = _sign(PK, calls, 0, deadline);
        // Relayer redirects the deposit to itself.
        calls[1].data = abi.encodeCall(MockVenue.depositETH, (relayer));
        vm.prank(relayer);
        // Recovered signer is some other address; exact value is not knowable
        // ahead of time, so match on selector only.
        vm.expectPartialRevert(ExitExecutorV1.BadSigner.selector);
        _exec(eoa).execute(calls, 0, deadline, sig);
    }

    function test_expired_rejected() public {
        ExitExecutorV1.Call[] memory calls = _lighterBatch(1 ether);
        uint256 deadline = block.timestamp + 10;
        bytes memory sig = _sign(PK, calls, 0, deadline);
        vm.warp(deadline + 1);
        vm.expectRevert(abi.encodeWithSelector(ExitExecutorV1.Expired.selector, deadline, deadline + 1));
        vm.prank(relayer);
        _exec(eoa).execute(calls, 0, deadline, sig);
    }

    function test_failed_call_reverts_whole_batch_with_reason() public {
        Reverter r = new Reverter();
        ExitExecutorV1.Call[] memory calls = new ExitExecutorV1.Call[](2);
        calls[0] =
            ExitExecutorV1.Call({to: address(weth), value: 0, data: abi.encodeCall(MockWETH.withdraw, (1 ether))});
        calls[1] = ExitExecutorV1.Call({to: address(r), value: 0, data: abi.encodeCall(Reverter.fail, (7))});
        uint256 deadline = block.timestamp + 1 hours;
        bytes memory sig = _sign(PK, calls, 0, deadline);
        bytes memory inner = abi.encodeWithSelector(Reverter.Nope.selector, 7);
        vm.expectRevert(abi.encodeWithSelector(ExitExecutorV1.CallFailed.selector, 1, address(r), inner));
        vm.prank(relayer);
        _exec(eoa).execute(calls, 0, deadline, sig);
        // Atomic: unwrap must not have persisted.
        require(weth.balanceOf(eoa) == 10 ether, "batch was not atomic");
    }

    function test_execute_self_requires_self() public {
        ExitExecutorV1.Call[] memory calls = _lighterBatch(1 ether);
        vm.expectRevert(ExitExecutorV1.NotSelf.selector);
        vm.prank(relayer);
        _exec(eoa).executeSelf(calls);
    }

    function test_execute_self_from_eoa() public {
        ExitExecutorV1.Call[] memory calls = _lighterBatch(2 ether);
        vm.prank(eoa);
        _exec(eoa).executeSelf(calls);
        require(venue.credited(eoa) == 2 ether, "self path failed");
        require(_exec(eoa).nonce() == 0, "self path must not touch nonce");
    }

    function test_signature_bound_to_chain_and_account() public {
        // Same batch signed for a different EOA must not verify here.
        uint256 pk2 = 0xC0FFEE;
        address eoa2 = vm.addr(pk2);
        vm.signAndAttachDelegation(address(impl), pk2);
        ExitExecutorV1.Call[] memory calls = _lighterBatch(1 ether);
        uint256 deadline = block.timestamp + 1 hours;
        bytes32 d2 = _exec(eoa2).digest(calls, 0, deadline);
        bytes32 d1 = _exec(eoa).digest(calls, 0, deadline);
        require(d1 != d2, "digest must bind to account");
        (uint8 v, bytes32 r, bytes32 s) = vm.sign(pk2, d2);
        bytes memory sig = abi.encodePacked(r, s, v);
        vm.expectPartialRevert(ExitExecutorV1.BadSigner.selector);
        vm.prank(relayer);
        _exec(eoa).execute(calls, 0, deadline, sig);
    }

    function test_high_s_rejected() public {
        ExitExecutorV1.Call[] memory calls = _lighterBatch(1 ether);
        uint256 deadline = block.timestamp + 1 hours;
        bytes32 d = _exec(eoa).digest(calls, 0, deadline);
        (uint8 v, bytes32 r, bytes32 s) = vm.sign(PK, d);
        // Flip to the high-s twin.
        bytes32 n = 0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEBAAEDCE6AF48A03BBFD25E8CD0364141;
        bytes32 s2 = bytes32(uint256(n) - uint256(s));
        uint8 v2 = v == 27 ? 28 : 27;
        bytes memory sig = abi.encodePacked(r, s2, v2);
        vm.expectRevert(ExitExecutorV1.BadSignatureS.selector);
        vm.prank(relayer);
        _exec(eoa).execute(calls, 0, deadline, sig);
    }

    event DigestFixture(uint256 chainId, address impl, address eoa, bytes32 digest);

    /// Emits a fixture the Python signer test (StakeHub
    /// tests/test_shielded_exit.py) must reproduce byte-for-byte.
    function test_digest_fixture() public {
        ExitExecutorV1.Call[] memory calls = new ExitExecutorV1.Call[](2);
        calls[0] = ExitExecutorV1.Call({to: address(0x1111), value: 1, data: hex"1234"});
        calls[1] = ExitExecutorV1.Call({to: address(0x2222), value: 0, data: ""});
        bytes32 d = _exec(eoa).digest(calls, 7, 1_800_000_000);
        emit DigestFixture(block.chainid, address(impl), eoa, d);
        require(d != bytes32(0));
    }

    function test_nonce_storage_is_namespaced() public {
        ExitExecutorV1.Call[] memory calls = _lighterBatch(1 ether);
        uint256 deadline = block.timestamp + 1 hours;
        bytes memory sig = _sign(PK, calls, 0, deadline);
        vm.prank(relayer);
        _exec(eoa).execute(calls, 0, deadline, sig);
        bytes32 slot = 0xc2a1d4a08cd33c1c4e97e37dbec10e2dad7bd4fbecd8a90603ed905d9e37fb00;
        require(uint256(vm.load(eoa, slot)) == 1, "nonce not at erc7201 slot");
        require(uint256(vm.load(eoa, bytes32(0))) == 0, "slot 0 must be untouched");
    }
}
