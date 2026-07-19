// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.24;

import {
    ERC20BridgeVaultV2,
    IArbSysPfUsdcV1,
    IERC20BridgeTokenV2,
    IPFTLFinalityVerifierV1,
    IPfUsdcIngressAnchorV1
} from "../src/ERC20BridgeVaultV2.sol";
import {PFTLFinalityVerifierV1, ISP1Verifier} from "../src/PFTLFinalityVerifierV1.sol";
import {PfUsdcIngressAnchorV1, IArbitrumBridgeV1, IArbitrumOutboxV1} from "../src/PfUsdcIngressAnchorV1.sol";

contract Tier4MockToken is IERC20BridgeTokenV2 {
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

contract Tier4MockSP1Verifier is ISP1Verifier {
    bool public reject;

    function setReject(bool reject_) external {
        reject = reject_;
    }

    function verifyProof(bytes32, bytes calldata, bytes calldata) external view {
        require(!reject, "mock SP1 rejection");
    }
}

contract Tier4TemporaryFinality is IPFTLFinalityVerifierV1 {
    function verifyAndConsume(bytes calldata, bytes calldata)
        external
        pure
        returns (address, uint256, bytes32, bytes32, bytes32)
    {
        revert("temporary");
    }
}

contract Tier4FixedFinality is IPFTLFinalityVerifierV1 {
    address private immutable recipient;
    uint256 private immutable amount;

    constructor(address recipient_, uint256 amount_) {
        recipient = recipient_;
        amount = amount_;
    }

    function verifyAndConsume(bytes calldata, bytes calldata)
        external
        view
        returns (address, uint256, bytes32, bytes32, bytes32)
    {
        return (recipient, amount, keccak256("fee-withdrawal"), keccak256("fee-burn"), keccak256("fee-packet"));
    }
}

contract Tier4FeeToken is IERC20BridgeTokenV2 {
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
        balanceOf[to] += amount - 1;
        return true;
    }

    function transferFrom(address from, address to, uint256 amount) external returns (bool) {
        allowance[from][msg.sender] -= amount;
        balanceOf[from] -= amount;
        balanceOf[to] += amount - 1;
        return true;
    }
}

contract Tier4MockArbSys is IArbSysPfUsdcV1 {
    address public destination;
    bytes public data;
    uint256 public nextOutputIndex = 77;
    bool public reject;

    function setReject(bool reject_) external {
        reject = reject_;
    }

    function sendTxToL1(address destination_, bytes calldata data_) external payable returns (uint256) {
        require(!reject, "mock ArbSys rejection");
        destination = destination_;
        data = data_;
        return nextOutputIndex++;
    }
}

contract Tier4MockIngressAnchor is IPfUsdcIngressAnchorV1 {
    function recordDepositV1(
        bytes32,
        address,
        bytes32,
        string calldata,
        uint256,
        bytes32,
        bytes32,
        uint256,
        address,
        address
    ) external {}
}

contract Tier4MockBridge is IArbitrumBridgeV1 {
    address public activeOutbox;

    function setActiveOutbox(address outbox) external {
        activeOutbox = outbox;
    }
}

contract Tier4MockOutbox is IArbitrumOutboxV1 {
    address public l2ToL1Sender;

    function setL2ToL1Sender(address sender) external {
        l2ToL1Sender = sender;
    }

    function relay(address destination, bytes calldata data) external returns (bool, bytes memory) {
        return destination.call(data);
    }
}

