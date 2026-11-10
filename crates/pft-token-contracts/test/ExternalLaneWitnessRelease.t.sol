// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.24;

import {PFTToken} from "../src/PFTToken.sol";
import {PFTLockV1} from "../src/PFTLockV1.sol";
import {PFTLFinalityVerifierV1, ISP1Verifier} from "../../ethereum-contracts/src/PFTLFinalityVerifierV1.sol";
import {ExternalLaneWitnessReleaseFixture as F} from "./fixtures/ExternalLaneWitnessReleaseFixture.sol";

interface IWitnessReleaseVm {
    function chainId(uint256 newChainId) external;
    function etch(address target, bytes calldata code) external;
    function getNonce(address account) external view returns (uint64);
    function computeCreateAddress(address deployer, uint256 nonce) external pure returns (address);
    function expectRevert(bytes calldata revertData) external;
    function toString(address value) external pure returns (string memory);
    function toString(bytes32 value) external pure returns (string memory);
}

/// Accepts any proof: the test is about the binding, not SP1.
contract WitnessAcceptingSP1Verifier is ISP1Verifier {
    function verifyProof(bytes32, bytes calldata, bytes calldata) external pure {}
}

/// `release` on a deployed PFTLockV1 calls the real PFTLFinalityVerifierV1,
/// constructed with the lane's derived route profile and asset id, on public
/// values the lane export statement returned on a node test chain (crates/node/src/tests/external_lane_export_proof_tests.rs).
contract ExternalLaneWitnessReleaseTest {
    IWitnessReleaseVm private constant vm = IWitnessReleaseVm(address(uint160(uint256(keccak256("hevm cheat code")))));
    uint256 private constant SUPPLY = 1_000_000_000_000;
    bytes private constant PROOF = hex"01";

    PFTToken private token;
    PFTLockV1 private lock;
    PFTLFinalityVerifierV1 private verifier;

    function setUp() public {
        vm.chainId(F.SOURCE_CHAIN_ID);
        token = new PFTToken("Post Fiat Token", "PFT", SUPPLY, address(this));
        WitnessAcceptingSP1Verifier sp1 = new WitnessAcceptingSP1Verifier();
        // The lock's code hash depends on the verifier's address and the verifier
        // pins the lock's code hash (design note, section 5): predict the
        // verifier's address, deploy the lock against it, then the verifier there.
        address predicted = vm.computeCreateAddress(address(this), vm.getNonce(address(this)) + 1);
        vm.etch(predicted, hex"00");
        lock = new PFTLockV1(address(token), predicted, address(token).codehash, uint64(SUPPLY), 1_000_000);
        vm.etch(predicted, "");
        verifier = new PFTLFinalityVerifierV1(
            PFTLFinalityVerifierV1.Config({
                sp1Verifier: sp1,
                programVKey: keccak256("lane-program-vkey"),
                pftlChainIdHash: keccak256(bytes(F.PFTL_CHAIN_ID)),
                pftlGenesisHashCommitment: keccak256(F.PFTL_GENESIS_HASH),
                pftlProtocolVersion: F.PFTL_PROTOCOL_VERSION,
                routeProfileHashCommitment: keccak256(F.ROUTE_PROFILE_HASH),
                routeEpoch: F.ROUTE_EPOCH,
                assetIdCommitment: keccak256(F.ASSET_ID),
                arbitrumChainId: F.SOURCE_CHAIN_ID,
                vaultRuntimeCodeHash: address(lock).codehash,
                token: address(token),
                tokenRuntimeCodeHash: address(token).codehash,
                maxProofBytes: 4096,
                maxPublicValuesBytes: 16384,
                initialCheckpointCommitment: keccak256(F.PRIOR_CHECKPOINT),
                initialFinalizedHeight: 10,
                initialCommitteeRootCommitment: keccak256(F.COMMITTEE_ROOT)
            })
        );
        require(address(verifier) == predicted, "verifier address");
        token.approve(address(lock), F.AMOUNT);
        lock.lock(F.AMOUNT, bytes("pftl-recipient"));
    }

    /// The lane in the fixture names this deployment; on a bytecode change the
    /// message gives the values for the Rust constants.
    function testFixtureLaneNamesTheDeployedLockAndToken() public view {
        require(
            address(lock) == address(F.LOCK) && address(lock).codehash == F.LOCK_CODE_HASH
                && address(token) == address(F.TOKEN) && address(token).codehash == F.TOKEN_CODE_HASH,
            string.concat(
                "deployed lock ",
                vm.toString(address(lock)),
                " ",
                vm.toString(address(lock).codehash),
                " token ",
                vm.toString(address(token)),
                " ",
                vm.toString(address(token).codehash)
            )
        );
    }

    function testReleasePaysOnceThroughTheRealVerifier() public {
        address recipient = address(F.RECIPIENT);
        lock.release(F.PUBLIC_VALUES, PROOF);
        require(token.balanceOf(recipient) == F.AMOUNT, "recipient paid");
        require(lock.totalReleased() == F.AMOUNT, "released");
        require(verifier.consumedProofNullifier(F.PROOF_NULLIFIER), "nullifier consumed");
        require(verifier.consumedWithdrawalIdCommitment(keccak256(F.WITHDRAWAL_ID)), "withdrawal consumed");
        vm.expectRevert(abi.encodeWithSelector(PFTLFinalityVerifierV1.ProofAlreadyConsumed.selector, F.PROOF_NULLIFIER));
        lock.release(F.PUBLIC_VALUES, PROOF);
    }

    function testReleaseRefusesAnotherVaultTokenOrChain() public {
        vm.expectRevert(abi.encodeWithSelector(PFTLFinalityVerifierV1.WrongBinding.selector, bytes32("vault")));
        lock.release(F.WRONG_VAULT, PROOF);
        vm.expectRevert(abi.encodeWithSelector(PFTLFinalityVerifierV1.WrongBinding.selector, bytes32("token")));
        lock.release(F.WRONG_TOKEN, PROOF);
        vm.expectRevert(
            abi.encodeWithSelector(PFTLFinalityVerifierV1.WrongBinding.selector, bytes32("arbitrum_chain_id"))
        );
        lock.release(F.WRONG_CHAIN_ID, PROOF);
        require(token.balanceOf(address(F.RECIPIENT)) == 0, "nothing paid");
    }
}
