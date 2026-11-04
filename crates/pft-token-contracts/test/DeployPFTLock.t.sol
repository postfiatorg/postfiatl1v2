// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.24;

import {PFTToken} from "../src/PFTToken.sol";
import {PFTLockV1, IPFTLockV1} from "../src/PFTLockV1.sol";
import {DeployPFTLock, IPFTLVerifierProbe} from "../script/DeployPFTLock.s.sol";
import {PFTLFinalityVerifierV1, ISP1Verifier} from "../../ethereum-contracts/src/PFTLFinalityVerifierV1.sol";

interface IDeployLockTestVm {
    function chainId(uint256 newChainId) external;
    function exists(string calldata path) external view returns (bool);
    function removeFile(string calldata path) external;
    function setNonce(address account, uint64 newNonce) external;
    function setNonceUnsafe(address account, uint64 newNonce) external;
    function getNonce(address account) external view returns (uint64);
    function computeCreateAddress(address deployer, uint256 nonce) external pure returns (address);
    function etch(address target, bytes calldata code) external;
    function setEnv(string calldata name, string calldata value) external;
    function expectRevert(bytes calldata revertData) external;
    function expectRevert(bytes4 revertData) external;
    function expectRevert() external;
    function readFile(string calldata path) external view returns (string memory data);
    function parseJsonUint(string calldata json, string calldata key) external pure returns (uint256);
    function parseJsonString(string calldata json, string calldata key) external pure returns (string memory);
    function parseJsonAddress(string calldata json, string calldata key) external pure returns (address);
    function parseJsonBool(string calldata json, string calldata key) external pure returns (bool);
    function parseJsonBytes(string calldata json, string calldata key) external pure returns (bytes memory);
    function parseJsonBytes32(string calldata json, string calldata key) external pure returns (bytes32);
    function toString(address value) external pure returns (string memory);
    function toString(bytes32 value) external pure returns (string memory);
    function toString(bytes calldata value) external pure returns (string memory);
    function mockCall(address callee, bytes calldata data, bytes calldata returnData) external;
    function prank(address msgSender) external;
    function isContext(uint8 context) external view returns (bool);
    function randomUint() external returns (uint256);
    function writeFile(string calldata path, string calldata data) external;
    function writeJson(string calldata json, string calldata path, string calldata valueKey) external;
    function createDir(string calldata path, bool recursive) external;
    function replace(string calldata input, string calldata from, string calldata to)
        external
        pure
        returns (string memory);
}

/// A stand-in for the PFTL finality verifier, which does not exist on Base yet.
contract MockLockVerifier {
    error NotDeployed();

    function verifyAndConsume(bytes calldata, bytes calldata)
        external
        pure
        returns (address, uint256, bytes32, bytes32, bytes32)
    {
        revert NotDeployed();
    }
}

/// A stand-in with the anchor, route and PFTL-side getters of
/// PFTLFinalityVerifierV1. Its chain id is the current one unless set. Once
/// `watch` has code (the lock deployed), the checkpoint reads as `swapped`
/// and, when `swappedVault` is set, the vault runtime code hash as that.
contract AnchoredLockVerifier {
    bytes32 public pftlChainIdHash;
    bytes32 public pftlGenesisHashCommitment;
    bytes32 public routeProfileHashCommitment;
    uint64 public routeEpoch;
    bytes32 public assetIdCommitment;
    bytes32 private vault;
    bytes32 private swappedVault;
    bytes32 private checkpoint;
    uint64 public latestFinalizedHeight;
    bytes32 public latestCommitteeRootCommitment;
    address public token;
    bytes32 public tokenRuntimeCodeHash;
    uint64 private chainId;
    address private watch;
    bytes32 private swapped;

    constructor(address token_) {
        token = token_;
        tokenRuntimeCodeHash = token_.codehash;
    }

    function setAnchor(bytes32 checkpoint_, uint64 height, bytes32 committeeRoot) external {
        checkpoint = checkpoint_;
        latestFinalizedHeight = height;
        latestCommitteeRootCommitment = committeeRoot;
    }

    function setPftl(bytes32 chainIdHash, bytes32 genesis, bytes32 routeProfile, uint64 epoch, bytes32 asset) external {
        pftlChainIdHash = chainIdHash;
        pftlGenesisHashCommitment = genesis;
        routeProfileHashCommitment = routeProfile;
        routeEpoch = epoch;
        assetIdCommitment = asset;
    }

    function setVault(bytes32 vault_) external {
        vault = vault_;
    }

    function swapVaultOnceDeployed(address watch_, bytes32 swapped_) external {
        watch = watch_;
        swappedVault = swapped_;
    }

    function vaultRuntimeCodeHash() external view returns (bytes32) {
        return swappedVault != 0 && watch.code.length != 0 ? swappedVault : vault;
    }

    function setRoute(address token_, bytes32 tokenHash, uint64 chainId_) external {
        token = token_;
        tokenRuntimeCodeHash = tokenHash;
        chainId = chainId_;
    }

    function arbitrumChainId() external view returns (uint64) {
        return chainId == 0 ? uint64(block.chainid) : chainId;
    }

    function swapOnceDeployed(address watch_, bytes32 swapped_) external {
        watch = watch_;
        swapped = swapped_;
    }

    function latestCheckpointCommitment() external view returns (bytes32) {
        return swapped != 0 && watch.code.length != 0 ? swapped : checkpoint;
    }

    function verifyAndConsume(bytes calldata, bytes calldata)
        external
        pure
        returns (address, uint256, bytes32, bytes32, bytes32)
    {
        revert MockLockVerifier.NotDeployed();
    }
}

/// Answers exactly the getters it is given, each with one word.
contract ScriptedVerifier {
    mapping(bytes4 => bytes32) private answers;

    function answer(bytes4 selector, bytes32 word) external {
        answers[selector] = word;
    }

    fallback(bytes calldata data) external returns (bytes memory) {
        bytes32 word = answers[bytes4(data)];
        if (word == 0) revert();
        return abi.encode(word);
    }
}

/// An ERC-20 lookalike with the decimals and supply it is built with.
contract MockLockToken {
    uint8 public immutable decimals;
    uint256 public immutable totalSupply;

    constructor(uint8 decimals_, uint256 totalSupply_) {
        decimals = decimals_;
        totalSupply = totalSupply_;
    }
}

/// Answers `decimals()` only.
contract DecimalsOnlyToken {
    function decimals() external pure returns (uint8) {
        return 6;
    }
}

/// Answers neither token read.
contract NotAToken {
    function ping() external pure returns (uint256) {
        return 1;
    }
}

/// The broadcaster: a fresh account of its own, as in the token script's tests.
/// Creates the real verifier from its own nonce, so the test predicts its
/// address without depending on how forge counts the test contract's nonce.
contract VerifierDeployer {
    function deploy(PFTLFinalityVerifierV1.Config memory c) external returns (PFTLFinalityVerifierV1) {
        return new PFTLFinalityVerifierV1(c);
    }
}

contract LockBroadcaster {
    function deploy(DeployPFTLock script, DeployPFTLock.Config memory cfg) external returns (PFTLockV1) {
        return script.deploy(cfg);
    }

    function run(DeployPFTLock script) external returns (PFTLockV1) {
        return script.run();
    }
}

