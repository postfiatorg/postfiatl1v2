// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.24;

import {MarketOpsEnvelope} from "../src/MarketOpsEnvelope.sol";
import {
    IMintBridgeAdapter,
    IMintableEscrowToken,
    IMintSettlementVerifier,
    MintController
} from "../src/MintController.sol";
import {ThresholdMintSettlementVerifier} from "../src/ThresholdMintSettlementVerifier.sol";

interface ThresholdMintVm {
    function addr(uint256 private_key) external returns (address);
    function sign(uint256 private_key, bytes32 digest) external returns (uint8 v, bytes32 r, bytes32 s);
    function expectRevert() external;
}

contract ThresholdMintSettlementVerifierTest {
    ThresholdMintVm private constant vm = ThresholdMintVm(address(uint160(uint256(keccak256("hevm cheat code")))));

    bytes32 private constant PENDING_ID = keccak256("pending-1");
    bytes private constant PFTL_STATE_ROOT =
        hex"111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111";
    bytes private constant PFTL_RECEIPT_HASH =
        hex"222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222";
    bytes private constant ROUTE_CONFIG_DIGEST =
        hex"333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333";

    ThresholdMintToken private token;
    ThresholdMintAdapter private adapter;
    MintController private controller;
    ThresholdMintSettlementVerifier private verifier;
    MarketOpsEnvelope private envelope;
    uint256[] private signer_keys;

    function setUp() public {
        token = new ThresholdMintToken();
        adapter = new ThresholdMintAdapter();
        controller = new MintController(IMintableEscrowToken(address(token)), address(this), 1);
        controller.setBridgeAdapter(IMintBridgeAdapter(address(adapter)));

        signer_keys.push(0xB101);
        signer_keys.push(0xB102);
        signer_keys.push(0xB103);
        signer_keys.push(0xB104);
        _sortKeysByAddress(signer_keys);
        verifier = _newVerifier(address(controller));
        controller.setSettlementVerifier(IMintSettlementVerifier(address(verifier)), address(verifier).codehash);

        envelope.encoding_version = 1;
        envelope.chain_id = 65_100;
        envelope.adapter_address = address(adapter);
        envelope.mint_controller_address = address(controller);
        envelope.asset_id = keccak256("a651");
        envelope.epoch = 59;
        envelope.nav_floor_usd_e8 = 1;
        envelope.verified_net_assets_usd_e8 = 1_000;
        envelope.valid_global_supply_atoms = 0;
        adapter.configure(PENDING_ID, keccak256(abi.encode(envelope)), 1_000);
    }

    function testExactBftSettlementCertificateReleasesBoundEscrowOnce() public {
        bytes32 escrow_id = controller.requestMint(envelope, 100);
        ThresholdMintSettlementVerifier.SettlementCertificate memory certificate = _certificate(escrow_id);
        bytes32 digest = verifier.certificateDigest(certificate);
        bytes32 proof_hash = verifier.submitSettlementCertificate(certificate, _signatures(digest, 3));

        (uint256 proceeds, uint256 liquidity) =
            verifier.verifiedSettlement(PENDING_ID, escrow_id, address(this), 100, proof_hash);
        _assertEq(proceeds, 100, "certified proceeds");
        _assertEq(liquidity, 0, "certified liquidity");

        controller.releaseMint(
            envelope,
            escrow_id,
            MintController.SettlementProof({
                recipient: address(this),
                settled_proceeds_usd_e8: 100,
                locked_liquidity_usd_e8: 0,
                proceeds_settled: true,
                liquidity_locked: false,
                proof_hash: proof_hash
            })
        );
        _assertEq(token.balanceOf(address(this)), 100, "released balance");
        _assertEq(token.balanceOf(address(controller)), 0, "escrow drained once");

        vm.expectRevert();
        verifier.submitSettlementCertificate(certificate, _signatures(digest, 3));
    }

    function testUnderQuorumDuplicateRejectedReceiptAndTamperingCannotRelease() public {
        bytes32 escrow_id = controller.requestMint(envelope, 100);
        ThresholdMintSettlementVerifier.SettlementCertificate memory certificate = _certificate(escrow_id);
        bytes32 digest = verifier.certificateDigest(certificate);

        vm.expectRevert();
        verifier.submitSettlementCertificate(certificate, _signatures(digest, 2));

        bytes[] memory duplicate = new bytes[](3);
        duplicate[0] = _signature(signer_keys[0], digest);
        duplicate[1] = _signature(signer_keys[0], digest);
        duplicate[2] = _signature(signer_keys[1], digest);
        vm.expectRevert();
        verifier.submitSettlementCertificate(certificate, duplicate);

        certificate.receipt_code = keccak256("rejected");
        vm.expectRevert();
        verifier.submitSettlementCertificate(certificate, _signatures(digest, 3));
        certificate.receipt_code = verifier.ACCEPTED_RECEIPT_CODE();

        certificate.amount_atoms = 101;
        vm.expectRevert();
        verifier.submitSettlementCertificate(certificate, _signatures(digest, 3));

        bytes32 nonexistent_proof = keccak256("not-certified");
        (uint256 proceeds, uint256 liquidity) =
            verifier.verifiedSettlement(PENDING_ID, escrow_id, address(this), 100, nonexistent_proof);
        _assertEq(proceeds, 0, "uncertified proceeds");
        _assertEq(liquidity, 0, "uncertified liquidity");
        _expectReleaseRevert(escrow_id, nonexistent_proof);
        _assertEq(token.balanceOf(address(this)), 0, "no fabricated release");
        _assertEq(token.balanceOf(address(controller)), 100, "escrow remains");
    }

    function testCertificateCannotReplayAcrossVerifierOrControllerDomain() public {
        bytes32 escrow_id = controller.requestMint(envelope, 100);
        ThresholdMintSettlementVerifier.SettlementCertificate memory certificate = _certificate(escrow_id);
        bytes32 first_digest = verifier.certificateDigest(certificate);
        bytes[] memory first_signatures = _signatures(first_digest, 3);

        ThresholdMintSettlementVerifier second = _newVerifier(address(controller));
        vm.expectRevert();
        second.submitSettlementCertificate(certificate, first_signatures);

        MintController other_controller = new MintController(IMintableEscrowToken(address(token)), address(this), 1);
        ThresholdMintSettlementVerifier third = _newVerifier(address(other_controller));
        vm.expectRevert();
        third.submitSettlementCertificate(certificate, first_signatures);
    }

    function testFuzzValidCertificateForWrongAmountCannotReleaseEscrow(uint96 wrong_amount_seed, uint96 proceeds_seed)
        public
    {
        bytes32 escrow_id = controller.requestMint(envelope, 100);
        ThresholdMintSettlementVerifier.SettlementCertificate memory certificate = _certificate(escrow_id);
        uint256 wrong_amount = uint256(wrong_amount_seed) % 1_000 + 1;
        if (wrong_amount == 100) {
            wrong_amount = 101;
        }
        uint256 proceeds = uint256(proceeds_seed) + 1;
        certificate.amount_atoms = wrong_amount;
        certificate.settled_proceeds_usd_e8 = proceeds;
        bytes32 digest = verifier.certificateDigest(certificate);
        bytes32 proof_hash = verifier.submitSettlementCertificate(certificate, _signatures(digest, 3));

        vm.expectRevert();
        controller.releaseMint(
            envelope,
            escrow_id,
            MintController.SettlementProof({
                recipient: address(this),
                settled_proceeds_usd_e8: proceeds,
                locked_liquidity_usd_e8: 0,
                proceeds_settled: true,
                liquidity_locked: false,
                proof_hash: proof_hash
            })
        );
        _assertEq(token.balanceOf(address(this)), 0, "wrong-amount certificate released value");
        _assertEq(token.balanceOf(address(controller)), 100, "wrong-amount certificate drained escrow");
    }

    function testConstructorRejectsThresholdLooseningAndUnsortedCommittee() public {
        address[] memory signers = _signerAddresses(signer_keys);
        vm.expectRevert();
        new ThresholdMintSettlementVerifier(
            keccak256("postfiat-devnet"), keccak256("genesis"), 1, 7, address(controller), address(token), signers, 2
        );

        (signers[0], signers[1]) = (signers[1], signers[0]);
        vm.expectRevert();
        new ThresholdMintSettlementVerifier(
            keccak256("postfiat-devnet"), keccak256("genesis"), 1, 7, address(controller), address(token), signers, 3
        );
    }

    function _newVerifier(address mint_controller) private returns (ThresholdMintSettlementVerifier) {
        return new ThresholdMintSettlementVerifier(
            keccak256("postfiat-devnet"),
            keccak256("genesis"),
            1,
            7,
            mint_controller,
            address(token),
            _signerAddresses(signer_keys),
            3
        );
    }

    function _certificate(bytes32 escrow_id)
        private
        view
        returns (ThresholdMintSettlementVerifier.SettlementCertificate memory)
    {
        return ThresholdMintSettlementVerifier.SettlementCertificate({
            pending_id: PENDING_ID,
            escrow_id: escrow_id,
            recipient: address(this),
            amount_atoms: 100,
            settled_proceeds_usd_e8: 100,
            locked_liquidity_usd_e8: 0,
            pftl_finalized_height: 1_212,
            pftl_finalized_state_root: PFTL_STATE_ROOT,
            pftl_receipt_hash: PFTL_RECEIPT_HASH,
            route_config_digest: ROUTE_CONFIG_DIGEST,
            receipt_code: verifier.ACCEPTED_RECEIPT_CODE()
        });
    }

    function _expectReleaseRevert(bytes32 escrow_id, bytes32 proof_hash) private {
        vm.expectRevert();
        controller.releaseMint(
            envelope,
            escrow_id,
            MintController.SettlementProof({
                recipient: address(this),
                settled_proceeds_usd_e8: 100,
                locked_liquidity_usd_e8: 0,
                proceeds_settled: true,
                liquidity_locked: false,
                proof_hash: proof_hash
            })
        );
    }

    function _signatures(bytes32 digest, uint256 count) private returns (bytes[] memory signatures) {
        signatures = new bytes[](count);
        for (uint256 i = 0; i < count; i++) {
            signatures[i] = _signature(signer_keys[i], digest);
        }
    }

    function _signature(uint256 private_key, bytes32 digest) private returns (bytes memory) {
        (uint8 v, bytes32 r, bytes32 s) = vm.sign(private_key, digest);
        return abi.encodePacked(r, s, v);
    }

    function _signerAddresses(uint256[] storage keys) private returns (address[] memory signers) {
        signers = new address[](keys.length);
        for (uint256 i = 0; i < keys.length; i++) {
            signers[i] = vm.addr(keys[i]);
        }
    }

    function _sortKeysByAddress(uint256[] storage keys) private {
        for (uint256 i = 1; i < keys.length; i++) {
            uint256 key = keys[i];
            address signer = vm.addr(key);
            uint256 cursor = i;
            while (cursor > 0 && vm.addr(keys[cursor - 1]) > signer) {
                keys[cursor] = keys[cursor - 1];
                cursor -= 1;
            }
            keys[cursor] = key;
        }
    }

    function _assertEq(uint256 actual, uint256 expected, string memory message) private pure {
        if (actual != expected) {
            revert(message);
        }
    }
}

