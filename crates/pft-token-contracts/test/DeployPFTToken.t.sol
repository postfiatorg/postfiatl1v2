// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.24;

import {PFTToken} from "../src/PFTToken.sol";
import {DeployPFTToken, ISafeProbe, ISafeSetupCall} from "../script/DeployPFTToken.s.sol";
import {SafeFixtures, ISafeProxyFactory} from "./SafeFixtures.sol";

interface IDeployTestVm {
    function chainId(uint256 newChainId) external;
    function etch(address target, bytes calldata newRuntimeBytecode) external;
    function store(address target, bytes32 slot, bytes32 value) external;
    function exists(string calldata path) external view returns (bool);
    function removeFile(string calldata path) external;
    function parseJsonBool(string calldata json, string calldata key) external pure returns (bool);
    function parseJsonAddressArray(string calldata json, string calldata key) external pure returns (address[] memory);
    function setNonce(address account, uint64 newNonce) external;
    function setNonceUnsafe(address account, uint64 newNonce) external;
    function setEnv(string calldata name, string calldata value) external;
    function expectRevert(bytes calldata revertData) external;
    function expectRevert(bytes4 revertData) external;
    function expectRevert() external;
    function readFile(string calldata path) external view returns (string memory data);
    function parseJsonUint(string calldata json, string calldata key) external pure returns (uint256);
    function parseJsonString(string calldata json, string calldata key) external pure returns (string memory);
    function parseJsonAddress(string calldata json, string calldata key) external pure returns (address);
    function toString(address value) external pure returns (string memory);
    function toString(bytes calldata value) external pure returns (string memory);
    function load(address target, bytes32 slot) external view returns (bytes32);
    function mockCall(address callee, bytes calldata data, bytes calldata returnData) external;
    function mockCallRevert(address callee, bytes calldata data, bytes calldata revertData) external;
    function clearMockedCalls() external;
    function prank(address msgSender) external;
    function isContext(uint8 context) external view returns (bool);
    function randomUint() external returns (uint256);
    function writeFile(string calldata path, string calldata data) external;
    function writeJson(string calldata json, string calldata path, string calldata valueKey) external;
    function createDir(string calldata path, bool recursive) external;
    function parseJsonBytes(string calldata json, string calldata key) external pure returns (bytes memory);
    function parseJsonBytes32(string calldata json, string calldata key) external pure returns (bytes32);
    function parseBytes(string calldata value) external pure returns (bytes memory);
    function replace(string calldata input, string calldata from, string calldata to)
        external
        pure
        returns (string memory);
}

/// The module surface of a real Safe; only the Safe itself may call it.
interface ISafeModuleAdmin {
    function enableModule(address module) external;
    function getModulesPaginated(address start, uint256 pageSize)
        external
        view
        returns (address[] memory array, address next);
}

/// A Safe lookalike: answers the probe with threshold 2 of 3 owners; the CLI
/// fixture writes an allowed singleton into its slot 0.
contract FakeSafe is ISafeProbe {
    function getThreshold() external pure returns (uint256) {
        return 2;
    }

    function getOwners() external pure returns (address[] memory owners) {
        owners = new address[](3);
        owners[0] = address(0x0A1);
        owners[1] = address(0x0A2);
        owners[2] = address(0x0A3);
    }
}

/// Answers the probe with the values it is built with; its constructor writes
/// slot 0, so no cheatcode is needed to fake the singleton.
contract ConfigurableSafe is ISafeProbe {
    address private singleton;
    uint256 private threshold;
    address[] private owners;

    constructor(address singleton_, uint256 threshold_, address[] memory owners_) {
        singleton = singleton_;
        threshold = threshold_;
        owners = owners_;
    }

    function getThreshold() external view returns (uint256) {
        return threshold;
    }

    function getOwners() external view returns (address[] memory) {
        return owners;
    }
}

/// A contract without the Safe functions and without a fallback.
contract NotASafe {
    function ping() external pure returns (uint256) {
        return 1;
    }
}

/// The broadcaster. Forge counts every top-level call of the test contract as a
/// transaction of its own, so the test contract cannot stand in for a fresh key.
contract Broadcaster {
    function deploy(DeployPFTToken script, DeployPFTToken.Config memory cfg) external returns (PFTToken) {
        return script.deploy(cfg);
    }

    function run(DeployPFTToken script) external returns (PFTToken) {
        return script.run();
    }
}

interface ISafeProxyFactoryCode {
    function proxyCreationCode() external pure returns (bytes memory);
}

