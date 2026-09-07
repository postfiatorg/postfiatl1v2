// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.24;

import {IERC20BridgeTokenV2, IPFTLFinalityVerifierV1} from "../src/ERC20BridgeVaultV2.sol";
import {WETHBridgeVaultL1} from "../src/WETHBridgeVaultL1.sol";

interface WETHVaultVm {
    function load(address target, bytes32 slot) external view returns (bytes32);
}

contract WETHVaultMockToken is IERC20BridgeTokenV2 {
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

contract WETHVaultFixedFinality is IPFTLFinalityVerifierV1 {
    address private immutable recipient;
    uint256 private immutable amountAtoms;
    bytes32 private immutable withdrawalCommitment;
    bytes32 private immutable burnCommitment;
    bytes32 private immutable packetDigest;

    constructor(address recipient_, uint256 amountAtoms_) {
        recipient = recipient_;
        amountAtoms = amountAtoms_;
        withdrawalCommitment = keccak256("pfeth-withdrawal");
        burnCommitment = keccak256("pfeth-burn");
        packetDigest = keccak256("pfeth-packet");
    }

    function verifyAndConsume(bytes calldata, bytes calldata)
        external
        view
        returns (address, uint256, bytes32, bytes32, bytes32)
    {
        return (recipient, amountAtoms, withdrawalCommitment, burnCommitment, packetDigest);
    }
}

contract WETHBridgeVaultL1Test {
    address private constant WITHDRAW_RECIPIENT = address(0xBEEF);
    uint256 private constant ONE_ETH_ATOMS = 1_000_000_000;
    uint256 private constant ONE_ETH_WEI = 1 ether;
    WETHVaultVm private constant VM = WETHVaultVm(address(uint160(uint256(keccak256("hevm cheat code")))));

    WETHVaultMockToken private token;
    WETHVaultFixedFinality private finality;
    WETHBridgeVaultL1 private vault;

    function setUp() public {
        token = new WETHVaultMockToken();
        finality = new WETHVaultFixedFinality(WITHDRAW_RECIPIENT, ONE_ETH_ATOMS);
        vault = new WETHBridgeVaultL1(
            IERC20BridgeTokenV2(address(token)),
            IPFTLFinalityVerifierV1(address(finality)),
            address(token).codehash,
            address(this)
        );
    }

    function testConstructorPinsScaleAndBindings() public view {
        require(address(vault.token()) == address(token), "token drift");
        require(address(vault.finalityVerifier()) == address(finality), "verifier drift");
        require(vault.tokenRuntimeCodeHash() == address(token).codehash, "codehash drift");
        require(vault.WETH_WEI_PER_PFETH_ATOM() == 1e9, "scale drift");
        require(vault.MAX_DEPOSIT_AMOUNT() == type(uint64).max, "max amount drift");
    }

    function testOneEthDepositStoresOneBillionPfethAtoms() public {
        token.mint(address(this), ONE_ETH_WEI);
        token.approve(address(vault), ONE_ETH_WEI);
        bytes32 routeBinding = keccak256("ethereum-mainnet-weth-v1-route-binding");
        bytes32 depositId = vault.depositV2(ONE_ETH_ATOMS, "pf1pfethholder", bytes32(uint256(0x1234)), routeBinding);

        require(token.balanceOf(address(vault)) == ONE_ETH_WEI, "vault WETH delta");
        require(vault.totalObligations() == ONE_ETH_ATOMS, "pfETH obligation");
        require(vault.depositSeen(depositId), "deposit missing");

        bytes32 base = keccak256(abi.encode(depositId, uint256(1)));
        bytes32 packed = VM.load(address(vault), base);
        require(uint256(VM.load(address(vault), bytes32(0))) == ONE_ETH_ATOMS, "slot-0 atoms");
        require(address(uint160(uint256(packed))) == address(this), "packed depositor");
        require(uint256(packed) >> 160 == ONE_ETH_ATOMS, "packed amount atoms");
    }

    function testOneEthWithdrawalTransfersOneEthWeth() public {
        _deposit(2 * ONE_ETH_ATOMS);
        uint256 beforeBalance = token.balanceOf(WITHDRAW_RECIPIENT);
        vault.withdrawWithProof(bytes(""), bytes(""));
        require(token.balanceOf(WITHDRAW_RECIPIENT) - beforeBalance == ONE_ETH_WEI, "recipient WETH");
        require(vault.totalObligations() == ONE_ETH_ATOMS, "remaining atoms");
        require(token.balanceOf(address(vault)) == ONE_ETH_WEI, "remaining WETH");
    }

    function testMultipleNotesCanExceedOldWeiU64Ceiling() public {
        uint256 twentyEthAtoms = 20 * ONE_ETH_ATOMS;
        _deposit(twentyEthAtoms);
        require(vault.totalObligations() == twentyEthAtoms, "twenty ETH atoms");
        require(token.balanceOf(address(vault)) == 20 ether, "twenty WETH");
        require(twentyEthAtoms < type(uint64).max, "scaled amount must fit");
        require(20 ether > type(uint64).max, "test must exceed old raw-wei cap");
    }

    function testDepositRejectsTotalPfethSupplyOverflow() public {
        uint256 first = type(uint64).max - 1;
        token.mint(address(this), first * 1e9);
        token.approve(address(vault), first * 1e9);
        vault.depositV2(
            first, "pf1pfethholder", bytes32(uint256(1)), keccak256("ethereum-mainnet-weth-v1-route-binding")
        );

        token.mint(address(this), 2 * 1e9);
        token.approve(address(vault), 2 * 1e9);
        try vault.depositV2(
            2, "pf1pfethholder", bytes32(uint256(2)), keccak256("ethereum-mainnet-weth-v1-route-binding")
        ) {
            revert("total obligations overflow accepted");
        } catch {}
    }

    function testDepositReplayAndWithdrawalReplayReject() public {
        token.mint(address(this), 2 * ONE_ETH_WEI);
        token.approve(address(vault), 2 * ONE_ETH_WEI);
        bytes32 binding = keccak256("ethereum-mainnet-weth-v1-route-binding");
        vault.depositV2(ONE_ETH_ATOMS, "pf1pfethholder", bytes32(uint256(7)), binding);
        try vault.depositV2(ONE_ETH_ATOMS, "pf1pfethholder", bytes32(uint256(7)), binding) {
            revert("duplicate deposit accepted");
        } catch {}

        vault.withdrawWithProof(bytes(""), bytes(""));
        try vault.withdrawWithProof(bytes(""), bytes("")) {
            revert("withdrawal replay accepted");
        } catch {}
    }

    function _deposit(uint256 atoms) private {
        uint256 weiAmount = atoms * 1e9;
        token.mint(address(this), weiAmount);
        token.approve(address(vault), weiAmount);
        vault.depositV2(
            atoms,
            "pf1pfethholder",
            keccak256(abi.encode("nonce", atoms)),
            keccak256("ethereum-mainnet-weth-v1-route-binding")
        );
    }
}
