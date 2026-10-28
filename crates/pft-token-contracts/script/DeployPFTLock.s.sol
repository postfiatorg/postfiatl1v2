// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.24;

import {PFTLockV1} from "../src/PFTLockV1.sol";

interface IPFTLockDeployVm {
    function envString(string calldata name) external view returns (string memory);
    function envOr(string calldata name, string calldata defaultValue) external view returns (string memory);
    function parseUint(string calldata value) external pure returns (uint256);
    function parseAddress(string calldata value) external pure returns (address);
    function parseBytes32(string calldata value) external pure returns (bytes32);
    function toString(bytes32 value) external pure returns (string memory);
    function toString(uint256 value) external pure returns (string memory);
    function toString(bytes calldata value) external pure returns (string memory);
    function getNonce(address account) external view returns (uint64);
    function exists(string calldata path) external view returns (bool);
    function isContext(uint8 context) external view returns (bool);
    function startBroadcast(address signer) external;
    function stopBroadcast() external;
    function readFile(string calldata path) external view returns (string memory);
    function parseJsonString(string calldata json, string calldata key) external pure returns (string memory);
    function parseJsonUint(string calldata json, string calldata key) external pure returns (uint256);
    function parseJsonBool(string calldata json, string calldata key) external pure returns (bool);
    function parseJsonBytes(string calldata json, string calldata key) external pure returns (bytes memory);
    function serializeString(string calldata objectKey, string calldata valueKey, string calldata value)
        external
        returns (string memory);
    function serializeAddress(string calldata objectKey, string calldata valueKey, address value)
        external
        returns (string memory);
    function serializeUint(string calldata objectKey, string calldata valueKey, uint256 value)
        external
        returns (string memory);
    function serializeBool(string calldata objectKey, string calldata valueKey, bool value)
        external
        returns (string memory);
    function serializeBytes32(string calldata objectKey, string calldata valueKey, bytes32 value)
        external
        returns (string memory);
    function createDir(string calldata path, bool recursive) external;
    function writeFile(string calldata path, string calldata data) external;
}

/// The two token reads the script makes, as checked staticcalls.
interface IPFTTokenProbe {
    function decimals() external view returns (uint8);
    function totalSupply() external view returns (uint256);
}