contract DeployPFTLockTest {
    IDeployLockTestVm private constant VM = IDeployLockTestVm(address(uint160(uint256(keccak256("hevm cheat code")))));

    uint256 private constant BASE = 8453;
    uint256 private constant BASE_SEPOLIA = 84532;
    uint256 private constant SUPPLY = 100_000_000_000 * 1_000_000;
    uint256 private constant CAP_PFT = 2_000_000_000;
    uint256 private constant CAP = CAP_PFT * 1_000_000;
    /// The PFTL account reserve; the env example pins the same value.
    uint256 private constant MINIMUM = 10;
    address private constant TREASURY = address(0x7EA5);
    string private constant COMMIT = "0123456789abcdef0123456789abcdef01234567";
    string private constant ARTIFACT = "out/PFTLockV1.sol/PFTLockV1.json";
    /// `ForgeContext.Coverage` in forge-std's Vm.
    uint8 private constant COVERAGE = 2;
    /// The one-time deployer of the Base Sepolia test token (nonce 0 then), from its record.
    address private constant TEST_TOKEN_DEPLOYER = 0xDEe78dB2bC6f5EFCcAB7A046997E5c413C26316D;
    address private constant TEST_TOKEN_TREASURY = 0x7424F4F4A620d37CFB54A4558a1Ec7f60b71A0CB;
    bytes32 private constant CHECKPOINT = keccak256("finalized checkpoint");
    uint64 private constant HEIGHT = 4_200;
    bytes32 private constant COMMITTEE_ROOT = keccak256("committee root");
    bytes32 private constant PFTL_CHAIN_ID_HASH = keccak256("postfiat-testnet");
    bytes32 private constant GENESIS = keccak256("the 48-byte genesis hash");
    bytes32 private constant ROUTE_PROFILE = keccak256("route profile");
    uint64 private constant ROUTE_EPOCH = 7;
    bytes32 private constant ASSET_ID = keccak256("asset id");

    DeployPFTLock private script;
    LockBroadcaster private broadcaster;
    PFTToken private token;
    AnchoredLockVerifier private verifier;
    /// The output directory whose PFTLockV1 artifact is this build's lock contract.
    string private outDir = "out";

    function setUp() public {
        VM.chainId(BASE_SEPOLIA);
        script = new DeployPFTLock();
        broadcaster = new LockBroadcaster();
        token = new PFTToken("Post Fiat Token", "PFT", SUPPLY, TREASURY);
        verifier = new AnchoredLockVerifier(address(token));
        verifier.setAnchor(CHECKPOINT, HEIGHT, COMMITTEE_ROOT);
        verifier.setPftl(PFTL_CHAIN_ID_HASH, GENESIS, ROUTE_PROFILE, ROUTE_EPOCH, ASSET_ID);
        verifier.setVault(_lockHash(address(token), CAP));
        VM.setNonceUnsafe(address(broadcaster), 0);
        if (VM.isContext(COVERAGE)) outDir = _coverageOutDir();
    }

    /// `forge coverage` builds without the optimizer and writes no artifact: a
    /// copy of out/'s artifact carrying this build's creation code stands in.
    function _coverageOutDir() private returns (string memory dir) {
        dir = string.concat("cache/deploy-test/lock-coverage-out-", VM.toString(abi.encode(VM.randomUint())));
        string memory path = string.concat(dir, "/PFTLockV1.sol/PFTLockV1.json");
        VM.createDir(string.concat(dir, "/PFTLockV1.sol"), true);
        VM.writeFile(path, VM.readFile(ARTIFACT));
        VM.writeJson(string.concat('"', VM.toString(type(PFTLockV1).creationCode), '"'), path, ".bytecode.object");
        bytes memory runtime =
            address(
            new PFTLockV1(address(token), address(verifier), address(token).codehash, uint64(CAP), uint64(MINIMUM))
        )
        .code;
        VM.writeJson(string.concat('"', VM.toString(runtime), '"'), path, ".deployedBytecode.object");
    }

    // ---------------------------------------------------------------- accepted

    function testDeploysOnBaseSepoliaWithATestVerifierAndWritesTheRecord() public {
        DeployPFTLock.Config memory cfg = _config(BASE_SEPOLIA);
        cfg.allowTestVerifier = true;
        cfg.verifierCodeHash = 0; // no PFTL finality verifier is pinned on Base yet
        cfg.recordDir = "cache/deploy-test/lock-base-sepolia";
        cfg.writeRecord = true;
        PFTLockV1 lock = broadcaster.deploy(script, cfg);
        _checkLock(lock);
        _checkRecord(cfg.recordDir, lock, BASE_SEPOLIA, "test", false);
    }

    function testDeploysOnBaseMainnetWithAPinnedVerifier() public {
        VM.chainId(BASE);
        DeployPFTLock.Config memory cfg = _config(BASE);
        cfg.recordDir = "cache/deploy-test/lock-base";
        cfg.writeRecord = true;
        PFTLockV1 lock = broadcaster.deploy(script, cfg);
        _checkLock(lock);
        _checkRecord(cfg.recordDir, lock, BASE, "test", true);
    }

    function testTheTestSwitchAdmitsAnUnpinnedVerifierOnBaseSepolia() public {
        DeployPFTLock.Config memory cfg = _config(BASE_SEPOLIA);
        cfg.allowTestVerifier = true;
        cfg.verifierCodeHash = keccak256("another verifier");
        _checkLock(broadcaster.deploy(script, cfg));
    }

    function testDryRunAgainstTheRecordedBaseSepoliaTestToken() public {
        // The recorded deployer at nonce 0 on 84532 creates the token at its
        // recorded address, with its recorded runtime code (this build's
        // optimizer settings; `forge coverage` builds without them).
        VM.prank(TEST_TOKEN_DEPLOYER);
        PFTToken testToken = new PFTToken("Post Fiat Token", "PFT", SUPPLY, TEST_TOKEN_TREASURY);
        _assertTrue(address(testToken) == script.BASE_SEPOLIA_TEST_TOKEN(), "test token address");
        if (!VM.isContext(COVERAGE)) {
            _assertTrue(
                address(testToken).codehash == script.BASE_SEPOLIA_TEST_TOKEN_CODEHASH(), "test token code hash"
            );
        }
        DeployPFTLock.Config memory cfg = _config(BASE_SEPOLIA);
        cfg.token = address(testToken);
        cfg.tokenCodeHash = address(testToken).codehash;
        verifier.setRoute(address(testToken), address(testToken).codehash, 0);
        verifier.setVault(_lockHash(address(testToken), CAP));
        cfg.allowTestVerifier = true;
        // The recorded token needs no switch; `forge coverage`'s build has another code hash.
        cfg.allowTestToken = VM.isContext(COVERAGE);
        PFTLockV1 lock = broadcaster.deploy(script, cfg);
        _assertTrue(lock.token() == script.BASE_SEPOLIA_TEST_TOKEN(), "lock token");
    }

    function testRunReadsTheEnvironment() public {
        // The only test of this contract that touches the process environment.
        VM.setEnv("PFT_LOCK_CHAIN_ID", "84532");
        VM.setEnv("PFT_LOCK_TOKEN", VM.toString(address(token)));
        VM.setEnv("PFT_LOCK_TOKEN_CODEHASH", VM.toString(address(token).codehash));
        VM.setEnv("PFT_LOCK_VERIFIER", VM.toString(address(verifier)));
        VM.setEnv("PFT_LOCK_VERIFIER_CODEHASH", VM.toString(address(verifier).codehash));
        VM.setEnv("PFT_LOCK_VERIFIER_EXPECTED_CHECKPOINT_COMMITMENT", VM.toString(CHECKPOINT));
        VM.setEnv("PFT_LOCK_VERIFIER_EXPECTED_FINALIZED_HEIGHT", "4200");
        VM.setEnv("PFT_LOCK_VERIFIER_EXPECTED_COMMITTEE_ROOT_COMMITMENT", VM.toString(COMMITTEE_ROOT));
        VM.setEnv("PFT_LOCK_VERIFIER_EXPECTED_PFTL_CHAIN_ID_HASH", VM.toString(PFTL_CHAIN_ID_HASH));
        VM.setEnv("PFT_LOCK_VERIFIER_EXPECTED_GENESIS_HASH_COMMITMENT", VM.toString(GENESIS));
        VM.setEnv("PFT_LOCK_VERIFIER_EXPECTED_ROUTE_PROFILE_HASH_COMMITMENT", VM.toString(ROUTE_PROFILE));
        VM.setEnv("PFT_LOCK_VERIFIER_EXPECTED_ROUTE_EPOCH", "7");
        VM.setEnv("PFT_LOCK_VERIFIER_EXPECTED_ASSET_ID_COMMITMENT", VM.toString(ASSET_ID));
        VM.setEnv("PFT_LOCK_MAX_LOCK_AMOUNT", "2000000000000000");
        VM.setEnv("PFT_LOCK_MAX_LOCK_AMOUNT_PFT", "2000000000");
        VM.setEnv("PFT_LOCK_MINIMUM_LOCK_AMOUNT", "10");
        VM.setEnv("PFT_LOCK_ALLOW_TEST_VERIFIER", "");
        VM.setEnv("PFT_LOCK_ALLOW_USED_DEPLOYER", "0");
        VM.setEnv("PFT_LOCK_ALLOW_TEST_TOKEN", "");
        VM.setEnv("PFT_LOCK_REAL_DEPLOYMENT", "");
        VM.setEnv("PFT_LOCK_SOURCE_COMMIT", COMMIT);

        DeployPFTLock.Config memory cfg = script.readConfig();
        _assertEq(cfg.chainId, BASE_SEPOLIA, "chain id");
        _assertTrue(cfg.token == address(token) && cfg.tokenCodeHash == address(token).codehash, "token");
        _assertTrue(cfg.verifier == address(verifier), "verifier");
        _assertTrue(cfg.verifierCodeHash == address(verifier).codehash, "verifier code hash");
        _assertTrue(cfg.expectedCheckpointCommitment == CHECKPOINT, "expected checkpoint");
        _assertEq(cfg.expectedFinalizedHeight, HEIGHT, "expected height");
        _assertTrue(cfg.expectedCommitteeRootCommitment == COMMITTEE_ROOT, "expected committee root");
        _assertTrue(cfg.expectedPftlChainIdHash == PFTL_CHAIN_ID_HASH, "expected PFTL chain id hash");
        _assertTrue(cfg.expectedGenesisHashCommitment == GENESIS, "expected genesis");
        _assertTrue(cfg.expectedRouteProfileHashCommitment == ROUTE_PROFILE, "expected route profile");
        _assertEq(cfg.expectedRouteEpoch, ROUTE_EPOCH, "expected route epoch");
        _assertTrue(cfg.expectedAssetIdCommitment == ASSET_ID, "expected asset id");
        _assertEq(cfg.maxLockAmount, CAP, "cap");
        _assertEq(cfg.maxLockAmountPft, CAP_PFT, "cap in PFT");
        _assertEq(cfg.minimumLockAmount, MINIMUM, "minimum");
        _assertTrue(
            !cfg.allowTestVerifier && !cfg.allowUsedDeployer && !cfg.allowTestToken && !cfg.realDeployment,
            "switches off"
        );
        _assertTrue(!cfg.writeRecord, "no record outside a broadcast");
        _assertTrue(_eq(cfg.recordDir, "cache/deploy-test/pft-lock-base-84532"), "test record path");
        _assertTrue(_eq(cfg.sourceCommit, COMMIT), "source commit");
        // FOUNDRY_OUT is shared with the token script's tests, which run in parallel.
        _assertTrue(_endsWith(cfg.artifact, "/PFTLockV1.sol/PFTLockV1.json"), "lock artifact");

        VM.setEnv("PFT_LOCK_REAL_DEPLOYMENT", "true");
        _assertTrue(!script.readConfig().realDeployment, "only exactly 1 is on");
        VM.setEnv("PFT_LOCK_REAL_DEPLOYMENT", "1");
        cfg = script.readConfig();
        _assertTrue(cfg.realDeployment, "real deployment");
        _assertTrue(_eq(cfg.recordDir, "deployments/pft-lock-base-84532"), "durable record path");
        VM.setEnv("PFT_LOCK_SOURCE_COMMIT", "");
        VM.expectRevert(DeployPFTLock.SourceCommitMissing.selector);
        broadcaster.run(script);
        VM.setEnv("PFT_LOCK_SOURCE_COMMIT", "not-a-commit");
        VM.expectRevert(abi.encodeWithSelector(DeployPFTLock.SourceCommitInvalid.selector, "not-a-commit"));
        broadcaster.run(script);
        VM.setEnv("PFT_LOCK_SOURCE_COMMIT", COMMIT);
        VM.setEnv("PFT_LOCK_ALLOW_TEST_TOKEN", "1");
        _assertTrue(script.readConfig().allowTestToken, "test token switch on");
        VM.expectRevert(
            abi.encodeWithSelector(DeployPFTLock.RealDeploymentWithTestSwitch.selector, "PFT_LOCK_ALLOW_TEST_TOKEN")
        );
        broadcaster.run(script);
        VM.setEnv("PFT_LOCK_ALLOW_TEST_TOKEN", "");

        // The verifier switch makes the pinned hash optional; it is still read when set.
        VM.setEnv("PFT_LOCK_ALLOW_TEST_VERIFIER", "1");
        VM.setEnv("PFT_LOCK_ALLOW_USED_DEPLOYER", "1");
        cfg = script.readConfig();
        _assertTrue(cfg.allowTestVerifier && cfg.allowUsedDeployer, "switches on");
        _assertTrue(cfg.verifierCodeHash == address(verifier).codehash, "pinned hash still read");
        VM.setEnv("PFT_LOCK_VERIFIER_CODEHASH", "0xreplace_with_the_verifier_runtime_codehash");
        VM.setEnv("PFT_LOCK_VERIFIER_EXPECTED_FINALIZED_HEIGHT", "replace_with_the_height");
        cfg = script.readConfig();
        _assertTrue(cfg.verifierCodeHash == 0, "placeholder hash unset under the switch");
        _assertEq(cfg.expectedFinalizedHeight, 0, "placeholder height unset under the switch");
        _assertTrue(cfg.expectedCheckpointCommitment == CHECKPOINT, "expected checkpoint still read");
        VM.setEnv("PFT_LOCK_VERIFIER_EXPECTED_FINALIZED_HEIGHT", "4200");
        VM.expectRevert(
            abi.encodeWithSelector(DeployPFTLock.RealDeploymentWithTestSwitch.selector, "PFT_LOCK_ALLOW_TEST_VERIFIER")
        );
        broadcaster.run(script);
        VM.setEnv("PFT_LOCK_ALLOW_TEST_VERIFIER", "");
        VM.setEnv("PFT_LOCK_ALLOW_USED_DEPLOYER", "");
        VM.setEnv("PFT_LOCK_REAL_DEPLOYMENT", "");
        VM.expectRevert(_placeholder("PFT_LOCK_VERIFIER_CODEHASH"));
        broadcaster.run(script);
        VM.setEnv("PFT_LOCK_VERIFIER_CODEHASH", VM.toString(address(verifier).codehash));

        string[15] memory names = [
            "PFT_LOCK_CHAIN_ID",
            "PFT_LOCK_TOKEN",
            "PFT_LOCK_TOKEN_CODEHASH",
            "PFT_LOCK_VERIFIER",
            "PFT_LOCK_VERIFIER_EXPECTED_CHECKPOINT_COMMITMENT",
            "PFT_LOCK_VERIFIER_EXPECTED_FINALIZED_HEIGHT",
            "PFT_LOCK_VERIFIER_EXPECTED_COMMITTEE_ROOT_COMMITMENT",
            "PFT_LOCK_VERIFIER_EXPECTED_PFTL_CHAIN_ID_HASH",
            "PFT_LOCK_VERIFIER_EXPECTED_GENESIS_HASH_COMMITMENT",
            "PFT_LOCK_VERIFIER_EXPECTED_ROUTE_PROFILE_HASH_COMMITMENT",
            "PFT_LOCK_VERIFIER_EXPECTED_ROUTE_EPOCH",
            "PFT_LOCK_VERIFIER_EXPECTED_ASSET_ID_COMMITMENT",
            "PFT_LOCK_MAX_LOCK_AMOUNT",
            "PFT_LOCK_MAX_LOCK_AMOUNT_PFT",
            "PFT_LOCK_MINIMUM_LOCK_AMOUNT"
        ];
        for (uint256 i = 0; i < names.length; i++) {
            string memory value = _envValue(names[i]);
            VM.setEnv(names[i], "REPLACE_me");
            VM.expectRevert(_placeholder(names[i]));
            broadcaster.run(script);
            VM.setEnv(names[i], " ");
            VM.expectRevert(_placeholder(names[i]));
            broadcaster.run(script);
            VM.setEnv(names[i], value);
        }
    }

    // -------------------------------------------------------------- chain id

    function testRefusesAChainOtherThanBase() public {
        VM.chainId(1);
        VM.expectRevert(abi.encodeWithSelector(DeployPFTLock.UnsupportedChain.selector, 1));
        broadcaster.deploy(script, _config(1));
    }

    function testRefusesAChainIdMismatch() public {
        VM.expectRevert(abi.encodeWithSelector(DeployPFTLock.WrongChain.selector, BASE, BASE_SEPOLIA));
        broadcaster.deploy(script, _config(BASE));
        VM.chainId(BASE);
        VM.expectRevert(abi.encodeWithSelector(DeployPFTLock.WrongChain.selector, BASE_SEPOLIA, BASE));
        broadcaster.deploy(script, _config(BASE_SEPOLIA));
    }

    function testRefusesTheTestSwitchesOnBaseMainnet() public {
        VM.chainId(BASE);
        DeployPFTLock.Config memory cfg = _config(BASE);
        cfg.allowTestVerifier = true;
        VM.expectRevert(
            abi.encodeWithSelector(DeployPFTLock.TestSwitchOnMainnet.selector, "PFT_LOCK_ALLOW_TEST_VERIFIER")
        );
        broadcaster.deploy(script, cfg);
        cfg = _config(BASE);
        cfg.allowUsedDeployer = true;
        VM.expectRevert(
            abi.encodeWithSelector(DeployPFTLock.TestSwitchOnMainnet.selector, "PFT_LOCK_ALLOW_USED_DEPLOYER")
        );
        broadcaster.deploy(script, cfg);
        cfg = _config(BASE);
        cfg.allowTestToken = true;
        VM.expectRevert(abi.encodeWithSelector(DeployPFTLock.TestSwitchOnMainnet.selector, "PFT_LOCK_ALLOW_TEST_TOKEN"));
        broadcaster.deploy(script, cfg);
    }

    // ------------------------------------------------------------ test token

    function testRequiresTheRecordedTestTokenOnBaseSepoliaWithoutTheSwitch() public {
        DeployPFTLock.Config memory cfg = _config(BASE_SEPOLIA);
        cfg.allowTestToken = false;
        VM.expectRevert(abi.encodeWithSelector(DeployPFTLock.TokenNotTheTestToken.selector, address(token)));
        broadcaster.deploy(script, cfg);
        // The recorded address with another pinned code hash is not the test token either.
        VM.prank(TEST_TOKEN_DEPLOYER);
        PFTToken testToken = new PFTToken("Post Fiat Token", "PFT", SUPPLY, TEST_TOKEN_TREASURY);
        cfg.token = address(testToken);
        cfg.tokenCodeHash = keccak256("another token");
        VM.expectRevert(abi.encodeWithSelector(DeployPFTLock.TokenNotTheTestToken.selector, address(testToken)));
        broadcaster.deploy(script, cfg);
        // The switch admits the other token.
        cfg = _config(BASE_SEPOLIA);
        cfg.allowTestToken = true;
        _checkLock(broadcaster.deploy(script, cfg));
    }

    function testRefusesTheTestTokenSwitchForARealDeployment() public {
        DeployPFTLock.Config memory cfg = _config(BASE_SEPOLIA);
        cfg.realDeployment = true;
        cfg.sourceCommit = COMMIT;
        cfg.allowTestToken = true;
        VM.expectRevert(
            abi.encodeWithSelector(DeployPFTLock.RealDeploymentWithTestSwitch.selector, "PFT_LOCK_ALLOW_TEST_TOKEN")
        );
        broadcaster.deploy(script, cfg);
    }

    // ----------------------------------------------------------------- token

    function testRefusesZeroValues() public {
        DeployPFTLock.Config memory cfg = _config(BASE_SEPOLIA);
        cfg.token = address(0);
        VM.expectRevert(_placeholder("PFT_LOCK_TOKEN"));
        broadcaster.deploy(script, cfg);
        cfg = _config(BASE_SEPOLIA);
        cfg.tokenCodeHash = 0;
        VM.expectRevert(_placeholder("PFT_LOCK_TOKEN_CODEHASH"));
        broadcaster.deploy(script, cfg);
        cfg = _config(BASE_SEPOLIA);
        cfg.verifier = address(0);
        VM.expectRevert(_placeholder("PFT_LOCK_VERIFIER"));
        broadcaster.deploy(script, cfg);
        cfg = _config(BASE_SEPOLIA);
        cfg.maxLockAmount = 0;
        VM.expectRevert(_placeholder("PFT_LOCK_MAX_LOCK_AMOUNT"));
        broadcaster.deploy(script, cfg);
        cfg = _config(BASE_SEPOLIA);
        cfg.minimumLockAmount = 0;
        VM.expectRevert(_placeholder("PFT_LOCK_MINIMUM_LOCK_AMOUNT"));
        broadcaster.deploy(script, cfg);
    }

    /// The minimum is the PFTL account reserve, not an operator choice: any other
    /// value is refused with the test switches and without them, so a Base
    /// Sepolia rehearsal deploys what the real deployment will.
    function testRefusesAMinimumOtherThanTheAccountReserve() public {
        _assertEq(script.PFTL_ACCOUNT_RESERVE_UNITS(), MINIMUM, "script constant");
        uint256[4] memory values = [uint256(1), 9, 11, CAP + 1];
        for (uint256 i = 0; i < values.length; i++) {
            DeployPFTLock.Config memory cfg = _config(BASE_SEPOLIA);
            cfg.allowTestVerifier = true;
            cfg.allowUsedDeployer = true;
            cfg.minimumLockAmount = values[i];
            VM.expectRevert(_notTheReserve(values[i]));
            broadcaster.deploy(script, cfg);
            VM.chainId(BASE);
            cfg = _config(BASE);
            cfg.realDeployment = true;
            cfg.sourceCommit = COMMIT;
            cfg.minimumLockAmount = values[i];
            VM.expectRevert(_notTheReserve(values[i]));
            broadcaster.deploy(script, cfg);
            VM.chainId(BASE_SEPOLIA);
        }
    }

    function _notTheReserve(uint256 value) private pure returns (bytes memory) {
        return abi.encodeWithSelector(DeployPFTLock.MinimumLockAmountNotTheAccountReserve.selector, value, MINIMUM);
    }

    function testRefusesATokenWithoutCode() public {
        DeployPFTLock.Config memory cfg = _config(BASE_SEPOLIA);
        cfg.token = address(0x70CE);
        VM.expectRevert(abi.encodeWithSelector(DeployPFTLock.TokenNotAContract.selector, address(0x70CE)));
        broadcaster.deploy(script, cfg);
    }

    function testRefusesATokenCodeHashMismatch() public {
        DeployPFTLock.Config memory cfg = _config(BASE_SEPOLIA);
        cfg.tokenCodeHash = keccak256("another token");
        VM.expectRevert(
            abi.encodeWithSelector(
                DeployPFTLock.TokenCodeHashMismatch.selector, keccak256("another token"), address(token).codehash
            )
        );
        broadcaster.deploy(script, cfg);
        // Another token whose hash is pinned instead is refused the same way.
        PFTToken other = new PFTToken("Other Token", "OTH", SUPPLY, TREASURY);
        cfg = _config(BASE_SEPOLIA);
        cfg.token = address(other);
        VM.expectRevert(
            abi.encodeWithSelector(
                DeployPFTLock.TokenCodeHashMismatch.selector, address(token).codehash, address(other).codehash
            )
        );
        broadcaster.deploy(script, cfg);
    }

    function testRefusesATokenWithOtherDecimals() public {
        MockLockToken eighteen = new MockLockToken(18, SUPPLY);
        DeployPFTLock.Config memory cfg = _withToken(address(eighteen));
        VM.expectRevert(abi.encodeWithSelector(DeployPFTLock.TokenDecimalsMismatch.selector, 18));
        broadcaster.deploy(script, cfg);
    }

    function testRefusesATokenWithoutSupply() public {
        MockLockToken empty = new MockLockToken(6, 0);
        VM.expectRevert(DeployPFTLock.TokenSupplyZero.selector);
        broadcaster.deploy(script, _withToken(address(empty)));
    }

    function testRefusesATokenThatDoesNotAnswer() public {
        address notAToken = address(new NotAToken());
        VM.expectRevert(abi.encodeWithSelector(DeployPFTLock.TokenUnreadable.selector, "decimals"));
        broadcaster.deploy(script, _withToken(notAToken));
        address decimalsOnly = address(new DecimalsOnlyToken());
        VM.expectRevert(abi.encodeWithSelector(DeployPFTLock.TokenUnreadable.selector, "totalSupply"));
        broadcaster.deploy(script, _withToken(decimalsOnly));
    }

    // -------------------------------------------------------------- verifier

    function testRefusesAVerifierWithoutCode() public {
        DeployPFTLock.Config memory cfg = _config(BASE_SEPOLIA);
        cfg.verifier = address(0x7E21);
        cfg.allowTestVerifier = true; // the switch does not admit an account without code
        VM.expectRevert(abi.encodeWithSelector(DeployPFTLock.VerifierNotAContract.selector, address(0x7E21)));
        broadcaster.deploy(script, cfg);
    }

    function testRefusesAVerifierCodeHashMismatchWithoutTheSwitch() public {
        DeployPFTLock.Config memory cfg = _config(BASE_SEPOLIA);
        cfg.verifierCodeHash = keccak256("the real verifier");
        VM.expectRevert(
            abi.encodeWithSelector(
                DeployPFTLock.VerifierCodeHashMismatch.selector,
                keccak256("the real verifier"),
                address(verifier).codehash
            )
        );
        broadcaster.deploy(script, cfg);
        VM.chainId(BASE);
        cfg = _config(BASE);
        cfg.verifierCodeHash = 0;
        VM.expectRevert(
            abi.encodeWithSelector(DeployPFTLock.VerifierCodeHashMismatch.selector, 0, address(verifier).codehash)
        );
        broadcaster.deploy(script, cfg);
    }

    // ------------------------------------------------------------------- cap

    // ------------------------------------------------------- verifier anchor

    function testRefusesAVerifierWithAnotherCheckpointCommitment() public {
        verifier.setAnchor(keccak256("another chain's checkpoint"), HEIGHT, COMMITTEE_ROOT);
        _expectAnchor("checkpoint_commitment", CHECKPOINT, keccak256("another chain's checkpoint"));
        broadcaster.deploy(script, _config(BASE_SEPOLIA));
    }

    function testRefusesAVerifierWithAnotherFinalizedHeight() public {
        verifier.setAnchor(CHECKPOINT, HEIGHT - 1, COMMITTEE_ROOT);
        _expectAnchor("finalized_height", bytes32(uint256(HEIGHT)), bytes32(uint256(HEIGHT - 1)));
        broadcaster.deploy(script, _config(BASE_SEPOLIA));
    }

    function testRefusesAVerifierWithAnotherCommitteeRoot() public {
        verifier.setAnchor(CHECKPOINT, HEIGHT, keccak256("chosen committee"));
        _expectAnchor("committee_root_commitment", COMMITTEE_ROOT, keccak256("chosen committee"));
        broadcaster.deploy(script, _config(BASE_SEPOLIA));
    }

    function testRefusesAVerifierWhoseAnchorChangesAfterThePreCheck() public {
        // The lock is the broadcaster's first creation; once it has code the
        // verifier's anchor reads differently: only the post-deployment check sees it.
        address lock = _createAddress(address(broadcaster), 0);
        verifier.swapOnceDeployed(lock, keccak256("swapped"));
        _expectAnchor("checkpoint_commitment", CHECKPOINT, keccak256("swapped"));
        broadcaster.deploy(script, _config(BASE_SEPOLIA));
        VM.chainId(BASE);
        verifier.setRoute(address(token), address(token).codehash, uint64(BASE));
        _expectAnchor("checkpoint_commitment", CHECKPOINT, keccak256("swapped"));
        broadcaster.deploy(script, _config(BASE));
    }

    function testDeploysWhenTheVerifierAnchorMatches() public {
        PFTLockV1 lock = broadcaster.deploy(script, _config(BASE_SEPOLIA));
        _checkLock(lock);
        script.checkLock(_config(BASE_SEPOLIA), lock);
    }

    function testRefusesAnUnsetExpectedAnchor() public {
        DeployPFTLock.Config memory cfg = _config(BASE_SEPOLIA);
        cfg.expectedCheckpointCommitment = 0;
        VM.expectRevert(_placeholder("PFT_LOCK_VERIFIER_EXPECTED_CHECKPOINT_COMMITMENT"));
        broadcaster.deploy(script, cfg);
        cfg = _config(BASE_SEPOLIA);
        cfg.expectedFinalizedHeight = 0;
        cfg.allowTestVerifier = true; // a verifier with anchor getters is checked under the switch
        VM.expectRevert(_placeholder("PFT_LOCK_VERIFIER_EXPECTED_FINALIZED_HEIGHT"));
        broadcaster.deploy(script, cfg);
        cfg = _config(BASE_SEPOLIA);
        cfg.expectedCommitteeRootCommitment = 0;
        VM.expectRevert(_placeholder("PFT_LOCK_VERIFIER_EXPECTED_COMMITTEE_ROOT_COMMITMENT"));
        broadcaster.deploy(script, cfg);
    }

    function testRefusesAVerifierBoundToAnotherRoute() public {
        address other = address(new MockLockToken(6, SUPPLY));
        verifier.setRoute(other, address(token).codehash, uint64(BASE_SEPOLIA));
        VM.expectRevert(
            abi.encodeWithSelector(
                DeployPFTLock.VerifierBindingMismatch.selector,
                "token",
                bytes32(uint256(uint160(address(token)))),
                bytes32(uint256(uint160(other)))
            )
        );
        broadcaster.deploy(script, _config(BASE_SEPOLIA));
        verifier.setRoute(address(token), other.codehash, uint64(BASE_SEPOLIA));
        VM.expectRevert(
            abi.encodeWithSelector(
                DeployPFTLock.VerifierBindingMismatch.selector,
                "token_code_hash",
                address(token).codehash,
                other.codehash
            )
        );
        broadcaster.deploy(script, _config(BASE_SEPOLIA));
        verifier.setRoute(address(token), address(token).codehash, uint64(BASE));
        VM.expectRevert(
            abi.encodeWithSelector(
                DeployPFTLock.VerifierBindingMismatch.selector,
                "chain_id",
                bytes32(uint256(BASE_SEPOLIA)),
                bytes32(uint256(BASE))
            )
        );
        broadcaster.deploy(script, _config(BASE_SEPOLIA));
    }

    // ------------------------------------------- verifier PFTL-side bindings

    function testRefusesAVerifierOfAnotherPftlChain() public {
        verifier.setPftl(keccak256("another chain"), GENESIS, ROUTE_PROFILE, ROUTE_EPOCH, ASSET_ID);
        _expectBinding("pftl_chain_id_hash", PFTL_CHAIN_ID_HASH, keccak256("another chain"));
        broadcaster.deploy(script, _config(BASE_SEPOLIA));
    }

    function testRefusesAVerifierOfAnotherGenesis() public {
        verifier.setPftl(PFTL_CHAIN_ID_HASH, keccak256("another genesis"), ROUTE_PROFILE, ROUTE_EPOCH, ASSET_ID);
        _expectBinding("pftl_genesis_hash_commitment", GENESIS, keccak256("another genesis"));
        broadcaster.deploy(script, _config(BASE_SEPOLIA));
    }

    function testRefusesAVerifierOfAnotherRouteProfile() public {
        verifier.setPftl(PFTL_CHAIN_ID_HASH, GENESIS, keccak256("another route"), ROUTE_EPOCH, ASSET_ID);
        _expectBinding("route_profile_hash_commitment", ROUTE_PROFILE, keccak256("another route"));
        broadcaster.deploy(script, _config(BASE_SEPOLIA));
    }

    function testRefusesAVerifierOfAnotherRouteEpoch() public {
        verifier.setPftl(PFTL_CHAIN_ID_HASH, GENESIS, ROUTE_PROFILE, ROUTE_EPOCH + 1, ASSET_ID);
        _expectBinding("route_epoch", bytes32(uint256(ROUTE_EPOCH)), bytes32(uint256(ROUTE_EPOCH + 1)));
        broadcaster.deploy(script, _config(BASE_SEPOLIA));
    }

    function testRefusesAVerifierOfAnotherAsset() public {
        verifier.setPftl(PFTL_CHAIN_ID_HASH, GENESIS, ROUTE_PROFILE, ROUTE_EPOCH, keccak256("another asset"));
        _expectBinding("asset_id_commitment", ASSET_ID, keccak256("another asset"));
        broadcaster.deploy(script, _config(BASE_SEPOLIA));
    }

    function testRefusesAVerifierWithAnotherVaultRuntimeCodeHashBeforeTheBroadcast() public {
        bytes32 lockHash = _lockHash(address(token), CAP);
        verifier.setVault(keccak256("another lock"));
        VM.expectRevert(
            abi.encodeWithSelector(
                DeployPFTLock.VaultRuntimeCodeHashMismatch.selector, lockHash, keccak256("another lock")
            )
        );
        broadcaster.deploy(script, _config(BASE_SEPOLIA));
        // A verifier built for the lock with another cap: the immutables differ.
        verifier.setVault(lockHash);
        DeployPFTLock.Config memory cfg = _config(BASE_SEPOLIA);
        cfg.maxLockAmount = CAP - 1_000_000;
        cfg.maxLockAmountPft = CAP_PFT - 1;
        VM.expectRevert(
            abi.encodeWithSelector(
                DeployPFTLock.VaultRuntimeCodeHashMismatch.selector,
                _lockHash(address(token), CAP - 1_000_000),
                lockHash
            )
        );
        broadcaster.deploy(script, cfg);
        _assertEq(address(_createAddress(address(broadcaster), 0)).code.length, 0, "nothing deployed");
    }

    function testRefusesAVaultRuntimeCodeHashThatDiffersAfterDeployment() public {
        // Matches before the broadcast; once the lock has code the verifier
        // answers another hash: only checkLock's comparison with the deployed codehash sees it.
        bytes32 lockHash = _lockHash(address(token), CAP);
        verifier.swapVaultOnceDeployed(_createAddress(address(broadcaster), 0), keccak256("swapped"));
        VM.expectRevert(
            abi.encodeWithSelector(DeployPFTLock.VaultRuntimeCodeHashMismatch.selector, lockHash, keccak256("swapped"))
        );
        broadcaster.deploy(script, _config(BASE_SEPOLIA));
    }

    function testTheComputedLockRuntimeCodeHashIsTheDeployedLocks() public {
        bytes32 computed = script.lockRuntimeCodeHash(_config(BASE_SEPOLIA));
        _assertTrue(computed == _lockHash(address(token), CAP), "computed as the test computes it");
        _assertTrue(address(broadcaster.deploy(script, _config(BASE_SEPOLIA))).codehash == computed, "deployed");
    }

    function testRefusesAVerifierWithoutAVaultRuntimeCodeHash() public {
        ScriptedVerifier scripted = new ScriptedVerifier();
        scripted.answer(IPFTLVerifierProbe.latestCheckpointCommitment.selector, CHECKPOINT);
        scripted.answer(IPFTLVerifierProbe.latestFinalizedHeight.selector, bytes32(uint256(HEIGHT)));
        scripted.answer(IPFTLVerifierProbe.latestCommitteeRootCommitment.selector, COMMITTEE_ROOT);
        scripted.answer(IPFTLVerifierProbe.token.selector, bytes32(uint256(uint160(address(token)))));
        scripted.answer(IPFTLVerifierProbe.tokenRuntimeCodeHash.selector, address(token).codehash);
        scripted.answer(IPFTLVerifierProbe.arbitrumChainId.selector, bytes32(BASE_SEPOLIA));
        scripted.answer(IPFTLVerifierProbe.pftlChainIdHash.selector, PFTL_CHAIN_ID_HASH);
        scripted.answer(IPFTLVerifierProbe.pftlGenesisHashCommitment.selector, GENESIS);
        scripted.answer(IPFTLVerifierProbe.routeProfileHashCommitment.selector, ROUTE_PROFILE);
        scripted.answer(IPFTLVerifierProbe.routeEpoch.selector, bytes32(uint256(ROUTE_EPOCH)));
        scripted.answer(IPFTLVerifierProbe.assetIdCommitment.selector, ASSET_ID);
        DeployPFTLock.Config memory cfg = _config(BASE_SEPOLIA);
        cfg.verifier = address(scripted);
        cfg.verifierCodeHash = address(scripted).codehash;
        VM.expectRevert(abi.encodeWithSelector(DeployPFTLock.VerifierUnreadable.selector, "vault_runtime_code_hash"));
        broadcaster.deploy(script, cfg);
    }

    function testRefusesAnExpectedFinalizedHeightAboveUint64() public {
        DeployPFTLock.Config memory cfg = _config(BASE_SEPOLIA);
        cfg.expectedFinalizedHeight = uint256(type(uint64).max) + 1;
        VM.expectRevert(
            abi.encodeWithSelector(
                DeployPFTLock.ValueTooLarge.selector,
                "PFT_LOCK_VERIFIER_EXPECTED_FINALIZED_HEIGHT",
                uint256(type(uint64).max) + 1
            )
        );
        broadcaster.deploy(script, cfg);
        cfg.expectedFinalizedHeight = type(uint64).max; // in range: the anchor comparison refuses it
        _expectAnchor("finalized_height", bytes32(uint256(type(uint64).max)), bytes32(uint256(HEIGHT)));
        broadcaster.deploy(script, cfg);
    }

    function testRefusesAnExpectedRouteEpochAboveUint64() public {
        DeployPFTLock.Config memory cfg = _config(BASE_SEPOLIA);
        cfg.expectedRouteEpoch = uint256(type(uint64).max) + 1;
        VM.expectRevert(
            abi.encodeWithSelector(
                DeployPFTLock.ValueTooLarge.selector,
                "PFT_LOCK_VERIFIER_EXPECTED_ROUTE_EPOCH",
                uint256(type(uint64).max) + 1
            )
        );
        broadcaster.deploy(script, cfg);
    }

    function testRefusesUnsetExpectedBindings() public {
        string[5] memory names = [
            "PFT_LOCK_VERIFIER_EXPECTED_PFTL_CHAIN_ID_HASH",
            "PFT_LOCK_VERIFIER_EXPECTED_GENESIS_HASH_COMMITMENT",
            "PFT_LOCK_VERIFIER_EXPECTED_ROUTE_PROFILE_HASH_COMMITMENT",
            "PFT_LOCK_VERIFIER_EXPECTED_ROUTE_EPOCH",
            "PFT_LOCK_VERIFIER_EXPECTED_ASSET_ID_COMMITMENT"
        ];
        for (uint256 i = 0; i < names.length; i++) {
            DeployPFTLock.Config memory cfg = _config(BASE_SEPOLIA);
            cfg.allowTestVerifier = i % 2 == 1; // zero is refused inside the anchor check, switch or not
            if (i == 0) cfg.expectedPftlChainIdHash = 0;
            if (i == 1) cfg.expectedGenesisHashCommitment = 0;
            if (i == 2) cfg.expectedRouteProfileHashCommitment = 0;
            if (i == 3) cfg.expectedRouteEpoch = 0;
            if (i == 4) cfg.expectedAssetIdCommitment = 0;
            VM.expectRevert(_placeholder(names[i]));
            broadcaster.deploy(script, cfg);
        }
    }

    function testRecordsTheAnchorSkip() public {
        MockLockVerifier standIn = new MockLockVerifier();
        DeployPFTLock.Config memory cfg = _config(BASE_SEPOLIA);
        cfg.verifier = address(standIn);
        cfg.verifierCodeHash = address(standIn).codehash;
        cfg.allowTestVerifier = true;
        cfg.recordDir = "cache/deploy-test/lock-anchor-skip";
        cfg.writeRecord = true;
        broadcaster.deploy(script, cfg);
        string memory json = VM.readFile("cache/deploy-test/lock-anchor-skip/deployment.json");
        _assertTrue(!VM.parseJsonBool(json, ".finality_verifier_anchor_checked"), "record anchor skipped");
    }

    /// The probe's selectors against the real PFTLFinalityVerifierV1, then a
    /// deployment bound to it: its address predicted, the lock's runtime code
    /// hash for that address computed first. A test recipe (vm.etch, a
    /// dedicated deployer), not the deployment procedure (README).
    function testTheProbeReadsTheRealVerifier() public {
        VerifierDeployer deployer = new VerifierDeployer();
        address predicted = VM.computeCreateAddress(address(deployer), VM.getNonce(address(deployer)));
        VM.etch(predicted, hex"00"); // the lock's constructor requires code at the verifier
        bytes32 lockHash =
            address(new PFTLockV1(address(token), predicted, address(token).codehash, uint64(CAP), uint64(MINIMUM)))
        .codehash;
        VM.etch(predicted, "");
        PFTLFinalityVerifierV1.Config memory c;
        c.sp1Verifier = ISP1Verifier(address(0x5B1));
        c.programVKey = keccak256("program vkey");
        c.pftlChainIdHash = PFTL_CHAIN_ID_HASH;
        c.pftlGenesisHashCommitment = GENESIS;
        c.pftlProtocolVersion = 1;
        c.routeProfileHashCommitment = ROUTE_PROFILE;
        c.routeEpoch = ROUTE_EPOCH;
        c.assetIdCommitment = ASSET_ID;
        c.arbitrumChainId = uint64(BASE_SEPOLIA);
        c.vaultRuntimeCodeHash = lockHash;
        c.token = address(token);
        c.tokenRuntimeCodeHash = address(token).codehash;
        c.maxProofBytes = 1;
        c.maxPublicValuesBytes = 1;
        c.initialCheckpointCommitment = CHECKPOINT;
        c.initialFinalizedHeight = HEIGHT;
        c.initialCommitteeRootCommitment = COMMITTEE_ROOT;
        PFTLFinalityVerifierV1 real = deployer.deploy(c);
        _assertTrue(address(real) == predicted, "predicted verifier address");

        bytes4[12] memory selectors = [
            IPFTLVerifierProbe.latestCheckpointCommitment.selector,
            IPFTLVerifierProbe.latestFinalizedHeight.selector,
            IPFTLVerifierProbe.latestCommitteeRootCommitment.selector,
            IPFTLVerifierProbe.token.selector,
            IPFTLVerifierProbe.tokenRuntimeCodeHash.selector,
            IPFTLVerifierProbe.arbitrumChainId.selector,
            IPFTLVerifierProbe.pftlChainIdHash.selector,
            IPFTLVerifierProbe.pftlGenesisHashCommitment.selector,
            IPFTLVerifierProbe.routeProfileHashCommitment.selector,
            IPFTLVerifierProbe.routeEpoch.selector,
            IPFTLVerifierProbe.assetIdCommitment.selector,
            IPFTLVerifierProbe.vaultRuntimeCodeHash.selector
        ];
        bytes32[12] memory words = [
            CHECKPOINT,
            bytes32(uint256(HEIGHT)),
            COMMITTEE_ROOT,
            bytes32(uint256(uint160(address(token)))),
            address(token).codehash,
            bytes32(BASE_SEPOLIA),
            PFTL_CHAIN_ID_HASH,
            GENESIS,
            ROUTE_PROFILE,
            bytes32(uint256(ROUTE_EPOCH)),
            ASSET_ID,
            lockHash
        ];
        for (uint256 i = 0; i < selectors.length; i++) {
            (bool ok, bytes memory data) = address(real).staticcall(abi.encodeWithSelector(selectors[i]));
            _assertTrue(ok && data.length == 32 && abi.decode(data, (bytes32)) == words[i], "probe selector");
        }

        DeployPFTLock.Config memory cfg = _config(BASE_SEPOLIA);
        cfg.verifier = address(real);
        cfg.verifierCodeHash = address(real).codehash;
        PFTLockV1 lock = broadcaster.deploy(script, cfg);
        _assertTrue(address(lock).codehash == lockHash && lock.finalityVerifier() == address(real), "bound lock");
    }

    function testTheTestSwitchSkipsTheAnchorOnlyForAStandInWithoutGetters() public {
        MockLockVerifier standIn = new MockLockVerifier();
        DeployPFTLock.Config memory cfg = _config(BASE_SEPOLIA);
        cfg.verifier = address(standIn);
        cfg.verifierCodeHash = address(standIn).codehash;
        // Pinned but without the switch: the anchor must be readable.
        VM.expectRevert(abi.encodeWithSelector(DeployPFTLock.VerifierUnreadable.selector, "checkpoint_commitment"));
        broadcaster.deploy(script, cfg);
        // The switch on 84532 skips it (and prints the skip), with or without expected values.
        cfg.allowTestVerifier = true;
        cfg.expectedCheckpointCommitment = 0;
        PFTLockV1 lock = broadcaster.deploy(script, cfg);
        _assertTrue(lock.finalityVerifier() == address(standIn), "stand-in verifier");
        // A verifier answering only part of the anchor or none of the route is refused under the switch.
        ScriptedVerifier scripted = new ScriptedVerifier();
        cfg = _config(BASE_SEPOLIA);
        cfg.allowTestVerifier = true;
        cfg.verifier = address(scripted);
        scripted.answer(IPFTLVerifierProbe.latestCheckpointCommitment.selector, CHECKPOINT);
        VM.expectRevert(abi.encodeWithSelector(DeployPFTLock.VerifierUnreadable.selector, "finalized_height"));
        broadcaster.deploy(script, cfg);
        scripted.answer(IPFTLVerifierProbe.latestFinalizedHeight.selector, bytes32(uint256(HEIGHT)));
        VM.expectRevert(abi.encodeWithSelector(DeployPFTLock.VerifierUnreadable.selector, "committee_root_commitment"));
        broadcaster.deploy(script, cfg);
        scripted.answer(IPFTLVerifierProbe.latestCommitteeRootCommitment.selector, COMMITTEE_ROOT);
        VM.expectRevert(abi.encodeWithSelector(DeployPFTLock.VerifierUnreadable.selector, "token"));
        broadcaster.deploy(script, cfg);
        // A verifier with getters is checked under the switch.
        verifier.setAnchor(keccak256("stale"), HEIGHT, COMMITTEE_ROOT);
        cfg = _config(BASE_SEPOLIA);
        cfg.allowTestVerifier = true;
        _expectAnchor("checkpoint_commitment", CHECKPOINT, keccak256("stale"));
        broadcaster.deploy(script, cfg);
        // 8453: the stand-in is refused, pinned or not, and the switch is refused.
        VM.chainId(BASE);
        cfg = _config(BASE);
        cfg.verifier = address(standIn);
        cfg.verifierCodeHash = address(standIn).codehash;
        VM.expectRevert(abi.encodeWithSelector(DeployPFTLock.VerifierUnreadable.selector, "checkpoint_commitment"));
        broadcaster.deploy(script, cfg);
        cfg.allowTestVerifier = true;
        VM.expectRevert(
            abi.encodeWithSelector(DeployPFTLock.TestSwitchOnMainnet.selector, "PFT_LOCK_ALLOW_TEST_VERIFIER")
        );
        broadcaster.deploy(script, cfg);
    }

    function testRefusesACapPairMismatch() public {
        DeployPFTLock.Config memory cfg = _config(BASE_SEPOLIA);
        cfg.maxLockAmountPft = CAP_PFT + 1;
        VM.expectRevert(abi.encodeWithSelector(DeployPFTLock.MaxLockAmountMismatch.selector, CAP, CAP_PFT + 1));
        broadcaster.deploy(script, cfg);
        cfg.maxLockAmountPft = type(uint256).max; // no overflow
        VM.expectRevert(abi.encodeWithSelector(DeployPFTLock.MaxLockAmountMismatch.selector, CAP, type(uint256).max));
        broadcaster.deploy(script, cfg);
        cfg.maxLockAmount = CAP + 1; // not whole PFT
        cfg.maxLockAmountPft = CAP_PFT;
        VM.expectRevert(abi.encodeWithSelector(DeployPFTLock.MaxLockAmountMismatch.selector, CAP + 1, CAP_PFT));
        broadcaster.deploy(script, cfg);
    }

    function testRefusesACapAboveUint64() public {
        DeployPFTLock.Config memory cfg = _config(BASE_SEPOLIA);
        cfg.maxLockAmount = uint256(type(uint64).max) + 1;
        cfg.maxLockAmountPft = cfg.maxLockAmount / 1_000_000;
        VM.expectRevert(abi.encodeWithSelector(DeployPFTLock.MaxLockAmountTooLarge.selector, cfg.maxLockAmount));
        broadcaster.deploy(script, cfg);
    }

    function testAcceptsTheLargestWholePftCap() public {
        DeployPFTLock.Config memory cfg = _config(BASE_SEPOLIA);
        cfg.maxLockAmountPft = type(uint64).max / 1_000_000;
        cfg.maxLockAmount = cfg.maxLockAmountPft * 1_000_000;
        verifier.setVault(_lockHash(address(token), cfg.maxLockAmount));
        _assertEq(broadcaster.deploy(script, cfg).maxLockAmount(), cfg.maxLockAmount, "largest cap");
    }

    // -------------------------------------------------------------- deployer

    function testRefusesAUsedDeployer() public {
        VM.setNonce(address(broadcaster), 5);
        VM.expectRevert(abi.encodeWithSelector(DeployPFTLock.DeployerAlreadyUsed.selector, address(broadcaster), 5));
        broadcaster.deploy(script, _config(BASE_SEPOLIA));
        VM.chainId(BASE);
        VM.expectRevert(abi.encodeWithSelector(DeployPFTLock.DeployerAlreadyUsed.selector, address(broadcaster), 5));
        broadcaster.deploy(script, _config(BASE));
    }

    function testAllowsAUsedDeployerOnBaseSepoliaOnlyWithTheTestSwitch() public {
        VM.setNonce(address(broadcaster), 5);
        DeployPFTLock.Config memory cfg = _config(BASE_SEPOLIA);
        cfg.allowUsedDeployer = true;
        _checkLock(broadcaster.deploy(script, cfg));
    }

    // ------------------------------------------------------- deployment record

    function testRefusesARealDeploymentWithATestSwitch() public {
        DeployPFTLock.Config memory cfg = _config(BASE_SEPOLIA);
        cfg.allowTestToken = false;
        cfg.realDeployment = true;
        cfg.sourceCommit = COMMIT;
        cfg.allowTestVerifier = true;
        VM.expectRevert(
            abi.encodeWithSelector(DeployPFTLock.RealDeploymentWithTestSwitch.selector, "PFT_LOCK_ALLOW_TEST_VERIFIER")
        );
        broadcaster.deploy(script, cfg);
        cfg.allowTestVerifier = false;
        cfg.allowUsedDeployer = true;
        VM.expectRevert(
            abi.encodeWithSelector(DeployPFTLock.RealDeploymentWithTestSwitch.selector, "PFT_LOCK_ALLOW_USED_DEPLOYER")
        );
        broadcaster.deploy(script, cfg);
    }

    function testRefusesARealDeploymentWithoutASourceCommit() public {
        string[3] memory commits = ["", " ", "replace_with_the_deployed_git_commit"];
        for (uint256 i = 0; i < commits.length; i++) {
            DeployPFTLock.Config memory cfg = _config(BASE_SEPOLIA);
            cfg.allowTestToken = false;
            cfg.realDeployment = true;
            cfg.sourceCommit = commits[i];
            VM.expectRevert(DeployPFTLock.SourceCommitMissing.selector);
            broadcaster.deploy(script, cfg);
        }
    }

    function testRefusesARealDeploymentWithASourceCommitOfAnotherShape() public {
        // A 1-byte value is shorter than "replace": the placeholder search must not underflow.
        string[6] memory invalid = [
            "a",
            "not-a-commit",
            "0123456789abcdef0123456789abcdef0123456",
            "0123456789abcdef0123456789abcdef012345678",
            "0123456789abcdef0123456789abcdef0123456g",
            " 123456789abcdef0123456789abcdef01234567"
        ];
        for (uint256 i = 0; i < invalid.length; i++) {
            DeployPFTLock.Config memory cfg = _config(BASE_SEPOLIA);
            cfg.allowTestToken = false;
            cfg.realDeployment = true;
            cfg.sourceCommit = invalid[i];
            VM.expectRevert(abi.encodeWithSelector(DeployPFTLock.SourceCommitInvalid.selector, invalid[i]));
            broadcaster.deploy(script, cfg);
        }
    }

    function testAcceptsASourceCommitOfFortyHexCharacters() public {
        // The shape only: the value is the operator's statement, so 40 zeros pass.
        VM.chainId(BASE);
        string[3] memory valid =
            [COMMIT, "0000000000000000000000000000000000000000", "0123456789ABCDEF0123456789abcdef01234567"];
        for (uint256 i = 0; i < valid.length; i++) {
            DeployPFTLock.Config memory cfg = _config(BASE);
            cfg.realDeployment = true;
            cfg.recordDir = "cache/deploy-test/lock-commit-shape-never-written";
            cfg.sourceCommit = valid[i];
            LockBroadcaster fresh = new LockBroadcaster(); // a fresh deployer each time
            VM.setNonceUnsafe(address(fresh), 0);
            PFTLockV1 lock = fresh.deploy(script, cfg);
            _assertTrue(lock.token() == address(token) && lock.totalLocked() == 0, "lock deployed");
        }
    }

    function testRealDeploymentWritesOnceAndRefusesToOverwrite() public {
        // On 8453: on 84532 a real deployment needs the recorded test token.
        VM.chainId(BASE);
        DeployPFTLock.Config memory cfg = _config(BASE);
        cfg.realDeployment = true;
        cfg.recordDir = "cache/deploy-test/lock-real-once";
        cfg.writeRecord = true;
        cfg.sourceCommit = COMMIT;
        string memory path = string.concat(cfg.recordDir, "/deployment.json");
        if (VM.exists(path)) VM.removeFile(path);
        PFTLockV1 lock = broadcaster.deploy(script, cfg);
        _checkRecord(cfg.recordDir, lock, BASE, "real", true);

        // A second run, even a dry run that writes nothing, stops before deploying.
        VM.setNonceUnsafe(address(broadcaster), 0);
        cfg.writeRecord = false;
        VM.expectRevert(abi.encodeWithSelector(DeployPFTLock.RecordExists.selector, path));
        broadcaster.deploy(script, cfg);
    }

    function testADryRunWritesNoRecord() public {
        DeployPFTLock.Config memory cfg = _config(BASE_SEPOLIA);
        cfg.recordDir = "cache/deploy-test/lock-dry-run";
        string memory path = string.concat(cfg.recordDir, "/deployment.json");
        if (VM.exists(path)) VM.removeFile(path);
        broadcaster.deploy(script, cfg);
        _assertTrue(!VM.exists(path), "no record");
    }

    // ------------------------------------------------------- deployed state

    function testCheckLockRefusesADeployedStateThatDiffers() public {
        DeployPFTLock.Config memory cfg = _config(BASE_SEPOLIA);
        PFTLockV1 lock = broadcaster.deploy(script, cfg);
        script.checkLock(cfg, lock);
        bytes[6] memory calls = [
            abi.encodeCall(IPFTLockV1.token, ()),
            abi.encodeCall(IPFTLockV1.finalityVerifier, ()),
            abi.encodeCall(IPFTLockV1.maxLockAmount, ()),
            abi.encodeCall(IPFTLockV1.minimumLockAmount, ()),
            abi.encodeCall(IPFTLockV1.nextNonce, ()),
            abi.encodeCall(IPFTLockV1.totalLocked, ())
        ];
        string[6] memory fields =
            ["token", "finality_verifier", "max_lock_amount", "minimum_lock_amount", "next_nonce", "total_locked"];
        for (uint256 i = 0; i < calls.length; i++) {
            PFTLockV1 other =
                new PFTLockV1(address(token), address(verifier), address(token).codehash, uint64(CAP), uint64(MINIMUM));
            VM.mockCall(address(other), calls[i], abi.encode(uint256(1)));
            VM.expectRevert(abi.encodeWithSelector(DeployPFTLock.LockStateMismatch.selector, fields[i]));
            script.checkLock(cfg, other);
        }
    }

    // --------------------------------------------------------------- artifact

    function testRefusesAnArtifactOfAnotherBuild() public {
        string memory path = _artifactCopy("other-build");
        VM.writeJson(string.concat('"', VM.toString(bytes(hex"6080")), '"'), path, ".bytecode.object");
        DeployPFTLock.Config memory cfg = _config(BASE_SEPOLIA);
        cfg.artifact = path;
        VM.expectRevert(
            abi.encodeWithSelector(
                DeployPFTLock.ArtifactMismatch.selector,
                path,
                keccak256(hex"6080"),
                keccak256(type(PFTLockV1).creationCode)
            )
        );
        broadcaster.deploy(script, cfg);
        // The token's artifact is not the lock's.
        cfg.artifact = "out/PFTToken.sol/PFTToken.json";
        VM.expectRevert();
        broadcaster.deploy(script, cfg);
    }

    function testRefusesAnArtifactWithoutTheIpfsMetadataHash() public {
        string memory path = _artifactCopy("no-metadata");
        VM.writeJson('"none"', path, ".metadata.settings.metadata.bytecodeHash");
        VM.expectRevert(DeployPFTLock.MetadataHashDisabled.selector);
        script.checkArtifactMetadata(path, VM.readFile(path), _outCreationCode());
    }

    function testRefusesAnArtifactWhoseMetadataIsFromAnotherBuild() public {
        string memory path = _artifactCopy("spliced");
        string memory raw = VM.parseJsonString(VM.readFile(path), ".rawMetadata");
        string memory spliced = VM.replace(raw, '"runs":200', '"runs":201');
        _assertTrue(!_eq(raw, spliced), "spliced metadata differs");
        VM.writeJson(string.concat('"', _escape(spliced), '"'), path, ".rawMetadata");
        bytes memory creation = _outCreationCode();
        VM.expectRevert(abi.encodeWithSelector(DeployPFTLock.ArtifactMetadataMismatch.selector, path));
        script.checkArtifactMetadata(path, VM.readFile(path), creation);
        path = _artifactCopy("runtime");
        VM.writeJson(string.concat('"', VM.toString(bytes(hex"00")), '"'), path, ".deployedBytecode.object");
        VM.expectRevert(abi.encodeWithSelector(DeployPFTLock.ArtifactMetadataMismatch.selector, path));
        script.checkArtifactMetadata(path, VM.readFile(path), creation);
        script.checkArtifactMetadata(ARTIFACT, VM.readFile(ARTIFACT), creation);
    }

    function testRecordsTheCompilerSettingsOfTheRawMetadata() public {
        // The metadata object is not covered by the code's IPFS digest; the record ignores it.
        string memory dir =
            string.concat("cache/deploy-test/lock-edited-metadata-", VM.toString(abi.encode(VM.randomUint())));
        string memory path = string.concat(dir, "/PFTLockV1.sol/PFTLockV1.json");
        VM.createDir(string.concat(dir, "/PFTLockV1.sol"), true);
        VM.writeFile(path, VM.readFile(_artifact()));
        VM.writeJson('"0.8.20+commit.a1b79de6"', path, ".metadata.compiler.version");
        VM.writeJson("999", path, ".metadata.settings.optimizer.runs");
        VM.writeJson("false", path, ".metadata.settings.optimizer.enabled");
        VM.writeJson('"paris"', path, ".metadata.settings.evmVersion");
        DeployPFTLock.Config memory cfg = _config(BASE_SEPOLIA);
        cfg.artifact = path;
        cfg.recordDir = string.concat(dir, "/record");
        cfg.writeRecord = true;
        broadcaster.deploy(script, cfg);
        string memory json = VM.readFile(string.concat(cfg.recordDir, "/deployment.json"));
        _assertTrue(_eq(VM.parseJsonString(json, ".solc_version"), "0.8.24+commit.e11b9ed9"), "solc of rawMetadata");
        _assertEq(VM.parseJsonUint(json, ".optimizer_runs"), 200, "runs of rawMetadata");
        _assertTrue(VM.parseJsonBool(json, ".optimizer"), "optimizer of rawMetadata");
        _assertTrue(_eq(VM.parseJsonString(json, ".evm_version"), "cancun"), "evm version of rawMetadata");
        _assertTrue(_eq(VM.parseJsonString(json, ".bytecode_hash"), "ipfs"), "bytecode hash of rawMetadata");
    }

    function testRefusesATrailerOnlyRuntimeCode() public {
        bytes memory runtime = VM.parseJsonBytes(VM.readFile(ARTIFACT), ".deployedBytecode.object");
        bytes memory trailer = script.metadataTrailer(runtime);
        _assertTrue(trailer.length == 53 && script.metadataDigest(trailer) != 0, "a 53-byte trailer with the digest");
        string memory path = _artifactCopy("trailer-only");
        VM.writeJson(string.concat('"', VM.toString(trailer), '"'), path, ".deployedBytecode.object");
        VM.expectRevert(abi.encodeWithSelector(DeployPFTLock.ArtifactMetadataMismatch.selector, path));
        script.checkArtifactMetadata(path, VM.readFile(path), _outCreationCode());
        // After the broadcast: the deployed code has the artifact's length and trailer.
        PFTLockV1 lock = broadcaster.deploy(script, _config(BASE_SEPOLIA));
        bytes memory built = VM.parseJsonBytes(VM.readFile(_artifact()), ".deployedBytecode.object");
        script.checkDeployedCode(address(lock), built);
        uint256 length = address(lock).code.length;
        VM.expectRevert(abi.encodeWithSelector(DeployPFTLock.DeployedCodeMismatch.selector, address(lock), length, 53));
        script.checkDeployedCode(address(lock), trailer);
        bytes memory other = abi.encodePacked(built);
        other[other.length - 3] = bytes1(uint8(other[other.length - 3]) ^ 1);
        VM.expectRevert(
            abi.encodeWithSelector(DeployPFTLock.DeployedCodeMismatch.selector, address(lock), length, length)
        );
        script.checkDeployedCode(address(lock), other);
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
        _assertTrue(
            script.ipfsDigest("hello\n") == 0xa568e404d8eadaec925ac3bc1b36259c0f6d70000d729cd918c46a836f497657, "ipfs"
        );
        _assertTrue(script.ipfsDigest(new bytes(262145)) == 0, "one chunk only");
    }

    function testTheArtifactUnderTestIsThisBuild() public view {
        bytes memory code = VM.parseJsonBytes(VM.readFile(_artifact()), ".bytecode.object");
        _assertTrue(keccak256(code) == keccak256(type(PFTLockV1).creationCode), "artifact is this build");
    }

    // ---------------------------------------------------------------- helpers

    function _config(uint256 chainId) private view returns (DeployPFTLock.Config memory cfg) {
        cfg.chainId = chainId;
        cfg.token = address(token);
        cfg.tokenCodeHash = address(token).codehash;
        cfg.verifier = address(verifier);
        cfg.verifierCodeHash = address(verifier).codehash;
        cfg.expectedCheckpointCommitment = CHECKPOINT;
        cfg.expectedFinalizedHeight = HEIGHT;
        cfg.expectedCommitteeRootCommitment = COMMITTEE_ROOT;
        cfg.expectedPftlChainIdHash = PFTL_CHAIN_ID_HASH;
        cfg.expectedGenesisHashCommitment = GENESIS;
        cfg.expectedRouteProfileHashCommitment = ROUTE_PROFILE;
        cfg.expectedRouteEpoch = ROUTE_EPOCH;
        cfg.expectedAssetIdCommitment = ASSET_ID;
        cfg.maxLockAmount = CAP;
        cfg.maxLockAmountPft = CAP_PFT;
        cfg.minimumLockAmount = MINIMUM;
        cfg.artifact = _artifact();
        // The tests' token is not the recorded Base Sepolia test token.
        cfg.allowTestToken = chainId == BASE_SEPOLIA;
    }

    function _withToken(address t) private view returns (DeployPFTLock.Config memory cfg) {
        cfg = _config(BASE_SEPOLIA);
        cfg.token = t;
        cfg.tokenCodeHash = t.codehash;
    }

    function _envValue(string memory name) private view returns (string memory) {
        if (_eq(name, "PFT_LOCK_CHAIN_ID")) return "84532";
        if (_eq(name, "PFT_LOCK_TOKEN")) return VM.toString(address(token));
        if (_eq(name, "PFT_LOCK_TOKEN_CODEHASH")) return VM.toString(address(token).codehash);
        if (_eq(name, "PFT_LOCK_VERIFIER")) return VM.toString(address(verifier));
        if (_eq(name, "PFT_LOCK_VERIFIER_EXPECTED_CHECKPOINT_COMMITMENT")) return VM.toString(CHECKPOINT);
        if (_eq(name, "PFT_LOCK_VERIFIER_EXPECTED_FINALIZED_HEIGHT")) return "4200";
        if (_eq(name, "PFT_LOCK_VERIFIER_EXPECTED_COMMITTEE_ROOT_COMMITMENT")) return VM.toString(COMMITTEE_ROOT);
        if (_eq(name, "PFT_LOCK_VERIFIER_EXPECTED_PFTL_CHAIN_ID_HASH")) return VM.toString(PFTL_CHAIN_ID_HASH);
        if (_eq(name, "PFT_LOCK_VERIFIER_EXPECTED_GENESIS_HASH_COMMITMENT")) return VM.toString(GENESIS);
        if (_eq(name, "PFT_LOCK_VERIFIER_EXPECTED_ROUTE_PROFILE_HASH_COMMITMENT")) return VM.toString(ROUTE_PROFILE);
        if (_eq(name, "PFT_LOCK_VERIFIER_EXPECTED_ROUTE_EPOCH")) return "7";
        if (_eq(name, "PFT_LOCK_VERIFIER_EXPECTED_ASSET_ID_COMMITMENT")) return VM.toString(ASSET_ID);
        if (_eq(name, "PFT_LOCK_MINIMUM_LOCK_AMOUNT")) return "10";
        if (_eq(name, "PFT_LOCK_MAX_LOCK_AMOUNT")) return "2000000000000000";
        return "2000000000";
    }

    function _checkLock(PFTLockV1 lock) private view {
        _assertTrue(lock.token() == address(token), "lock token");
        _assertTrue(lock.finalityVerifier() == address(verifier), "lock verifier");
        _assertEq(lock.nextNonce(), 0, "lock next nonce");
        _assertEq(lock.totalLocked(), 0, "lock total locked");
        _assertEq(lock.totalReleased(), 0, "lock total released");
        _assertEq(token.balanceOf(address(broadcaster)), 0, "deployer holds nothing");
    }

    function _checkRecord(string memory dir, PFTLockV1 lock, uint256 chainId, string memory mode, bool pinned)
        private
        view
    {
        string memory json = VM.readFile(string.concat(dir, "/deployment.json"));
        _assertTrue(_eq(VM.parseJsonString(json, ".mode"), mode), "record mode");
        _assertTrue(_eq(VM.parseJsonString(json, ".tx_hash"), "pending"), "record tx hash");
        _assertTrue(_eq(VM.parseJsonString(json, ".block_number"), "pending"), "record block number");
        _assertTrue(VM.parseJsonAddress(json, ".lock") == address(lock), "record lock");
        _assertTrue(VM.parseJsonBytes32(json, ".lock_runtime_codehash") == address(lock).codehash, "record lock hash");
        _assertEq(VM.parseJsonUint(json, ".chain_id"), chainId, "record chain id");
        _assertTrue(VM.parseJsonAddress(json, ".token") == address(token), "record token");
        _assertTrue(VM.parseJsonBytes32(json, ".token_codehash") == address(token).codehash, "record token hash");
        _assertTrue(VM.parseJsonAddress(json, ".finality_verifier") == address(verifier), "record verifier");
        _assertTrue(
            VM.parseJsonBytes32(json, ".finality_verifier_codehash") == address(verifier).codehash,
            "record verifier hash"
        );
        _assertTrue(VM.parseJsonBool(json, ".finality_verifier_codehash_pinned") == pinned, "record pinned");
        _assertTrue(VM.parseJsonBool(json, ".finality_verifier_anchor_checked"), "record anchor checked");
        _assertTrue(VM.parseJsonBytes32(json, ".expected_checkpoint_commitment") == CHECKPOINT, "record checkpoint");
        _assertEq(VM.parseJsonUint(json, ".expected_finalized_height"), HEIGHT, "record height");
        _assertTrue(VM.parseJsonBytes32(json, ".expected_committee_root_commitment") == COMMITTEE_ROOT, "record root");
        _assertTrue(VM.parseJsonBytes32(json, ".expected_pftl_chain_id_hash") == PFTL_CHAIN_ID_HASH, "record chain");
        _assertTrue(VM.parseJsonBytes32(json, ".expected_genesis_hash_commitment") == GENESIS, "record genesis");
        _assertTrue(
            VM.parseJsonBytes32(json, ".expected_route_profile_hash_commitment") == ROUTE_PROFILE, "record route"
        );
        _assertEq(VM.parseJsonUint(json, ".expected_route_epoch"), ROUTE_EPOCH, "record route epoch");
        _assertTrue(VM.parseJsonBytes32(json, ".expected_asset_id_commitment") == ASSET_ID, "record asset id");
        _assertTrue(
            VM.parseJsonBytes32(json, ".computed_vault_runtime_codehash") == address(lock).codehash,
            "record vault runtime code hash"
        );
        _assertEq(VM.parseJsonUint(json, ".max_lock_amount_units"), CAP, "record cap");
        _assertEq(VM.parseJsonUint(json, ".max_lock_amount_pft"), CAP_PFT, "record cap in PFT");
        bytes memory args =
            abi.encode(address(token), address(verifier), address(token).codehash, uint64(CAP), uint64(MINIMUM));
        _assertTrue(_eq(VM.parseJsonString(json, ".constructor_args"), VM.toString(args)), "record constructor args");
        _assertEq(VM.parseJsonUint(json, ".minimum_lock_amount_units"), MINIMUM, "record minimum");
        _assertTrue(VM.parseJsonAddress(json, ".deployer") == address(broadcaster), "record deployer");
        _assertTrue(_eq(VM.parseJsonString(json, ".artifact"), _artifact()), "record artifact");
        _assertTrue(
            VM.parseJsonBytes32(json, ".creation_code_keccak256") == keccak256(type(PFTLockV1).creationCode),
            "record creation code"
        );
        string memory artifact = VM.readFile(_artifact());
        _assertTrue(_eq(VM.parseJsonString(json, ".solc_version"), "0.8.24+commit.e11b9ed9"), "record solc");
        _assertTrue(_eq(VM.parseJsonString(json, ".evm_version"), "cancun"), "record evm version");
        _assertTrue(_eq(VM.parseJsonString(json, ".bytecode_hash"), "ipfs"), "record bytecode hash");
        _assertEq(VM.parseJsonUint(json, ".optimizer_runs"), 200, "record optimizer runs");
        _assertTrue(VM.parseJsonBool(json, ".optimizer"), "record optimizer");
        _assertTrue(
            VM.parseJsonBytes32(json, ".runtime_code_keccak256")
                == keccak256(VM.parseJsonBytes(artifact, ".deployedBytecode.object")),
            "record runtime code"
        );
        _assertTrue(
            VM.parseJsonBytes32(json, ".deployed_code_keccak256") == keccak256(address(lock).code),
            "record deployed code"
        );
        _assertTrue(_eq(VM.parseJsonString(json, ".source_commit"), _eq(mode, "real") ? COMMIT : ""), "record commit");
        _assertTrue(
            _eq(VM.parseJsonString(json, ".openzeppelin_commit"), "cab19933c33c2ad1d4c7a84864a3601dddfd16f3"),
            "record OpenZeppelin commit"
        );
    }

    /// A copy of the artifact under test, to be altered by the caller.
    function _artifactCopy(string memory name) private returns (string memory path) {
        string memory dir =
            string.concat("cache/deploy-test/lock-artifact-", name, "-", VM.toString(abi.encode(VM.randomUint())));
        VM.createDir(dir, true);
        path = string.concat(dir, "/PFTLockV1.json");
        VM.writeFile(path, VM.readFile(ARTIFACT));
    }

    /// The creation code of out/'s build (under coverage, not this build).
    function _outCreationCode() private view returns (bytes memory) {
        return VM.parseJsonBytes(VM.readFile(ARTIFACT), ".bytecode.object");
    }

    function _artifact() private view returns (string memory) {
        return string.concat(outDir, "/PFTLockV1.sol/PFTLockV1.json");
    }

    /// The runtime code hash of a lock on `t` with cap `cap` and the test verifier, computed here.
    function _lockHash(address t, uint256 cap) private returns (bytes32) {
        return address(new PFTLockV1(t, address(verifier), t.codehash, uint64(cap), uint64(MINIMUM))).codehash;
    }

    function _expectBinding(string memory field, bytes32 expected, bytes32 actual) private {
        VM.expectRevert(abi.encodeWithSelector(DeployPFTLock.VerifierBindingMismatch.selector, field, expected, actual));
    }

    function _expectAnchor(string memory field, bytes32 expected, bytes32 actual) private {
        VM.expectRevert(abi.encodeWithSelector(DeployPFTLock.VerifierAnchorMismatch.selector, field, expected, actual));
    }

    /// The CREATE address of `deployer` at `nonce` (< 0x80).
    function _createAddress(address deployer, uint8 nonce) private pure returns (address) {
        bytes memory n = nonce == 0 ? bytes(hex"80") : abi.encodePacked(bytes1(nonce));
        return address(uint160(uint256(keccak256(abi.encodePacked(hex"d694", deployer, n)))));
    }

    function _placeholder(string memory name) private pure returns (bytes memory) {
        return abi.encodeWithSelector(DeployPFTLock.PlaceholderValue.selector, name);
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

    function _endsWith(string memory value, string memory tail) private pure returns (bool) {
        bytes memory v = bytes(value);
        bytes memory t = bytes(tail);
        if (t.length > v.length) return false;
        for (uint256 i = 0; i < t.length; i++) {
            if (v[v.length - t.length + i] != t[i]) return false;
        }
        return true;
    }

    function _eq(string memory a, string memory b) private pure returns (bool) {
        return keccak256(bytes(a)) == keccak256(bytes(b));
    }

    function _assertTrue(bool value, string memory message) private pure {
        if (!value) revert(message);
    }

    function _assertEq(uint256 actual, uint256 expected, string memory message) private pure {
        if (actual != expected) revert(message);
    }
}
