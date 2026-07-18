// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.24;

import {IPfUsdcIngressAnchorV1} from "./ERC20BridgeVaultV2.sol";

interface IArbitrumBridgeV1 {
    function activeOutbox() external view returns (address);
}

interface IArbitrumOutboxV1 {
    function l2ToL1Sender() external view returns (address);
}

/// @notice Parent-chain destination for canonical pfUSDC Tier-4 Nitro sends.
/// @dev PFTL authorization comes from the finalized Nitro sendRoot proof. This
///      contract gives the output a production, code-hash-pinned destination
///      and also validates the message if an operator executes it on Ethereum.
contract PfUsdcIngressAnchorV1 is IPfUsdcIngressAnchorV1 {
    struct DepositRecordV1 {
        bytes32 depositId;
        address depositor;
        bytes32 pftlRecipientHash;
        string pftlRecipient;
        uint256 amount;
        bytes32 nonce;
        bytes32 routeBinding;
        uint256 sourceChainId;
        address vault;
        address token;
    }

    error NotActiveOutbox(address caller, address activeOutbox);
    error WrongL2Sender(address sender, address expected);
    error WrongRouteField(bytes32 field);
    error InvalidDeposit();
    error DuplicateDeposit(bytes32 depositId);

    event Tier4DepositRecorded(
        bytes32 indexed depositId,
        address indexed depositor,
        bytes32 indexed pftlRecipientHash,
        string pftlRecipient,
        uint256 amount,
        bytes32 nonce,
        bytes32 routeBinding
    );

    uint256 public constant MAX_PFTL_RECIPIENT_BYTES = 256;

    IArbitrumBridgeV1 public immutable bridge;
    address public immutable l2Vault;
    address public immutable l2Token;
    uint256 public immutable l2ChainId;
    bytes32 public immutable governedRouteBinding;
    mapping(bytes32 => bool) public depositSeen;

    constructor(
        IArbitrumBridgeV1 bridge_,
        address l2Vault_,
        address l2Token_,
        uint256 l2ChainId_,
        bytes32 governedRouteBinding_
    ) {
        if (
            address(bridge_) == address(0) || l2Vault_ == address(0) || l2Token_ == address(0) || l2ChainId_ == 0
                || governedRouteBinding_ == bytes32(0)
        ) revert InvalidDeposit();
        bridge = bridge_;
        l2Vault = l2Vault_;
        l2Token = l2Token_;
        l2ChainId = l2ChainId_;
        governedRouteBinding = governedRouteBinding_;
    }

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
    ) external {
        // The function ABI encodes ten top-level fields. A single dynamic
        // tuple decode additionally expects the standard 0x20 head offset.
        DepositRecordV1 memory deposit = abi.decode(bytes.concat(bytes32(uint256(32)), msg.data[4:]), (DepositRecordV1));
        _recordDeposit(deposit);
    }

    function _recordDeposit(DepositRecordV1 memory deposit) private {
        address activeOutbox = bridge.activeOutbox();
        if (msg.sender != activeOutbox) revert NotActiveOutbox(msg.sender, activeOutbox);
        address l2Sender = IArbitrumOutboxV1(msg.sender).l2ToL1Sender();
        if (l2Sender != l2Vault) revert WrongL2Sender(l2Sender, l2Vault);
        if (deposit.routeBinding != governedRouteBinding) revert WrongRouteField("route_binding");
        if (deposit.sourceChainId != l2ChainId) revert WrongRouteField("source_chain_id");
        if (deposit.vault != l2Vault) revert WrongRouteField("vault");
        if (deposit.token != l2Token) revert WrongRouteField("token");
        bytes memory recipient = bytes(deposit.pftlRecipient);
        if (
            deposit.depositId == bytes32(0) || deposit.depositor == address(0)
                || deposit.pftlRecipientHash == bytes32(0) || recipient.length == 0
                || recipient.length > MAX_PFTL_RECIPIENT_BYTES || deposit.amount == 0 || deposit.nonce == bytes32(0)
                || keccak256(recipient) != deposit.pftlRecipientHash
        ) revert InvalidDeposit();
        if (depositSeen[deposit.depositId]) revert DuplicateDeposit(deposit.depositId);
        depositSeen[deposit.depositId] = true;
        emit Tier4DepositRecorded(
            deposit.depositId,
            deposit.depositor,
            deposit.pftlRecipientHash,
            deposit.pftlRecipient,
            deposit.amount,
            deposit.nonce,
            deposit.routeBinding
        );
    }
}
