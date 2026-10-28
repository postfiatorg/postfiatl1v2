// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.24;

import {PFTToken} from "../src/PFTToken.sol";

interface IPFTTokenDeployVm {
    function envString(string calldata name) external view returns (string memory);
    function envOr(string calldata name, string calldata defaultValue) external view returns (string memory);
    function parseUint(string calldata value) external pure returns (uint256);
    function parseAddress(string calldata value) external pure returns (address);
    function parseBytes(string calldata value) external pure returns (bytes memory);
    function parseBytes32(string calldata value) external pure returns (bytes32);
    function rpc(string calldata method, string calldata params) external returns (bytes memory data);
    function toString(bytes32 value) external pure returns (string memory);
    function getNonce(address account) external view returns (uint64);
    function load(address target, bytes32 slot) external view returns (bytes32 data);
    function exists(string calldata path) external view returns (bool);
    function isContext(uint8 context) external view returns (bool);
    function startBroadcast(address signer) external;
    function stopBroadcast() external;
    function toString(uint256 value) external pure returns (string memory);
    function toString(bytes calldata value) external pure returns (string memory);
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
    function serializeAddress(string calldata objectKey, string calldata valueKey, address[] calldata values)
        external
        returns (string memory);
    function createDir(string calldata path, bool recursive) external;
    function writeFile(string calldata path, string calldata data) external;
}

/// The read-only part of the Safe interface the probe calls with checked staticcalls.
interface ISafeProbe {
    function getThreshold() external view returns (uint256);
    function getOwners() external view returns (address[] memory);
}

/// The module list of a Safe (1.3.0 and 1.4.1), read with a checked staticcall.
interface ISafeModulesProbe {
    function getModulesPaginated(address start, uint256 pageSize)
        external
        view
        returns (address[] memory array, address next);
}

/// The canonical SafeProxyFactory (1.3.0 and 1.4.1), read with a checked staticcall.
interface ISafeProxyFactoryProbe {
    function proxyCreationCode() external pure returns (bytes memory);
    function createProxyWithNonce(address singleton, bytes memory initializer, uint256 saltNonce)
        external
        returns (address proxy);
}

/// The Safe initializer (1.3.0 and 1.4.1) a treasury must have been created with.
interface ISafeSetupCall {
    function setup(
        address[] calldata owners,
        uint256 threshold,
        address to,
        bytes calldata data,
        address fallbackHandler,
        address paymentToken,
        uint256 payment,
        address payable paymentReceiver
    ) external;
}

