// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.24;

import {IMintableEscrowToken, IMintSettlementVerifier, MintController} from "../src/MintController.sol";
import {ThresholdMintSettlementVerifier} from "../src/ThresholdMintSettlementVerifier.sol";

interface OfficialForkVm {
    function createSelectFork(string calldata url, uint256 blockNumber) external returns (uint256);
    function envString(string calldata key) external returns (string memory);
    function addr(uint256 private_key) external returns (address);
    function sign(uint256 private_key, bytes32 digest) external returns (uint8 v, bytes32 r, bytes32 s);
}

contract PFTLUniswapOfficialForkTest {
    OfficialForkVm private constant vm = OfficialForkVm(address(uint160(uint256(keccak256("hevm cheat code")))));

    uint256 private constant MAINNET_CHAIN_ID = 1;
    uint256 private constant FORK_BLOCK = 25_440_306;

    address private constant POOL_MANAGER = 0x000000000004444c5dc75cB358380D2e3dE08A90;
    address private constant POSITION_MANAGER = 0xbD216513d74C8cf14cf4747E6AaA6420FF64ee9e;
    address private constant UNIVERSAL_ROUTER = 0x66a9893cC07D91D95644AEDD05D03f95e1dBA8Af;
    address private constant PERMIT2 = 0x000000000022D473030F116dDEE9F6B43aC78BA3;
    address private constant STATE_VIEW = 0x7fFE42C4a5DEeA5b0feC41C94C136Cf115597227;

    function testOfficialUniswapV4DeploymentsHaveCodeOnMainnetFork() public {
        _selectOfficialFork();

        _assertCode(POOL_MANAGER, "PoolManager");
        _assertCode(POSITION_MANAGER, "PositionManager");
        _assertCode(UNIVERSAL_ROUTER, "UniversalRouter");
        _assertCode(PERMIT2, "Permit2");
        _assertCode(STATE_VIEW, "StateView");
    }

    function testThresholdMintSettlementCertificateOnPinnedMainnetFork() public {
        _selectOfficialFork();
        OfficialForkMintToken token = new OfficialForkMintToken();
        MintController controller = new MintController(IMintableEscrowToken(address(token)), address(this), 1);
        uint256[] memory keys = new uint256[](4);
        keys[0] = 0xD101;
        keys[1] = 0xD102;
        keys[2] = 0xD103;
        keys[3] = 0xD104;
        _sortKeysByAddress(keys);
        address[] memory signers = new address[](keys.length);
        for (uint256 i = 0; i < keys.length; i++) {
            signers[i] = vm.addr(keys[i]);
        }
        ThresholdMintSettlementVerifier verifier = new ThresholdMintSettlementVerifier(
            keccak256("postfiat-pinned-fork"),
            keccak256("pinned-fork-genesis"),
            1,
            11,
            address(controller),
            address(token),
            signers,
            3
        );
        controller.setSettlementVerifier(IMintSettlementVerifier(address(verifier)), address(verifier).codehash);
        ThresholdMintSettlementVerifier.SettlementCertificate memory certificate =
            ThresholdMintSettlementVerifier.SettlementCertificate({
                pending_id: keccak256("pinned-fork-pending"),
                escrow_id: keccak256("pinned-fork-escrow"),
                recipient: address(0xBEEF),
                amount_atoms: 100,
                settled_proceeds_usd_e8: 100,
                locked_liquidity_usd_e8: 0,
                pftl_finalized_height: 1_212,
                pftl_finalized_state_root: _pftlBytes(0x51),
                pftl_receipt_hash: _pftlBytes(0x52),
                route_config_digest: _pftlBytes(0x53),
                receipt_code: verifier.ACCEPTED_RECEIPT_CODE()
            });
        bytes32 digest = verifier.certificateDigest(certificate);
        bytes[] memory signatures = new bytes[](3);
        for (uint256 i = 0; i < signatures.length; i++) {
            (uint8 v, bytes32 r, bytes32 s) = vm.sign(keys[i], digest);
            signatures[i] = abi.encodePacked(r, s, v);
        }
        bytes32 proof_hash = verifier.submitSettlementCertificate(certificate, signatures);
        (uint256 proceeds, uint256 liquidity) = verifier.verifiedSettlement(
            certificate.pending_id, certificate.escrow_id, certificate.recipient, certificate.amount_atoms, proof_hash
        );
        require(proceeds == 100 && liquidity == 0, "pinned-fork settlement certificate mismatch");
    }

    function _selectOfficialFork() private {
        // `--fork-url` may have selected the official fork before this test starts.
        // Otherwise the environment variable is mandatory: an offline invocation
        // must fail rather than silently reporting fork coverage that never ran.
        if (block.chainid != MAINNET_CHAIN_ID || POOL_MANAGER.code.length == 0) {
            string memory rpc_url = vm.envString("ETHEREUM_MAINNET_RPC_URL");
            vm.createSelectFork(rpc_url, FORK_BLOCK);
        }

        require(block.chainid == MAINNET_CHAIN_ID, "official fork must use Ethereum mainnet chain id");
    }

    function _assertCode(address target, string memory label) private view {
        if (target.code.length == 0) {
            revert(string.concat(label, " code missing"));
        }
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

contract OfficialForkMintToken {
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
