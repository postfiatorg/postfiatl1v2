// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.24;

/// @title ExitExecutorV1
/// @notice EIP-7702 delegate for gasless, relayer-sponsored execution from a
///         fresh EOA. The EOA signs a 7702 authorization pointing its code at
///         this contract, plus an EIP-712 signature over a call batch. Any
///         relayer submits the type-4 transaction and pays gas. The EOA never
///         needs ETH.
///
///         Storage lives in the delegating EOA under an ERC-7201 namespaced
///         slot, so a later delegation to a different implementation cannot
///         collide with the nonce.
///
///         No owner, no upgrade path, no custody: the contract executes only
///         batches signed by the EOA whose code it is running as.
contract ExitExecutorV1 {
    struct Call {
        address to;
        uint256 value;
        bytes data;
    }

    /// @custom:storage-location erc7201:postfiat.exit_executor.v1
    struct Storage {
        uint256 nonce;
    }

    // keccak256(abi.encode(uint256(keccak256("postfiat.exit_executor.v1")) - 1)) & ~bytes32(uint256(0xff))
    bytes32 private constant STORAGE_SLOT = 0xc2a1d4a08cd33c1c4e97e37dbec10e2dad7bd4fbecd8a90603ed905d9e37fb00;

    bytes32 private constant CALL_TYPEHASH = keccak256("Call(address to,uint256 value,bytes data)");
    bytes32 private constant BATCH_TYPEHASH =
        keccak256("Batch(Call[] calls,uint256 nonce,uint256 deadline)Call(address to,uint256 value,bytes data)");
    bytes32 private constant DOMAIN_TYPEHASH =
        keccak256("EIP712Domain(string name,string version,uint256 chainId,address verifyingContract,bytes32 salt)");
    bytes32 private constant NAME_HASH = keccak256("PostFiatExitExecutor");
    bytes32 private constant VERSION_HASH = keccak256("1");

    /// @dev Implementation address, captured at deploy. Under 7702 the code runs
    ///      with address(this) == EOA, so this immutable is the only way to bind
    ///      signatures to this specific implementation.
    address private immutable IMPLEMENTATION;

    event BatchExecuted(address indexed account, uint256 indexed nonce, uint256 calls);

    error Expired(uint256 deadline, uint256 nowTs);
    error BadNonce(uint256 expected, uint256 got);
    error BadSigner(address recovered);
    error BadSignatureLength(uint256 length);
    error BadSignatureS();
    error CallFailed(uint256 index, address to, bytes returnData);
    error NotSelf();

    constructor() {
        IMPLEMENTATION = address(this);
    }

    receive() external payable {}

    function nonce() external view returns (uint256) {
        return _storage().nonce;
    }

    function implementation() external view returns (address) {
        return IMPLEMENTATION;
    }

    /// @notice Executes a signed batch. Anyone may call; the EOA pays nothing.
    /// @param calls   Calls to execute from the EOA, in order. All must succeed.
    /// @param nonce_  Must equal the current nonce; incremented before execution.
    /// @param deadline Unix time after which the batch is invalid.
    /// @param sig     65-byte EIP-712 signature by the EOA (address(this)).
    function execute(Call[] calldata calls, uint256 nonce_, uint256 deadline, bytes calldata sig) external payable {
        if (block.timestamp > deadline) revert Expired(deadline, block.timestamp);
        Storage storage s = _storage();
        if (nonce_ != s.nonce) revert BadNonce(s.nonce, nonce_);
        s.nonce = nonce_ + 1;

        address signer = _recover(_digest(calls, nonce_, deadline), sig);
        if (signer != address(this)) revert BadSigner(signer);

        _run(calls);
        emit BatchExecuted(address(this), nonce_, calls.length);
    }

    /// @notice Direct path for an EOA that does hold gas: the EOA sends a
    ///         transaction to itself. No signature or nonce needed; the
    ///         transaction's own signature and nonce cover it.
    function executeSelf(Call[] calldata calls) external payable {
        if (msg.sender != address(this)) revert NotSelf();
        _run(calls);
    }

    /// @notice EIP-712 digest a wallet must sign for `execute`.
    function digest(Call[] calldata calls, uint256 nonce_, uint256 deadline) external view returns (bytes32) {
        return _digest(calls, nonce_, deadline);
    }

    function domainSeparator() external view returns (bytes32) {
        return _domainSeparator();
    }

    function _run(Call[] calldata calls) private {
        uint256 n = calls.length;
        for (uint256 i = 0; i < n; ++i) {
            Call calldata c = calls[i];
            (bool ok, bytes memory ret) = c.to.call{value: c.value}(c.data);
            if (!ok) revert CallFailed(i, c.to, ret);
        }
    }

    function _digest(Call[] calldata calls, uint256 nonce_, uint256 deadline) private view returns (bytes32) {
        uint256 n = calls.length;
        bytes32[] memory callHashes = new bytes32[](n);
        for (uint256 i = 0; i < n; ++i) {
            callHashes[i] = keccak256(abi.encode(CALL_TYPEHASH, calls[i].to, calls[i].value, keccak256(calls[i].data)));
        }
        bytes32 structHash =
            keccak256(abi.encode(BATCH_TYPEHASH, keccak256(abi.encodePacked(callHashes)), nonce_, deadline));
        return keccak256(abi.encodePacked("\x19\x01", _domainSeparator(), structHash));
    }

    function _domainSeparator() private view returns (bytes32) {
        // verifyingContract = the EOA; salt = implementation address, so a
        // batch signed for this delegate cannot be replayed against another.
        return keccak256(
            abi.encode(
                DOMAIN_TYPEHASH,
                NAME_HASH,
                VERSION_HASH,
                block.chainid,
                address(this),
                bytes32(uint256(uint160(IMPLEMENTATION)))
            )
        );
    }

    function _recover(bytes32 hash, bytes calldata sig) private pure returns (address) {
        if (sig.length != 65) revert BadSignatureLength(sig.length);
        bytes32 r;
        bytes32 s;
        uint8 v;
        assembly {
            r := calldataload(sig.offset)
            s := calldataload(add(sig.offset, 32))
            v := byte(0, calldataload(add(sig.offset, 64)))
        }
        // EIP-2: reject high-s.
        if (uint256(s) > 0x7FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF5D576E7357A4501DDFE92F46681B20A0) revert BadSignatureS();
        if (v < 27) v += 27;
        address a = ecrecover(hash, v, r, s);
        if (a == address(0)) revert BadSigner(a);
        return a;
    }

    function _storage() private pure returns (Storage storage s) {
        bytes32 slot = STORAGE_SLOT;
        assembly {
            s.slot := slot
        }
    }
}