contract PFUSDCTier4Test {
    bytes private constant SCHEMA = "postfiat.pfusdc.egress_public_values.v1";
    bytes private constant CHECKPOINT_SCHEMA = "postfiat.pfusdc.checkpoint_public_values.v1";
    bytes32 private constant PROGRAM_VKEY = keccak256("program-vkey");
    address private constant RECIPIENT = address(0xBEEF);

    Tier4MockToken private token;
    Tier4MockSP1Verifier private sp1;
    PFTLFinalityVerifierV1 private verifier;
    ERC20BridgeVaultV2 private vault;
    Tier4MockArbSys private arbSys;
    Tier4MockIngressAnchor private ingressAnchor;
    bytes private genesis48;
    bytes private route48;
    bytes private asset48;
    bytes private initialCheckpoint48;
    bytes32 private initialCheckpoint;

    function setUp() public {
        token = new Tier4MockToken();
        sp1 = new Tier4MockSP1Verifier();
        arbSys = new Tier4MockArbSys();
        ingressAnchor = new Tier4MockIngressAnchor();
        genesis48 = _h48(0x11);
        route48 = _h48(0x22);
        asset48 = _h48(0x33);
        initialCheckpoint48 = _h48(0x10);
        initialCheckpoint = keccak256(initialCheckpoint48);

        Tier4TemporaryFinality temporaryFinality = new Tier4TemporaryFinality();
        ERC20BridgeVaultV2 temporaryVault = new ERC20BridgeVaultV2(
            token,
            IPFTLFinalityVerifierV1(address(temporaryFinality)),
            address(token).codehash,
            arbSys,
            address(ingressAnchor),
            address(this)
        );
        bytes32 vaultCodeHash = address(temporaryVault).codehash;
        PFTLFinalityVerifierV1.Config memory config = PFTLFinalityVerifierV1.Config({
            sp1Verifier: sp1,
            programVKey: PROGRAM_VKEY,
            pftlChainIdHash: keccak256(bytes("postfiat-tier4-test")),
            pftlGenesisHashCommitment: keccak256(genesis48),
            pftlProtocolVersion: 1,
            routeProfileHashCommitment: keccak256(route48),
            routeEpoch: 7,
            assetIdCommitment: keccak256(asset48),
            arbitrumChainId: uint64(block.chainid),
            vaultRuntimeCodeHash: vaultCodeHash,
            token: address(token),
            tokenRuntimeCodeHash: address(token).codehash,
            maxProofBytes: 4096,
            maxPublicValuesBytes: 16384,
            initialCheckpointCommitment: initialCheckpoint,
            initialFinalizedHeight: 10,
            initialCommitteeRootCommitment: keccak256(_h48(0x41))
        });
        verifier = new PFTLFinalityVerifierV1(config);
        vault = new ERC20BridgeVaultV2(
            token,
            IPFTLFinalityVerifierV1(address(verifier)),
            address(token).codehash,
            arbSys,
            address(ingressAnchor),
            address(this)
        );
        _assertEq(address(vault).codehash, vaultCodeHash, "vault code hash must be constructor-independent");
        token.mint(address(vault), 1_000_000);
    }

    function testDepositEmitsCanonicalTier4SendAndRevertsAtomicallyWhenSendFails() public {
        token.mint(address(this), 1_000);
        token.approve(address(vault), 1_000);
        bytes32 nonce = keccak256("deposit-nonce");
        bytes32 routeBinding = keccak256("route-binding");
        bytes32 depositId = vault.depositV2(125, "pf-tier4-recipient", nonce, routeBinding);

        _assertTrue(arbSys.destination() == address(ingressAnchor), "ingress anchor destination");
        bytes memory expected = abi.encodeCall(
            IPfUsdcIngressAnchorV1.recordDepositV1,
            (
                depositId,
                address(this),
                keccak256(bytes("pf-tier4-recipient")),
                "pf-tier4-recipient",
                125,
                nonce,
                routeBinding,
                block.chainid,
                address(vault),
                address(token)
            )
        );
        _assertTrue(keccak256(arbSys.data()) == keccak256(expected), "canonical send calldata");

        arbSys.setReject(true);
        uint256 walletBefore = token.balanceOf(address(this));
        uint256 vaultBefore = token.balanceOf(address(vault));
        (bool ok,) = address(vault)
            .call(abi.encodeCall(ERC20BridgeVaultV2.depositV2, (100, "pf-rejected", bytes32(uint256(2)), routeBinding)));
        _assertTrue(!ok, "failed commitment must revert deposit");
        _assertEq(token.balanceOf(address(this)), walletBefore, "failed send refunds depositor");
        _assertEq(token.balanceOf(address(vault)), vaultBefore, "failed send leaves vault unchanged");
    }

    function testProductionIngressAnchorPinsOutboxSenderRouteAndReplay() public {
        Tier4MockBridge bridge = new Tier4MockBridge();
        Tier4MockOutbox outbox = new Tier4MockOutbox();
        bridge.setActiveOutbox(address(outbox));
        outbox.setL2ToL1Sender(address(vault));
        bytes32 routeBinding = keccak256("production-anchor-route");
        PfUsdcIngressAnchorV1 anchor =
            new PfUsdcIngressAnchorV1(bridge, address(vault), address(token), block.chainid, routeBinding);
        bytes32 depositId = keccak256("production-anchor-deposit");
        bytes memory callData = abi.encodeCall(
            IPfUsdcIngressAnchorV1.recordDepositV1,
            (
                depositId,
                address(this),
                keccak256(bytes("pf-production-recipient")),
                "pf-production-recipient",
                125,
                keccak256("production-anchor-nonce"),
                routeBinding,
                block.chainid,
                address(vault),
                address(token)
            )
        );
        (bool ok,) = outbox.relay(address(anchor), callData);
        _assertTrue(ok, "active outbox message accepted");
        _assertTrue(anchor.depositSeen(depositId), "anchor replay key consumed");

        (ok,) = outbox.relay(address(anchor), callData);
        _assertTrue(!ok, "duplicate anchor message rejected");
        outbox.setL2ToL1Sender(address(0xCAFE));
        bytes memory secondCall = abi.encodeCall(
            IPfUsdcIngressAnchorV1.recordDepositV1,
            (
                keccak256("second-production-anchor-deposit"),
                address(this),
                keccak256(bytes("pf-production-recipient")),
                "pf-production-recipient",
                125,
                keccak256("second-production-anchor-nonce"),
                routeBinding,
                block.chainid,
                address(vault),
                address(token)
            )
        );
        (ok,) = outbox.relay(address(anchor), secondCall);
        _assertTrue(!ok, "wrong L2 sender rejected");
    }

    function testProductionIngressAnchorRuntimeHashDoesNotDependOnRouteBinding() public {
        Tier4MockBridge bridge = new Tier4MockBridge();
        PfUsdcIngressAnchorV1 first =
            new PfUsdcIngressAnchorV1(bridge, address(vault), address(token), block.chainid, keccak256("route-a"));
        PfUsdcIngressAnchorV1 second =
            new PfUsdcIngressAnchorV1(bridge, address(vault), address(token), block.chainid, keccak256("route-b"));
        _assertEq(
            address(first).codehash, address(second).codehash, "route binding must not change anchor runtime hash"
        );
        _assertTrue(
            first.governedRouteBinding() != second.governedRouteBinding(), "route binding remains instance-pinned"
        );
    }

    function testProofNativeWithdrawalPaysExactlyAndConsumesReplayKeys() public {
        bytes memory publicValues = _publicValues(address(vault), true, 11, _h48(0x44), _h32(0x99));
        uint256 vaultBefore = token.balanceOf(address(vault));
        uint256 recipientBefore = token.balanceOf(RECIPIENT);

        bytes32 withdrawalCommitment = vault.withdrawWithProof(publicValues, hex"01020304");

        _assertEq(token.balanceOf(address(vault)), vaultBefore - 125, "vault debit");
        _assertEq(token.balanceOf(RECIPIENT), recipientBefore + 125, "recipient credit");
        _assertTrue(vault.consumedWithdrawalIdCommitment(withdrawalCommitment), "withdrawal consumed");
        _assertEq(verifier.latestFinalizedHeight(), 11, "checkpoint height");
        _expectWithdrawRevert(publicValues, hex"01020304");
    }

    function testRejectedReceiptCodeCannotReachSP1OrMoveMoney() public {
        bytes memory publicValues = _publicValues(address(vault), false, 11, _h48(0x44), _h32(0x99));
        uint256 beforeBalance = token.balanceOf(address(vault));
        _expectWithdrawRevert(publicValues, hex"01020304");
        _assertEq(token.balanceOf(address(vault)), beforeBalance, "rejected vault unchanged");
        _assertEq(token.balanceOf(RECIPIENT), 0, "rejected recipient unchanged");
    }

    function testWrongVaultAndInvalidSP1ProofFailBeforeMutation() public {
        bytes memory wrongVault = _publicValues(address(0xCAFE), true, 11, _h48(0x44), _h32(0x99));
        _expectWithdrawRevert(wrongVault, hex"01020304");

        sp1.setReject(true);
        bytes memory publicValues = _publicValues(address(vault), true, 11, _h48(0x44), _h32(0x99));
        _expectWithdrawRevert(publicValues, hex"01020304");
        _assertEq(verifier.latestFinalizedHeight(), 10, "invalid proof checkpoint unchanged");
        _assertEq(token.balanceOf(RECIPIENT), 0, "invalid proof recipient unchanged");
    }

    function testUnprovedCommitteeChangeFailsBeforeMutation() public {
        bytes memory publicValues =
            _publicValuesWithCommittee(address(vault), true, 11, _h48(0x44), _h32(0x99), _h48(0x42), bytes(""));
        _expectWithdrawRevert(publicValues, hex"01020304");
        _assertEq(verifier.latestFinalizedHeight(), 10, "committee mismatch checkpoint unchanged");
        _assertEq(token.balanceOf(RECIPIENT), 0, "committee mismatch recipient unchanged");
    }

    function testProvedCommitteeChangeMustStartAtStoredCommitteeRoot() public {
        bytes memory wrongStart =
            _publicValuesWithCommittee(address(vault), true, 11, _h48(0x44), _h32(0x98), _h48(0x42), _h48(0x40));
        _expectWithdrawRevert(wrongStart, hex"01020304");

        bytes memory provedTransition =
            _publicValuesWithCommittee(address(vault), true, 11, _h48(0x44), _h32(0x99), _h48(0x42), _h48(0x41));
        vault.withdrawWithProof(provedTransition, hex"01020304");
        _assertEq(
            verifier.latestCommitteeRootCommitment(), keccak256(_h48(0x42)), "proved transition advances committee root"
        );
    }

    function testWrongProofProgramVersionFailsBeforeSP1AndMutation() public {
        bytes memory publicValues = _publicValues(address(vault), true, 11, _h48(0x44), _h32(0x99));
        // Canonical tag 2 starts after magic, schema-length, schema, and field 1.
        uint256 versionOffset = 17 + 4 + SCHEMA.length + 6 + SCHEMA.length + 6;
        publicValues[versionOffset + 3] = 0x02;
        _expectWithdrawRevert(publicValues, hex"01020304");
        _assertEq(verifier.latestFinalizedHeight(), 10, "wrong program version checkpoint unchanged");
        _assertEq(token.balanceOf(RECIPIENT), 0, "wrong program version recipient unchanged");
    }

    function testCheckpointOnlyProofKeepsFinalityWindowLive() public {
        bytes memory checkpoint11 = _checkpointPublicValues(initialCheckpoint48, _h48(0x44), 11, _h48(0x41), bytes(""));
        verifier.advanceCheckpoint(checkpoint11, hex"01020304");
        _assertEq(verifier.latestFinalizedHeight(), 11, "checkpoint-only height");
        _assertEq(verifier.latestCheckpointCommitment(), keccak256(_h48(0x44)), "checkpoint-only root");

        bytes memory checkpoint12 = _checkpointPublicValues(_h48(0x44), _h48(0x45), 12, _h48(0x41), bytes(""));
        verifier.advanceCheckpoint(checkpoint12, hex"01020304");
        _assertEq(verifier.latestFinalizedHeight(), 12, "second checkpoint height");

        // Advancing the maintenance checkpoint must not strand an unclaimed
        // withdrawal finalized at the earlier accepted checkpoint.
        bytes memory historicalWithdrawal = _publicValues(address(vault), true, 11, _h48(0x44), _h32(0x97));
        vault.withdrawWithProof(historicalWithdrawal, hex"01020304");
        _assertEq(token.balanceOf(RECIPIENT), 125, "historical withdrawal remains claimable");
        _assertEq(verifier.latestFinalizedHeight(), 12, "historical claim cannot roll back tip");

        (bool replayOk,) = address(verifier)
            .call(abi.encodeCall(PFTLFinalityVerifierV1.advanceCheckpoint, (checkpoint11, hex"01020304")));
        _assertTrue(!replayOk, "stale checkpoint replay must fail");
    }

    function testCheckpointOnlyProofRejectsWrongCommitteeAndInvalidProof() public {
        bytes memory wrongCommittee =
            _checkpointPublicValues(initialCheckpoint48, _h48(0x44), 11, _h48(0x42), bytes(""));
        (bool committeeOk,) = address(verifier)
            .call(abi.encodeCall(PFTLFinalityVerifierV1.advanceCheckpoint, (wrongCommittee, hex"01020304")));
        _assertTrue(!committeeOk, "unproved checkpoint committee change must fail");

        sp1.setReject(true);
        bytes memory valid = _checkpointPublicValues(initialCheckpoint48, _h48(0x44), 11, _h48(0x41), bytes(""));
        (bool proofOk,) =
            address(verifier).call(abi.encodeCall(PFTLFinalityVerifierV1.advanceCheckpoint, (valid, hex"01020304")));
        _assertTrue(!proofOk, "invalid checkpoint proof must fail");
        _assertEq(verifier.latestFinalizedHeight(), 10, "invalid checkpoint proof does not mutate");
    }

    function testNonExactTokenDeltasRevertIngressAndEgressAtomically() public {
        Tier4FeeToken feeToken = new Tier4FeeToken();
        Tier4TemporaryFinality temporary = new Tier4TemporaryFinality();
        ERC20BridgeVaultV2 ingressVault = new ERC20BridgeVaultV2(
            feeToken, temporary, address(feeToken).codehash, arbSys, address(ingressAnchor), address(this)
        );
        feeToken.mint(address(this), 100);
        feeToken.approve(address(ingressVault), 100);
        (bool depositOk,) = address(ingressVault)
            .call(
                abi.encodeCall(
                    ERC20BridgeVaultV2.depositV2,
                    (100, "pf-fee-recipient", keccak256("fee-deposit"), keccak256("fee-route"))
                )
            );
        _assertTrue(!depositOk, "fee token ingress accepted");
        _assertEq(feeToken.balanceOf(address(this)), 100, "fee ingress changed sender");
        _assertEq(feeToken.balanceOf(address(ingressVault)), 0, "fee ingress changed vault");

        Tier4FixedFinality fixedFinality = new Tier4FixedFinality(RECIPIENT, 100);
        ERC20BridgeVaultV2 egressVault = new ERC20BridgeVaultV2(
            feeToken, fixedFinality, address(feeToken).codehash, arbSys, address(ingressAnchor), address(this)
        );
        feeToken.mint(address(egressVault), 100);
        (bool withdrawalOk,) =
            address(egressVault).call(abi.encodeCall(ERC20BridgeVaultV2.withdrawWithProof, (hex"01", hex"01")));
        _assertTrue(!withdrawalOk, "fee token egress accepted");
        _assertEq(feeToken.balanceOf(address(egressVault)), 100, "fee egress changed vault");
        _assertEq(feeToken.balanceOf(RECIPIENT), 0, "fee egress changed recipient");
    }

    function testPauseBlocksNewWorkWithoutRewritingConsumedClaims() public {
        token.mint(address(this), 1_000);
        token.approve(address(vault), 1_000);
        bytes32 routeBinding = keccak256("pause-route");
        bytes32 depositId = vault.depositV2(125, "pf-pause-recipient", keccak256("pause-deposit"), routeBinding);
        bytes memory publicValues = _publicValues(address(vault), true, 11, _h48(0x44), _h32(0x99));
        bytes32 withdrawalCommitment = vault.withdrawWithProof(publicValues, hex"01020304");
        _assertTrue(vault.depositSeen(depositId), "accepted deposit replay key missing");
        _assertTrue(vault.consumedWithdrawalIdCommitment(withdrawalCommitment), "withdrawal replay key missing");

        vault.setPaused(true);
        uint256 walletBefore = token.balanceOf(address(this));
        uint256 vaultBefore = token.balanceOf(address(vault));
        (bool depositOk,) = address(vault)
            .call(
                abi.encodeCall(
                    ERC20BridgeVaultV2.depositV2, (100, "pf-paused-new", keccak256("paused-new"), routeBinding)
                )
            );
        _assertTrue(!depositOk, "paused vault accepted new deposit");
        _assertEq(token.balanceOf(address(this)), walletBefore, "paused deposit changed wallet");
        _assertEq(token.balanceOf(address(vault)), vaultBefore, "paused deposit changed vault");
        _assertTrue(vault.depositSeen(depositId), "pause rewrote deposit replay key");
        _assertTrue(vault.consumedWithdrawalIdCommitment(withdrawalCommitment), "pause rewrote withdrawal replay key");

        vault.setPaused(false);
        _assertTrue(vault.depositSeen(depositId), "resume rewrote deposit replay key");
        _assertTrue(vault.consumedWithdrawalIdCommitment(withdrawalCommitment), "resume rewrote withdrawal replay key");
    }

    function testFuzzMalformedPublicValuesCannotMutate(bytes memory publicValues, bytes memory proof) public {
        uint256 vaultBefore = token.balanceOf(address(vault));
        uint256 recipientBefore = token.balanceOf(RECIPIENT);
        (bool ok,) = address(vault).call(abi.encodeCall(ERC20BridgeVaultV2.withdrawWithProof, (publicValues, proof)));
        _assertTrue(!ok, "malformed public values unexpectedly withdrew");
        _assertEq(token.balanceOf(address(vault)), vaultBefore, "malformed public values changed vault");
        _assertEq(token.balanceOf(RECIPIENT), recipientBefore, "malformed public values changed recipient");
    }

    function testFuzzRejectedSP1ProofCannotMutate(bytes memory proof) public {
        sp1.setReject(true);
        bytes memory publicValues = _publicValues(address(vault), true, 11, _h48(0x44), _h32(0x99));
        uint256 vaultBefore = token.balanceOf(address(vault));
        uint256 recipientBefore = token.balanceOf(RECIPIENT);
        (bool ok,) = address(vault).call(abi.encodeCall(ERC20BridgeVaultV2.withdrawWithProof, (publicValues, proof)));
        _assertTrue(!ok, "rejected SP1 proof unexpectedly withdrew");
        _assertEq(verifier.latestFinalizedHeight(), 10, "rejected SP1 proof changed checkpoint");
        _assertEq(token.balanceOf(address(vault)), vaultBefore, "rejected SP1 proof changed vault");
        _assertEq(token.balanceOf(RECIPIENT), recipientBefore, "rejected SP1 proof changed recipient");
    }

    function testFuzzExactDepositDeltaAndReplay(uint64 rawAmount, bytes32 nonceSeed, bytes32 routeSeed) public {
        uint256 amount = uint256(rawAmount) + 1;
        bytes32 routeBinding = keccak256(abi.encode("pfusdc-fuzz-route", routeSeed));
        token.mint(address(this), amount);
        token.approve(address(vault), amount);
        uint256 walletBefore = token.balanceOf(address(this));
        uint256 vaultBefore = token.balanceOf(address(vault));
        bytes32 depositId = vault.depositV2(amount, "pf-fuzz-recipient", nonceSeed, routeBinding);
        _assertTrue(vault.depositSeen(depositId), "fuzz deposit replay key missing");
        _assertEq(walletBefore - token.balanceOf(address(this)), amount, "fuzz deposit wallet delta");
        _assertEq(token.balanceOf(address(vault)) - vaultBefore, amount, "fuzz deposit vault delta");

        uint256 walletAfter = token.balanceOf(address(this));
        uint256 vaultAfter = token.balanceOf(address(vault));
        (bool replayOk,) = address(vault)
            .call(abi.encodeCall(ERC20BridgeVaultV2.depositV2, (amount, "pf-fuzz-recipient", nonceSeed, routeBinding)));
        _assertTrue(!replayOk, "fuzz deposit replay accepted");
        _assertEq(token.balanceOf(address(this)), walletAfter, "fuzz replay changed wallet");
        _assertEq(token.balanceOf(address(vault)), vaultAfter, "fuzz replay changed vault");
    }

    function invariantFrozenTier4BindingsRemainPinned() public view {
        _assertTrue(address(vault.token()) == address(token), "vault token binding changed");
        _assertTrue(address(vault.finalityVerifier()) == address(verifier), "vault verifier binding changed");
        _assertTrue(vault.tokenRuntimeCodeHash() == address(token).codehash, "vault token code hash changed");
        _assertTrue(address(verifier.sp1Verifier()) == address(sp1), "SP1 verifier binding changed");
        _assertTrue(verifier.programVKey() == PROGRAM_VKEY, "program vkey changed");
    }

    function _publicValues(
        address vaultAddress,
        bool accepted,
        uint64 finalizedHeight,
        bytes memory resultingCheckpoint,
        bytes memory nullifier
    ) private view returns (bytes memory out) {
        return _publicValuesWithCommittee(
            vaultAddress, accepted, finalizedHeight, resultingCheckpoint, nullifier, _h48(0x41), bytes("")
        );
    }

    function _checkpointPublicValues(
        bytes memory priorCheckpoint,
        bytes memory resultingCheckpoint,
        uint64 finalizedHeight,
        bytes memory committeeRoot,
        bytes memory transitionStartRoot
    ) private view returns (bytes memory out) {
        out = abi.encodePacked("PFTL-PFUSDC-TIER4", uint32(CHECKPOINT_SCHEMA.length), CHECKPOINT_SCHEMA);
        out = bytes.concat(out, _field(1, CHECKPOINT_SCHEMA));
        out = bytes.concat(out, _field(2, abi.encodePacked(uint32(1))));
        out = bytes.concat(out, _field(3, bytes("postfiat-tier4-test")));
        out = bytes.concat(out, _field(4, genesis48));
        out = bytes.concat(out, _field(5, abi.encodePacked(uint32(1))));
        out = bytes.concat(out, _field(6, priorCheckpoint));
        out = bytes.concat(out, _field(7, resultingCheckpoint));
        out = bytes.concat(out, _field(8, abi.encodePacked(uint64(1))));
        out = bytes.concat(out, _field(9, committeeRoot));
        out = bytes.concat(out, _field(10, transitionStartRoot));
        out = bytes.concat(out, _field(11, abi.encodePacked(finalizedHeight)));
        out = bytes.concat(out, _field(12, abi.encodePacked(uint64(0))));
        out = bytes.concat(out, _field(13, resultingCheckpoint));
        out = bytes.concat(out, _field(14, priorCheckpoint));
        out = bytes.concat(out, _field(15, _h48(0x66)));
    }

    function _publicValuesWithCommittee(
        address vaultAddress,
        bool accepted,
        uint64 finalizedHeight,
        bytes memory resultingCheckpoint,
        bytes memory nullifier,
        bytes memory committeeRoot,
        bytes memory transitionStartRoot
    ) private view returns (bytes memory out) {
        out = abi.encodePacked("PFTL-PFUSDC-TIER4", uint32(SCHEMA.length), SCHEMA);
        out = bytes.concat(out, _field(1, SCHEMA));
        out = bytes.concat(out, _field(2, abi.encodePacked(uint32(1))));
        out = bytes.concat(out, _field(3, bytes("postfiat-tier4-test")));
        out = bytes.concat(out, _field(4, genesis48));
        out = bytes.concat(out, _field(5, abi.encodePacked(uint32(1))));
        out = bytes.concat(out, _field(6, route48));
        out = bytes.concat(out, _field(7, abi.encodePacked(uint64(7))));
        out = bytes.concat(out, _field(8, initialCheckpoint48));
        out = bytes.concat(out, _field(9, resultingCheckpoint));
        out = bytes.concat(out, _field(10, abi.encodePacked(uint64(1))));
        out = bytes.concat(out, _field(11, committeeRoot));
        out = bytes.concat(out, _field(12, transitionStartRoot));
        out = bytes.concat(out, _field(13, abi.encodePacked(finalizedHeight)));
        out = bytes.concat(out, _field(14, abi.encodePacked(uint64(0))));
        out = bytes.concat(out, _field(15, resultingCheckpoint));
        out = bytes.concat(out, _field(16, _h48(0x55)));
        out = bytes.concat(out, _field(17, _h48(0x66)));
        out = bytes.concat(out, _field(18, _h48(0x77)));
        out = bytes.concat(out, _field(19, abi.encodePacked(uint64(0))));
        out = bytes.concat(out, _field(20, _h48(0x78)));
        out = bytes.concat(out, _field(21, _h48(0x79)));
        out = bytes.concat(out, _field(22, accepted ? bytes("accepted") : bytes("rejected")));
        out = bytes.concat(out, _field(23, asset48));
        out = bytes.concat(out, _field(24, _h48(0x80)));
        out = bytes.concat(out, _field(25, _h48(0x81)));
        out = bytes.concat(out, _field(26, _h48(0x82)));
        out = bytes.concat(out, _field(27, abi.encodePacked(uint64(125))));
        out = bytes.concat(out, _field(28, abi.encodePacked(RECIPIENT)));
        out = bytes.concat(out, _field(29, _h48(0x83)));
        out = bytes.concat(out, _field(30, _h48(0x84)));
        out = bytes.concat(out, _field(31, abi.encodePacked(finalizedHeight)));
        out = bytes.concat(out, _field(32, abi.encodePacked(uint64(block.chainid))));
        out = bytes.concat(out, _field(33, abi.encodePacked(vaultAddress)));
        out = bytes.concat(out, _field(34, abi.encodePacked(address(vault).codehash)));
        out = bytes.concat(out, _field(35, abi.encodePacked(address(token))));
        out = bytes.concat(out, _field(36, abi.encodePacked(address(token).codehash)));
        out = bytes.concat(out, _field(37, _h32(0x85)));
        out = bytes.concat(out, _field(38, _h48(0x86)));
        out = bytes.concat(out, _field(39, nullifier));
    }

    function _field(uint16 tag, bytes memory value) private pure returns (bytes memory) {
        return abi.encodePacked(tag, uint32(value.length), value);
    }

    function _h32(uint8 value) private pure returns (bytes memory out) {
        out = new bytes(32);
        for (uint256 i = 0; i < out.length; i++) {
            out[i] = bytes1(value);
        }
    }

    function _h48(uint8 value) private pure returns (bytes memory out) {
        out = new bytes(48);
        for (uint256 i = 0; i < out.length; i++) {
            out[i] = bytes1(value);
        }
    }

    function _expectWithdrawRevert(bytes memory publicValues, bytes memory proof) private {
        (bool ok,) = address(vault).call(abi.encodeCall(ERC20BridgeVaultV2.withdrawWithProof, (publicValues, proof)));
        _assertTrue(!ok, "withdrawal must revert");
    }

    function _assertTrue(bool value, string memory message) private pure {
        require(value, message);
    }

    function _assertEq(uint256 actual, uint256 expected, string memory message) private pure {
        require(actual == expected, message);
    }

    function _assertEq(bytes32 actual, bytes32 expected, string memory message) private pure {
        require(actual == expected, message);
    }
}