contract ThresholdMintSettlementVerifierInvariantTest {
    ThresholdMintVm private constant vm = ThresholdMintVm(address(uint160(uint256(keccak256("hevm cheat code")))));

    ThresholdMintSettlementVerifier private verifier;
    bytes32 private proof_hash;
    bytes32 private pending_id;
    bytes32 private escrow_id;
    address private recipient;

    function setUp() public {
        uint256[] memory keys = new uint256[](4);
        keys[0] = 0xC101;
        keys[1] = 0xC102;
        keys[2] = 0xC103;
        keys[3] = 0xC104;
        _sortKeysByAddress(keys);
        address[] memory signers = new address[](keys.length);
        for (uint256 i = 0; i < keys.length; i++) {
            signers[i] = vm.addr(keys[i]);
        }
        verifier = new ThresholdMintSettlementVerifier(
            keccak256("postfiat-invariant"),
            keccak256("invariant-genesis"),
            1,
            9,
            address(0xC0DE),
            address(0xA651),
            signers,
            3
        );
        pending_id = keccak256("invariant-pending");
        escrow_id = keccak256("invariant-escrow");
        recipient = address(0xBEEF);
        ThresholdMintSettlementVerifier.SettlementCertificate memory certificate =
            ThresholdMintSettlementVerifier.SettlementCertificate({
                pending_id: pending_id,
                escrow_id: escrow_id,
                recipient: recipient,
                amount_atoms: 100,
                settled_proceeds_usd_e8: 70,
                locked_liquidity_usd_e8: 30,
                pftl_finalized_height: 1_212,
                pftl_finalized_state_root: _pftlBytes(0x41),
                pftl_receipt_hash: _pftlBytes(0x42),
                route_config_digest: _pftlBytes(0x43),
                receipt_code: verifier.ACCEPTED_RECEIPT_CODE()
            });
        bytes32 digest = verifier.certificateDigest(certificate);
        bytes[] memory signatures = new bytes[](3);
        for (uint256 i = 0; i < signatures.length; i++) {
            (uint8 v, bytes32 r, bytes32 s) = vm.sign(keys[i], digest);
            signatures[i] = abi.encodePacked(r, s, v);
        }
        proof_hash = verifier.submitSettlementCertificate(certificate, signatures);
    }

    function invariantOnlyTheExactCertifiedTupleReturnsSettlementValue() public view {
        (uint256 proceeds, uint256 liquidity) =
            verifier.verifiedSettlement(pending_id, escrow_id, recipient, 100, proof_hash);
        require(proceeds == 70 && liquidity == 30, "exact settlement record changed");
        _requireZero(keccak256("wrong"), escrow_id, recipient, 100, proof_hash);
        _requireZero(pending_id, keccak256("wrong"), recipient, 100, proof_hash);
        _requireZero(pending_id, escrow_id, address(0xBAD), 100, proof_hash);
        _requireZero(pending_id, escrow_id, recipient, 101, proof_hash);
        _requireZero(pending_id, escrow_id, recipient, 100, keccak256("wrong"));
    }

    function _requireZero(
        bytes32 checked_pending_id,
        bytes32 checked_escrow_id,
        address checked_recipient,
        uint256 checked_amount,
        bytes32 checked_proof_hash
    ) private view {
        (uint256 proceeds, uint256 liquidity) = verifier.verifiedSettlement(
            checked_pending_id, checked_escrow_id, checked_recipient, checked_amount, checked_proof_hash
        );
        require(proceeds == 0 && liquidity == 0, "mismatched tuple returned settlement value");
    }

    function _pftlBytes(bytes1 fill) private pure returns (bytes memory value) {
        value = new bytes(48);
        for (uint256 i = 0; i < value.length; i++) {
            value[i] = fill;
        }
    }

    function _sortKeysByAddress(uint256[] memory keys) private {
        for (uint256 i = 1; i < keys.length; i++) {
            uint256 key = keys[i];
            address signer = vm.addr(key);
            uint256 cursor = i;
            while (cursor > 0 && vm.addr(keys[cursor - 1]) > signer) {
                keys[cursor] = keys[cursor - 1];
                cursor -= 1;
            }
            keys[cursor] = key;
        }
    }
}