/// @notice Foundry deployment script for the fixed-supply PFT token on Base.
/// @dev No forge-std dependency is required; the script declares only the
///      cheatcodes it uses. The values come from the environment (see
///      `script/pft-token.env.example`); the RPC URL and the one-time deployer
///      key are given to `forge script` on the command line and are never read
///      from a file by this script. Every check runs before the broadcast.
contract DeployPFTToken {
    IPFTTokenDeployVm private constant VM = IPFTTokenDeployVm(address(uint160(uint256(keccak256("hevm cheat code")))));
    address private constant CONSOLE = 0x000000000000000000636F6e736F6c652e6c6f67;

    uint256 public constant BASE = 8453;
    uint256 public constant BASE_SEPOLIA = 84532;
    uint256 public constant UNITS_PER_PFT = 1_000_000;
    /// OpenZeppelin v5.7.0 EIP712 stores the name as a ShortString (at most 31 bytes).
    uint256 public constant MAX_NAME_BYTES = 31;
    string public constant OPENZEPPELIN_COMMIT = "cab19933c33c2ad1d4c7a84864a3601dddfd16f3";
    /// `ForgeContext.ScriptBroadcast` in forge-std's Vm.
    uint8 private constant SCRIPT_BROADCAST = 6;
    /// `ForgeContext.Coverage` in forge-std's Vm.
    uint8 private constant COVERAGE = 2;

    /// Official Safe singletons, the address a Safe proxy keeps in storage slot 0,
    /// and the keccak256 of their runtime code.
    /// Source: safe-global/safe-deployments (commit b45417dd009957effb3575928941e736f8fd37d1),
    /// the "canonical" deployment's address and codeHash in src/assets/v1.4.1/safe.json,
    /// src/assets/v1.4.1/safe_l2.json, src/assets/v1.3.0/gnosis_safe.json and
    /// src/assets/v1.3.0/gnosis_safe_l2.json, listed there for 8453 and 84532.
    /// The v1.3.0 "eip155" deployment is listed for both chains too and is not pinned.
    address public constant SAFE_1_4_1 = 0x41675C099F32341bf84BFc5382aF534df5C7461a;
    address public constant SAFE_L2_1_4_1 = 0x29fcB43b46531BcA003ddC8FCB67FFE91900C762;
    address public constant SAFE_1_3_0 = 0xd9Db270c1B5E3Bd161E8c8503c55cEABeE709552;
    address public constant SAFE_L2_1_3_0 = 0x3E5c63644E683549055b9Be8653de26E0B4CD36E;
    bytes32 public constant SAFE_1_4_1_CODEHASH = 0x1fe2df852ba3299d6534ef416eefa406e56ced995bca886ab7a553e6d0c5e1c4;
    bytes32 public constant SAFE_L2_1_4_1_CODEHASH = 0xb1f926978a0f44a2c0ec8fe822418ae969bd8c3f18d61e5103100339894f81ff;
    bytes32 public constant SAFE_1_3_0_CODEHASH = 0xbba688fbdb21ad2bb58bc320638b43d94e7d100f6f3ebaab0a4e4de6304b1c2e;
    bytes32 public constant SAFE_L2_1_3_0_CODEHASH = 0x21842597390c4c6e3c1239e434a682b054bd9548eee5e9b1d6a4482731023c0f;
    /// keccak256 of the runtime code of a SafeProxy 1.4.1 and a GnosisSafeProxy
    /// 1.3.0. The proxy keeps its singleton in storage, so every proxy of a
    /// version has this code. safe-deployments b45417dd carries no proxy
    /// bytecode: these are the code the canonical factories deploy, whose
    /// codeHash it lists (src/assets/v1.4.1/safe_proxy_factory.json 0x50c3cdc4...,
    /// src/assets/v1.3.0/proxy_factory.json 0x337d7f54...). The factory bytecode
    /// is the deployedBytecode of npm packages safe-global/safe-contracts 1.4.1 and
    /// gnosis.pm/safe-contracts 1.3.0 (npm scopes), kept in test/fixtures/safe/; the tests
    /// check it against those codeHashes, deploy a proxy with it and assert these.
    bytes32 public constant SAFE_PROXY_1_4_1_CODEHASH =
        0xd7d408ebcd99b2b70be43e20253d6d92a8ea8fab29bd3be7f55b10032331fb4c;
    bytes32 public constant SAFE_PROXY_1_3_0_CODEHASH =
        0xb89c1b3bdf2cf8827818646bce9a8f6e372885f8c55e5c07acbd307cb133b000;
    /// The canonical SafeProxyFactory and CompatibilityFallbackHandler of each
    /// version and the keccak256 of their runtime code, from the "canonical"
    /// deployment in safe-deployments b45417dd (src/assets/v1.4.1/safe_proxy_factory.json,
    /// src/assets/v1.3.0/proxy_factory.json, src/assets/v1.4.1/compatibility_fallback_handler.json,
    /// src/assets/v1.3.0/compatibility_fallback_handler.json), listed there for 8453 and 84532.
    address public constant SAFE_PROXY_FACTORY_1_4_1 = 0x4e1DCf7AD4e460CfD30791CCC4F9c8a4f820ec67;
    address public constant SAFE_PROXY_FACTORY_1_3_0 = 0xa6B71E26C5e0845f74c812102Ca7114b6a896AB2;
    bytes32 public constant SAFE_PROXY_FACTORY_1_4_1_CODEHASH =
        0x50c3cdc4074750a7a974204a716c999edd37482f907608d960b2b025ee0b3317;
    bytes32 public constant SAFE_PROXY_FACTORY_1_3_0_CODEHASH =
        0x337d7f54be11b6ed55fef7b667ea5488db53db8320a05d1146aa4bd169a39a9b;
    address public constant FALLBACK_HANDLER_1_4_1 = 0xfd0732Dc9E303f09fCEf3a7388Ad10A83459Ec99;
    address public constant FALLBACK_HANDLER_1_3_0 = 0xf48f2B2d2a534e402487b3ee7C18c33Aec0Fe5e4;
    bytes32 public constant FALLBACK_HANDLER_1_4_1_CODEHASH =
        0x7c6007a5d711cea8dfd5d91f5940ec29c7f200fe511eb1fc1397b367af3c42f9;
    bytes32 public constant FALLBACK_HANDLER_1_3_0_CODEHASH =
        0x03e69f7ce809e81687c69b19a7d7cca45b6d551ffdec73d9bb87178476de1abf;
    /// `masterCopy()`, answered by the proxy itself with its singleton.
    bytes4 public constant MASTER_COPY_SELECTOR = 0xa619486e;
    /// The compiled token, under forge's output directory (FOUNDRY_OUT, else out),
    /// whose metadata is the record's build provenance. Its creation code must
    /// be the code this script deploys.
    string public constant TOKEN_ARTIFACT = "/PFTToken.sol/PFTToken.json";
    /// The artifact's runtime code must exceed its CBOR trailer by at least this many bytes.
    uint256 public constant MIN_RUNTIME_CODE_BEYOND_TRAILER = 512;
    uint256 public constant MIN_SAFE_THRESHOLD = 2;
    /// A module executes through the Safe without owner signatures, so the
    /// treasury must have none. SENTINEL_MODULES heads the Safe's module list.
    address public constant SENTINEL_MODULES = address(0x1);
    uint256 public constant MODULE_PAGE_SIZE = 10;
    /// GuardManager's storage slot in Safe 1.3.0 and 1.4.1. A reverting guard
    /// blocks every owner transaction, its own removal included, so a guard is
    /// refused unless PFT_TOKEN_ALLOW_GUARD names exactly that address.
    bytes32 public constant GUARD_STORAGE_SLOT = keccak256("guard_manager.guard.address");

    struct Config {
        uint256 chainId;
        string name;
        string symbol;
        uint256 totalSupply; // six-decimal units
        uint256 totalSupplyPft; // whole PFT
        address treasury;
        bool allowEoaTreasury; // Base Sepolia dry run only
        bool allowUsedDeployer; // Base Sepolia dry run only
        bool realDeployment; // PFT_TOKEN_REAL_DEPLOYMENT=1: the durable record under deployments/
        string recordDir; // deployments/... for a real deployment, else cache/deploy-test/...
        bool writeRecord; // only `forge script --broadcast` writes the record
        string sourceCommit; // PFT_TOKEN_SOURCE_COMMIT, required for a real deployment
        string artifact; // the compiled token; must be the code deployed, its metadata goes in the record
        address allowedGuard; // PFT_TOKEN_ALLOW_GUARD: the one guard accepted after an independent review
        bytes32 creationTx; // PFT_TOKEN_TREASURY_CREATION_TX: the treasury Safe's creation transaction
        bytes creationRawTx; // PFT_TOKEN_TREASURY_CREATION_RAW_TX, else read by eth_getRawTransactionByHash
    }

    struct SafeInfo {
        address singleton;
        uint256 threshold;
        address[] owners;
        address[] modules; // always empty for an accepted Safe
        address guard; // refused unless named by PFT_TOKEN_ALLOW_GUARD
        bytes32 creationTx; // the verified creation transaction, zero in test mode without one
        address factory; // the canonical factory that created the Safe
        address fallbackHandler; // zero or the canonical CompatibilityFallbackHandler
    }

    /// The decoded `setup` call a Safe was created with.
    struct SafeSetup {
        address[] owners;
        uint256 threshold;
        address to;
        bytes data;
        address fallbackHandler;
        address paymentToken;
        uint256 payment;
        address paymentReceiver;
    }

    error PlaceholderValue(string name);
    error UnsupportedChain(uint256 chainId);
    error WrongChain(uint256 expected, uint256 actual);
    error TestSwitchOnMainnet(string name);
    error NameTooLong(uint256 length);
    error SupplyMismatch(uint256 units, uint256 pft);
    error TreasuryIsDeployer(address treasury);
    error TreasuryNotAContract(address treasury);
    error TreasuryIsNotASafe(address treasury);
    error ProxyCodeMismatch(address treasury, bytes32 codehash);
    error MasterCopyMismatch(address treasury, address singleton);
    error SingletonCodeMismatch(address singleton, bytes32 codehash);
    error SourceCommitMissing();
    error UnofficialSafeSingleton(address treasury, address singleton);
    error SafeThresholdTooLow(uint256 threshold);
    error SafeOwnersInvalid(uint256 owners, uint256 distinctNonZero, uint256 threshold);
    error DeployerAlreadyUsed(address deployer, uint256 nonce);
    error RealDeploymentWithTestSwitch(string name);
    error RecordExists(string path);
    error ModulesEnabled(uint256 count);
    error ArtifactMismatch(string path, bytes32 artifactCodeHash, bytes32 compiledCodeHash);
    error GuardEnabled(address guard);
    error MetadataHashDisabled();
    error ArtifactMetadataMismatch(string path);
    error TreasuryCreationUnknown(bytes32 txHash);
    error TreasuryCreationUnreadable();
    error TreasuryCreationNotFromFactory(address to);
    error TreasuryInitializerDelegates(address to, uint256 dataLength);
    error TreasuryCreationMismatch(string field);
    error TreasuryCreationWrongChain(uint256 chainId);
    error TreasurySetupHasPayment(address paymentToken, uint256 payment);
    error DeployedCodeMismatch(address token, uint256 codeLength, uint256 artifactLength);

    event PFTTokenDeployed(
        address indexed token, address indexed treasury, uint256 total_supply, string name, string symbol
    );

    function run() external returns (PFTToken) {
        return deploy(readConfig());
    }

    /// Reads the deployment values from the environment. The record is written
    /// only by `forge script --broadcast`, never by a dry run. Only
    /// PFT_TOKEN_REAL_DEPLOYMENT=1 selects the durable deployments/ path; every
    /// other run writes under the ignored cache/deploy-test/.
    function readConfig() public view returns (Config memory cfg) {
        cfg.chainId = VM.parseUint(_required("PFT_TOKEN_CHAIN_ID"));
        cfg.name = _required("PFT_TOKEN_NAME");
        cfg.symbol = _required("PFT_TOKEN_SYMBOL");
        cfg.totalSupply = VM.parseUint(_required("PFT_TOKEN_TOTAL_SUPPLY"));
        cfg.totalSupplyPft = VM.parseUint(_required("PFT_TOKEN_TOTAL_SUPPLY_PFT"));
        cfg.treasury = VM.parseAddress(_required("PFT_TOKEN_TREASURY"));
        cfg.allowEoaTreasury = _switch("PFT_TOKEN_ALLOW_EOA_TREASURY");
        cfg.allowUsedDeployer = _switch("PFT_TOKEN_ALLOW_USED_DEPLOYER");
        cfg.realDeployment = _switch("PFT_TOKEN_REAL_DEPLOYMENT");
        string memory root = cfg.realDeployment ? "deployments/" : "cache/deploy-test/";
        cfg.recordDir = string.concat(root, "pft-token-base-", VM.toString(block.chainid));
        cfg.writeRecord = VM.isContext(SCRIPT_BROADCAST);
        cfg.sourceCommit = VM.envOr("PFT_TOKEN_SOURCE_COMMIT", string(""));
        string memory out = VM.envOr("FOUNDRY_OUT", string(""));
        cfg.artifact = string.concat(bytes(out).length == 0 ? "out" : out, TOKEN_ARTIFACT);
        string memory creation = cfg.allowEoaTreasury
            ? VM.envOr("PFT_TOKEN_TREASURY_CREATION_TX", string(""))
            : _required("PFT_TOKEN_TREASURY_CREATION_TX");
        if (bytes(creation).length != 0) cfg.creationTx = VM.parseBytes32(creation);
        string memory raw = VM.envOr("PFT_TOKEN_TREASURY_CREATION_RAW_TX", string(""));
        if (bytes(raw).length != 0) cfg.creationRawTx = VM.parseBytes(raw);
        string memory guard = VM.envOr("PFT_TOKEN_ALLOW_GUARD", string(""));
        if (bytes(guard).length != 0) cfg.allowedGuard = VM.parseAddress(guard);
    }

    /// Checks `cfg` against the target chain, then deploys from `msg.sender`
    /// (the broadcaster) and writes the record.
    function deploy(Config memory cfg) public returns (PFTToken token) {
        address deployer = msg.sender;
        _checkChain(cfg);
        _checkMetadata(cfg);
        _checkSupply(cfg);
        SafeInfo memory safe = _checkTreasury(cfg, deployer);
        if (cfg.treasury.code.length != 0) _checkCreation(cfg, safe);
        uint256 nonce = VM.getNonce(deployer);
        if (nonce != 0 && !cfg.allowUsedDeployer) {
            revert DeployerAlreadyUsed(deployer, nonce);
        }
        string memory recordPath = string.concat(cfg.recordDir, "/deployment.json");
        if (cfg.realDeployment && VM.exists(recordPath)) revert RecordExists(recordPath);
        string memory artifact = _checkArtifact(cfg);

        _log("chain id", block.chainid);
        _log("name", cfg.name);
        _log("symbol", cfg.symbol);
        _log("deployer", deployer);
        _log("deployer nonce", nonce);
        _log("treasury", cfg.treasury);
        _log("treasury Safe singleton", safe.singleton);
        _log("treasury Safe threshold", safe.threshold);
        _log("treasury Safe owners", safe.owners.length);
        _log("treasury Safe modules", safe.modules.length);
        _log("treasury Safe guard", safe.guard);
        if (safe.guard != address(0)) _log("WARNING: the treasury Safe has a guard named by PFT_TOKEN_ALLOW_GUARD");
        _log("treasury Safe creation tx", VM.toString(safe.creationTx));
        _log("treasury Safe factory", safe.factory);
        _log("treasury Safe fallback handler", safe.fallbackHandler);
        _log("token artifact", cfg.artifact);
        _log("deployment record", cfg.realDeployment ? "real" : "test");
        _log("total supply (six-decimal units)", cfg.totalSupply);
        _log("total supply (whole PFT)", cfg.totalSupplyPft);

        VM.startBroadcast(deployer);
        token = new PFTToken(cfg.name, cfg.symbol, cfg.totalSupply, cfg.treasury);
        VM.stopBroadcast();
        checkDeployedCode(address(token), VM.parseJsonBytes(artifact, ".deployedBytecode.object"));

        emit PFTTokenDeployed(address(token), cfg.treasury, cfg.totalSupply, cfg.name, cfg.symbol);
        _log("PFTToken deployed at", address(token));
        if (cfg.writeRecord) {
            _writeRecord(cfg, token, deployer, safe, artifact, recordPath);
        }
    }

    /// The pinned runtime code hash of an official Safe singleton on 8453 or
    /// 84532, zero for any other address or chain.
    function singletonCodeHash(uint256 chainId, address singleton) public pure returns (bytes32) {
        if (chainId != BASE && chainId != BASE_SEPOLIA) return 0;
        if (singleton == SAFE_1_4_1) return SAFE_1_4_1_CODEHASH;
        if (singleton == SAFE_L2_1_4_1) return SAFE_L2_1_4_1_CODEHASH;
        if (singleton == SAFE_1_3_0) return SAFE_1_3_0_CODEHASH;
        if (singleton == SAFE_L2_1_3_0) return SAFE_L2_1_3_0_CODEHASH;
        return 0;
    }

    /// True for a pinned official Safe singleton on 8453 or 84532.
    function isOfficialSafeSingleton(uint256 chainId, address singleton) public pure returns (bool) {
        return singletonCodeHash(chainId, singleton) != 0;
    }

    /// Fails closed unless `treasury` carries the pinned Safe proxy code, its
    /// slot 0 and its `masterCopy()` name the same pinned singleton, that
    /// singleton carries its pinned code on this chain, and the Safe has
    /// threshold >= 2 with at least that many distinct non-zero owners, and no
    /// module (getModulesPaginated(0x1, 10) is an empty page ending at 0x1).
    /// The guard is read; _checkTreasury refuses one that is not named. Uses checked staticcalls only (no self-call).
    function checkSafe(uint256 chainId, address treasury) public view returns (SafeInfo memory safe) {
        bytes32 proxyHash = treasury.codehash;
        if (proxyHash != SAFE_PROXY_1_4_1_CODEHASH && proxyHash != SAFE_PROXY_1_3_0_CODEHASH) {
            revert ProxyCodeMismatch(treasury, proxyHash);
        }
        bytes32 slot0 = VM.load(treasury, bytes32(0));
        safe.singleton = address(uint160(uint256(slot0)));
        bytes32 singletonHash = singletonCodeHash(chainId, safe.singleton);
        if (uint256(slot0) >> 160 != 0 || singletonHash == 0) {
            revert UnofficialSafeSingleton(treasury, safe.singleton);
        }
        (bool ok, bytes memory ret) = treasury.staticcall(abi.encodeWithSelector(MASTER_COPY_SELECTOR));
        if (!ok || ret.length != 32 || abi.decode(ret, (bytes32)) != slot0) {
            revert MasterCopyMismatch(treasury, safe.singleton);
        }
        if (safe.singleton.codehash != singletonHash) {
            revert SingletonCodeMismatch(safe.singleton, safe.singleton.codehash);
        }
        (ok, ret) = treasury.staticcall(abi.encodeCall(ISafeProbe.getThreshold, ()));
        if (!ok || ret.length != 32) revert TreasuryIsNotASafe(treasury);
        safe.threshold = abi.decode(ret, (uint256));
        if (safe.threshold < MIN_SAFE_THRESHOLD) revert SafeThresholdTooLow(safe.threshold);
        (ok, ret) = treasury.staticcall(abi.encodeCall(ISafeProbe.getOwners, ()));
        if (!ok || ret.length < 64) revert TreasuryIsNotASafe(treasury);
        (uint256 offset, uint256 count) = abi.decode(ret, (uint256, uint256));
        if (offset != 32 || count > (ret.length - 64) / 32 || ret.length != 64 + count * 32) {
            revert TreasuryIsNotASafe(treasury);
        }
        safe.owners = abi.decode(ret, (address[]));
        uint256 distinct = _distinctNonZero(safe.owners);
        if (distinct != count || distinct < MIN_SAFE_THRESHOLD || safe.threshold > distinct) {
            revert SafeOwnersInvalid(count, distinct, safe.threshold);
        }
        (ok, ret) = treasury.staticcall(
            abi.encodeCall(ISafeModulesProbe.getModulesPaginated, (SENTINEL_MODULES, MODULE_PAGE_SIZE))
        );
        if (!ok || ret.length < 96) revert TreasuryIsNotASafe(treasury);
        if (keccak256(ret) != keccak256(abi.encode(safe.modules, SENTINEL_MODULES))) {
            (,, uint256 modules) = abi.decode(ret, (uint256, uint256, uint256));
            revert ModulesEnabled(modules);
        }
        safe.guard = address(uint160(uint256(VM.load(treasury, GUARD_STORAGE_SLOT))));
    }

    /// The Safe's creation, from its signed creation transaction (not from the
    /// RPC's JSON, which forge hands over as a node-specific tuple): the raw
    /// bytes from PFT_TOKEN_TREASURY_CREATION_RAW_TX, else eth_getRawTransactionByHash,
    /// must hash to PFT_TOKEN_TREASURY_CREATION_TX. A `setup` delegatecall can
    /// write a module that getModulesPaginated never lists, so only a plain
    /// setup is accepted. Optional only in the Base Sepolia test mode.
    function _checkCreation(Config memory cfg, SafeInfo memory safe) private {
        if (cfg.creationTx == 0) {
            if (!cfg.allowEoaTreasury) revert TreasuryCreationUnknown(0);
            _log("WARNING: test-only run without the treasury Safe's creation transaction");
            return;
        }
        bytes memory raw = cfg.creationRawTx.length != 0 ? cfg.creationRawTx : fetchRawTransaction(cfg.creationTx);
        if (keccak256(raw) != cfg.creationTx) revert TreasuryCreationUnknown(cfg.creationTx);
        (safe.factory, safe.fallbackHandler) = checkCreation(cfg.chainId, cfg.treasury, safe, raw);
        safe.creationTx = cfg.creationTx;
    }

    /// The signed transaction `txHash` as the RPC serves it (unknown: 32 zero bytes).
    function fetchRawTransaction(bytes32 txHash) public returns (bytes memory) {
        // forge returns a hex-string result as its bytes, not ABI-encoded.
        return VM.rpc("eth_getRawTransactionByHash", string.concat('["', VM.toString(txHash), '"]'));
    }

    /// Fails closed unless the signed transaction `raw` was signed for this chain
    /// (a CREATE2 address and the canonical factories are the same on 8453 and
    /// 84532, the signature is not), called a pinned canonical
    /// factory (its code checked on this chain) with createProxyWithNonce for the
    /// Safe's singleton, the factory's CREATE2 address for that call is the
    /// treasury, and its initializer is a plain setup with the live owners and
    /// threshold and a zero or canonical fallback handler.
    function checkCreation(uint256 chainId, address treasury, SafeInfo memory safe, bytes memory raw)
        public
        view
        returns (address factory, address fallbackHandler)
    {
        bytes memory data;
        {
            uint256 txChainId;
            (txChainId, factory, data) = decodeRawTransaction(raw);
            if (txChainId != block.chainid) revert TreasuryCreationWrongChain(txChainId);
        }
        bytes32 factoryHash = factoryCodeHash(chainId, factory);
        if (factoryHash == 0 || factory.codehash != factoryHash) revert TreasuryCreationNotFromFactory(factory);
        (address singleton, bytes memory initializer, uint256 saltNonce) = decodeCreateProxy(data);
        if (singleton != safe.singleton) revert TreasuryCreationMismatch("singleton");
        (bool ok, bytes memory code) = factory.staticcall(abi.encodeCall(ISafeProxyFactoryProbe.proxyCreationCode, ()));
        if (!ok) revert TreasuryCreationNotFromFactory(factory);
        if (proxyAddress(factory, abi.decode(code, (bytes)), singleton, initializer, saltNonce) != treasury) {
            revert TreasuryCreationMismatch("proxy address");
        }
        fallbackHandler = checkSetup(singleton, initializer, safe.owners, safe.threshold);
        if (fallbackHandler != address(0) && fallbackHandler.codehash != fallbackHandlerCodeHash(fallbackHandler)) {
            revert TreasuryCreationMismatch("fallback handler code");
        }
    }

    /// The pinned runtime code hash of a canonical SafeProxyFactory on 8453 or 84532, else zero.
    function factoryCodeHash(uint256 chainId, address factory) public pure returns (bytes32) {
        if (chainId != BASE && chainId != BASE_SEPOLIA) return 0;
        if (factory == SAFE_PROXY_FACTORY_1_4_1) return SAFE_PROXY_FACTORY_1_4_1_CODEHASH;
        if (factory == SAFE_PROXY_FACTORY_1_3_0) return SAFE_PROXY_FACTORY_1_3_0_CODEHASH;
        return 0;
    }

    /// The pinned runtime code hash of a canonical CompatibilityFallbackHandler, else zero.
    function fallbackHandlerCodeHash(address handler) public pure returns (bytes32) {
        if (handler == FALLBACK_HANDLER_1_4_1) return FALLBACK_HANDLER_1_4_1_CODEHASH;
        if (handler == FALLBACK_HANDLER_1_3_0) return FALLBACK_HANDLER_1_3_0_CODEHASH;
        return 0;
    }

    /// The address SafeProxyFactory 1.3.0 and 1.4.1 give createProxyWithNonce.
    function proxyAddress(
        address factory,
        bytes memory proxyCreationCode,
        address singleton,
        bytes memory initializer,
        uint256 saltNonce
    ) public pure returns (address) {
        bytes32 salt = keccak256(abi.encodePacked(keccak256(initializer), saltNonce));
        bytes32 initCodeHash = keccak256(abi.encodePacked(proxyCreationCode, abi.encode(singleton)));
        return address(uint160(uint256(keccak256(abi.encodePacked(bytes1(0xff), factory, salt, initCodeHash)))));
    }

    /// The chain id, `to` and calldata of a signed type-2 (EIP-1559) or legacy
    /// transaction, whose whole envelope is read: exactly 12 items (chainId, nonce,
    /// maxPriorityFeePerGas, maxFeePerGas, gasLimit, to, value, data, accessList,
    /// yParity, r, s) or 9 (nonce, gasPrice, gasLimit, to, value, data, v, r, s);
    /// integers of at most 32 bytes, `to` of 20, the access list a list (its
    /// elements unread), yParity 0 or 1, r and s non-zero. A legacy chain id is
    /// (v - 35) / 2, so v is 2 x chainId + 35 or + 36; v 27 or 28 gives 0.
    function decodeRawTransaction(bytes memory raw)
        public
        pure
        returns (uint256 chainId, address to, bytes memory data)
    {
        bool typed = raw.length != 0 && uint8(raw[0]) == 2;
        if (!typed && (raw.length == 0 || uint8(raw[0]) < 0xc0)) revert TreasuryCreationUnreadable();
        (uint256 pos,, uint256 next, bool list) = _rlpItem(raw, typed ? 1 : 0);
        if (!list || next != raw.length) revert TreasuryCreationUnreadable();
        uint256 count = typed ? 12 : 9;
        uint256 toIndex = typed ? 5 : 3;
        for (uint256 i = 0; i < count; i++) {
            uint256 start;
            uint256 length;
            (start, length, next, list) = _rlpItem(raw, pos);
            if (list != (typed && i == 8)) revert TreasuryCreationUnreadable();
            if (i == toIndex) {
                if (length != 20) revert TreasuryCreationUnreadable();
                to = address(bytes20(_slice(raw, start, 20)));
            } else if (i == toIndex + 2) {
                data = _slice(raw, start, length);
            } else if (!list) {
                uint256 value = _rlpUint(raw, start, length);
                if (i + 2 >= count && value == 0) revert TreasuryCreationUnreadable(); // r, s
                if (typed && i == 9 && value > 1) revert TreasuryCreationUnreadable(); // yParity
                if (typed && i == 0) chainId = value;
                if (!typed && i == 6) chainId = value >= 35 ? (value - 35) / 2 : 0; // EIP-155 v
            }
            pos = next;
        }
        if (pos != raw.length) revert TreasuryCreationUnreadable();
    }

    /// singleton, initializer and saltNonce of a createProxyWithNonce call.
    function decodeCreateProxy(bytes memory data)
        public
        pure
        returns (address singleton, bytes memory initializer, uint256 saltNonce)
    {
        if (data.length < 4 || bytes4(data) != ISafeProxyFactoryProbe.createProxyWithNonce.selector) {
            revert TreasuryCreationMismatch("createProxyWithNonce");
        }
        return abi.decode(_slice(data, 4, data.length - 4), (address, bytes, uint256));
    }

    /// The fallback handler of a plain `setup` initializer (no delegatecall: `to`
    /// zero, `data` empty) with these owners and this threshold, whose fallback
    /// handler is zero or the canonical one of the singleton's version, and no
    /// payment (paymentToken zero, payment 0; the receiver is then unused).
    function checkSetup(address singleton, bytes memory initializer, address[] memory owners, uint256 threshold)
        public
        pure
        returns (address)
    {
        SafeSetup memory setup = decodeSetup(initializer);
        if (setup.to != address(0) || setup.data.length != 0) {
            revert TreasuryInitializerDelegates(setup.to, setup.data.length);
        }
        if (keccak256(abi.encode(setup.owners)) != keccak256(abi.encode(owners))) {
            revert TreasuryCreationMismatch("owners");
        }
        if (setup.threshold != threshold) revert TreasuryCreationMismatch("threshold");
        if (setup.paymentToken != address(0) || setup.payment != 0) {
            revert TreasurySetupHasPayment(setup.paymentToken, setup.payment);
        }
        bool v141 = singleton == SAFE_1_4_1 || singleton == SAFE_L2_1_4_1;
        address canonical = v141 ? FALLBACK_HANDLER_1_4_1 : FALLBACK_HANDLER_1_3_0;
        if (setup.fallbackHandler != address(0) && setup.fallbackHandler != canonical) {
            revert TreasuryCreationMismatch("fallback handler");
        }
        return setup.fallbackHandler;
    }

    /// The arguments of a `setup` call.
    function decodeSetup(bytes memory initializer) public pure returns (SafeSetup memory setup) {
        if (initializer.length < 4 || bytes4(initializer) != ISafeSetupCall.setup.selector) {
            revert TreasuryCreationMismatch("setup");
        }
        bytes memory args = _slice(initializer, 4, initializer.length - 4);
        (setup.owners, setup.threshold, setup.to, setup.data, setup.fallbackHandler) =
            abi.decode(args, (address[], uint256, address, bytes, address));
        (,,,,, setup.paymentToken, setup.payment, setup.paymentReceiver) =
            abi.decode(args, (address[], uint256, address, bytes, address, address, uint256, address));
    }

    /// One RLP item at `pos`: its payload's start and length, the next item's
    /// position, and whether it is a list. Reverts beyond the end of `raw`.
    function _rlpItem(bytes memory raw, uint256 pos)
        private
        pure
        returns (uint256 start, uint256 length, uint256 next, bool list)
    {
        if (pos >= raw.length) revert TreasuryCreationUnreadable();
        uint256 b = uint8(raw[pos]);
        if (b < 0x80) return (pos, 1, pos + 1, false);
        list = b >= 0xc0;
        uint256 short = list ? b - 0xc0 : b - 0x80;
        if (short < 56) {
            (start, length) = (pos + 1, short);
        } else {
            uint256 lengthBytes = short - 55;
            if (lengthBytes > 4 || pos + 1 + lengthBytes > raw.length) revert TreasuryCreationUnreadable();
            for (uint256 i = 0; i < lengthBytes; i++) {
                length = (length << 8) | uint8(raw[pos + 1 + i]);
            }
            start = pos + 1 + lengthBytes;
        }
        next = start + length;
        if (next > raw.length) revert TreasuryCreationUnreadable();
    }

    /// The integer of an RLP string of at most 32 bytes.
    function _rlpUint(bytes memory raw, uint256 start, uint256 length) private pure returns (uint256 value) {
        if (length > 32) revert TreasuryCreationUnreadable();
        for (uint256 i = 0; i < length; i++) {
            value = (value << 8) | uint8(raw[start + i]);
        }
    }

    function _slice(bytes memory raw, uint256 start, uint256 length) private pure returns (bytes memory out) {
        out = new bytes(length);
        for (uint256 i = 0; i < length; i++) {
            out[i] = raw[start + i];
        }
    }

    /// The artifact whose metadata goes in the record must be this build of the
    /// token: its creation code is the code `deploy` sends.
    /// Its recorded metadata must be the metadata of that code: the build keeps
    /// the IPFS metadata hash (the rawMetadata's settings.metadata.bytecodeHash is
    /// "ipfs"; `--no-metadata` and bytecode_hash = "none" drop it, and two builds
    /// with other settings would then share one code), the creation code ends
    /// with the artifact's runtime code (PFTToken has immutables, so
    /// `type(PFTToken).runtimeCode` does not exist), which has the length the
    /// creation code declares (`declaredRuntimeLength`) and exceeds its CBOR
    /// trailer by at least MIN_RUNTIME_CODE_BEYOND_TRAILER bytes, and the IPFS
    /// digest of the artifact's rawMetadata is the digest in that runtime code's
    /// CBOR trailer. All of it runs before the broadcast.
    function _checkArtifact(Config memory cfg) private view returns (string memory artifact) {
        artifact = VM.readFile(cfg.artifact);
        bytes32 built = keccak256(VM.parseJsonBytes(artifact, ".bytecode.object"));
        bytes memory creation = type(PFTToken).creationCode;
        if (built != keccak256(creation)) revert ArtifactMismatch(cfg.artifact, built, keccak256(creation));
        // `forge coverage` writes no artifact, so none carries its build's metadata;
        // under coverage the tests call checkArtifactMetadata on out/'s build instead.
        // `forge script` never runs in that context.
        if (!VM.isContext(COVERAGE)) checkArtifactMetadata(cfg.artifact, artifact, creation);
    }

    /// The metadata check of `_checkArtifact` for the artifact `json` at `path`
    /// and the creation code `creation` it must describe.
    function checkArtifactMetadata(string memory path, string memory json, bytes memory creation) public pure {
        string memory metadata = VM.parseJsonString(json, ".rawMetadata");
        string memory hashKind = VM.parseJsonString(metadata, ".settings.metadata.bytecodeHash");
        if (keccak256(bytes(hashKind)) != keccak256("ipfs")) revert MetadataHashDisabled();
        bytes memory runtime = VM.parseJsonBytes(json, ".deployedBytecode.object");
        bytes32 digest = metadataDigest(runtime);
        if (
            digest == 0 || runtime.length < metadataTrailer(runtime).length + MIN_RUNTIME_CODE_BEYOND_TRAILER
                || runtime.length != declaredRuntimeLength(creation) || !endsWith(creation, runtime)
                || ipfsDigest(bytes(metadata)) != digest
        ) revert ArtifactMetadataMismatch(path);
    }

    /// The runtime length `creation` declares: the size of its constructor's
    /// `PUSH size PUSH offset PUSH0 CODECOPY` (PUSH1 0x00 before Shanghai) that
    /// copies the runtime, the first such copy to memory 0 with offset + size the
    /// code's length. solc 0.8.24 emits it for PFTToken (immutables) as
    /// `PUSH2 0x0e03 PUSH3 0x0006b9 PUSH0 CODECOPY`. Zero if there is none.
    function declaredRuntimeLength(bytes memory creation) public pure returns (uint256) {
        uint256 pushes = 0; // consecutive PUSH instructions just before `i`
        (uint256 size, uint256 offset, uint256 dest) = (0, 0, 0); // their last three values
        uint256 i = 0;
        while (i < creation.length) {
            uint256 op = uint8(creation[i]);
            if (op == 0x39 && pushes >= 3 && dest == 0 && size <= creation.length && offset == creation.length - size) {
                return size;
            }
            uint256 width = op >= 0x5f && op <= 0x7f ? op - 0x5f : 0; // PUSH0 to PUSH32
            if (i + width >= creation.length) return 0;
            uint256 value = 0;
            for (uint256 j = 1; j <= width; j++) {
                value = (value << 8) | uint8(creation[i + j]);
            }
            (size, offset, dest) = (offset, dest, value);
            pushes = op >= 0x5f && op <= 0x7f ? pushes + 1 : 0;
            i += 1 + width;
        }
        return 0;
    }

    /// The sha256 digest of the IPFS multihash (0x12 0x20 <digest>) under the
    /// "ipfs" key of the CBOR trailer of `code` (the trailer's length is the
    /// last two bytes), zero if there is none.
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

    /// The sha256 IPFS (CIDv0) digest solc embeds for a metadata file: the
    /// sha256 of the dag-pb node wrapping a single UnixFS file chunk, not of
    /// the bytes themselves. Zero beyond one chunk (262144 bytes).
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
    function metadataTrailer(bytes memory code) public pure returns (bytes memory) {
        if (code.length < 2) return "";
        uint256 length = uint256(uint16(bytes2(abi.encodePacked(code[code.length - 2], code[code.length - 1])))) + 2;
        if (length > code.length) return "";
        return _slice(code, code.length - length, length);
    }

    /// Right after the broadcast (the confirmation; the length was pinned before
    /// it by `checkArtifactMetadata`): the token's code has the length and the CBOR
    /// trailer of the artifact's runtime code `runtime` (the immutables make the
    /// rest differ).
    function checkDeployedCode(address token, bytes memory runtime) public view {
        bytes memory code = token.code;
        if (code.length != runtime.length || !endsWith(code, metadataTrailer(runtime))) {
            revert DeployedCodeMismatch(token, code.length, runtime.length);
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

    function _checkChain(Config memory cfg) private view {
        if (cfg.chainId != BASE && cfg.chainId != BASE_SEPOLIA) {
            revert UnsupportedChain(cfg.chainId);
        }
        if (cfg.chainId != block.chainid) {
            revert WrongChain(cfg.chainId, block.chainid);
        }
        if (cfg.chainId == BASE) {
            if (cfg.allowEoaTreasury) revert TestSwitchOnMainnet("PFT_TOKEN_ALLOW_EOA_TREASURY");
            if (cfg.allowUsedDeployer) revert TestSwitchOnMainnet("PFT_TOKEN_ALLOW_USED_DEPLOYER");
        }
        if (cfg.realDeployment) {
            if (cfg.allowEoaTreasury) revert RealDeploymentWithTestSwitch("PFT_TOKEN_ALLOW_EOA_TREASURY");
            if (cfg.allowUsedDeployer) revert RealDeploymentWithTestSwitch("PFT_TOKEN_ALLOW_USED_DEPLOYER");
            if (_isPlaceholder(bytes(cfg.sourceCommit))) revert SourceCommitMissing();
        }
    }

    function _checkMetadata(Config memory cfg) private pure {
        if (_isPlaceholder(bytes(cfg.name))) revert PlaceholderValue("PFT_TOKEN_NAME");
        if (_isPlaceholder(bytes(cfg.symbol))) revert PlaceholderValue("PFT_TOKEN_SYMBOL");
        if (bytes(cfg.name).length > MAX_NAME_BYTES) revert NameTooLong(bytes(cfg.name).length);
    }

    function _checkSupply(Config memory cfg) private pure {
        if (cfg.totalSupply == 0) revert PlaceholderValue("PFT_TOKEN_TOTAL_SUPPLY");
        if (
            cfg.totalSupplyPft > type(uint256).max / UNITS_PER_PFT
                || cfg.totalSupply != cfg.totalSupplyPft * UNITS_PER_PFT
        ) {
            revert SupplyMismatch(cfg.totalSupply, cfg.totalSupplyPft);
        }
    }

    /// The only relaxation: the test switch admits a treasury without code on
    /// 84532 (refused on 8453 by _checkChain). A contract must pass checkSafe.
    function _checkTreasury(Config memory cfg, address deployer) private view returns (SafeInfo memory safe) {
        if (cfg.treasury == address(0)) revert PlaceholderValue("PFT_TOKEN_TREASURY");
        if (cfg.treasury == deployer) revert TreasuryIsDeployer(cfg.treasury);
        if (cfg.treasury.code.length == 0) {
            if (!cfg.allowEoaTreasury) revert TreasuryNotAContract(cfg.treasury);
            _log("WARNING: test-only PFT_TOKEN_ALLOW_EOA_TREASURY; the treasury has no code");
            return safe;
        }
        safe = checkSafe(cfg.chainId, cfg.treasury);
        if (safe.guard != address(0) && safe.guard != cfg.allowedGuard) revert GuardEnabled(safe.guard);
    }

    function _distinctNonZero(address[] memory owners) private pure returns (uint256 distinct) {
        for (uint256 i = 0; i < owners.length; i++) {
            bool repeated = owners[i] == address(0);
            for (uint256 j = 0; j < i && !repeated; j++) {
                repeated = owners[j] == owners[i];
            }
            if (!repeated) distinct++;
        }
    }

    /// The broadcast transaction hash and block are not known while the script
    /// runs; they are "pending" here and copied in from broadcast/.../run-latest.json.
    /// The compiler settings are the effective ones of this build, read from the
    /// artifact's rawMetadata, whose IPFS digest `_checkArtifact` bound to the
    /// deployed code (so `--optimizer-runs` and other overrides are recorded), not
    /// from the artifact's uncovered metadata object or foundry.toml.
    function _writeRecord(
        Config memory cfg,
        PFTToken token,
        address deployer,
        SafeInfo memory safe,
        string memory artifact,
        string memory path
    ) private {
        string memory o = "pft_token_deployment";
        VM.serializeString(o, "mode", cfg.realDeployment ? "real" : "test");
        VM.serializeString(o, "tx_hash", "pending");
        VM.serializeString(o, "block_number", "pending");
        VM.serializeAddress(o, "token", address(token));
        VM.serializeUint(o, "chain_id", block.chainid);
        VM.serializeString(o, "name", cfg.name);
        VM.serializeString(o, "symbol", cfg.symbol);
        VM.serializeString(o, "total_supply_units", VM.toString(cfg.totalSupply));
        VM.serializeString(o, "total_supply_pft", VM.toString(cfg.totalSupplyPft));
        VM.serializeAddress(o, "treasury", cfg.treasury);
        VM.serializeString(
            o, "constructor_args", VM.toString(abi.encode(cfg.name, cfg.symbol, cfg.totalSupply, cfg.treasury))
        );
        VM.serializeAddress(o, "deployer", deployer);
        VM.serializeAddress(o, "treasury_safe_singleton", safe.singleton);
        VM.serializeUint(o, "treasury_safe_threshold", safe.threshold);
        VM.serializeUint(o, "treasury_safe_owner_count", safe.owners.length);
        VM.serializeAddress(o, "treasury_safe_owners", safe.owners);
        VM.serializeAddress(o, "treasury_safe_modules", safe.modules);
        VM.serializeAddress(o, "treasury_safe_guard", safe.guard);
        VM.serializeBytes32(o, "treasury_safe_creation_tx", safe.creationTx);
        VM.serializeAddress(o, "treasury_safe_factory", safe.factory);
        VM.serializeAddress(o, "treasury_safe_fallback_handler", safe.fallbackHandler);
        VM.serializeString(o, "artifact", cfg.artifact);
        VM.serializeBytes32(o, "creation_code_keccak256", keccak256(type(PFTToken).creationCode));
        VM.serializeBytes32(
            o, "runtime_code_keccak256", keccak256(VM.parseJsonBytes(artifact, ".deployedBytecode.object"))
        );
        VM.serializeBytes32(o, "deployed_code_keccak256", keccak256(address(token).code));
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
        if (needle.length > haystack.length) {
            return false;
        }
        for (uint256 i = 0; i <= haystack.length - needle.length; i++) {
            uint256 j = 0;
            while (j < needle.length && haystack[i + j] == needle[j]) {
                j++;
            }
            if (j == needle.length) {
                return true;
            }
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
