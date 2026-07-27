// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.24;

interface IPFTLReceiptFinalityVerifierV1 {
    function routeTrustClass() external view returns (bytes32);
    function isReceiptAccepted(
        bytes calldata sourceReceiptRoot,
        bytes calldata sourceReceiptHash,
        bytes calldata routeConfigDigest,
        bytes32 routeTrustClass,
        bytes32 packetDigest
    ) external view returns (bool);
}

interface IA666WrappedToken {
    function decimals() external view returns (uint8);
    function mint(address to, uint256 amount) external;
    function burnFromBridge(address from, uint256 amount) external;
}

/// @notice Immutable, permissionless, mint-only destination controller for a666.
/// @dev Swap execution is deliberately absent at launch. A proven PFTL export
///      can only mint the exact wrapped amount to its committed recipient.
contract PFTLUniswapPrimaryMarketV2 {
    struct Config {
        uint256 destinationChainId;
        bytes settlementAssetId;
        bytes nativeNavAssetId;
        bytes32 uniswapPoolId;
        uint256 routeSupplyCapAtoms;
        uint256 packetNotionalCapAtoms;
        address governance;
    }

    struct MintPacket {
        bytes routeConfigDigest;
        bytes sourcePacketHash;
        bytes reservationId;
        bytes sourceReceiptHash;
        bytes sourceReceiptRoot;
        bytes settlementAssetId;
        bytes nativeNavAssetId;
        bytes pricingReservePacketHash;
        bytes32 policyHashCommitment;
        uint64 routeEpoch;
        uint64 pricingNavEpoch;
        uint64 deadline;
        bytes32 nonce;
        uint256 destinationChainId;
        address destinationController;
        address wrappedToken;
        address ethereumRecipient;
        uint256 mintAmountAtoms;
        uint256 settlementValueAtoms;
    }

    error ZeroAddress(bytes32 field);
    error ZeroValue(bytes32 field);
    error InvalidPftlBytes(bytes32 field);
    error PacketBindingMismatch(bytes32 field);
    error DeadlineExpired(uint64 nowTimestamp, uint64 deadline);
    error PacketReplay(bytes32 packetDigest);
    error SourcePacketReplay(bytes32 sourcePacketCommitment);
    error SourceReceiptReplay(bytes32 sourceReceiptCommitment);
    error ReturnNonceReplay(bytes32 returnNonce);
    error ReceiptNotAccepted(bytes32 packetDigest);
    error PacketNotionalCapExceeded(uint256 amount, uint256 cap);
    error RouteSupplyCapExceeded(uint256 amount, uint256 cap);
    error ReentrantCall();
    error NotGovernance(address caller);
    error MintingPaused();

    event PacketConsumed(
        bytes32 indexed packetDigest,
        bytes32 indexed sourcePacketCommitment,
        bytes32 indexed sourceReceiptCommitment,
        address recipient,
        uint256 mintAmountAtoms
    );
    event ReturnBurned(
        bytes32 indexed returnBurnId,
        address indexed ethereumSender,
        bytes32 indexed returnNonce,
        string pftlRecipient,
        uint256 amountAtoms
    );
    event MintPauseSet(bool paused);

    bytes32 public constant TRUST_CLASS_TRUSTLESS_FINALITY = keccak256("TRUSTLESS_FINALITY");
    uint256 public constant A666_ROUTE_SUPPLY_CAP_ATOMS = 2_000_000e6;
    uint256 public constant A666_PACKET_NOTIONAL_CAP_ATOMS = 250_000e6;

    IA666WrappedToken public immutable wrappedToken;
    IPFTLReceiptFinalityVerifierV1 public immutable receiptVerifier;
    uint256 public immutable destinationChainId;
    bytes32 public immutable uniswapPoolId;
    uint256 public immutable routeSupplyCapAtoms;
    uint256 public immutable packetNotionalCapAtoms;
    address public immutable governance;

    bytes public settlementAssetId;
    bytes public nativeNavAssetId;

    uint256 public totalMintedAtoms;
    uint256 public totalReturnBurnedAtoms;
    mapping(bytes32 => bool) public consumedPacket;
    mapping(bytes32 => bool) public consumedSourcePacket;
    mapping(bytes32 => bool) public consumedSourceReceipt;
    mapping(bytes32 => bool) public consumedReturnNonce;
    uint256 private reentrancyLock;
    bool public mintPaused;

    modifier nonReentrant() {
        if (reentrancyLock != 0) revert ReentrantCall();
        reentrancyLock = 1;
        _;
        reentrancyLock = 0;
    }

    constructor(
        IA666WrappedToken wrappedToken_,
        IPFTLReceiptFinalityVerifierV1 receiptVerifier_,
        Config memory config
    ) {
        if (address(wrappedToken_) == address(0)) revert ZeroAddress("wrapped_token");
        if (address(receiptVerifier_) == address(0)) revert ZeroAddress("receipt_verifier");
        if (config.governance == address(0)) revert ZeroAddress("governance");
        if (
            config.destinationChainId == 0 || config.uniswapPoolId == bytes32(0)
                || config.routeSupplyCapAtoms == 0 || config.packetNotionalCapAtoms == 0
        ) revert ZeroValue("config");
        if (config.destinationChainId != block.chainid) revert PacketBindingMismatch("destination_chain_id");
        if (config.packetNotionalCapAtoms > config.routeSupplyCapAtoms) {
            revert PacketBindingMismatch("packet_notional_cap");
        }
        if (
            config.routeSupplyCapAtoms != A666_ROUTE_SUPPLY_CAP_ATOMS
                || config.packetNotionalCapAtoms != A666_PACKET_NOTIONAL_CAP_ATOMS
                || wrappedToken_.decimals() != 6
        ) revert PacketBindingMismatch("a666_launch_parameters");
        if (receiptVerifier_.routeTrustClass() != TRUST_CLASS_TRUSTLESS_FINALITY) {
            revert PacketBindingMismatch("receipt_verifier_trust_class");
        }
        _requirePftlBytes(config.settlementAssetId, "settlement_asset_id");
        _requirePftlBytes(config.nativeNavAssetId, "native_nav_asset_id");

        wrappedToken = wrappedToken_;
        receiptVerifier = receiptVerifier_;
        destinationChainId = config.destinationChainId;
        settlementAssetId = config.settlementAssetId;
        nativeNavAssetId = config.nativeNavAssetId;
        uniswapPoolId = config.uniswapPoolId;
        routeSupplyCapAtoms = config.routeSupplyCapAtoms;
        packetNotionalCapAtoms = config.packetNotionalCapAtoms;
        governance = config.governance;
        mintPaused = true;
        emit MintPauseSet(true);
    }

    function outstandingMintedAtoms() public view returns (uint256) {
        return totalMintedAtoms - totalReturnBurnedAtoms;
    }

    function consumeMintOnly(MintPacket calldata packet)
        external
        nonReentrant
        returns (bytes32 consumedDigest)
    {
        if (mintPaused) revert MintingPaused();
        consumedDigest = _packetDigest(packet);
        _validatePacket(packet, consumedDigest);
        bytes32 sourcePacketCommitment =
            keccak256(abi.encode("postfiat.pftl_uniswap.source_packet.v1", packet.sourcePacketHash));
        bytes32 sourceReceiptCommitment = keccak256(
            abi.encode(
                "postfiat.pftl_uniswap.source_receipt.v1",
                packet.sourceReceiptRoot,
                packet.sourceReceiptHash
            )
        );
        if (consumedPacket[consumedDigest]) revert PacketReplay(consumedDigest);
        if (consumedSourcePacket[sourcePacketCommitment]) revert SourcePacketReplay(sourcePacketCommitment);
        if (consumedSourceReceipt[sourceReceiptCommitment]) revert SourceReceiptReplay(sourceReceiptCommitment);
        if (
            !receiptVerifier.isReceiptAccepted(
                packet.sourceReceiptRoot,
                packet.sourceReceiptHash,
                packet.routeConfigDigest,
                TRUST_CLASS_TRUSTLESS_FINALITY,
                consumedDigest
            )
        ) revert ReceiptNotAccepted(consumedDigest);

        uint256 outstandingAfter = outstandingMintedAtoms() + packet.mintAmountAtoms;
        if (outstandingAfter > routeSupplyCapAtoms) {
            revert RouteSupplyCapExceeded(outstandingAfter, routeSupplyCapAtoms);
        }
        consumedPacket[consumedDigest] = true;
        consumedSourcePacket[sourcePacketCommitment] = true;
        consumedSourceReceipt[sourceReceiptCommitment] = true;
        totalMintedAtoms += packet.mintAmountAtoms;
        wrappedToken.mint(packet.ethereumRecipient, packet.mintAmountAtoms);
        emit PacketConsumed(
            consumedDigest,
            sourcePacketCommitment,
            sourceReceiptCommitment,
            packet.ethereumRecipient,
            packet.mintAmountAtoms
        );
    }

    function setMintPaused(bool paused) external {
        if (msg.sender != governance) revert NotGovernance(msg.sender);
        mintPaused = paused;
        emit MintPauseSet(paused);
    }

    function burnForPftlReturn(
        uint256 amountAtoms,
        string calldata pftlRecipient,
        bytes calldata destinationNativeNavAssetId,
        bytes32 returnNonce
    ) external nonReentrant returns (bytes32 returnBurnId) {
        if (amountAtoms == 0 || bytes(pftlRecipient).length == 0 || returnNonce == bytes32(0)) {
            revert ZeroValue("return");
        }
        _requirePftlBytes(destinationNativeNavAssetId, "destination_native_nav_asset_id");
        if (keccak256(destinationNativeNavAssetId) != keccak256(nativeNavAssetId)) {
            revert PacketBindingMismatch("destination_native_nav_asset_id");
        }
        if (consumedReturnNonce[returnNonce]) revert ReturnNonceReplay(returnNonce);
        consumedReturnNonce[returnNonce] = true;
        totalReturnBurnedAtoms += amountAtoms;
        wrappedToken.burnFromBridge(msg.sender, amountAtoms);
        returnBurnId = keccak256(
            abi.encode(
                "postfiat.pftl_uniswap.return_burn.v1",
                block.chainid,
                address(this),
                address(wrappedToken),
                destinationNativeNavAssetId,
                msg.sender,
                pftlRecipient,
                amountAtoms,
                returnNonce,
                block.number
            )
        );
        emit ReturnBurned(returnBurnId, msg.sender, returnNonce, pftlRecipient, amountAtoms);
    }

    function packetDigest(MintPacket calldata packet) external pure returns (bytes32) {
        return _packetDigest(packet);
    }

    function _validatePacket(MintPacket calldata packet, bytes32 computedPacketDigest) private view {
        uint64 nowTimestamp = uint64(block.timestamp);
        if (packet.deadline < nowTimestamp) revert DeadlineExpired(nowTimestamp, packet.deadline);
        if (packet.mintAmountAtoms == 0) revert ZeroValue("mint_amount_atoms");
        if (packet.mintAmountAtoms > packetNotionalCapAtoms) {
            revert PacketNotionalCapExceeded(packet.mintAmountAtoms, packetNotionalCapAtoms);
        }
        if (packet.nonce == bytes32(0) || computedPacketDigest == bytes32(0)) {
            revert ZeroValue("packet");
        }
        _requirePftlBytes(packet.routeConfigDigest, "route_config_digest");
        _requirePftlBytes(packet.sourcePacketHash, "source_packet_hash");
        _requirePftlBytes(packet.reservationId, "reservation_id");
        _requirePftlBytes(packet.sourceReceiptHash, "source_receipt_hash");
        _requirePftlBytes(packet.sourceReceiptRoot, "source_receipt_root");
        _requirePftlBytes(packet.settlementAssetId, "settlement_asset_id");
        _requirePftlBytes(packet.nativeNavAssetId, "native_nav_asset_id");
        _requirePftlBytes(packet.pricingReservePacketHash, "pricing_reserve_packet_hash");
        if (
            keccak256(packet.settlementAssetId) != keccak256(settlementAssetId)
                || keccak256(packet.nativeNavAssetId) != keccak256(nativeNavAssetId)
                || packet.policyHashCommitment == bytes32(0) || packet.routeEpoch == 0
                || packet.pricingNavEpoch == 0 || packet.destinationChainId != destinationChainId
                || packet.destinationController != address(this) || packet.wrappedToken != address(wrappedToken)
                || packet.ethereumRecipient == address(0)
        ) revert PacketBindingMismatch("packet");
    }

    function _packetDigest(MintPacket calldata packet) private pure returns (bytes32) {
        bytes memory identifiers = abi.encodePacked(
            keccak256("postfiat.pftl_uniswap.mint_packet.v2"),
            keccak256(packet.routeConfigDigest),
            keccak256(packet.sourcePacketHash),
            keccak256(packet.reservationId),
            keccak256(packet.settlementAssetId),
            keccak256(packet.nativeNavAssetId),
            keccak256(packet.pricingReservePacketHash),
            packet.policyHashCommitment
        );
        bytes memory destination = abi.encodePacked(
            packet.routeEpoch,
            packet.pricingNavEpoch,
            packet.deadline,
            packet.nonce,
            packet.destinationChainId,
            packet.destinationController,
            packet.wrappedToken,
            packet.ethereumRecipient,
            packet.mintAmountAtoms,
            packet.settlementValueAtoms
        );
        return keccak256(bytes.concat(identifiers, destination));
    }

    function _requirePftlBytes(bytes memory value, bytes32 field) private pure {
        if (value.length != 48) revert InvalidPftlBytes(field);
        bool nonzero;
        for (uint256 i = 0; i < value.length; i++) {
            if (value[i] != 0) {
                nonzero = true;
                break;
            }
        }
        if (!nonzero) revert InvalidPftlBytes(field);
    }
}