contract ThresholdMintToken {
    mapping(address => uint256) public balanceOf;

    function transfer(address to, uint256 amount) external returns (bool) {
        if (balanceOf[msg.sender] < amount) {
            return false;
        }
        balanceOf[msg.sender] -= amount;
        balanceOf[to] += amount;
        return true;
    }

    function mint(address to, uint256 amount) external {
        balanceOf[to] += amount;
    }
}

contract ThresholdMintAdapter {
    bytes32 private pending_id;
    bytes32 private envelope_digest;
    uint256 private cap_atoms;

    function configure(bytes32 pending_id_, bytes32 envelope_digest_, uint256 cap_atoms_) external {
        pending_id = pending_id_;
        envelope_digest = envelope_digest_;
        cap_atoms = cap_atoms_;
    }

    function accepted_envelope_by_asset_epoch(bytes32) external view returns (bytes32) {
        return pending_id;
    }

    function assetEpochKey(bytes32 asset_id, uint64 epoch) external pure returns (bytes32) {
        return keccak256(abi.encode(asset_id, epoch));
    }

    function getEvmEnvelopeDigest(bytes32) external view returns (bytes32) {
        return envelope_digest;
    }

    function isEnvelopeExecutable(bytes32) external pure returns (bool) {
        return true;
    }

    function mintCapAtoms(bytes32) external view returns (uint256) {
        return cap_atoms;
    }
}
