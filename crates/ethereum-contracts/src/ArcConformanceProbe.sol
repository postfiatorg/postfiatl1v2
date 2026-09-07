// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.24;

/// @notice Testnet-only probe for the precompiles required by pfUSDC proof verification.
contract ArcConformanceProbe {
    error InvalidInputLength(uint256 expectedMultiple, uint256 actual);
    error PrecompileCallFailed(address precompile);
    error InvalidOutputLength(uint256 expected, uint256 actual);

    function sha256Probe(bytes calldata input) external view returns (bytes32 digest, uint256 gasUsed) {
        uint256 gasBefore = gasleft();
        digest = sha256(input);
        gasUsed = gasBefore - gasleft();
    }

    function bn254Add(bytes calldata input) external view returns (bytes memory output, uint256 gasUsed) {
        if (input.length != 128) revert InvalidInputLength(128, input.length);
        return _run(0x0000000000000000000000000000000000000006, input, 64);
    }

    function bn254Mul(bytes calldata input) external view returns (bytes memory output, uint256 gasUsed) {
        if (input.length != 96) revert InvalidInputLength(96, input.length);
        return _run(0x0000000000000000000000000000000000000007, input, 64);
    }

    function bn254Pairing(bytes calldata input) external view returns (bool valid, uint256 gasUsed) {
        if (input.length % 192 != 0) revert InvalidInputLength(192, input.length);
        (bytes memory output, uint256 measured) = _run(0x0000000000000000000000000000000000000008, input, 32);
        uint256 result;
        assembly ("memory-safe") {
            result := mload(add(output, 0x20))
        }
        return (result == 1, measured);
    }

    function _run(address precompile, bytes memory input, uint256 outputLength)
        private
        view
        returns (bytes memory output, uint256 gasUsed)
    {
        output = new bytes(outputLength);
        bool ok;
        uint256 target = uint160(precompile);
        uint256 gasBefore = gasleft();
        assembly ("memory-safe") {
            ok := staticcall(gas(), target, add(input, 0x20), mload(input), add(output, 0x20), outputLength)
        }
        gasUsed = gasBefore - gasleft();
        if (!ok) revert PrecompileCallFailed(precompile);
        if (output.length != outputLength) revert InvalidOutputLength(outputLength, output.length);
    }
}