contract DeployPFTTokenTest is SafeFixtures {
    IDeployTestVm private constant VM = IDeployTestVm(address(uint160(uint256(keccak256("hevm cheat code")))));

    uint256 private constant BASE = 8453;
    uint256 private constant BASE_SEPOLIA = 84532;
    /// The 2-of-3 SafeL2 1.4.1 created by the anvil fixture transaction (same
    /// factory, initializer and salt nonce, so the same CREATE2 address).
    address private TREASURY;
    bytes private creationRaw;
    string private constant NAME = "Post Fiat Token";
    string private constant SYMBOL = "PFT";
    uint256 private constant SUPPLY_PFT = 1_000_000;
    uint256 private constant SUPPLY = SUPPLY_PFT * 1_000_000;
    /// Safe v1.4.1 SafeL2, the canonical deployment of safe-global/safe-deployments.
    address private constant SAFE_L2 = 0x29fcB43b46531BcA003ddC8FCB67FFE91900C762;
    string private constant COMMIT = "0123456789abcdef0123456789abcdef01234567";
    string private constant ARTIFACT = "out/PFTToken.sol/PFTToken.json";
    /// `ForgeContext.Coverage` in forge-std's Vm.
    uint8 private constant COVERAGE = 2;
    address private constant MODULE = address(0xD0D0);
    address private constant GUARD = address(0x6A2D);

    DeployPFTToken private script;
    Broadcaster private broadcaster;
    /// The output directory whose PFTToken artifact is this build's token.
    string private outDir = "out";

    function setUp() public {
        script = new DeployPFTToken();
        broadcaster = new Broadcaster();
        _installSafeCode();
        TREASURY = ISafeProxyFactory(FACTORY_1_4_1).createProxyWithNonce(SAFE_L2, _plainSetup(_threeOwners(), 2), 0);
        creationRaw = _fixtureRaw("type2");
        VM.setNonceUnsafe(address(broadcaster), 0);
        VM.chainId(BASE_SEPOLIA);
        if (VM.isContext(COVERAGE)) outDir = _coverageOutDir();
    }

    /// `forge coverage` builds without the optimizer and writes no artifact, so
    /// out/ is not this build. A copy of out/'s artifact carrying this build's
    /// creation code stands in for it, in a directory of its own because the
    /// tests run in parallel.
    function _coverageOutDir() private returns (string memory dir) {
        dir = string.concat("cache/deploy-test/coverage-out-", VM.toString(abi.encode(VM.randomUint())));
        string memory path = string.concat(dir, "/PFTToken.sol/PFTToken.json");
        VM.createDir(string.concat(dir, "/PFTToken.sol"), true);
        VM.writeFile(path, VM.readFile(ARTIFACT));
        VM.writeJson(string.concat('"', VM.toString(type(PFTToken).creationCode), '"'), path, ".bytecode.object");
        bytes memory runtime = address(new PFTToken(NAME, SYMBOL, SUPPLY, address(0xE0A))).code;
        VM.writeJson(string.concat('"', VM.toString(runtime), '"'), path, ".deployedBytecode.object");
    }

    // ---------------------------------------------------------------- accepted

    function testDeploysOnBaseSepoliaAndWritesTheRecord() public {
        DeployPFTToken.Config memory cfg = _config(BASE_SEPOLIA);
        cfg.recordDir = "cache/deploy-test/base-sepolia";
        cfg.writeRecord = true;
        PFTToken token = broadcaster.deploy(script, cfg);
        _checkToken(token, NAME, SUPPLY);
        _checkRecord(cfg.recordDir, token, BASE_SEPOLIA, "test", SAFE_L2, 2, 3);
    }

    function testDeploysOnBaseMainnetAndWritesTheRecord() public {
        VM.chainId(BASE);
        DeployPFTToken.Config memory cfg = _config(BASE);
        cfg.recordDir = "cache/deploy-test/base";
        cfg.writeRecord = true;
        PFTToken token = broadcaster.deploy(script, cfg);
        _checkToken(token, NAME, SUPPLY);
        _checkRecord(cfg.recordDir, token, BASE, "test", SAFE_L2, 2, 3);
    }

    function testAcceptsAThirtyOneByteName() public {
        DeployPFTToken.Config memory cfg = _config(BASE_SEPOLIA);
        cfg.name = "Post Fiat Token on Base, 31 byt";
        _assertEq(bytes(cfg.name).length, 31, "fixture length");
        _checkToken(broadcaster.deploy(script, cfg), cfg.name, SUPPLY);
    }

    function testRunReadsTheEnvironment() public {
        // The only test that touches the process environment, so parallel
        // tests cannot race on it.
        VM.setEnv("PFT_TOKEN_CHAIN_ID", "84532");
        VM.setEnv("PFT_TOKEN_NAME", NAME);
        VM.setEnv("PFT_TOKEN_SYMBOL", SYMBOL);
        VM.setEnv("PFT_TOKEN_TOTAL_SUPPLY", "1000000000000");
        VM.setEnv("PFT_TOKEN_TOTAL_SUPPLY_PFT", "1000000");
        VM.setEnv("PFT_TOKEN_TREASURY", VM.toString(TREASURY));
        VM.setEnv("PFT_TOKEN_ALLOW_EOA_TREASURY", "");
        VM.setEnv("PFT_TOKEN_ALLOW_USED_DEPLOYER", "0");
        VM.setEnv("PFT_TOKEN_REAL_DEPLOYMENT", "");
        VM.setEnv("PFT_TOKEN_SOURCE_COMMIT", COMMIT);
        VM.setEnv("PFT_TOKEN_TREASURY_CREATION_TX", VM.toString(abi.encodePacked(keccak256(creationRaw))));
        VM.setEnv("PFT_TOKEN_TREASURY_CREATION_RAW_TX", VM.toString(creationRaw));
        VM.setEnv("PFT_TOKEN_ALLOW_GUARD", "");
        // The artifact follows FOUNDRY_OUT, the output directory forge itself uses.
        VM.setEnv("FOUNDRY_OUT", "");
        _assertTrue(_eq(script.readConfig().artifact, ARTIFACT), "default artifact");
        VM.setEnv("FOUNDRY_OUT", "cache/deploy-test/elsewhere");
        _assertTrue(
            _eq(script.readConfig().artifact, "cache/deploy-test/elsewhere/PFTToken.sol/PFTToken.json"),
            "FOUNDRY_OUT artifact"
        );
        VM.setEnv("FOUNDRY_OUT", outDir);

        DeployPFTToken.Config memory cfg = script.readConfig();
        _assertEq(cfg.chainId, BASE_SEPOLIA, "chain id");
        _assertEq(cfg.totalSupplyPft, SUPPLY_PFT, "supply in PFT");
        _assertTrue(cfg.treasury == TREASURY, "treasury");
        _assertTrue(!cfg.allowEoaTreasury && !cfg.allowUsedDeployer && !cfg.realDeployment, "switches off");
        _assertTrue(!cfg.writeRecord, "no record outside a broadcast");
        _assertTrue(_eq(cfg.recordDir, "cache/deploy-test/pft-token-base-84532"), "test record path");
        _assertTrue(_eq(cfg.sourceCommit, COMMIT), "source commit");
        _assertTrue(_eq(cfg.artifact, _artifact()), "token artifact");
        _assertTrue(cfg.creationTx == keccak256(creationRaw), "creation tx");
        _assertTrue(keccak256(cfg.creationRawTx) == keccak256(creationRaw), "creation raw tx");
        // The real-deployment switch alone selects the durable path.
        VM.setEnv("PFT_TOKEN_REAL_DEPLOYMENT", "true");
        _assertTrue(!script.readConfig().realDeployment, "only exactly 1 is on");
        VM.setEnv("PFT_TOKEN_REAL_DEPLOYMENT", "1");
        cfg = script.readConfig();
        _assertTrue(cfg.realDeployment, "real deployment");
        _assertTrue(_eq(cfg.recordDir, "deployments/pft-token-base-84532"), "durable record path");
        VM.setEnv("PFT_TOKEN_SOURCE_COMMIT", "");
        VM.expectRevert(DeployPFTToken.SourceCommitMissing.selector);
        broadcaster.run(script);
        VM.setEnv("PFT_TOKEN_SOURCE_COMMIT", COMMIT);
        VM.setEnv("PFT_TOKEN_REAL_DEPLOYMENT", "");
        _checkToken(broadcaster.run(script), NAME, SUPPLY);

        VM.setEnv("PFT_TOKEN_ALLOW_EOA_TREASURY", "1");
        VM.setEnv("PFT_TOKEN_ALLOW_USED_DEPLOYER", "1");
        VM.setEnv("PFT_TOKEN_TREASURY_CREATION_TX", "");
        VM.setEnv("PFT_TOKEN_TREASURY_CREATION_RAW_TX", "");
        VM.setEnv("PFT_TOKEN_ALLOW_GUARD", VM.toString(GUARD));
        cfg = script.readConfig();
        _assertTrue(cfg.allowEoaTreasury && cfg.allowUsedDeployer, "switches on");
        _assertTrue(cfg.creationTx == 0 && cfg.creationRawTx.length == 0, "creation optional in test mode");
        _assertTrue(cfg.allowedGuard == GUARD, "allowed guard");
        VM.setEnv("PFT_TOKEN_REAL_DEPLOYMENT", "1");
        VM.expectRevert(
            abi.encodeWithSelector(DeployPFTToken.RealDeploymentWithTestSwitch.selector, "PFT_TOKEN_ALLOW_EOA_TREASURY")
        );
        broadcaster.run(script);
        VM.setEnv("PFT_TOKEN_REAL_DEPLOYMENT", "");

        VM.setEnv("PFT_TOKEN_TOTAL_SUPPLY_PFT", "REPLACE_with_whole_pft");
        VM.expectRevert(_placeholder("PFT_TOKEN_TOTAL_SUPPLY_PFT"));
        broadcaster.run(script);
        VM.setEnv("PFT_TOKEN_TOTAL_SUPPLY_PFT", " ");
        VM.expectRevert(_placeholder("PFT_TOKEN_TOTAL_SUPPLY_PFT"));
        broadcaster.run(script);
    }

    // -------------------------------------------------------------- chain id

    function testRefusesAChainOtherThanBase() public {
        VM.chainId(1);
        VM.expectRevert(abi.encodeWithSelector(DeployPFTToken.UnsupportedChain.selector, 1));
        broadcaster.deploy(script, _config(1));
    }

    function testRefusesAChainIdMismatch() public {
        VM.expectRevert(abi.encodeWithSelector(DeployPFTToken.WrongChain.selector, BASE, BASE_SEPOLIA));
        broadcaster.deploy(script, _config(BASE));
        VM.chainId(BASE);
        VM.expectRevert(abi.encodeWithSelector(DeployPFTToken.WrongChain.selector, BASE_SEPOLIA, BASE));
        broadcaster.deploy(script, _config(BASE_SEPOLIA));
    }

    function testRefusesTheTestSwitchesOnBaseMainnet() public {
        VM.chainId(BASE);
        DeployPFTToken.Config memory cfg = _config(BASE);
        cfg.allowEoaTreasury = true;
        VM.expectRevert(
            abi.encodeWithSelector(DeployPFTToken.TestSwitchOnMainnet.selector, "PFT_TOKEN_ALLOW_EOA_TREASURY")
        );
        broadcaster.deploy(script, cfg);
        cfg = _config(BASE);
        cfg.allowUsedDeployer = true;
        VM.expectRevert(
            abi.encodeWithSelector(DeployPFTToken.TestSwitchOnMainnet.selector, "PFT_TOKEN_ALLOW_USED_DEPLOYER")
        );
        broadcaster.deploy(script, cfg);
    }

    // -------------------------------------------------------------- metadata

    function testRefusesPlaceholderNames() public {
        string[5] memory names = ["", "   ", "\t\n", "replace_with_token_name", "Post Fiat RePlAcE"];
        for (uint256 i = 0; i < names.length; i++) {
            DeployPFTToken.Config memory cfg = _config(BASE_SEPOLIA);
            cfg.name = names[i];
            VM.expectRevert(_placeholder("PFT_TOKEN_NAME"));
            broadcaster.deploy(script, cfg);
        }
    }

    function testRefusesPlaceholderSymbols() public {
        string[4] memory symbols = ["", " ", "REPLACE", "replace_with_token_symbol"];
        for (uint256 i = 0; i < symbols.length; i++) {
            DeployPFTToken.Config memory cfg = _config(BASE_SEPOLIA);
            cfg.symbol = symbols[i];
            VM.expectRevert(_placeholder("PFT_TOKEN_SYMBOL"));
            broadcaster.deploy(script, cfg);
        }
    }

    function testRefusesANameLongerThanThirtyOneBytes() public {
        DeployPFTToken.Config memory cfg = _config(BASE_SEPOLIA);
        cfg.name = "Post Fiat Token on Base, 32 byte";
        VM.expectRevert(abi.encodeWithSelector(DeployPFTToken.NameTooLong.selector, 32));
        broadcaster.deploy(script, cfg);
        // The reason for the cap: OpenZeppelin's EIP712 keeps the name as a ShortString.
        VM.expectRevert(abi.encodeWithSignature("StringTooLong(string)", cfg.name));
        new PFTToken(cfg.name, SYMBOL, SUPPLY, TREASURY);
    }

    // ---------------------------------------------------------------- supply

    function testRefusesAZeroSupply() public {
        DeployPFTToken.Config memory cfg = _config(BASE_SEPOLIA);
        cfg.totalSupply = 0;
        cfg.totalSupplyPft = 0;
        VM.expectRevert(_placeholder("PFT_TOKEN_TOTAL_SUPPLY"));
        broadcaster.deploy(script, cfg);
    }

    function testRefusesASupplyMismatch() public {
        DeployPFTToken.Config memory cfg = _config(BASE_SEPOLIA);
        cfg.totalSupplyPft = SUPPLY; // units given twice: off by 10^6
        VM.expectRevert(abi.encodeWithSelector(DeployPFTToken.SupplyMismatch.selector, SUPPLY, SUPPLY));
        broadcaster.deploy(script, cfg);
        cfg.totalSupplyPft = SUPPLY_PFT + 1;
        VM.expectRevert(abi.encodeWithSelector(DeployPFTToken.SupplyMismatch.selector, SUPPLY, SUPPLY_PFT + 1));
        broadcaster.deploy(script, cfg);
        // A whole-PFT value whose unit count would overflow is a mismatch, not a panic.
        cfg.totalSupply = type(uint256).max;
        cfg.totalSupplyPft = type(uint256).max;
        VM.expectRevert(
            abi.encodeWithSelector(DeployPFTToken.SupplyMismatch.selector, type(uint256).max, type(uint256).max)
        );
        broadcaster.deploy(script, cfg);
    }

    // -------------------------------------------------------------- treasury

    function testRefusesAZeroTreasury() public {
        DeployPFTToken.Config memory cfg = _config(BASE_SEPOLIA);
        cfg.treasury = address(0);
        cfg.allowEoaTreasury = true;
        VM.expectRevert(_placeholder("PFT_TOKEN_TREASURY"));
        broadcaster.deploy(script, cfg);
    }

    function testRefusesTheDeployerAsTreasury() public {
        DeployPFTToken.Config memory cfg = _config(BASE_SEPOLIA);
        cfg.treasury = address(broadcaster);
        VM.expectRevert(abi.encodeWithSelector(DeployPFTToken.TreasuryIsDeployer.selector, address(broadcaster)));
        broadcaster.deploy(script, cfg);
    }

    function testRefusesAnEoaTreasury() public {
        address eoa = address(0xE0A);
        DeployPFTToken.Config memory cfg = _config(BASE_SEPOLIA);
        cfg.treasury = eoa;
        VM.expectRevert(abi.encodeWithSelector(DeployPFTToken.TreasuryNotAContract.selector, eoa));
        broadcaster.deploy(script, cfg);
        VM.chainId(BASE);
        cfg.chainId = BASE;
        VM.expectRevert(abi.encodeWithSelector(DeployPFTToken.TreasuryNotAContract.selector, eoa));
        broadcaster.deploy(script, cfg);
    }

    function testAllowsAnEoaTreasuryOnBaseSepoliaOnlyWithTheTestSwitch() public {
        DeployPFTToken.Config memory cfg = _config(BASE_SEPOLIA);
        cfg.treasury = address(0xE0A);
        cfg.allowEoaTreasury = true;
        cfg.recordDir = "cache/deploy-test/eoa-treasury";
        cfg.writeRecord = true;
        PFTToken token = broadcaster.deploy(script, cfg);
        _assertEq(token.balanceOf(address(0xE0A)), SUPPLY, "EOA treasury balance");
        _checkRecord(cfg.recordDir, token, BASE_SEPOLIA, "test", address(0), 0, 0);
    }

    function testTheEoaSwitchDoesNotRelaxTheSafeCheckOfAContract() public {
        VM.store(TREASURY, bytes32(0), bytes32(0));
        DeployPFTToken.Config memory cfg = _config(BASE_SEPOLIA);
        cfg.allowEoaTreasury = true;
        VM.expectRevert(abi.encodeWithSelector(DeployPFTToken.UnofficialSafeSingleton.selector, TREASURY, address(0)));
        broadcaster.deploy(script, cfg);
    }

    function testAcceptsEveryPinnedSingletonOnBothChains() public view {
        address[4] memory singletons = [SINGLETON_1_4_1, SINGLETON_L2_1_4_1, SINGLETON_1_3_0, SINGLETON_L2_1_3_0];
        bytes32[4] memory hashes =
            [SINGLETON_1_4_1_HASH, SINGLETON_L2_1_4_1_HASH, SINGLETON_1_3_0_HASH, SINGLETON_L2_1_3_0_HASH];
        uint256[2] memory chains = [BASE, BASE_SEPOLIA];
        for (uint256 c = 0; c < chains.length; c++) {
            for (uint256 i = 0; i < singletons.length; i++) {
                _assertTrue(script.isOfficialSafeSingleton(chains[c], singletons[i]), "pinned singleton");
                _assertTrue(script.singletonCodeHash(chains[c], singletons[i]) == hashes[i], "pinned code hash");
                _assertTrue(singletons[i].codehash == hashes[i], "fixture code is the pinned code");
            }
            // The v1.3.0 eip155 deployment is listed for both chains but not pinned.
            _assertTrue(
                !script.isOfficialSafeSingleton(chains[c], 0xfb1bffC9d739B8D520DaF37dF666da4C687191EA), "eip155 L2"
            );
            _assertTrue(!script.isOfficialSafeSingleton(chains[c], address(0)), "zero singleton");
        }
        _assertTrue(!script.isOfficialSafeSingleton(1, singletons[0]), "other chain");
        _assertTrue(script.singletonCodeHash(1, singletons[0]) == bytes32(0), "no hash on another chain");
    }

    function testPinnedProxyCodeIsWhatTheCanonicalFactoriesDeploy() public {
        address p141 = _newProxy(FACTORY_1_4_1, SINGLETON_1_4_1);
        address p130 = _newProxy(FACTORY_1_3_0, SINGLETON_1_3_0);
        _assertTrue(p141.codehash == script.SAFE_PROXY_1_4_1_CODEHASH(), "SafeProxy 1.4.1 code hash");
        _assertTrue(p130.codehash == script.SAFE_PROXY_1_3_0_CODEHASH(), "GnosisSafeProxy 1.3.0 code hash");
        _assertTrue(
            p141.codehash == 0xd7d408ebcd99b2b70be43e20253d6d92a8ea8fab29bd3be7f55b10032331fb4c, "1.4.1 literal"
        );
        _assertTrue(
            p130.codehash == 0xb89c1b3bdf2cf8827818646bce9a8f6e372885f8c55e5c07acbd307cb133b000, "1.3.0 literal"
        );
        (bool ok, bytes memory ret) = p141.staticcall(abi.encodeWithSelector(script.MASTER_COPY_SELECTOR()));
        _assertTrue(ok && abi.decode(ret, (address)) == SINGLETON_1_4_1, "proxy answers masterCopy()");
    }

    function testAcceptsARealSafeOfEveryPinnedVersionOnBothChains() public {
        address[4] memory singletons = [SINGLETON_1_4_1, SINGLETON_L2_1_4_1, SINGLETON_1_3_0, SINGLETON_L2_1_3_0];
        address[4] memory factories = [FACTORY_1_4_1, FACTORY_1_4_1, FACTORY_1_3_0, FACTORY_1_3_0];
        uint256[2] memory chains = [BASE, BASE_SEPOLIA];
        for (uint256 c = 0; c < chains.length; c++) {
            VM.chainId(chains[c]);
            for (uint256 i = 0; i < singletons.length; i++) {
                bytes memory call = abi.encodeCall(
                    ISafeProxyFactory.createProxyWithNonce,
                    (singletons[i], _plainSetup(_threeOwners(), 2), 100 + c * 16 + i)
                );
                (, bytes memory ret) = factories[i].call(call);
                address treasury = abi.decode(ret, (address));
                DeployPFTToken.SafeInfo memory safe = script.checkSafe(chains[c], treasury);
                _assertTrue(safe.singleton == singletons[i], "singleton");
                DeployPFTToken.Config memory cfg = _config(chains[c]);
                cfg.treasury = treasury;
                cfg.creationRawTx = _rawTx(factories[i], call);
                cfg.creationTx = keccak256(cfg.creationRawTx);
                Broadcaster fresh = new Broadcaster(); // nonce 0, as the script requires
                VM.setNonceUnsafe(address(fresh), 0);
                PFTToken token = fresh.deploy(script, cfg);
                _assertEq(token.balanceOf(treasury), SUPPLY, "Safe holds the supply");
            }
        }
    }

    function testCheckSafeReadsTheSafe() public view {
        DeployPFTToken.SafeInfo memory safe = script.checkSafe(BASE_SEPOLIA, TREASURY);
        _assertTrue(safe.singleton == SAFE_L2, "singleton");
        _assertEq(safe.threshold, 2, "threshold");
        _assertEq(safe.owners.length, 3, "owners");
        _assertTrue(safe.owners[2] == address(0x0A3), "third owner");
    }

    function testRefusesAConstructorDeployedLookalikeOnBothChains() public {
        // The review's case: no cheatcode; the constructor writes an allowed
        // singleton into slot 0 and the probe answers look right.
        address safe = address(new ConfigurableSafe(SAFE_L2, 2, _threeOwners()));
        _assertTrue(uint256(VM.load(safe, bytes32(0))) == uint256(uint160(SAFE_L2)), "slot 0 names the singleton");
        bytes[3] memory codes = [address(new FakeSafe()).code, address(new NotASafe()).code, bytes(hex"00")];
        uint256[2] memory chains = [BASE, BASE_SEPOLIA];
        for (uint256 c = 0; c < chains.length; c++) {
            VM.chainId(chains[c]);
            DeployPFTToken.Config memory cfg = _config(chains[c]);
            cfg.treasury = safe;
            VM.expectRevert(abi.encodeWithSelector(DeployPFTToken.ProxyCodeMismatch.selector, safe, safe.codehash));
            broadcaster.deploy(script, cfg);
            for (uint256 i = 0; i < codes.length; i++) {
                VM.etch(TREASURY, codes[i]);
                VM.store(TREASURY, bytes32(0), bytes32(uint256(uint160(SAFE_L2))));
                VM.expectRevert(
                    abi.encodeWithSelector(DeployPFTToken.ProxyCodeMismatch.selector, TREASURY, keccak256(codes[i]))
                );
                broadcaster.deploy(script, _config(chains[c]));
            }
        }
    }

    function testRefusesALookalikeWithAWrongSingletonOnBothChains() public {
        address wrong = address(new FakeSafe()); // real proxy code, wrong singleton
        uint256[2] memory chains = [BASE, BASE_SEPOLIA];
        for (uint256 c = 0; c < chains.length; c++) {
            VM.chainId(chains[c]);
            VM.store(TREASURY, bytes32(0), bytes32(uint256(uint160(wrong))));
            VM.expectRevert(abi.encodeWithSelector(DeployPFTToken.UnofficialSafeSingleton.selector, TREASURY, wrong));
            broadcaster.deploy(script, _config(chains[c]));
            // An official address with dirty upper bits is not the singleton.
            VM.store(TREASURY, bytes32(0), bytes32(uint256(uint160(SAFE_L2)) | (uint256(1) << 160)));
            VM.expectRevert(abi.encodeWithSelector(DeployPFTToken.UnofficialSafeSingleton.selector, TREASURY, SAFE_L2));
            broadcaster.deploy(script, _config(chains[c]));
        }
    }

    function testRefusesAMasterCopyThatIsNotSlotZero() public {
        bytes memory call = abi.encodeWithSelector(script.MASTER_COPY_SELECTOR());
        bytes memory mismatch = abi.encodeWithSelector(DeployPFTToken.MasterCopyMismatch.selector, TREASURY, SAFE_L2);
        VM.mockCall(TREASURY, call, abi.encode(SINGLETON_1_4_1));
        VM.expectRevert(mismatch);
        broadcaster.deploy(script, _config(BASE_SEPOLIA));
        VM.clearMockedCalls();
        VM.mockCall(TREASURY, call, hex"01");
        VM.expectRevert(mismatch);
        broadcaster.deploy(script, _config(BASE_SEPOLIA));
        VM.clearMockedCalls();
        VM.mockCallRevert(TREASURY, call, "");
        VM.expectRevert(mismatch);
        broadcaster.deploy(script, _config(BASE_SEPOLIA));
    }

    function testRefusesASingletonWithoutItsPinnedCode() public {
        // The claimed singleton has another Safe's code, then no code at all.
        VM.etch(SAFE_L2, address(SINGLETON_1_4_1).code);
        VM.expectRevert(
            abi.encodeWithSelector(DeployPFTToken.SingletonCodeMismatch.selector, SAFE_L2, SINGLETON_1_4_1_HASH)
        );
        broadcaster.deploy(script, _config(BASE_SEPOLIA));
        VM.etch(SAFE_L2, "");
        VM.expectRevert(
            abi.encodeWithSelector(DeployPFTToken.SingletonCodeMismatch.selector, SAFE_L2, SAFE_L2.codehash)
        );
        broadcaster.deploy(script, _config(BASE_SEPOLIA));
    }

    function testRefusesAThresholdBelowTwoOnBothChains() public {
        for (uint256 threshold = 0; threshold < 2; threshold++) {
            // Threshold 0: a proxy that was never set up.
            address safe = address(uint160(0x5AFE10 + threshold));
            _installSafe(safe, FACTORY_1_4_1, SAFE_L2, _threeOwners(), threshold);
            uint256[2] memory chains = [BASE, BASE_SEPOLIA];
            for (uint256 c = 0; c < chains.length; c++) {
                VM.chainId(chains[c]);
                DeployPFTToken.Config memory cfg = _config(chains[c]);
                cfg.treasury = safe;
                VM.expectRevert(abi.encodeWithSelector(DeployPFTToken.SafeThresholdTooLow.selector, threshold));
                broadcaster.deploy(script, cfg);
            }
        }
    }

    function testRefusesSafeOwnersThatAreTooFewZeroRepeatedOrBelowTheThreshold() public {
        // A real Safe cannot hold these; the answers are mocked to keep the check honest.
        address[][4] memory sets;
        sets[0] = new address[](1);
        sets[0][0] = address(0x0A1);
        sets[1] = new address[](2);
        sets[1][0] = address(0x0A1); // sets[1][1] stays zero
        sets[2] = new address[](2);
        (sets[2][0], sets[2][1]) = (address(0x0A1), address(0x0A1));
        sets[3] = new address[](2);
        (sets[3][0], sets[3][1]) = (address(0x0A1), address(0x0A2));
        uint256[4] memory thresholds = [uint256(2), 2, 2, 3];
        uint256[4] memory distinct = [uint256(1), 1, 1, 2];
        for (uint256 i = 0; i < sets.length; i++) {
            VM.mockCall(TREASURY, abi.encodeCall(ISafeProbe.getThreshold, ()), abi.encode(thresholds[i]));
            VM.mockCall(TREASURY, abi.encodeCall(ISafeProbe.getOwners, ()), abi.encode(sets[i]));
            VM.expectRevert(
                abi.encodeWithSelector(
                    DeployPFTToken.SafeOwnersInvalid.selector, sets[i].length, distinct[i], thresholds[i]
                )
            );
            broadcaster.deploy(script, _config(BASE_SEPOLIA));
            VM.clearMockedCalls();
        }
    }

    function testRefusesASafeThatDoesNotAnswerTheProbeOnBothChains() public {
        bytes memory threshold = abi.encodeCall(ISafeProbe.getThreshold, ());
        bytes memory owners = abi.encodeCall(ISafeProbe.getOwners, ());
        bytes[4] memory badOwners = [
            abi.encode(uint256(0x20)), // shorter than an array head
            abi.encode(uint256(0x40), uint256(1)), // wrong offset
            abi.encode(uint256(0x20), uint256(5)), // count beyond the data
            abi.encode(uint256(0x20), uint256(0), uint256(7)) // trailing word
        ];
        uint256[2] memory chains = [BASE, BASE_SEPOLIA];
        for (uint256 c = 0; c < chains.length; c++) {
            VM.chainId(chains[c]);
            VM.mockCallRevert(TREASURY, threshold, "");
            _expectNotASafe(chains[c]);
            VM.clearMockedCalls();
            VM.mockCall(TREASURY, threshold, hex"02");
            _expectNotASafe(chains[c]);
            VM.clearMockedCalls();
            VM.mockCallRevert(TREASURY, owners, "");
            _expectNotASafe(chains[c]);
            VM.clearMockedCalls();
            for (uint256 i = 0; i < badOwners.length; i++) {
                VM.mockCall(TREASURY, owners, badOwners[i]);
                _expectNotASafe(chains[c]);
                VM.clearMockedCalls();
            }
        }
    }

    function testRefusesARealSafeWithAModuleOfEveryPinnedVersion() public {
        // The review's case: a genuine Safe, threshold 2, with an EOA module
        // that could move the treasury without owner signatures, in a
        // real-deployment dry run.
        address[4] memory singletons = [SINGLETON_1_4_1, SINGLETON_L2_1_4_1, SINGLETON_1_3_0, SINGLETON_L2_1_3_0];
        address[4] memory factories = [FACTORY_1_4_1, FACTORY_1_4_1, FACTORY_1_3_0, FACTORY_1_3_0];
        for (uint256 i = 0; i < singletons.length; i++) {
            address treasury = address(uint160(0x5AFE40 + i));
            _installSafe(treasury, factories[i], singletons[i], _threeOwners(), 2);
            DeployPFTToken.Config memory cfg = _config(BASE_SEPOLIA);
            cfg.treasury = treasury;
            cfg.realDeployment = true;
            cfg.sourceCommit = COMMIT;
            script.checkSafe(BASE_SEPOLIA, treasury); // the clean Safe passes
            VM.prank(treasury);
            ISafeModuleAdmin(treasury).enableModule(MODULE);
            (address[] memory modules,) = ISafeModuleAdmin(treasury).getModulesPaginated(address(0x1), 10);
            _assertTrue(modules.length == 1 && modules[0] == MODULE, "module enabled");
            VM.expectRevert(abi.encodeWithSelector(DeployPFTToken.ModulesEnabled.selector, 1));
            broadcaster.deploy(script, cfg);
        }
    }

    function testRefusesAModuleListThatIsNotEmpty() public {
        bytes memory call = abi.encodeCall(ISafeModuleAdmin.getModulesPaginated, (address(0x1), 10));
        address[] memory none = new address[](0);
        // Empty page whose next pointer is not the sentinel: more modules follow.
        VM.mockCall(TREASURY, call, abi.encode(none, address(0xD0D1)));
        VM.expectRevert(abi.encodeWithSelector(DeployPFTToken.ModulesEnabled.selector, 0));
        broadcaster.deploy(script, _config(BASE_SEPOLIA));
        VM.clearMockedCalls();
        VM.mockCallRevert(TREASURY, call, "");
        _expectNotASafe(BASE_SEPOLIA);
        VM.clearMockedCalls();
        VM.mockCall(TREASURY, call, abi.encode(uint256(0x40), uint256(1)));
        _expectNotASafe(BASE_SEPOLIA);
    }

    function testRefusesAGuardUnlessItIsNamed() public {
        VM.store(TREASURY, script.GUARD_STORAGE_SLOT(), bytes32(uint256(uint160(GUARD))));
        _assertTrue(script.checkSafe(BASE_SEPOLIA, TREASURY).guard == GUARD, "guard read");
        DeployPFTToken.Config memory cfg = _config(BASE_SEPOLIA);
        VM.expectRevert(abi.encodeWithSelector(DeployPFTToken.GuardEnabled.selector, GUARD));
        broadcaster.deploy(script, cfg);
        cfg.allowedGuard = address(0x6A2E);
        VM.expectRevert(abi.encodeWithSelector(DeployPFTToken.GuardEnabled.selector, GUARD));
        broadcaster.deploy(script, cfg);
        cfg.allowedGuard = GUARD;
        cfg.recordDir = "cache/deploy-test/guard";
        cfg.writeRecord = true;
        PFTToken token = broadcaster.deploy(script, cfg);
        _checkRecord(cfg.recordDir, token, BASE_SEPOLIA, "test", SAFE_L2, 2, 3);
        string memory json = VM.readFile(string.concat(cfg.recordDir, "/deployment.json"));
        _assertTrue(VM.parseJsonAddress(json, ".treasury_safe_guard") == GUARD, "record guard");
    }

    function _expectNotASafe(uint256 chainId) private {
        VM.expectRevert(abi.encodeWithSelector(DeployPFTToken.TreasuryIsNotASafe.selector, TREASURY));
        broadcaster.deploy(script, _config(chainId));
    }

    // ------------------------------------------------------- deployment record

    function testRefusesARealDeploymentWithATestSwitch() public {
        DeployPFTToken.Config memory cfg = _config(BASE_SEPOLIA);
        cfg.realDeployment = true;
        cfg.allowEoaTreasury = true;
        VM.expectRevert(
            abi.encodeWithSelector(DeployPFTToken.RealDeploymentWithTestSwitch.selector, "PFT_TOKEN_ALLOW_EOA_TREASURY")
        );
        broadcaster.deploy(script, cfg);
        cfg.allowEoaTreasury = false;
        cfg.allowUsedDeployer = true;
        VM.expectRevert(
            abi.encodeWithSelector(
                DeployPFTToken.RealDeploymentWithTestSwitch.selector, "PFT_TOKEN_ALLOW_USED_DEPLOYER"
            )
        );
        broadcaster.deploy(script, cfg);
    }

    function testRefusesARealDeploymentWithoutASourceCommit() public {
        string[3] memory commits = ["", " ", "replace_with_the_deployed_git_commit"];
        for (uint256 i = 0; i < commits.length; i++) {
            DeployPFTToken.Config memory cfg = _config(BASE_SEPOLIA);
            cfg.realDeployment = true;
            cfg.sourceCommit = commits[i];
            VM.expectRevert(DeployPFTToken.SourceCommitMissing.selector);
            broadcaster.deploy(script, cfg);
        }
    }

    function testRefusesAnArtifactOfAnotherBuild() public {
        // out/PFTToken.sol/PFTToken.json of `forge build --optimizer-runs 201`;
        // this build uses foundry.toml's 200, so its metadata would misdescribe
        // the deployed code. A dry run refuses it too.
        string memory other = "test/fixtures/PFTToken.optimizer-runs-201.json";
        bytes memory code = VM.parseJsonBytes(VM.readFile(other), ".bytecode.object");
        DeployPFTToken.Config memory cfg = _config(BASE_SEPOLIA);
        cfg.artifact = other;
        VM.expectRevert(
            abi.encodeWithSelector(
                DeployPFTToken.ArtifactMismatch.selector, other, keccak256(code), keccak256(type(PFTToken).creationCode)
            )
        );
        broadcaster.deploy(script, cfg);
        cfg.realDeployment = true;
        cfg.sourceCommit = COMMIT;
        cfg.writeRecord = true;
        cfg.recordDir = "cache/deploy-test/stale-artifact"; // never written
        VM.expectRevert(
            abi.encodeWithSelector(
                DeployPFTToken.ArtifactMismatch.selector, other, keccak256(code), keccak256(type(PFTToken).creationCode)
            )
        );
        broadcaster.deploy(script, cfg);
    }

    function testRefusesAnArtifactWithoutTheIpfsMetadataHash() public {
        // The hash kind is read from rawMetadata, the string the code's trailer digests.
        string memory path = _artifactCopy("no-metadata");
        string memory raw = VM.parseJsonString(VM.readFile(path), ".rawMetadata");
        string memory none = VM.replace(raw, '"bytecodeHash":"ipfs"', '"bytecodeHash":"none"');
        _assertTrue(!_eq(raw, none), "rawMetadata names none");
        VM.writeJson(string.concat('"', _escape(none), '"'), path, ".rawMetadata");
        VM.expectRevert(DeployPFTToken.MetadataHashDisabled.selector);
        script.checkArtifactMetadata(path, VM.readFile(path), _outCreationCode());
    }

    function testTheEditableMetadataObjectDoesNotGateTheHashKind() public {
        // Only the unchecked metadata object says none; the trailer still carries ipfs.
        string memory path = _artifactCopy("object-none");
        VM.writeJson('"none"', path, ".metadata.settings.metadata.bytecodeHash");
        script.checkArtifactMetadata(path, VM.readFile(path), _outCreationCode());
    }

    function testRefusesATruncatedRuntimeBeforeTheBroadcast() public {
        // The runtime cut to its last 1,000 bytes still ends the creation code and
        // keeps the trailer; the creation code declares the full runtime length.
        bytes memory creation = _outCreationCode();
        bytes memory runtime = VM.parseJsonBytes(VM.readFile(ARTIFACT), ".deployedBytecode.object");
        _assertEq(script.declaredRuntimeLength(creation), runtime.length, "declared runtime length");
        string memory path = _artifactCopy("truncated");
        VM.writeJson(string.concat('"', VM.toString(_last(runtime, 1000)), '"'), path, ".deployedBytecode.object");
        VM.expectRevert(abi.encodeWithSelector(DeployPFTToken.ArtifactMetadataMismatch.selector, path));
        script.checkArtifactMetadata(path, VM.readFile(path), creation);
        // deploy refuses it before the broadcast (not DeployedCodeMismatch after it).
        // Under coverage deploy skips the metadata check; see _checkArtifact.
        if (!VM.isContext(COVERAGE)) {
            DeployPFTToken.Config memory cfg = _config(BASE_SEPOLIA);
            cfg.artifact = path;
            VM.expectRevert(abi.encodeWithSelector(DeployPFTToken.ArtifactMetadataMismatch.selector, path));
            broadcaster.deploy(script, cfg);
        }
        script.checkArtifactMetadata(ARTIFACT, VM.readFile(ARTIFACT), creation);
        // No `PUSH size PUSH offset PUSH0 CODECOPY` covering the code's end, or a cut PUSH: none declared.
        _assertEq(script.declaredRuntimeLength(""), 0, "empty");
        _assertEq(script.declaredRuntimeLength(hex"6001600260003900"), 0, "offset + size is not the length");
        _assertEq(script.declaredRuntimeLength(hex"6001600139"), 0, "two pushes only");
        _assertEq(script.declaredRuntimeLength(hex"61"), 0, "a cut push");
        _assertEq(script.declaredRuntimeLength(hex"6001600860003900ff"), 1, "PUSH1 0x00 form");
        _assertEq(script.declaredRuntimeLength(hex"600160075f3900ff"), 1, "PUSH0 form");
    }

    function testRefusesAnArtifactWhoseMetadataIsFromAnotherBuild() public {
        // The code of this build with the metadata of a build with other optimizer runs.
        string memory path = _artifactCopy("spliced");
        string memory raw = VM.parseJsonString(VM.readFile(path), ".rawMetadata");
        string memory spliced = VM.replace(raw, '"runs":200', '"runs":201');
        _assertTrue(!_eq(raw, spliced), "spliced metadata differs");
        VM.writeJson(string.concat('"', _escape(spliced), '"'), path, ".rawMetadata");
        bytes memory creation = _outCreationCode();
        VM.expectRevert(abi.encodeWithSelector(DeployPFTToken.ArtifactMetadataMismatch.selector, path));
        script.checkArtifactMetadata(path, VM.readFile(path), creation);
        // A runtime code that is not the end of the creation code.
        path = _artifactCopy("runtime");
        VM.writeJson(string.concat('"', VM.toString(hex"00"), '"'), path, ".deployedBytecode.object");
        VM.expectRevert(abi.encodeWithSelector(DeployPFTToken.ArtifactMetadataMismatch.selector, path));
        script.checkArtifactMetadata(path, VM.readFile(path), creation);
        // out/'s own build passes.
        script.checkArtifactMetadata(ARTIFACT, VM.readFile(ARTIFACT), creation);
    }

    function testRecordsTheCompilerSettingsOfTheRawMetadata() public {
        // The metadata object is not covered by the code's IPFS digest; the record ignores it.
        string memory dir =
            string.concat("cache/deploy-test/edited-metadata-", VM.toString(abi.encode(VM.randomUint())));
        string memory path = string.concat(dir, "/PFTToken.sol/PFTToken.json");
        VM.createDir(string.concat(dir, "/PFTToken.sol"), true);
        VM.writeFile(path, VM.readFile(_artifact()));
        VM.writeJson('"0.8.20+commit.a1b79de6"', path, ".metadata.compiler.version");
        VM.writeJson("999", path, ".metadata.settings.optimizer.runs");
        VM.writeJson("false", path, ".metadata.settings.optimizer.enabled");
        VM.writeJson('"paris"', path, ".metadata.settings.evmVersion");
        DeployPFTToken.Config memory cfg = _config(BASE_SEPOLIA);
        cfg.artifact = path;
        cfg.recordDir = string.concat(dir, "/record");
        cfg.writeRecord = true;
        broadcaster.deploy(script, cfg);
        string memory json = VM.readFile(string.concat(cfg.recordDir, "/deployment.json"));
        _assertTrue(_eq(VM.parseJsonString(json, ".solc_version"), "0.8.24+commit.e11b9ed9"), "solc of rawMetadata");
        _assertEq(VM.parseJsonUint(json, ".optimizer_runs"), 200, "runs of rawMetadata");
        _assertTrue(VM.parseJsonBool(json, ".optimizer"), "optimizer of rawMetadata");
        _assertTrue(_eq(VM.parseJsonString(json, ".evm_version"), "cancun"), "evm version of rawMetadata");
    }

    function testRefusesATrailerOnlyRuntimeCode() public {
        bytes memory runtime = VM.parseJsonBytes(VM.readFile(ARTIFACT), ".deployedBytecode.object");
        bytes memory trailer = script.metadataTrailer(runtime);
        _assertTrue(trailer.length == 53 && script.metadataDigest(trailer) != 0, "a 53-byte trailer with the digest");
        string memory path = _artifactCopy("trailer-only");
        VM.writeJson(string.concat('"', VM.toString(trailer), '"'), path, ".deployedBytecode.object");
        VM.expectRevert(abi.encodeWithSelector(DeployPFTToken.ArtifactMetadataMismatch.selector, path));
        script.checkArtifactMetadata(path, VM.readFile(path), _outCreationCode());
        // After the broadcast: the deployed code has the artifact's length and trailer.
        PFTToken token = broadcaster.deploy(script, _config(BASE_SEPOLIA));
        bytes memory built = VM.parseJsonBytes(VM.readFile(_artifact()), ".deployedBytecode.object");
        script.checkDeployedCode(address(token), built);
        uint256 length = address(token).code.length;
        VM.expectRevert(
            abi.encodeWithSelector(DeployPFTToken.DeployedCodeMismatch.selector, address(token), length, 53)
        );
        script.checkDeployedCode(address(token), trailer);
        bytes memory other = abi.encodePacked(built);
        other[other.length - 3] = bytes1(uint8(other[other.length - 3]) ^ 1);
        VM.expectRevert(
            abi.encodeWithSelector(DeployPFTToken.DeployedCodeMismatch.selector, address(token), length, length)
        );
        script.checkDeployedCode(address(token), other);
        _assertTrue(
            script.metadataTrailer(hex"00").length == 0 && script.metadataTrailer(hex"ffff").length == 0, "none"
        );
    }

    function testMetadataDigestReadsTheCborTrailer() public view {
        bytes32 digest = sha256("metadata");
        bytes memory cbor = abi.encodePacked(hex"a264697066735822", hex"1220", digest, hex"64736f6c6343000818");
        bytes memory code = abi.encodePacked(hex"6080", cbor, uint16(cbor.length));
        _assertTrue(script.metadataDigest(code) == digest, "digest");
        _assertTrue(script.metadataDigest(hex"00") == 0, "too short");
        _assertTrue(script.metadataDigest(hex"ffff") == 0, "length beyond the code");
        _assertTrue(script.metadataDigest(abi.encodePacked(hex"6080", uint16(1))) == 0, "no ipfs key");
        bytes memory other = abi.encodePacked(hex"a264697066735822", hex"1221", digest);
        _assertTrue(script.metadataDigest(abi.encodePacked(other, uint16(other.length))) == 0, "not sha256");
        _assertTrue(script.endsWith(hex"010203", hex"0203") && !script.endsWith(hex"0203", hex"010203"), "ends");
        _assertTrue(!script.endsWith(hex"010203", "") && !script.endsWith(hex"010203", hex"0103"), "ends not");
        // `ipfs add --only-hash` of a file holding "hello\n" is QmZULkCELmmk5XNfCgTnCyFgAVxBRBXyDHGGMVoLFLiXEN.
        _assertTrue(
            script.ipfsDigest("hello\n") == 0xa568e404d8eadaec925ac3bc1b36259c0f6d70000d729cd918c46a836f497657, "ipfs"
        );
        _assertTrue(script.ipfsDigest(new bytes(262145)) == 0, "one chunk only");
    }

    function testTheArtifactUnderTestIsThisBuild() public view {
        bytes memory code = VM.parseJsonBytes(VM.readFile(_artifact()), ".bytecode.object");
        _assertTrue(keccak256(code) == keccak256(type(PFTToken).creationCode), "artifact is this build");
    }

    function testRealDeploymentWritesOnceAndRefusesToOverwrite() public {
        DeployPFTToken.Config memory cfg = _config(BASE_SEPOLIA);
        cfg.realDeployment = true;
        cfg.recordDir = "cache/deploy-test/real-once";
        cfg.writeRecord = true;
        cfg.sourceCommit = COMMIT;
        string memory path = string.concat(cfg.recordDir, "/deployment.json");
        if (VM.exists(path)) VM.removeFile(path);
        PFTToken token = broadcaster.deploy(script, cfg);
        _checkRecord(cfg.recordDir, token, BASE_SEPOLIA, "real", SAFE_L2, 2, 3);
        string memory json = VM.readFile(path);
        _assertTrue(_eq(VM.parseJsonString(json, ".source_commit"), COMMIT), "record source commit");

        // A second run, even a dry run that writes nothing, stops before deploying.
        VM.setNonceUnsafe(address(broadcaster), 0);
        cfg.writeRecord = false;
        VM.expectRevert(abi.encodeWithSelector(DeployPFTToken.RecordExists.selector, path));
        broadcaster.deploy(script, cfg);
    }

    // -------------------------------------------------------------- deployer

    function testRefusesAUsedDeployer() public {
        VM.setNonce(address(broadcaster), 5);
        VM.expectRevert(abi.encodeWithSelector(DeployPFTToken.DeployerAlreadyUsed.selector, address(broadcaster), 5));
        broadcaster.deploy(script, _config(BASE_SEPOLIA));
        VM.chainId(BASE);
        VM.expectRevert(abi.encodeWithSelector(DeployPFTToken.DeployerAlreadyUsed.selector, address(broadcaster), 5));
        broadcaster.deploy(script, _config(BASE));
    }

    function testAllowsAUsedDeployerOnBaseSepoliaOnlyWithTheTestSwitch() public {
        VM.setNonce(address(broadcaster), 5);
        DeployPFTToken.Config memory cfg = _config(BASE_SEPOLIA);
        cfg.allowUsedDeployer = true;
        _checkToken(broadcaster.deploy(script, cfg), NAME, SUPPLY);
    }

    // ------------------------------------------------------ Safe creation

    function testDecodesTheAnvilCreationTransactions() public {
        bytes memory init = _plainSetup(_threeOwners(), 2);
        string[2] memory names = ["type2", "legacy"];
        for (uint256 i = 0; i < 2; i++) {
            (uint256 chain, address to, bytes memory data) = script.decodeRawTransaction(_fixtureRaw(names[i]));
            _assertTrue(chain == BASE_SEPOLIA && to == FACTORY_1_4_1, "chain id and to");
            bytes memory expected = abi.encodeCall(ISafeProxyFactory.createProxyWithNonce, (SAFE_L2, init, i));
            _assertTrue(keccak256(data) == keccak256(expected), "calldata");
        }
        address legacy = ISafeProxyFactory(FACTORY_1_4_1).createProxyWithNonce(SAFE_L2, init, 1);
        DeployPFTToken.SafeInfo memory safe = script.checkSafe(BASE_SEPOLIA, legacy);
        (address factory, address handler) = script.checkCreation(BASE_SEPOLIA, legacy, safe, _fixtureRaw("legacy"));
        _assertTrue(factory == FACTORY_1_4_1 && handler == address(0), "legacy creation accepted");
    }

    function testRecordsTheCreation() public {
        DeployPFTToken.Config memory cfg = _config(BASE);
        VM.chainId(BASE);
        cfg.recordDir = "cache/deploy-test/creation";
        cfg.writeRecord = true;
        broadcaster.deploy(script, cfg);
        string memory json = VM.readFile(string.concat(cfg.recordDir, "/deployment.json"));
        _assertTrue(VM.parseJsonBytes32(json, ".treasury_safe_creation_tx") == cfg.creationTx, "tx");
        _assertTrue(VM.parseJsonAddress(json, ".treasury_safe_factory") == FACTORY_1_4_1, "factory");
        _assertTrue(VM.parseJsonAddress(json, ".treasury_safe_fallback_handler") == address(0), "handler");
    }

    function testRefusesACreationTransactionOfAnotherChain() public {
        bytes memory init = _plainSetup(_threeOwners(), 2);
        address legacy = ISafeProxyFactory(FACTORY_1_4_1).createProxyWithNonce(SAFE_L2, init, 1);
        DeployPFTToken.SafeInfo memory safe = script.checkSafe(BASE, TREASURY);
        DeployPFTToken.SafeInfo memory legacySafe = script.checkSafe(BASE, legacy);
        // The CREATE2 address and the factory are the same on both chains; the signature is not.
        VM.chainId(BASE);
        VM.expectRevert(abi.encodeWithSelector(DeployPFTToken.TreasuryCreationWrongChain.selector, BASE_SEPOLIA));
        script.checkCreation(BASE, TREASURY, safe, creationRaw);
        VM.expectRevert(abi.encodeWithSelector(DeployPFTToken.TreasuryCreationWrongChain.selector, BASE_SEPOLIA));
        script.checkCreation(BASE, legacy, legacySafe, _fixtureRaw("legacy"));
        DeployPFTToken.Config memory cfg = _config(BASE);
        cfg.creationRawTx = creationRaw;
        cfg.creationTx = keccak256(creationRaw);
        VM.expectRevert(abi.encodeWithSelector(DeployPFTToken.TreasuryCreationWrongChain.selector, BASE_SEPOLIA));
        broadcaster.deploy(script, cfg);
        (address factory,) = script.checkCreation(BASE, TREASURY, safe, _fixtureRaw("type2-8453"));
        _assertTrue(factory == FACTORY_1_4_1, "the 8453 creation accepted on 8453");
        VM.chainId(BASE_SEPOLIA);
        VM.expectRevert(abi.encodeWithSelector(DeployPFTToken.TreasuryCreationWrongChain.selector, BASE));
        script.checkCreation(BASE_SEPOLIA, TREASURY, safe, _fixtureRaw("type2-8453"));
        // A legacy v without EIP-155 (27, 28) names no chain.
        bytes memory signature = _last(_fixtureRaw("legacy"), 66);
        for (uint256 v = 27; v <= 28; v++) {
            bytes memory raw = _alter("legacy", 70, abi.encodePacked(uint8(v), signature));
            (uint256 chain,,) = script.decodeRawTransaction(raw);
            _assertEq(chain, 0, "no chain id");
            VM.expectRevert(abi.encodeWithSelector(DeployPFTToken.TreasuryCreationWrongChain.selector, 0));
            script.checkCreation(BASE_SEPOLIA, legacy, legacySafe, raw);
        }
        // 2 x 84532 + 35 and + 36 are 84532; + 37 is chain 84533.
        bytes memory v37 = _alter("legacy", 70, abi.encodePacked(hex"8302948d", signature));
        VM.expectRevert(abi.encodeWithSelector(DeployPFTToken.TreasuryCreationWrongChain.selector, BASE_SEPOLIA + 1));
        script.checkCreation(BASE_SEPOLIA, legacy, legacySafe, v37);
        bytes memory v35 = _alter("legacy", 70, abi.encodePacked(hex"8302948b", signature));
        (uint256 chain35,,) = script.decodeRawTransaction(v35);
        _assertEq(chain35, BASE_SEPOLIA, "v 2 x chain + 35");
        // + 36 is the other parity, not chain 84532.5: accepted as 84532.
        bytes memory v36 = _alter("legacy", 70, abi.encodePacked(hex"8302948c", signature));
        (uint256 chain36,,) = script.decodeRawTransaction(v36);
        _assertEq(chain36, BASE_SEPOLIA, "v 2 x chain + 36");
        script.checkCreation(BASE_SEPOLIA, legacy, legacySafe, v36);
    }

    function testRefusesAMissingOrWrongCreationTransaction() public {
        DeployPFTToken.Config memory cfg = _config(BASE_SEPOLIA);
        cfg.creationTx = 0;
        VM.expectRevert(abi.encodeWithSelector(DeployPFTToken.TreasuryCreationUnknown.selector, bytes32(0)));
        broadcaster.deploy(script, cfg);
        cfg.allowEoaTreasury = true; // test mode: optional
        _checkToken(broadcaster.deploy(script, cfg), NAME, SUPPLY);
        cfg = _config(BASE_SEPOLIA);
        bytes32 wrong = keccak256(creationRaw) ^ bytes32(uint256(1));
        cfg.creationTx = wrong;
        VM.expectRevert(abi.encodeWithSelector(DeployPFTToken.TreasuryCreationUnknown.selector, wrong));
        broadcaster.deploy(script, cfg);
        // Without the raw bytes the script asks the RPC; forge test has none.
        cfg.creationRawTx = "";
        VM.expectRevert();
        broadcaster.deploy(script, cfg);
        VM.expectRevert();
        script.fetchRawTransaction(wrong);
    }

    function testTheCreationDecodersReadTheFixture() public view {
        (,, bytes memory data) = script.decodeRawTransaction(creationRaw);
        (address singleton, bytes memory init, uint256 saltNonce) = script.decodeCreateProxy(data);
        _assertTrue(singleton == SAFE_L2 && saltNonce == 0, "createProxyWithNonce arguments");
        _assertTrue(keccak256(init) == keccak256(_plainSetup(_threeOwners(), 2)), "initializer");
        DeployPFTToken.SafeSetup memory setup = script.decodeSetup(init);
        _assertTrue(setup.threshold == 2 && setup.owners.length == 3 && setup.to == address(0), "setup");
        bytes memory code = ISafeProxyFactoryCode(FACTORY_1_4_1).proxyCreationCode();
        _assertTrue(script.proxyAddress(FACTORY_1_4_1, code, singleton, init, saltNonce) == TREASURY, "CREATE2");
    }

    function testRefusesACreationNotFromACanonicalFactory() public {
        DeployPFTToken.SafeInfo memory safe = script.checkSafe(BASE_SEPOLIA, TREASURY);
        bytes memory call =
            abi.encodeCall(ISafeProxyFactory.createProxyWithNonce, (SAFE_L2, _plainSetup(_threeOwners(), 2), 0));
        VM.expectRevert(abi.encodeWithSelector(DeployPFTToken.TreasuryCreationNotFromFactory.selector, address(0xBAD)));
        script.checkCreation(BASE_SEPOLIA, TREASURY, safe, _rawTx(address(0xBAD), call));
        VM.expectRevert(abi.encodeWithSelector(DeployPFTToken.TreasuryCreationNotFromFactory.selector, FACTORY_1_4_1));
        script.checkCreation(1, TREASURY, safe, _rawTx(FACTORY_1_4_1, call));
        VM.etch(FACTORY_1_3_0, hex"00");
        VM.expectRevert(abi.encodeWithSelector(DeployPFTToken.TreasuryCreationNotFromFactory.selector, FACTORY_1_3_0));
        script.checkCreation(BASE_SEPOLIA, TREASURY, safe, _rawTx(FACTORY_1_3_0, call));
        VM.mockCallRevert(FACTORY_1_4_1, abi.encodeWithSignature("proxyCreationCode()"), "");
        VM.expectRevert(abi.encodeWithSelector(DeployPFTToken.TreasuryCreationNotFromFactory.selector, FACTORY_1_4_1));
        script.checkCreation(BASE_SEPOLIA, TREASURY, safe, _rawTx(FACTORY_1_4_1, call));
        VM.clearMockedCalls();
        _assertTrue(script.factoryCodeHash(BASE, FACTORY_1_3_0) == FACTORY_1_3_0_HASH, "1.3.0 factory hash");
    }

    function testRefusesACreationOfAnotherSafe() public {
        DeployPFTToken.SafeInfo memory safe = script.checkSafe(BASE_SEPOLIA, TREASURY);
        bytes memory init = _plainSetup(_threeOwners(), 2);
        bytes memory other = abi.encodeCall(ISafeProxyFactory.createProxyWithNonce, (SAFE_L2, init, 7));
        VM.expectRevert(abi.encodeWithSelector(DeployPFTToken.TreasuryCreationMismatch.selector, "proxy address"));
        script.checkCreation(BASE_SEPOLIA, TREASURY, safe, _rawTx(FACTORY_1_4_1, other));
        other = abi.encodeCall(ISafeProxyFactory.createProxyWithNonce, (SINGLETON_1_4_1, init, 0));
        VM.expectRevert(abi.encodeWithSelector(DeployPFTToken.TreasuryCreationMismatch.selector, "singleton"));
        script.checkCreation(BASE_SEPOLIA, TREASURY, safe, _rawTx(FACTORY_1_4_1, other));
        VM.expectRevert(
            abi.encodeWithSelector(DeployPFTToken.TreasuryCreationMismatch.selector, "createProxyWithNonce")
        );
        script.checkCreation(BASE_SEPOLIA, TREASURY, safe, _rawTx(FACTORY_1_4_1, hex"0102"));
    }

    function testRefusesAFallbackHandlerWithoutItsCanonicalCode() public {
        address handler = script.FALLBACK_HANDLER_1_4_1();
        bytes memory init = _setup(_threeOwners(), 2, address(0), "", handler);
        address safeAddress = ISafeProxyFactory(FACTORY_1_4_1).createProxyWithNonce(SAFE_L2, init, 9);
        bytes memory call = abi.encodeCall(ISafeProxyFactory.createProxyWithNonce, (SAFE_L2, init, 9));
        DeployPFTToken.SafeInfo memory safe = script.checkSafe(BASE_SEPOLIA, safeAddress);
        VM.expectRevert(
            abi.encodeWithSelector(DeployPFTToken.TreasuryCreationMismatch.selector, "fallback handler code")
        );
        script.checkCreation(BASE_SEPOLIA, safeAddress, safe, _rawTx(FACTORY_1_4_1, call));
        _assertTrue(script.fallbackHandlerCodeHash(address(1)) == 0, "unpinned handler");
        _assertTrue(
            script.fallbackHandlerCodeHash(script.FALLBACK_HANDLER_1_3_0()) == script.FALLBACK_HANDLER_1_3_0_CODEHASH(),
            "1.3.0 handler hash"
        );
    }

    function testCheckSetupAcceptsOnlyAPlainSetup() public {
        address[] memory owners = _threeOwners();
        address h141 = script.FALLBACK_HANDLER_1_4_1();
        address h130 = script.FALLBACK_HANDLER_1_3_0();
        _assertTrue(script.checkSetup(SAFE_L2, _plainSetup(owners, 2), owners, 2) == address(0), "clean");
        _assertTrue(script.checkSetup(SAFE_L2, _setup(owners, 2, address(0), "", h141), owners, 2) == h141, "1.4.1");
        _assertTrue(
            script.checkSetup(SINGLETON_1_3_0, _setup(owners, 2, address(0), "", h130), owners, 2) == h130, "1.3.0"
        );
        VM.expectRevert(abi.encodeWithSelector(DeployPFTToken.TreasuryInitializerDelegates.selector, address(0xD1), 0));
        script.checkSetup(SAFE_L2, _setup(owners, 2, address(0xD1), "", address(0)), owners, 2);
        VM.expectRevert(abi.encodeWithSelector(DeployPFTToken.TreasuryInitializerDelegates.selector, address(0), 1));
        script.checkSetup(SAFE_L2, _setup(owners, 2, address(0), hex"01", address(0)), owners, 2);
        bytes memory paid = abi.encodeCall(
            ISafeSetupCall.setup, (owners, 2, address(0), "", address(0), address(0), 1, payable(address(0xFEE)))
        );
        VM.expectRevert(abi.encodeWithSelector(DeployPFTToken.TreasurySetupHasPayment.selector, address(0), 1));
        script.checkSetup(SAFE_L2, paid, owners, 2);
        paid = abi.encodeCall(
            ISafeSetupCall.setup, (owners, 2, address(0), "", address(0), address(0x70C), 0, payable(address(0)))
        );
        VM.expectRevert(abi.encodeWithSelector(DeployPFTToken.TreasurySetupHasPayment.selector, address(0x70C), 0));
        script.checkSetup(SAFE_L2, paid, owners, 2);
        address[] memory others = _threeOwners();
        others[2] = address(0x0A4);
        VM.expectRevert(abi.encodeWithSelector(DeployPFTToken.TreasuryCreationMismatch.selector, "owners"));
        script.checkSetup(SAFE_L2, _plainSetup(others, 2), owners, 2);
        VM.expectRevert(abi.encodeWithSelector(DeployPFTToken.TreasuryCreationMismatch.selector, "threshold"));
        script.checkSetup(SAFE_L2, _plainSetup(owners, 3), owners, 2);
        VM.expectRevert(abi.encodeWithSelector(DeployPFTToken.TreasuryCreationMismatch.selector, "fallback handler"));
        script.checkSetup(SAFE_L2, _setup(owners, 2, address(0), "", h130), owners, 2);
        VM.expectRevert(abi.encodeWithSelector(DeployPFTToken.TreasuryCreationMismatch.selector, "setup"));
        script.checkSetup(SAFE_L2, hex"01", owners, 2);
    }

    function testTheRlpReaderRefusesWhatItCannotRead() public {
        bytes memory raw = _fixtureRaw("type2");
        bytes[] memory bad = new bytes[](8);
        bad[0] = "";
        bad[1] = abi.encodePacked(hex"01", _slice(raw, 1));
        bad[2] = _truncate(raw, raw.length - 1); // list longer than the bytes
        bad[3] = abi.encodePacked(raw, hex"00"); // trailing bytes
        bad[4] = hex"02bc"; // a length of five bytes
        bad[5] = hex"02b9"; // a length beyond the end
        bad[6] = abi.encodePacked(hex"02", _rlpHeader(6, 0xc0), hex"0102030405c0"); // `to` a list
        bad[7] = abi.encodePacked(hex"02", _rlpHeader(9, 0xc0), hex"01020304058201028080"); // short `to`
        for (uint256 i = 0; i < bad.length; i++) {
            VM.expectRevert(DeployPFTToken.TreasuryCreationUnreadable.selector);
            script.decodeRawTransaction(bad[i]);
        }
        bytes memory listData = abi.encodePacked(hex"0102030405", _rlp(abi.encodePacked(address(1))), hex"80c0");
        VM.expectRevert(DeployPFTToken.TreasuryCreationUnreadable.selector);
        script.decodeRawTransaction(abi.encodePacked(hex"02", _rlpHeader(listData.length, 0xc0), listData));
        bytes memory shortList = abi.encodePacked(hex"0102030405");
        VM.expectRevert(DeployPFTToken.TreasuryCreationUnreadable.selector);
        script.decodeRawTransaction(abi.encodePacked(hex"02", _rlpHeader(shortList.length, 0xc0), shortList));
    }

    function testRefusesAnIncompleteOrMistypedEnvelope() public {
        // The fixtures end with accessList c0, yParity 01, r, s (type 2) and v, r, s (legacy).
        bytes memory rs = _last(creationRaw, 66);
        bytes memory s = _last(creationRaw, 33);
        bytes memory r = _truncate(rs, 33);
        bytes[] memory bad = new bytes[](14);
        bad[0] = _alter("type2", 68, ""); // no accessList, yParity, r, s
        bad[1] = _alter("type2", 66, ""); // no r, s
        bad[2] = _alter("type2", 33, ""); // no s
        bad[3] = _alter("type2", 0, hex"80"); // a thirteenth item
        bad[4] = _alter("type2", 68, abi.encodePacked(hex"8001", rs)); // a string as the access list
        bad[5] = _alter("type2", 67, abi.encodePacked(hex"02", rs)); // yParity 2
        bad[6] = _alter("type2", 66, abi.encodePacked(hex"80", s)); // r zero
        bad[7] = _alter("type2", 33, hex"80"); // s zero
        bad[8] = _alter("type2", 66, abi.encodePacked(hex"a100", _slice(r, 1), s)); // r of 33 bytes
        bad[9] = _alter("type2", 67, abi.encodePacked(hex"c0", rs)); // yParity a list
        bad[10] = _alter("legacy", 70, ""); // no v, r, s
        bad[11] = _alter("legacy", 33, ""); // no s
        bad[12] = _alter("legacy", 0, hex"80"); // a tenth item
        bad[13] = _alter("legacy", 33, abi.encodePacked(hex"a100", _last(_fixtureRaw("legacy"), 32))); // s of 33 bytes
        for (uint256 i = 0; i < bad.length; i++) {
            VM.expectRevert(DeployPFTToken.TreasuryCreationUnreadable.selector);
            script.decodeRawTransaction(bad[i]);
        }
        // The unaltered envelopes, re-wrapped, still read.
        script.decodeRawTransaction(_alter("type2", 0, ""));
        script.decodeRawTransaction(_alter("legacy", 0, ""));
        // By design only the access list's type is read: a list of malformed entries is accepted.
        (uint256 chain, address to, bytes memory data) =
            script.decodeRawTransaction(_alter("type2", 68, abi.encodePacked(hex"c50102c28080", hex"01", rs)));
        (uint256 chain0, address to0, bytes memory data0) = script.decodeRawTransaction(creationRaw);
        _assertTrue(chain == chain0 && to == to0 && keccak256(data) == keccak256(data0), "malformed entries read");
    }

    /// The fixture `name` with the last `drop` bytes of its list payload replaced by `tail`.
    function _alter(string memory name, uint256 drop, bytes memory tail) private view returns (bytes memory) {
        bytes memory raw = _fixtureRaw(name);
        bool typed = uint8(raw[0]) == 2;
        uint256 from = typed ? 4 : 3; // [0x02] 0xf9 <two length bytes>
        bytes memory payload = abi.encodePacked(_truncate(_slice(raw, from), raw.length - from - drop), tail);
        return abi.encodePacked(typed ? bytes(hex"02") : bytes(""), _rlpHeader(payload.length, 0xc0), payload);
    }

    function _last(bytes memory b, uint256 n) private pure returns (bytes memory) {
        return _slice(b, b.length - n);
    }

    function _slice(bytes memory b, uint256 from) private pure returns (bytes memory out) {
        out = new bytes(b.length - from);
        for (uint256 i = 0; i < out.length; i++) {
            out[i] = b[from + i];
        }
    }

    function _truncate(bytes memory b, uint256 length) private pure returns (bytes memory out) {
        out = new bytes(length);
        for (uint256 i = 0; i < length; i++) {
            out[i] = b[i];
        }
    }

    // --------------------------------------------------------------- helpers

    function _fixtureRaw(string memory name) private view returns (bytes memory) {
        return VM.parseBytes(VM.readFile(string.concat("test/fixtures/safe/creation-safe-l2-1.4.1-", name, ".raw.hex")));
    }

    function _plainSetup(address[] memory owners, uint256 threshold) private pure returns (bytes memory) {
        return _setup(owners, threshold, address(0), "", address(0));
    }

    function _setup(address[] memory owners, uint256 threshold, address to, bytes memory data, address handler)
        private
        pure
        returns (bytes memory)
    {
        return abi.encodeCall(
            ISafeSetupCall.setup, (owners, threshold, to, data, handler, address(0), 0, payable(address(0)))
        );
    }

    /// A type-2 transaction on this chain (8453 or 84532) to `to` with `data`
    /// (its signature is not checked).
    function _rawTx(address to, bytes memory data) private view returns (bytes memory) {
        bytes memory chain = block.chainid == BASE ? bytes(hex"822105") : bytes(hex"83014a34");
        bytes memory items = abi.encodePacked(chain, hex"80010183", hex"0f4240", _rlp(abi.encodePacked(to)), hex"80");
        items = abi.encodePacked(items, _rlp(data), hex"c080", _rlp(abi.encode(1)), _rlp(abi.encode(2)));
        return abi.encodePacked(hex"02", _rlpHeader(items.length, 0xc0), items);
    }

    function _rlp(bytes memory b) private pure returns (bytes memory) {
        if (b.length == 1 && uint8(b[0]) < 0x80) return b;
        return abi.encodePacked(_rlpHeader(b.length, 0x80), b);
    }

    function _rlpHeader(uint256 length, uint256 offset) private pure returns (bytes memory) {
        if (length < 56) return abi.encodePacked(uint8(offset + length));
        if (length < 256) return abi.encodePacked(uint8(offset + 56), uint8(length));
        return abi.encodePacked(uint8(offset + 57), uint16(length));
    }

    /// A copy of the artifact under test, to be altered by the caller.
    function _artifactCopy(string memory name) private returns (string memory path) {
        string memory dir =
            string.concat("cache/deploy-test/artifact-", name, "-", VM.toString(abi.encode(VM.randomUint())));
        VM.createDir(dir, true);
        path = string.concat(dir, "/PFTToken.json");
        VM.writeFile(path, VM.readFile(ARTIFACT));
    }

    /// The creation code of out/'s build (under coverage, not this build).
    function _outCreationCode() private view returns (bytes memory) {
        return VM.parseJsonBytes(VM.readFile(ARTIFACT), ".bytecode.object");
    }

    /// `value` as the inside of a JSON string.
    function _escape(string memory value) private pure returns (string memory) {
        bytes memory raw = bytes(value);
        bytes memory out = new bytes(raw.length * 2);
        uint256 n = 0;
        for (uint256 i = 0; i < raw.length; i++) {
            if (raw[i] == '"' || raw[i] == "\\") out[n++] = "\\";
            out[n++] = raw[i];
        }
        assembly {
            mstore(out, n)
        }
        return string(out);
    }

    function _artifact() private view returns (string memory) {
        return string.concat(outDir, "/PFTToken.sol/PFTToken.json");
    }

    function _config(uint256 chainId) private view returns (DeployPFTToken.Config memory cfg) {
        cfg.chainId = chainId;
        cfg.name = NAME;
        cfg.symbol = SYMBOL;
        cfg.totalSupply = SUPPLY;
        cfg.totalSupplyPft = SUPPLY_PFT;
        cfg.treasury = TREASURY;
        cfg.artifact = _artifact();
        cfg.creationRawTx = chainId == BASE ? _fixtureRaw("type2-8453") : creationRaw;
        cfg.creationTx = keccak256(cfg.creationRawTx);
    }

    function _placeholder(string memory name) private pure returns (bytes memory) {
        return abi.encodeWithSelector(DeployPFTToken.PlaceholderValue.selector, name);
    }

    function _checkToken(PFTToken token, string memory name, uint256 supply) private view {
        _assertEq(token.totalSupply(), supply, "total supply");
        _assertEq(token.balanceOf(_treasuryOf(token)), supply, "treasury balance");
        _assertTrue(keccak256(bytes(token.name())) == keccak256(bytes(name)), "name");
        _assertTrue(keccak256(bytes(token.symbol())) == keccak256(bytes(SYMBOL)), "symbol");
        _assertEq(token.balanceOf(address(broadcaster)), 0, "deployer holds nothing");
    }

    /// The fixtures use either the Safe at TREASURY or the EOA 0xE0A.
    function _treasuryOf(PFTToken token) private view returns (address) {
        return token.balanceOf(TREASURY) > 0 ? TREASURY : address(0xE0A);
    }

    function _checkRecord(
        string memory dir,
        PFTToken token,
        uint256 chainId,
        string memory mode,
        address singleton,
        uint256 threshold,
        uint256 owners
    ) private view {
        string memory json = VM.readFile(string.concat(dir, "/deployment.json"));
        _assertTrue(_eq(VM.parseJsonString(json, ".mode"), mode), "record mode");
        _assertTrue(VM.parseJsonAddress(json, ".treasury_safe_singleton") == singleton, "record singleton");
        _assertEq(VM.parseJsonAddressArray(json, ".treasury_safe_owners").length, owners, "record owner list");
        _assertTrue(_eq(VM.parseJsonString(json, ".tx_hash"), "pending"), "record tx hash");
        _assertTrue(_eq(VM.parseJsonString(json, ".block_number"), "pending"), "record block number");
        _assertTrue(_eq(VM.parseJsonString(json, ".bytecode_hash"), "ipfs"), "record bytecode hash");
        _assertTrue(VM.parseJsonAddress(json, ".token") == address(token), "record token");
        _assertEq(VM.parseJsonUint(json, ".chain_id"), chainId, "record chain id");
        _assertTrue(_eq(VM.parseJsonString(json, ".name"), token.name()), "record name");
        _assertTrue(_eq(VM.parseJsonString(json, ".symbol"), SYMBOL), "record symbol");
        _assertEq(VM.parseJsonUint(json, ".total_supply_units"), SUPPLY, "record supply");
        _assertEq(VM.parseJsonUint(json, ".total_supply_pft"), SUPPLY_PFT, "record supply in PFT");
        _assertTrue(VM.parseJsonAddress(json, ".treasury") == _treasuryOf(token), "record treasury");
        _assertTrue(VM.parseJsonAddress(json, ".deployer") == address(broadcaster), "record deployer");
        _assertEq(VM.parseJsonUint(json, ".treasury_safe_threshold"), threshold, "record threshold");
        _assertEq(VM.parseJsonUint(json, ".treasury_safe_owner_count"), owners, "record owners");
        _assertEq(VM.parseJsonAddressArray(json, ".treasury_safe_modules").length, 0, "record modules");
        if (_treasuryOf(token) == TREASURY && VM.load(TREASURY, script.GUARD_STORAGE_SLOT()) == 0) {
            _assertTrue(VM.parseJsonAddress(json, ".treasury_safe_guard") == address(0), "record no guard");
        }
        _assertTrue(
            VM.parseJsonBytes32(json, ".creation_code_keccak256") == keccak256(type(PFTToken).creationCode),
            "record creation code"
        );
        _assertTrue(_eq(VM.parseJsonString(json, ".artifact"), _artifact()), "record artifact");
        string memory artifact = VM.readFile(_artifact());
        _assertTrue(_eq(VM.parseJsonString(json, ".solc_version"), "0.8.24+commit.e11b9ed9"), "record solc");
        _assertTrue(_eq(VM.parseJsonString(json, ".evm_version"), "cancun"), "record evm version");
        _assertEq(VM.parseJsonUint(json, ".optimizer_runs"), 200, "record optimizer runs");
        _assertTrue(VM.parseJsonBool(json, ".optimizer"), "record optimizer");
        _assertTrue(
            VM.parseJsonBytes32(json, ".runtime_code_keccak256")
                == keccak256(VM.parseJsonBytes(artifact, ".deployedBytecode.object")),
            "record runtime code"
        );
        _assertTrue(
            VM.parseJsonBytes32(json, ".deployed_code_keccak256") == keccak256(address(token).code),
            "record deployed code"
        );
        _assertTrue(
            _eq(VM.parseJsonString(json, ".openzeppelin_commit"), "cab19933c33c2ad1d4c7a84864a3601dddfd16f3"),
            "record OpenZeppelin commit"
        );
        bytes memory args = abi.encode(token.name(), SYMBOL, SUPPLY, _treasuryOf(token));
        _assertTrue(_eq(VM.parseJsonString(json, ".constructor_args"), VM.toString(args)), "record constructor args");
    }

    function _eq(string memory a, string memory b) private pure returns (bool) {
        return keccak256(bytes(a)) == keccak256(bytes(b));
    }

    function _assertTrue(bool value, string memory message) private pure {
        if (!value) {
            revert(message);
        }
    }

    function _assertEq(uint256 actual, uint256 expected, string memory message) private pure {
        if (actual != expected) {
            revert(message);
        }
    }
}