/// @notice Foundry deployment script for the Phase 2 lock contract `PFTLockV1` on Base.
/// @dev The pattern and guards of `script/DeployPFTToken.s.sol`: no forge-std
///      dependency, values from the environment (`script/pft-lock.env.example`),
///      the RPC URL and the one-time deployer key on the `forge script` command
///      line only, every check before the broadcast. Design note
///      `docs/specs/pft-erc20-base-bridge-phase2-design-20261026.md` sections 5
///      and 6: the PFTL finality verifier is deployed on Base first, then this
///      lock contract; genesis later names the lock's address and runtime code hash.
contract DeployPFTLock {
    IPFTLockDeployVm private constant VM = IPFTLockDeployVm(address(uint160(uint256(keccak256("hevm cheat code")))));
    address private constant CONSOLE = 0x000000000000000000636F6e736F6c652e6c6f67;

    uint256 public constant BASE = 8453;
    uint256 public constant BASE_SEPOLIA = 84532;
    uint256 public constant UNITS_PER_PFT = 1_000_000;
    uint8 public constant TOKEN_DECIMALS = 6;
    string public constant OPENZEPPELIN_COMMIT = "cab19933c33c2ad1d4c7a84864a3601dddfd16f3";
    /// The Base Sepolia test token and its runtime keccak256, from
    /// `deployments/pft-token-base-84532/deployment.json` and
    /// `docs/listings/coinbase/pft-token-dry-run-20261026.md`: the expected
    /// PFT_LOCK_TOKEN and PFT_LOCK_TOKEN_CODEHASH on 84532, required there unless
    /// the test-only PFT_LOCK_ALLOW_TEST_TOKEN=1 is set.
    address public constant BASE_SEPOLIA_TEST_TOKEN = 0x18fE4ca51A0cE042688D5A54917319d03B527D92;
    bytes32 public constant BASE_SEPOLIA_TEST_TOKEN_CODEHASH =
        0xb0939a2407cd571d158498ba0f405f4ddb3857e688d2bc5ada170dcc655903df;
    /// `ForgeContext.ScriptBroadcast` in forge-std's Vm.
    uint8 private constant SCRIPT_BROADCAST = 6;
    /// `ForgeContext.Coverage` in forge-std's Vm.
    uint8 private constant COVERAGE = 2;
    /// The compiled lock contract under forge's output directory (FOUNDRY_OUT,
    /// else out); its creation code must be the code this script deploys.
    string public constant LOCK_ARTIFACT = "/PFTLockV1.sol/PFTLockV1.json";
    /// The artifact's runtime code must exceed its CBOR trailer by at least this many bytes.
    uint256 public constant MIN_RUNTIME_CODE_BEYOND_TRAILER = 512;

    struct Config {
        uint256 chainId;
        address token;
        bytes32 tokenCodeHash; // PFT_LOCK_TOKEN_CODEHASH: the token's runtime keccak256
        address verifier;
        bytes32 verifierCodeHash; // PFT_LOCK_VERIFIER_CODEHASH; optional only under the test verifier switch
        uint256 maxLockAmount; // six-decimal units, at most type(uint64).max
        uint256 maxLockAmountPft; // whole PFT
        bool allowTestVerifier; // Base Sepolia only: admits a verifier whose code hash is not pinned
        bool allowUsedDeployer; // Base Sepolia only
        bool allowTestToken; // Base Sepolia only: admits a token other than the recorded test token
        bool realDeployment; // PFT_LOCK_REAL_DEPLOYMENT=1: the durable record under deployments/
        string recordDir; // deployments/... for a real deployment, else cache/deploy-test/...
        bool writeRecord; // only `forge script --broadcast` writes the record
        string sourceCommit; // PFT_LOCK_SOURCE_COMMIT, required for a real deployment
        string artifact; // the compiled lock contract; must be the code deployed
    }

    /// What the script checked on chain before the broadcast.
    struct Checked {
        uint256 tokenDecimals;
        uint256 tokenSupply;
        bytes32 verifierCodeHash; // the verifier's code hash on chain
        bool verifierPinned; // equals PFT_LOCK_VERIFIER_CODEHASH
    }

    error PlaceholderValue(string name);
    error UnsupportedChain(uint256 chainId);
    error WrongChain(uint256 expected, uint256 actual);
    error TestSwitchOnMainnet(string name);
    error RealDeploymentWithTestSwitch(string name);
    error SourceCommitMissing();
    error SourceCommitInvalid(string value);
    error TokenNotTheTestToken(address token);
    error TokenNotAContract(address token);
    error TokenCodeHashMismatch(bytes32 expected, bytes32 actual);
    error TokenUnreadable(string field);
    error TokenDecimalsMismatch(uint256 decimals);
    error TokenSupplyZero();
    error VerifierNotAContract(address verifier);
    error VerifierCodeHashMismatch(bytes32 expected, bytes32 actual);
    error MaxLockAmountTooLarge(uint256 units);
    error MaxLockAmountMismatch(uint256 units, uint256 pft);
    error DeployerAlreadyUsed(address deployer, uint256 nonce);
    error RecordExists(string path);
    error ArtifactMismatch(string path, bytes32 artifactCodeHash, bytes32 compiledCodeHash);
    error MetadataHashDisabled();
    error ArtifactMetadataMismatch(string path);
    error LockStateMismatch(string field);
    error DeployedCodeMismatch(address lock, uint256 codeLength, uint256 artifactLength);

    event PFTLockDeployed(
        address indexed lock, address indexed token, address indexed finalityVerifier, uint64 maxLockAmount
    );

    function run() external returns (PFTLockV1) {
        return deploy(readConfig());
    }

    /// Reads the deployment values from the environment. The record is written
    /// only by `forge script --broadcast`; only PFT_LOCK_REAL_DEPLOYMENT=1
    /// selects the durable deployments/ path, every other run the ignored cache/deploy-test/.
    function readConfig() public view returns (Config memory cfg) {
        cfg.chainId = VM.parseUint(_required("PFT_LOCK_CHAIN_ID"));
        cfg.token = VM.parseAddress(_required("PFT_LOCK_TOKEN"));
        cfg.tokenCodeHash = VM.parseBytes32(_required("PFT_LOCK_TOKEN_CODEHASH"));
        cfg.verifier = VM.parseAddress(_required("PFT_LOCK_VERIFIER"));
        cfg.allowTestVerifier = _switch("PFT_LOCK_ALLOW_TEST_VERIFIER");
        string memory verifierHash = cfg.allowTestVerifier
            ? VM.envOr("PFT_LOCK_VERIFIER_CODEHASH", string(""))
            : _required("PFT_LOCK_VERIFIER_CODEHASH");
        if (!_isPlaceholder(bytes(verifierHash))) cfg.verifierCodeHash = VM.parseBytes32(verifierHash);
        cfg.maxLockAmount = VM.parseUint(_required("PFT_LOCK_MAX_LOCK_AMOUNT"));
        cfg.maxLockAmountPft = VM.parseUint(_required("PFT_LOCK_MAX_LOCK_AMOUNT_PFT"));
        cfg.allowUsedDeployer = _switch("PFT_LOCK_ALLOW_USED_DEPLOYER");
        cfg.allowTestToken = _switch("PFT_LOCK_ALLOW_TEST_TOKEN");
        cfg.realDeployment = _switch("PFT_LOCK_REAL_DEPLOYMENT");
        string memory root = cfg.realDeployment ? "deployments/" : "cache/deploy-test/";
        cfg.recordDir = string.concat(root, "pft-lock-base-", VM.toString(block.chainid));
        cfg.writeRecord = VM.isContext(SCRIPT_BROADCAST);
        cfg.sourceCommit = VM.envOr("PFT_LOCK_SOURCE_COMMIT", string(""));
        string memory out = VM.envOr("FOUNDRY_OUT", string(""));
        cfg.artifact = string.concat(bytes(out).length == 0 ? "out" : out, LOCK_ARTIFACT);
    }

    /// Checks `cfg` against the target chain, deploys from `msg.sender` (the
    /// broadcaster), re-reads the deployed lock and writes the record.
    function deploy(Config memory cfg) public returns (PFTLockV1 lock) {
        address deployer = msg.sender;
        _checkChain(cfg);
        Checked memory checked;
        (checked.tokenDecimals, checked.tokenSupply) = _checkToken(cfg);
        (checked.verifierCodeHash, checked.verifierPinned) = _checkVerifier(cfg);
        _checkCap(cfg);
        uint256 nonce = VM.getNonce(deployer);
        if (nonce != 0 && !cfg.allowUsedDeployer) revert DeployerAlreadyUsed(deployer, nonce);
        string memory recordPath = string.concat(cfg.recordDir, "/deployment.json");
        if (cfg.realDeployment && VM.exists(recordPath)) revert RecordExists(recordPath);
        string memory artifact = _checkArtifact(cfg);

        _log("chain id", block.chainid);
        _log("deployer", deployer);
        _log("deployer nonce", nonce);
        _log("token", cfg.token);
        _log("token code hash", VM.toString(cfg.token.codehash));
        _log("token decimals", checked.tokenDecimals);
        _log("token total supply (six-decimal units)", checked.tokenSupply);
        _log("finality verifier", cfg.verifier);
        _log("finality verifier code hash", VM.toString(checked.verifierCodeHash));
        _log("finality verifier pinned code hash", VM.toString(cfg.verifierCodeHash));
        if (!checked.verifierPinned) {
            _log("WARNING: test-only PFT_LOCK_ALLOW_TEST_VERIFIER; the verifier's code hash is not pinned");
        }
        _log("max lock amount (six-decimal units)", cfg.maxLockAmount);
        _log("max lock amount (whole PFT)", cfg.maxLockAmountPft);
        _log("lock artifact", cfg.artifact);
        _log("deployment record", cfg.realDeployment ? "real" : "test");

        VM.startBroadcast(deployer);
        lock = new PFTLockV1(cfg.token, cfg.verifier, cfg.tokenCodeHash, uint64(cfg.maxLockAmount));
        VM.stopBroadcast();
        checkDeployedCode(address(lock), VM.parseJsonBytes(artifact, ".deployedBytecode.object"));

        checkLock(cfg, lock);
        emit PFTLockDeployed(address(lock), cfg.token, cfg.verifier, uint64(cfg.maxLockAmount));
        _log("PFTLockV1 deployed at", address(lock));
        _log("PFTLockV1 runtime code hash", VM.toString(address(lock).codehash));
        if (cfg.writeRecord) _writeRecord(cfg, lock, deployer, checked, artifact, recordPath);
    }

    /// The deployed lock must hold exactly what was checked, with nothing locked yet.
    function checkLock(Config memory cfg, PFTLockV1 lock) public view {
        address token = lock.token();
        address verifier = lock.finalityVerifier();
        uint64 cap = lock.maxLockAmount();
        uint64 nextNonce = lock.nextNonce();
        uint256 totalLocked = lock.totalLocked();
        _log("lock token()", token);
        _log("lock finalityVerifier()", verifier);
        _log("lock maxLockAmount()", cap);
        _log("lock nextNonce()", nextNonce);
        _log("lock totalLocked()", totalLocked);
        if (token != cfg.token) revert LockStateMismatch("token");
        if (verifier != cfg.verifier) revert LockStateMismatch("finality_verifier");
        if (cap != cfg.maxLockAmount) revert LockStateMismatch("max_lock_amount");
        if (nextNonce != 0) revert LockStateMismatch("next_nonce");
        if (totalLocked != 0) revert LockStateMismatch("total_locked");
    }

    function _checkChain(Config memory cfg) private view {
        if (cfg.chainId != BASE && cfg.chainId != BASE_SEPOLIA) revert UnsupportedChain(cfg.chainId);
        if (cfg.chainId != block.chainid) revert WrongChain(cfg.chainId, block.chainid);
        if (cfg.chainId == BASE) {
            if (cfg.allowTestVerifier) revert TestSwitchOnMainnet("PFT_LOCK_ALLOW_TEST_VERIFIER");
            if (cfg.allowUsedDeployer) revert TestSwitchOnMainnet("PFT_LOCK_ALLOW_USED_DEPLOYER");
            if (cfg.allowTestToken) revert TestSwitchOnMainnet("PFT_LOCK_ALLOW_TEST_TOKEN");
        }
        if (cfg.realDeployment) {
            if (cfg.allowTestVerifier) revert RealDeploymentWithTestSwitch("PFT_LOCK_ALLOW_TEST_VERIFIER");
            if (cfg.allowUsedDeployer) revert RealDeploymentWithTestSwitch("PFT_LOCK_ALLOW_USED_DEPLOYER");
            if (cfg.allowTestToken) revert RealDeploymentWithTestSwitch("PFT_LOCK_ALLOW_TEST_TOKEN");
            if (_isPlaceholder(bytes(cfg.sourceCommit))) revert SourceCommitMissing();
            if (!_isCommitHash(bytes(cfg.sourceCommit))) revert SourceCommitInvalid(cfg.sourceCommit);
        }
    }

    /// Exactly 40 hexadecimal characters, either casing: the shape of a git
    /// commit. That it names the deployed source is the operator's statement.
    function _isCommitHash(bytes memory value) private pure returns (bool) {
        if (value.length != 40) return false;
        for (uint256 i = 0; i < 40; i++) {
            bytes1 c = value[i];
            if (!((c >= "0" && c <= "9") || (c >= "a" && c <= "f") || (c >= "A" && c <= "F"))) return false;
        }
        return true;
    }

    /// The token has code, the pinned runtime code hash, six decimals and a
    /// supply; on 84532 it is the recorded test token with its recorded code hash
    /// unless PFT_LOCK_ALLOW_TEST_TOKEN (refused on 8453 and for a real deployment).
    function _checkToken(Config memory cfg) private view returns (uint256 decimals, uint256 supply) {
        if (cfg.token == address(0)) revert PlaceholderValue("PFT_LOCK_TOKEN");
        if (cfg.tokenCodeHash == 0) revert PlaceholderValue("PFT_LOCK_TOKEN_CODEHASH");
        if (
            cfg.chainId == BASE_SEPOLIA && !cfg.allowTestToken
                && (cfg.token != BASE_SEPOLIA_TEST_TOKEN || cfg.tokenCodeHash != BASE_SEPOLIA_TEST_TOKEN_CODEHASH)
        ) revert TokenNotTheTestToken(cfg.token);
        if (cfg.token.code.length == 0) revert TokenNotAContract(cfg.token);
        if (cfg.token.codehash != cfg.tokenCodeHash) {
            revert TokenCodeHashMismatch(cfg.tokenCodeHash, cfg.token.codehash);
        }
        decimals = _readUint(cfg.token, IPFTTokenProbe.decimals.selector, "decimals");
        if (decimals != TOKEN_DECIMALS) revert TokenDecimalsMismatch(decimals);
        supply = _readUint(cfg.token, IPFTTokenProbe.totalSupply.selector, "totalSupply");
        if (supply == 0) revert TokenSupplyZero();
    }

    /// The verifier has code whose hash is PFT_LOCK_VERIFIER_CODEHASH. The only
    /// relaxation: PFT_LOCK_ALLOW_TEST_VERIFIER admits another hash on 84532
    /// (refused on 8453 and for a real deployment by _checkChain).
    function _checkVerifier(Config memory cfg) private view returns (bytes32 codehash, bool pinned) {
        if (cfg.verifier == address(0)) revert PlaceholderValue("PFT_LOCK_VERIFIER");
        if (cfg.verifier.code.length == 0) revert VerifierNotAContract(cfg.verifier);
        codehash = cfg.verifier.codehash;
        pinned = codehash == cfg.verifierCodeHash;
        if (!pinned && !cfg.allowTestVerifier) revert VerifierCodeHashMismatch(cfg.verifierCodeHash, codehash);
    }

    function _checkCap(Config memory cfg) private pure {
        if (cfg.maxLockAmount == 0) revert PlaceholderValue("PFT_LOCK_MAX_LOCK_AMOUNT");
        if (cfg.maxLockAmount > type(uint64).max) revert MaxLockAmountTooLarge(cfg.maxLockAmount);
        if (
            cfg.maxLockAmountPft > type(uint64).max / UNITS_PER_PFT
                || cfg.maxLockAmount != cfg.maxLockAmountPft * UNITS_PER_PFT
        ) {
            revert MaxLockAmountMismatch(cfg.maxLockAmount, cfg.maxLockAmountPft);
        }
    }

    /// A checked staticcall returning one word.
    function _readUint(address target, bytes4 selector, string memory field) private view returns (uint256) {
        (bool ok, bytes memory data) = target.staticcall(abi.encodeWithSelector(selector));
        if (!ok || data.length != 32) revert TokenUnreadable(field);
        return abi.decode(data, (uint256));
    }

    /// The artifact whose metadata goes in the record must be this build of the
    /// lock contract: its creation code is the code `deploy` sends, and its
    /// metadata is that code's (see DeployPFTToken._checkArtifact; PFTLockV1 has
    /// immutables, so the runtime code is the tail of the creation code). The
    /// runtime code exceeds its CBOR trailer by MIN_RUNTIME_CODE_BEYOND_TRAILER bytes.
    function _checkArtifact(Config memory cfg) private view returns (string memory artifact) {
        artifact = VM.readFile(cfg.artifact);
        bytes32 built = keccak256(VM.parseJsonBytes(artifact, ".bytecode.object"));
        bytes memory creation = type(PFTLockV1).creationCode;
        if (built != keccak256(creation)) revert ArtifactMismatch(cfg.artifact, built, keccak256(creation));
        // `forge coverage` writes no artifact; under coverage the tests call
        // checkArtifactMetadata on out/'s build instead. `forge script` never runs in that context.
        if (!VM.isContext(COVERAGE)) checkArtifactMetadata(cfg.artifact, artifact, creation);
    }

    /// The metadata check of `_checkArtifact` for the artifact `json` at `path`
    /// and the creation code `creation` it must describe.
    function checkArtifactMetadata(string memory path, string memory json, bytes memory creation) public pure {
        string memory hashKind = VM.parseJsonString(json, ".metadata.settings.metadata.bytecodeHash");
        if (keccak256(bytes(hashKind)) != keccak256("ipfs")) revert MetadataHashDisabled();
        bytes memory runtime = VM.parseJsonBytes(json, ".deployedBytecode.object");
        bytes32 digest = metadataDigest(runtime);
        if (
            digest == 0 || runtime.length < metadataTrailer(runtime).length + MIN_RUNTIME_CODE_BEYOND_TRAILER
                || !endsWith(creation, runtime) || ipfsDigest(bytes(VM.parseJsonString(json, ".rawMetadata"))) != digest
        ) revert ArtifactMetadataMismatch(path);
    }

    /// The sha256 digest under the "ipfs" key of the CBOR trailer of `code`, zero if there is none.
    function metadataDigest(bytes memory code) public pure returns (bytes32 digest) {
        if (code.length < 2) return 0;
        uint256 cborLength = uint16(bytes2(abi.encodePacked(code[code.length - 2], code[code.length - 1])));
        if (cborLength + 2 > code.length) return 0;
        uint256 start = code.length - 2 - cborLength;
        // map key text(4) "ipfs", value bytes(34) = 0x12 0x20 <32-byte digest>
        bytes memory key = hex"6469706673582212";
        for (uint256 i = start; i + key.length + 33 <= code.length - 2; i++) {
            uint256 j = 0;
            while (j < key.length && code[i + j] == key[j]) j++;
            if (j == key.length && code[i + j] == 0x20) {
                assembly {
                    digest := mload(add(add(code, 0x20), add(i, 9)))
                }
                return digest;
            }
        }
    }

    /// The sha256 IPFS (CIDv0) digest solc embeds for a metadata file of one
    /// UnixFS chunk; zero beyond one chunk (262144 bytes).
    function ipfsDigest(bytes memory content) public pure returns (bytes32) {
        if (content.length > 262144) return 0;
        bytes memory size = _varint(content.length);
        bytes memory file = abi.encodePacked(hex"080212", size, content, hex"18", size);
        return sha256(abi.encodePacked(hex"0a", _varint(file.length), file));
    }

    function _varint(uint256 value) private pure returns (bytes memory out) {
        while (value >= 0x80) {
            out = abi.encodePacked(out, bytes1(uint8(value & 0x7f) | 0x80));
            value >>= 7;
        }
        out = abi.encodePacked(out, bytes1(uint8(value)));
    }

    /// The CBOR trailer of `code` with its two length bytes, empty if there is none.
    function metadataTrailer(bytes memory code) public pure returns (bytes memory trailer) {
        if (code.length < 2) return "";
        uint256 length = uint256(uint16(bytes2(abi.encodePacked(code[code.length - 2], code[code.length - 1])))) + 2;
        if (length > code.length) return "";
        trailer = new bytes(length);
        for (uint256 i = 0; i < length; i++) {
            trailer[i] = code[code.length - length + i];
        }
    }

    /// Right after the broadcast: the lock's code has the length and the CBOR
    /// trailer of the artifact's runtime code `runtime` (the immutables make the rest differ).
    function checkDeployedCode(address lock, bytes memory runtime) public view {
        bytes memory code = lock.code;
        if (code.length != runtime.length || !endsWith(code, metadataTrailer(runtime))) {
            revert DeployedCodeMismatch(lock, code.length, runtime.length);
        }
    }

    /// True when `code` ends with the non-empty `tail`.
    function endsWith(bytes memory code, bytes memory tail) public pure returns (bool) {
        if (tail.length == 0 || tail.length > code.length) return false;
        uint256 offset = code.length - tail.length;
        for (uint256 i = 0; i < tail.length; i++) {
            if (code[offset + i] != tail[i]) return false;
        }
        return true;
    }

    /// The broadcast transaction hash and block are "pending" here and copied in
    /// from broadcast/.../run-latest.json. The compiler settings are the
    /// effective ones, read from the artifact's rawMetadata, whose IPFS digest
    /// `_checkArtifact` bound to the deployed code, never from its unchecked metadata object.
    function _writeRecord(
        Config memory cfg,
        PFTLockV1 lock,
        address deployer,
        Checked memory checked,
        string memory artifact,
        string memory path
    ) private {
        string memory o = "pft_lock_deployment";
        VM.serializeString(o, "mode", cfg.realDeployment ? "real" : "test");
        VM.serializeString(o, "tx_hash", "pending");
        VM.serializeString(o, "block_number", "pending");
        VM.serializeAddress(o, "lock", address(lock));
        VM.serializeBytes32(o, "lock_runtime_codehash", address(lock).codehash);
        VM.serializeUint(o, "chain_id", block.chainid);
        VM.serializeAddress(o, "token", cfg.token);
        VM.serializeBytes32(o, "token_codehash", cfg.tokenCodeHash);
        VM.serializeAddress(o, "finality_verifier", cfg.verifier);
        VM.serializeBytes32(o, "finality_verifier_codehash", checked.verifierCodeHash);
        VM.serializeBool(o, "finality_verifier_codehash_pinned", checked.verifierPinned);
        VM.serializeString(o, "max_lock_amount_units", VM.toString(cfg.maxLockAmount));
        VM.serializeString(o, "max_lock_amount_pft", VM.toString(cfg.maxLockAmountPft));
        VM.serializeString(
            o,
            "constructor_args",
            VM.toString(abi.encode(cfg.token, cfg.verifier, cfg.tokenCodeHash, uint64(cfg.maxLockAmount)))
        );
        VM.serializeAddress(o, "deployer", deployer);
        VM.serializeString(o, "artifact", cfg.artifact);
        VM.serializeBytes32(o, "creation_code_keccak256", keccak256(type(PFTLockV1).creationCode));
        VM.serializeBytes32(
            o, "runtime_code_keccak256", keccak256(VM.parseJsonBytes(artifact, ".deployedBytecode.object"))
        );
        VM.serializeBytes32(o, "deployed_code_keccak256", keccak256(address(lock).code));
        string memory metadata = VM.parseJsonString(artifact, ".rawMetadata");
        VM.serializeString(o, "solc_version", VM.parseJsonString(metadata, ".compiler.version"));
        VM.serializeBool(o, "optimizer", VM.parseJsonBool(metadata, ".settings.optimizer.enabled"));
        VM.serializeUint(o, "optimizer_runs", VM.parseJsonUint(metadata, ".settings.optimizer.runs"));
        VM.serializeString(o, "evm_version", VM.parseJsonString(metadata, ".settings.evmVersion"));
        VM.serializeString(o, "bytecode_hash", VM.parseJsonString(metadata, ".settings.metadata.bytecodeHash"));
        VM.serializeString(o, "source_commit", cfg.sourceCommit);
        string memory json = VM.serializeString(o, "openzeppelin_commit", OPENZEPPELIN_COMMIT);
        VM.createDir(cfg.recordDir, true);
        VM.writeFile(path, json);
        _log(string.concat("deployment record written to ", path));
    }

    /// Refuses an unset, empty, whitespace-only or `replace...` (any casing) value.
    function _required(string memory name) private view returns (string memory value) {
        value = VM.envString(name);
        if (_isPlaceholder(bytes(value))) revert PlaceholderValue(name);
    }

    /// A test-only switch is on only when set to exactly "1".
    function _switch(string memory name) private view returns (bool) {
        return keccak256(bytes(VM.envOr(name, string("")))) == keccak256("1");
    }

    function _isPlaceholder(bytes memory value) private pure returns (bool) {
        bool blank = true;
        bytes memory lower = new bytes(value.length);
        for (uint256 i = 0; i < value.length; i++) {
            bytes1 c = value[i];
            if (c >= "A" && c <= "Z") c = bytes1(uint8(c) + 32);
            lower[i] = c;
            if (c != " " && (c < 0x09 || c > 0x0d)) blank = false;
        }
        return blank || _contains(lower, bytes("replace"));
    }

    function _contains(bytes memory haystack, bytes memory needle) private pure returns (bool) {
        if (needle.length > haystack.length) return false;
        for (uint256 i = 0; i <= haystack.length - needle.length; i++) {
            uint256 j = 0;
            while (j < needle.length && haystack[i + j] == needle[j]) j++;
            if (j == needle.length) return true;
        }
        return false;
    }

    function _log(string memory label) private view {
        (bool ok,) = CONSOLE.staticcall(abi.encodeWithSignature("log(string)", label));
        ok;
    }

    function _log(string memory label, address value) private view {
        (bool ok,) = CONSOLE.staticcall(abi.encodeWithSignature("log(string,address)", label, value));
        ok;
    }

    function _log(string memory label, string memory value) private view {
        (bool ok,) = CONSOLE.staticcall(abi.encodeWithSignature("log(string,string)", label, value));
        ok;
    }

    function _log(string memory label, uint256 value) private view {
        (bool ok,) = CONSOLE.staticcall(abi.encodeWithSignature("log(string,uint256)", label, value));
        ok;
    }
}
