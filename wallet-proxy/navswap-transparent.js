'use strict';

function create(runtime) {
    const { A651_ASSET_ID,A652_ASSET_ID,ALLOWED_ORIGINS,ASSET_ORCHARD_ACTION_CLEAR_KEYS,ASSET_ORCHARD_INGRESS_FILE_SCHEMA,ASSET_ORCHARD_POOL_ID,ASSET_ORCHARD_PRIVATE_EGRESS_ACTION_SCHEMA,ASSET_ORCHARD_PRIVATE_EGRESS_FILE_SCHEMA,ASSET_ORCHARD_SWAP_ACTION_SCHEMA,DEFAULT_ASSET_ORCHARD_LOCAL_SERVICE_READINESS_TIMEOUT_MS,DEFAULT_ASSET_ORCHARD_LOCAL_SERVICE_URL,DEFAULT_RPC_FLEET,ENABLE_FINALITY_RESPONDER_READ_CACHE,ENABLE_FIRST_READY_SEQUENCED_READ,ENABLE_NATIVE_WALLET_SIGNER,ENABLE_PROPOSER_ROUTING,ENABLE_RPC_PARENT_WAIT_FINALITY_ROUTE,ENABLE_RPC_PARENT_WAIT_SEQUENCED_READ_ROUTE,ENABLE_UPSTREAM_KEEPALIVE,ETHEREUM_USDC_TOKEN,FASTPAY_BROADCAST_METHODS,FASTPAY_FLEET_STATUS_CACHE_MS,FASTPAY_OWNED_OBJECT_LOOKUP_LIMIT,FASTPAY_REQUIRE_PRIMARY_SUCCESS,FASTPAY_ROUTE_RETRY_MS,FASTPAY_ROUTE_TIMEOUT_MS,FINALITY_METHODS,FIRST_READY_SEQUENCED_READ_PROPOSERS,INJECT_RPC_CAPS,LEGACY_A651_ETH_TOKEN,LEGACY_A651_UNISWAP_POOL_ID,LISTEN_PORT,MAX_TCP_PER_WS,MAX_WS_MESSAGE_BYTES,NATIVE_WALLET_SIGNER_BIN,NATIVE_WALLET_SIGNER_TIMEOUT_MS,NAVSWAP_CAPABILITIES_SCHEMA,NAVSWAP_DEVNET_FUNDING_SCHEMA,NAVSWAP_IDEMPOTENCY_STORE_DEFAULT_PATH,NAVSWAP_IDEMPOTENCY_STORE_SCHEMA,NAVSWAP_IDEMPOTENCY_TTL_MS,NAVSWAP_MAX_LIVE_USD,NAVSWAP_NAV_PROOF_SCHEMA,NAVSWAP_PRIMARY_MINT_ROUTE_FAMILY,NAVSWAP_QUOTE_FRESHNESS_TTL_MS,NAVSWAP_QUOTE_SCHEMA,NAVSWAP_READINESS_SCHEMA,NAVSWAP_ROUTE_TRUST_CLASSES,NAVSWAP_RUN_EVENTS_SCHEMA,NAVSWAP_RUN_LIST_SCHEMA,NAVSWAP_RUN_RECEIPTS_SCHEMA,NAVSWAP_RUN_SCHEMA,NAVSWAP_RUN_STATUS_SCHEMA,NAVSWAP_RUN_STORE_DEFAULT_PATH,NAVSWAP_RUN_STORE_SCHEMA,NAVSWAP_RUN_STREAM_EVENT_SCHEMA,NAVSWAP_RUN_STREAM_SCHEMA,NAVSWAP_SETTLEMENT_RECEIPT_MAX_SNAPSHOT_AGE_BLOCKS,NAVSWAP_SETTLEMENT_RECEIPT_SAFETY_BLOCKS,NAVSWAP_STAKEHUB_TRANSPARENT_ACTION,NAVSWAP_TRANSPARENT_PLANNER_INPUTS_SCHEMA,NAVSWAP_WALLET_ACTION_BATCH_PREPARE_SCHEMA,NAVSWAP_WALLET_ACTION_PREPARE_SCHEMA,NAVSWAP_WALLET_ACTION_SCHEMA,OPTIMISTIC_CACHED_FINALITY_ROUTE,PFUSDC_ASSET_ID,PREFERRED_SEQUENCED_READ_VALIDATORS,PROPOSER_READY_RETRY_MS,PROPOSER_ROUTE_CACHE_MS,PROPOSER_ROUTE_RETRY_MS,PROPOSER_ROUTE_TIMEOUT_MS,RPC_CAPS,RPC_FLEET,RPC_HOST,RPC_PORT,SEQUENCED_ACCOUNT_METHODS,SHIELDED_NAVSWAP_EGRESS_POLICY_ID,SHIELDED_NAVSWAP_EGRESS_SCHEMA,SHIELDED_NAVSWAP_INGRESS_SCHEMA,SHIELDED_NAVSWAP_LIQUIDITY_MODES,SHIELDED_NAVSWAP_PREFLIGHT_SCHEMA,SHIELDED_NAVSWAP_QUOTE_SCHEMA,SHIELDED_NAVSWAP_STATUS_SCHEMA,SHIELDED_NAVSWAP_SWAP_SCHEMA,SHIELDED_PRIVATE_KEY_PATTERNS,SHIELDED_ROUND_TIMEOUT_DEFAULT_MS,TCP_TIMEOUT_MS,UpstreamRpcConnection,VAULT_BRIDGE_ALLOCATION_PURPOSE_NAV_SUBSCRIPTION,VAULT_BRIDGE_ALLOCATION_PURPOSE_SUPPLY,VAULT_BRIDGE_BUCKET_STATUS_ACTIVE,VAULT_BRIDGE_RECEIPT_STATUS_COUNTED,VAULT_BRIDGE_RECIPIENT_SPONSOR_AMOUNT,VAULT_BRIDGE_RECIPIENT_SPONSOR_MIN_AMOUNT_ATOMS,VAULT_BRIDGE_RELAY_DEFAULT_ACCOUNT,VAULT_BRIDGE_RELAY_EXPIRES_AT_HEIGHT,VAULT_BRIDGE_RELAY_POLICY_HASH,VAULT_BRIDGE_RELAY_SCHEMA,VAULT_BRIDGE_RELAY_SOURCE_CHAIN_ID,VAULT_BRIDGE_RELAY_SOURCE_RPC_URL,VAULT_BRIDGE_RELAY_TOKEN_ADDRESS,VAULT_BRIDGE_RELAY_VAULT_ADDRESS,WALLET_SUBSCRIPTION_INTERVAL_MS,WALLET_SUBSCRIPTION_MIN_INTERVAL_MS,WALLET_SUBSCRIPTION_READ_TIMEOUT_MS,crypto,execFileAsync,fs,http,navswapDevnetFundingUsage,navswapIdempotencyRecords,navswapRunStreams,navswapRuns,net,os,path,server,upstreamRpcConnections,wss } = runtime;
    let { fastpayFleetStatusCache,fastpayFleetStatusInFlight,latestFinalizedReadCache,preferredSequencedReadIndex,proposerRouteCache,shieldedCertifierLoopState } = runtime;
    const addProxyRouteEvent = (...args) => runtime.addProxyRouteEvent(...args);
    const annotateNavswapIdempotency = (...args) => runtime.annotateNavswapIdempotency(...args);
    const assertNoShieldedPrivateMaterial = (...args) => runtime.assertNoShieldedPrivateMaterial(...args);
    const assertVaultBridgeEvidenceMatches = (...args) => runtime.assertVaultBridgeEvidenceMatches(...args);
    const assetOrchardLocalServiceConfig = (...args) => runtime.assetOrchardLocalServiceConfig(...args);
    const bftQuorumThreshold = (...args) => runtime.bftQuorumThreshold(...args);
    const broadcastFastpayMutation = (...args) => runtime.broadcastFastpayMutation(...args);
    const buildNavswapRunResponse = (...args) => runtime.buildNavswapRunResponse(...args);
    const buildShieldedCertifiedRoundArgs = (...args) => runtime.buildShieldedCertifiedRoundArgs(...args);
    const buildUniswapHandoffQuoteBinding = (...args) => runtime.buildUniswapHandoffQuoteBinding(...args);
    const buildVaultBridgeRelayBundle = (...args) => runtime.buildVaultBridgeRelayBundle(...args);
    const cachedSelection = (...args) => runtime.cachedSelection(...args);
    const canonicalReadResult = (...args) => runtime.canonicalReadResult(...args);
    const certifiedRoundFailure = (...args) => runtime.certifiedRoundFailure(...args);
    const certifiedRoundHasQuorumCertificate = (...args) => runtime.certifiedRoundHasQuorumCertificate(...args);
    const certifiedRoundHeight = (...args) => runtime.certifiedRoundHeight(...args);
    const certifiedRoundReceipts = (...args) => runtime.certifiedRoundReceipts(...args);
    const certifyShieldedBatchViaWarmLoop = (...args) => runtime.certifyShieldedBatchViaWarmLoop(...args);
    const chooseOwnedVoteEndpoint = (...args) => runtime.chooseOwnedVoteEndpoint(...args);
    const chooseProposerEndpointCached = (...args) => runtime.chooseProposerEndpointCached(...args);
    const chooseProposerEndpointFromStatuses = (...args) => runtime.chooseProposerEndpointFromStatuses(...args);
    const chooseProposerEndpointWithRetry = (...args) => runtime.chooseProposerEndpointWithRetry(...args);
    const chooseSequencedAccountReadEndpoint = (...args) => runtime.chooseSequencedAccountReadEndpoint(...args);
    const chooseShieldedCatchUpSource = (...args) => runtime.chooseShieldedCatchUpSource(...args);
    const clearFastpayFleetStatusCache = (...args) => runtime.clearFastpayFleetStatusCache(...args);
    const clearNavswapDevnetFundingUsageForTest = (...args) => runtime.clearNavswapDevnetFundingUsageForTest(...args);
    const clearNavswapIdempotencyForTest = (...args) => runtime.clearNavswapIdempotencyForTest(...args);
    const clearNavswapRunsForTest = (...args) => runtime.clearNavswapRunsForTest(...args);
    const cloneJson = (...args) => runtime.cloneJson(...args);
    const closeUpstreamRpcConnections = (...args) => runtime.closeUpstreamRpcConnections(...args);
    const collectFastpayFleetStatuses = (...args) => runtime.collectFastpayFleetStatuses(...args);
    const collectFleetStatuses = (...args) => runtime.collectFleetStatuses(...args);
    const collectShieldedTopologyStatuses = (...args) => runtime.collectShieldedTopologyStatuses(...args);
    const compareNavswapRunsNewestFirst = (...args) => runtime.compareNavswapRunsNewestFirst(...args);
    const conciseRpcError = (...args) => runtime.conciseRpcError(...args);
    const convergedFleetGroup = (...args) => runtime.convergedFleetGroup(...args);
    const createNavswapRun = (...args) => runtime.createNavswapRun(...args);
    const createShieldedSwapBatchViaLocalService = (...args) => runtime.createShieldedSwapBatchViaLocalService(...args);
    const currentA652AssetId = (...args) => runtime.currentA652AssetId(...args);
    const deterministicProposer = (...args) => runtime.deterministicProposer(...args);
    const endpointStatusMeetsRoute = (...args) => runtime.endpointStatusMeetsRoute(...args);
    const endpointStatusMeetsSequencedReadRoute = (...args) => runtime.endpointStatusMeetsSequencedReadRoute(...args);
    const ensureVaultBridgeRecipientAccount = (...args) => runtime.ensureVaultBridgeRecipientAccount(...args);
    const executeNavswapAtomicTemplate = (...args) => runtime.executeNavswapAtomicTemplate(...args);
    const executeNavswapCapabilities = (...args) => runtime.executeNavswapCapabilities(...args);
    const executeNavswapIdempotentRequest = (...args) => runtime.executeNavswapIdempotentRequest(...args);
    const executeNavswapRun = (...args) => runtime.executeNavswapRun(...args);
    const executeShieldedNavswapBalances = (...args) => runtime.executeShieldedNavswapBalances(...args);
    const executeShieldedNavswapEgress = (...args) => runtime.executeShieldedNavswapEgress(...args);
    const executeShieldedNavswapIngress = (...args) => runtime.executeShieldedNavswapIngress(...args);
    const executeShieldedNavswapIngressPreflight = (...args) => runtime.executeShieldedNavswapIngressPreflight(...args);
    const executeShieldedNavswapNoteCapability = (...args) => runtime.executeShieldedNavswapNoteCapability(...args);
    const executeShieldedNavswapProverReadiness = (...args) => runtime.executeShieldedNavswapProverReadiness(...args);
    const executeShieldedNavswapQuote = (...args) => runtime.executeShieldedNavswapQuote(...args);
    const executeShieldedNavswapStatus = (...args) => runtime.executeShieldedNavswapStatus(...args);
    const executeShieldedNavswapSwap = (...args) => runtime.executeShieldedNavswapSwap(...args);
    const executeVaultBridgeRelay = (...args) => runtime.executeVaultBridgeRelay(...args);
    const fetchWalletSnapshot = (...args) => runtime.fetchWalletSnapshot(...args);
    const fileMtimeUnixMs = (...args) => runtime.fileMtimeUnixMs(...args);
    const findAssetOrchardActionCleartext = (...args) => runtime.findAssetOrchardActionCleartext(...args);
    const findShieldedPrivateMaterialPaths = (...args) => runtime.findShieldedPrivateMaterialPaths(...args);
    const finishNavswapRun = (...args) => runtime.finishNavswapRun(...args);
    const firstReadyEndpointForRoute = (...args) => runtime.firstReadyEndpointForRoute(...args);
    const firstStructuredFastpayResult = (...args) => runtime.firstStructuredFastpayResult(...args);
    const forwardStakehubTransparentRun = (...args) => runtime.forwardStakehubTransparentRun(...args);
    const handleNavswapHttp = (...args) => runtime.handleNavswapHttp(...args);
    const invalidateProposerRouteCache = (...args) => runtime.invalidateProposerRouteCache(...args);
    const isBadSequenceSubmitResponse = (...args) => runtime.isBadSequenceSubmitResponse(...args);
    const isFastpayBroadcastMethod = (...args) => runtime.isFastpayBroadcastMethod(...args);
    const isFinalityMethod = (...args) => runtime.isFinalityMethod(...args);
    const isNativeWalletSignMethod = (...args) => runtime.isNativeWalletSignMethod(...args);
    const isReplayableVaultBridgeRelayDuplicate = (...args) => runtime.isReplayableVaultBridgeRelayDuplicate(...args);
    const isSequencedAccountMethod = (...args) => runtime.isSequencedAccountMethod(...args);
    const jsonHeaders = (...args) => runtime.jsonHeaders(...args);
    const loadNavswapIdempotencyStore = (...args) => runtime.loadNavswapIdempotencyStore(...args);
    const loadNavswapRunStore = (...args) => runtime.loadNavswapRunStore(...args);
    const loadShieldedTopologyPeers = (...args) => runtime.loadShieldedTopologyPeers(...args);
    const lower = (...args) => runtime.lower(...args);
    const majorityRootAtHeight = (...args) => runtime.majorityRootAtHeight(...args);
    const markStoredNavswapRunInterrupted = (...args) => runtime.markStoredNavswapRunInterrupted(...args);
    const maxMtimeUnixMs = (...args) => runtime.maxMtimeUnixMs(...args);
    const msSpan = (...args) => runtime.msSpan(...args);
    const navswapAsyncRunRequested = (...args) => runtime.navswapAsyncRunRequested(...args);
    const navswapBridgeConfig = (...args) => runtime.navswapBridgeConfig(...args);
    const navswapCapabilities = (...args) => runtime.navswapCapabilities(...args);
    const navswapDevnetFundingUsageSnapshot = (...args) => runtime.navswapDevnetFundingUsageSnapshot(...args);
    const navswapDevnetFundingWindowUsage = (...args) => runtime.navswapDevnetFundingWindowUsage(...args);
    const navswapDevnetPfusdcFundingConfig = (...args) => runtime.navswapDevnetPfusdcFundingConfig(...args);
    const navswapIdempotencyHashBody = (...args) => runtime.navswapIdempotencyHashBody(...args);
    const navswapIdempotencyKeyFromRequest = (...args) => runtime.navswapIdempotencyKeyFromRequest(...args);
    const navswapIdempotencyStorePath = (...args) => runtime.navswapIdempotencyStorePath(...args);
    const navswapIdempotencyStoreSnapshot = (...args) => runtime.navswapIdempotencyStoreSnapshot(...args);
    const navswapInferTrustClass = (...args) => runtime.navswapInferTrustClass(...args);
    const navswapListLimit = (...args) => runtime.navswapListLimit(...args);
    const navswapNormalizeTrustClass = (...args) => runtime.navswapNormalizeTrustClass(...args);
    const navswapRoutePrivacy = (...args) => runtime.navswapRoutePrivacy(...args);
    const navswapRunEvents = (...args) => runtime.navswapRunEvents(...args);
    const navswapRunIsTerminal = (...args) => runtime.navswapRunIsTerminal(...args);
    const navswapRunList = (...args) => runtime.navswapRunList(...args);
    const navswapRunPublic = (...args) => runtime.navswapRunPublic(...args);
    const navswapRunReceipts = (...args) => runtime.navswapRunReceipts(...args);
    const navswapRunSortTime = (...args) => runtime.navswapRunSortTime(...args);
    const navswapRunStorePath = (...args) => runtime.navswapRunStorePath(...args);
    const navswapRunStoreSnapshot = (...args) => runtime.navswapRunStoreSnapshot(...args);
    const navswapRunStreamSnapshot = (...args) => runtime.navswapRunStreamSnapshot(...args);
    const navswapStableJson = (...args) => runtime.navswapStableJson(...args);
    const navswapStakehubTransparentConfig = (...args) => runtime.navswapStakehubTransparentConfig(...args);
    const navswapTransparentOperatorConfig = (...args) => runtime.navswapTransparentOperatorConfig(...args);
    const navswapTrustlessFinalityAgreement = (...args) => runtime.navswapTrustlessFinalityAgreement(...args);
    const navswapTruthyParam = (...args) => runtime.navswapTruthyParam(...args);
    const navswapUniswapBetaRouteState = (...args) => runtime.navswapUniswapBetaRouteState(...args);
    const navswapValidateIdempotencyKey = (...args) => runtime.navswapValidateIdempotencyKey(...args);
    const newNavswapRunId = (...args) => runtime.newNavswapRunId(...args);
    const normalizeAtomicTemplateParams = (...args) => runtime.normalizeAtomicTemplateParams(...args);
    const normalizeFastpayBroadcastRequest = (...args) => runtime.normalizeFastpayBroadcastRequest(...args);
    const normalizeShieldedKey = (...args) => runtime.normalizeShieldedKey(...args);
    const normalizeShieldedLiquidityMode = (...args) => runtime.normalizeShieldedLiquidityMode(...args);
    const normalizeStoredNavswapIdempotencyRecord = (...args) => runtime.normalizeStoredNavswapIdempotencyRecord(...args);
    const normalizeStoredNavswapRun = (...args) => runtime.normalizeStoredNavswapRun(...args);
    const normalizeVaultBridgeAddress = (...args) => runtime.normalizeVaultBridgeAddress(...args);
    const normalizeVaultBridgeBytes32 = (...args) => runtime.normalizeVaultBridgeBytes32(...args);
    const normalizeVaultBridgeTxHash = (...args) => runtime.normalizeVaultBridgeTxHash(...args);
    const normalizeWalletSubscriptionParams = (...args) => runtime.normalizeWalletSubscriptionParams(...args);
    const originAllowed = (...args) => runtime.originAllowed(...args);
    const parseRpcFleet = (...args) => runtime.parseRpcFleet(...args);
    const parseShieldedPrivateEgressJson = (...args) => runtime.parseShieldedPrivateEgressJson(...args);
    const parseShieldedSwapActionJson = (...args) => runtime.parseShieldedSwapActionJson(...args);
    const parseUniswapHandoffBytes32 = (...args) => runtime.parseUniswapHandoffBytes32(...args);
    const parseUniswapHandoffPositiveInteger = (...args) => runtime.parseUniswapHandoffPositiveInteger(...args);
    const persistNavswapIdempotencyRecord = (...args) => runtime.persistNavswapIdempotencyRecord(...args);
    const persistNavswapRun = (...args) => runtime.persistNavswapRun(...args);
    const preferredSequencedReadEndpoint = (...args) => runtime.preferredSequencedReadEndpoint(...args);
    const presentEnv = (...args) => runtime.presentEnv(...args);
    const presentPositiveSafeIntegerEnv = (...args) => runtime.presentPositiveSafeIntegerEnv(...args);
    const primeNextProposerRouteCache = (...args) => runtime.primeNextProposerRouteCache(...args);
    const primeNextProposerRouteCacheFromResponse = (...args) => runtime.primeNextProposerRouteCacheFromResponse(...args);
    const proposerEndpointForHeight = (...args) => runtime.proposerEndpointForHeight(...args);
    const pruneNavswapIdempotencyRecords = (...args) => runtime.pruneNavswapIdempotencyRecords(...args);
    const publishNavswapRunUpdate = (...args) => runtime.publishNavswapRunUpdate(...args);
    const readFleetRpcMajority = (...args) => runtime.readFleetRpcMajority(...args);
    const readGroupKey = (...args) => runtime.readGroupKey(...args);
    const readJsonBody = (...args) => runtime.readJsonBody(...args);
    const readNavswapKeyFileAddress = (...args) => runtime.readNavswapKeyFileAddress(...args);
    const recordNavswapRunEvent = (...args) => runtime.recordNavswapRunEvent(...args);
    const releaseNavswapDevnetFundingUsage = (...args) => runtime.releaseNavswapDevnetFundingUsage(...args);
    const rememberFinalizedReadEndpoint = (...args) => runtime.rememberFinalizedReadEndpoint(...args);
    const removeNavswapRunStreamSubscriber = (...args) => runtime.removeNavswapRunStreamSubscriber(...args);
    const requestWithProxyReadiness = (...args) => runtime.requestWithProxyReadiness(...args);
    const reserveNavswapDevnetFundingUsage = (...args) => runtime.reserveNavswapDevnetFundingUsage(...args);
    const resolveRpcTarget = (...args) => runtime.resolveRpcTarget(...args);
    const responseEnvelope = (...args) => runtime.responseEnvelope(...args);
    const routedRpcRead = (...args) => runtime.routedRpcRead(...args);
    const rpcTcpRequest = (...args) => runtime.rpcTcpRequest(...args);
    const rpcTcpRequestLine = (...args) => runtime.rpcTcpRequestLine(...args);
    const rpcTcpRequestOneShotLine = (...args) => runtime.rpcTcpRequestOneShotLine(...args);
    const runShieldedLaggardCatchUp = (...args) => runtime.runShieldedLaggardCatchUp(...args);
    const runShieldedRpcCatchUp = (...args) => runtime.runShieldedRpcCatchUp(...args);
    const sanitizeNavswapRunRequest = (...args) => runtime.sanitizeNavswapRunRequest(...args);
    const sendJson = (...args) => runtime.sendJson(...args);
    const sendNavswapRunStream = (...args) => runtime.sendNavswapRunStream(...args);
    const sendWalletNotification = (...args) => runtime.sendWalletNotification(...args);
    const shellQuote = (...args) => runtime.shellQuote(...args);
    const shieldedBatchExplicitActionIds = (...args) => runtime.shieldedBatchExplicitActionIds(...args);
    const shieldedCatchUpLaggards = (...args) => runtime.shieldedCatchUpLaggards(...args);
    const shieldedCatchUpSourceCandidates = (...args) => runtime.shieldedCatchUpSourceCandidates(...args);
    const shieldedCertifiedRoundEnv = (...args) => runtime.shieldedCertifiedRoundEnv(...args);
    const shieldedCertifierLoopBatchFile = (...args) => runtime.shieldedCertifierLoopBatchFile(...args);
    const shieldedCertifierLoopStartHeight = (...args) => runtime.shieldedCertifierLoopStartHeight(...args);
    const shieldedConvergenceSummary = (...args) => runtime.shieldedConvergenceSummary(...args);
    const shieldedEarlyQuorumEnabled = (...args) => runtime.shieldedEarlyQuorumEnabled(...args);
    const shieldedIngressSupportedAsset = (...args) => runtime.shieldedIngressSupportedAsset(...args);
    const shieldedLaggardCatchUpConfig = (...args) => runtime.shieldedLaggardCatchUpConfig(...args);
    const shieldedLiquidityModeLabel = (...args) => runtime.shieldedLiquidityModeLabel(...args);
    const shieldedNavswapEgressConfig = (...args) => runtime.shieldedNavswapEgressConfig(...args);
    const shieldedNavswapIngressConfig = (...args) => runtime.shieldedNavswapIngressConfig(...args);
    const shieldedNavswapQuoteConfig = (...args) => runtime.shieldedNavswapQuoteConfig(...args);
    const shieldedNavswapSwapConfig = (...args) => runtime.shieldedNavswapSwapConfig(...args);
    const shieldedPrivateEgressDisclosureFields = (...args) => runtime.shieldedPrivateEgressDisclosureFields(...args);
    const shieldedPrivateEgressDisclosureHash = (...args) => runtime.shieldedPrivateEgressDisclosureHash(...args);
    const shieldedQuoteAssetByInput = (...args) => runtime.shieldedQuoteAssetByInput(...args);
    const shieldedQuoteFromSubmitBody = (...args) => runtime.shieldedQuoteFromSubmitBody(...args);
    const shieldedQuotePairEnabled = (...args) => runtime.shieldedQuotePairEnabled(...args);
    const shieldedQuotePolicyHash = (...args) => runtime.shieldedQuotePolicyHash(...args);
    const shieldedRemoteDataDir = (...args) => runtime.shieldedRemoteDataDir(...args);
    const shieldedRemoteWorkDir = (...args) => runtime.shieldedRemoteWorkDir(...args);
    const shieldedRoundBatchIds = (...args) => runtime.shieldedRoundBatchIds(...args);
    const shieldedRoundPhaseTimings = (...args) => runtime.shieldedRoundPhaseTimings(...args);
    const shieldedRoundReceiptIds = (...args) => runtime.shieldedRoundReceiptIds(...args);
    const shieldedSwapProxyTimingReport = (...args) => runtime.shieldedSwapProxyTimingReport(...args);
    const shouldUseFirstReadySequencedRead = (...args) => runtime.shouldUseFirstReadySequencedRead(...args);
    const signAndSubmitVaultBridgeRecipientSponsor = (...args) => runtime.signAndSubmitVaultBridgeRecipientSponsor(...args);
    const signAndSubmitVaultBridgeRelayOperation = (...args) => runtime.signAndSubmitVaultBridgeRelayOperation(...args);
    const signWalletOwnedOrder = (...args) => runtime.signWalletOwnedOrder(...args);
    const sleep = (...args) => runtime.sleep(...args);
    const sseHeaders = (...args) => runtime.sseHeaders(...args);
    const startCachedSelectionReadinessProbe = (...args) => runtime.startCachedSelectionReadinessProbe(...args);
    const startShieldedCertifierLoop = (...args) => runtime.startShieldedCertifierLoop(...args);
    const startWalletSubscription = (...args) => runtime.startWalletSubscription(...args);
    const stopWalletSubscription = (...args) => runtime.stopWalletSubscription(...args);
    const swapAtomicTemplateParams = (...args) => runtime.swapAtomicTemplateParams(...args);
    const upstreamEndpointKey = (...args) => runtime.upstreamEndpointKey(...args);
    const upstreamRpcConnection = (...args) => runtime.upstreamRpcConnection(...args);
    const validateShieldedCertifierLoopReportForBatch = (...args) => runtime.validateShieldedCertifierLoopReportForBatch(...args);
    const validateShieldedEgressSubmit = (...args) => runtime.validateShieldedEgressSubmit(...args);
    const validateShieldedIngressPayload = (...args) => runtime.validateShieldedIngressPayload(...args);
    const validateShieldedPrivateEgressFile = (...args) => runtime.validateShieldedPrivateEgressFile(...args);
    const validateShieldedSwapAction = (...args) => runtime.validateShieldedSwapAction(...args);
    const validateShieldedSwapSubmit = (...args) => runtime.validateShieldedSwapSubmit(...args);
    const vaultBridgeAccountAssets = (...args) => runtime.vaultBridgeAccountAssets(...args);
    const vaultBridgeBodyTxHash = (...args) => runtime.vaultBridgeBodyTxHash(...args);
    const vaultBridgeEvidenceFromPlan = (...args) => runtime.vaultBridgeEvidenceFromPlan(...args);
    const vaultBridgeExpectedField = (...args) => runtime.vaultBridgeExpectedField(...args);
    const vaultBridgePftlAccountExists = (...args) => runtime.vaultBridgePftlAccountExists(...args);
    const vaultBridgeRelayConfig = (...args) => runtime.vaultBridgeRelayConfig(...args);
    const verifyAtomicTemplateResult = (...args) => runtime.verifyAtomicTemplateResult(...args);
    const verifyAtomicTemplateSymmetry = (...args) => runtime.verifyAtomicTemplateSymmetry(...args);
    const waitForCachedSelectionReady = (...args) => runtime.waitForCachedSelectionReady(...args);
    const waitForFastpayConvergedGroup = (...args) => runtime.waitForFastpayConvergedGroup(...args);
    const walletSnapshotDigest = (...args) => runtime.walletSnapshotDigest(...args);
    const writeSseEvent = (...args) => runtime.writeSseEvent(...args);

    function assetIdForNavswapSymbol(value) {
        if (value === 'PFT') return 'PFT';
        if (value === 'pfUSDC') return PFUSDC_ASSET_ID;
        if (value === 'a651') return A651_ASSET_ID;
        return value;
    }

    function navswapWalletActionId(parts) {
        const hash = crypto.createHash('sha256')
            .update(JSON.stringify(parts))
            .digest('hex');
        return `navswap-action-${hash.slice(0, 24)}`;
    }

    function navswapRandomHex(bytes) {
        return crypto.randomBytes(bytes).toString('hex');
    }

    function navswapHashHexDomain(domain, preimage) {
        return crypto.createHash('sha3-384')
            .update(domain)
            .update(Buffer.from([0]))
            .update(preimage)
            .digest('hex');
    }

    function navswapNavRedemptionId(chainId, owner, assetId, ownerSequence) {
        const sequence = parseNavswapActionInteger(ownerSequence, 'owner_sequence');
        const ownerAddress = parseNavswapWalletAddress(owner);
        const navAssetId = parseNavswapHexId(assetId, 'nav_redemption.asset_id');
        const chain = String(chainId || '').trim();
        if (!chain) {
            const err = new Error('chain_id is required to derive NAV redemption id');
            err.code = 'transparent_navswap_redemption_chain_id_missing';
            throw err;
        }
        const preimage = `chain_id=${chain}\nowner=${ownerAddress}\nasset_id=${navAssetId}\nowner_sequence=${sequence}\n`;
        return navswapHashHexDomain('postfiat.nav_redemption_id.v1', preimage);
    }

    function navswapSettlementReceiptHash(fields = {}) {
        const preimage = Object.entries(fields)
            .filter(([, value]) => value !== undefined && value !== null)
            .sort(([left], [right]) => left.localeCompare(right))
            .map(([key, value]) => `${key}=${value}\n`)
            .join('');
        return navswapHashHexDomain('postfiat.navswap.operator_settlement_receipt.v1', preimage);
    }

    function navswapSubscriptionId(value = '') {
        const text = String(value || '').trim();
        if (!text) {
            return `navsub-${Date.now().toString(36)}-${crypto.randomBytes(8).toString('hex')}`;
        }
        if (!/^[A-Za-z0-9._-]{1,96}$/.test(text)) {
            const err = new Error('subscription_id must be 1-96 characters using letters, numbers, dot, underscore, or dash');
            err.code = 'navswap_invalid_subscription_id';
            throw err;
        }
        return text;
    }

    function parseNavswapWalletAddress(value) {
        const address = String(value || '').trim();
        if (!/^pf[0-9a-f]{40}$/.test(address)) {
            const err = new Error('wallet_address must be a lowercase PostFiat account address');
            err.code = 'invalid_navswap_wallet_address';
            throw err;
        }
        return address;
    }

    function parseNavswapActionInteger(value, field) {
        const text = String(value ?? '').trim();
        if (!/^[1-9][0-9]*$/.test(text)) {
            const err = new Error(`${field} must be a positive whole number of atoms`);
            err.code = 'invalid_navswap_action_integer';
            throw err;
        }
        const parsed = Number.parseInt(text, 10);
        if (!Number.isSafeInteger(parsed)) {
            const err = new Error(`${field} exceeds the wallet adapter safe integer range`);
            err.code = 'invalid_navswap_action_integer';
            throw err;
        }
        return parsed;
    }

    function parseNavswapHexId(value, field, length = 96) {
        const text = String(value ?? '').trim().toLowerCase();
        if (!new RegExp(`^[0-9a-f]{${length}}$`).test(text)) {
            const err = new Error(`${field} must be ${length} lowercase hex characters`);
            err.code = 'invalid_navswap_hex_id';
            throw err;
        }
        return text;
    }

    function parseNavswapEvmAddress(value, field) {
        const text = String(value ?? '').trim();
        if (!/^0x[0-9a-fA-F]{40}$/.test(text)) {
            const err = new Error(`${field} must be a 20-byte 0x-prefixed Ethereum address`);
            err.code = 'invalid_navswap_evm_address';
            throw err;
        }
        return text;
    }

    function navswapAssetInfoIssuer(result) {
        const asset = result?.asset || result?.asset_definition || result?.definition || result;
        return asset?.issuer || asset?.owner || null;
    }

    async function navswapAssetIssuer(assetId, rpcRequest) {
        const rpcResponse = await rpcRequest(RPC_HOST, RPC_PORT, {
            version: 'postfiat-local-rpc-v1',
            id: `navswap-action-${Date.now()}`,
            method: 'asset_info',
            params: { asset_id: assetId },
        });
        if (rpcResponse.ok !== true) {
            const err = new Error(rpcResponse.error?.message || 'asset_info RPC failed while preparing NAVSwap action.');
            err.code = rpcResponse.error?.code || 'navswap_asset_info_failed';
            err.rpc_error = rpcResponse.error || null;
            throw err;
        }
        const issuer = navswapAssetInfoIssuer(rpcResponse.result);
        if (!issuer) {
            const err = new Error('asset_info did not return an issuer for the requested NAVSwap asset.');
            err.code = 'navswap_asset_issuer_missing';
            err.asset_info = rpcResponse.result;
            throw err;
        }
        return { issuer, asset_info: rpcResponse.result };
    }

    function navswapActionPrepareError(error, route, stage, extra = {}) {
        return {
            ok: false,
            schema: NAVSWAP_WALLET_ACTION_PREPARE_SCHEMA,
            route,
            stage,
            code: error.code || 'navswap_wallet_action_prepare_failed',
            message: error.message || 'NAVSwap wallet action preparation failed.',
            ...(error.rpc_error ? { rpc_error: error.rpc_error } : {}),
            ...(error.asset_info ? { asset_info: error.asset_info } : {}),
            ...extra,
        };
    }

    function navswapFreshnessFromBody(body = {}) {
        const source = body.navswap_freshness || body.freshness || body.quote_freshness || {};
        const generatedAtMs = source.quote_generated_at_ms
            || body.quote_generated_at_ms
            || source.generated_at_ms
            || null;
        const expiresAtMs = source.quote_expires_at_ms
            || body.quote_expires_at_ms
            || source.expires_at_ms
            || null;
        const reservePacketFresh = source.reserve_packet_fresh ?? body.reserve_packet_fresh;
        const supplyPacketFresh = source.supply_packet_fresh ?? body.supply_packet_fresh;
        const proofStatus = source.proof_status || body.proof_status || null;
        const fields = {};
        if (generatedAtMs !== null && generatedAtMs !== undefined) fields.quote_generated_at_ms = String(generatedAtMs);
        if (expiresAtMs !== null && expiresAtMs !== undefined) fields.quote_expires_at_ms = String(expiresAtMs);
        if (reservePacketFresh !== undefined) fields.reserve_packet_fresh = reservePacketFresh === true || reservePacketFresh === 'true';
        if (supplyPacketFresh !== undefined) fields.supply_packet_fresh = supplyPacketFresh === true || supplyPacketFresh === 'true';
        if (proofStatus) fields.proof_status = String(proofStatus);
        if (source.market_ops_status || body.market_ops_status) fields.market_ops_status = source.market_ops_status || body.market_ops_status;
        if (source.market_ops_envelope_epoch || body.market_ops_envelope_epoch || source.envelope_epoch || body.envelope_epoch) {
            fields.market_ops_envelope_epoch = String(
                source.market_ops_envelope_epoch
                || body.market_ops_envelope_epoch
                || source.envelope_epoch
                || body.envelope_epoch,
            );
        }
        if (source.nav_epoch || body.nav_epoch) fields.nav_epoch = String(source.nav_epoch || body.nav_epoch);
        if (source.reserve_packet_hash || body.reserve_packet_hash || body.nav_reserve_packet_hash) {
            fields.reserve_packet_hash = String(source.reserve_packet_hash || body.reserve_packet_hash || body.nav_reserve_packet_hash);
            fields.nav_reserve_packet_hash = fields.reserve_packet_hash;
        }
        return fields;
    }

    function navswapPrimaryMintIntentFields(body = {}) {
        const routeFamily = String(body.route_family || body.purchase_kind || NAVSWAP_PRIMARY_MINT_ROUTE_FAMILY).trim();
        if (
            routeFamily !== NAVSWAP_PRIMARY_MINT_ROUTE_FAMILY
            && routeFamily !== 'composite_primary_mint_to_ethereum_venue'
        ) {
            const err = new Error('NAV subscription route_family must be primary_pftl_mint or composite_primary_mint_to_ethereum_venue.');
            err.code = 'navswap_primary_mint_route_family_invalid';
            throw err;
        }
        const trustClass = navswapNormalizeTrustClass(
            body.route_trust_class || body.trust_class,
            'CONTROLLED',
        );
        const mintAmountAtoms = parseNavswapActionInteger(
            body.mint_amount_atoms || body.nav_amount_atoms || body.expected_output_atoms,
            'mint_amount_atoms',
        );
        const pricingNavEpoch = parseNavswapActionInteger(
            body.pricing_nav_epoch || body.nav_epoch || body.epoch,
            'pricing_nav_epoch',
        );
        const primaryNavPriceAtoms = parseNavswapActionInteger(
            body.primary_nav_price_atoms || body.nav_per_unit,
            'primary_nav_price_atoms',
        );
        const reservePacketHash = parseNavswapHexId(
            body.pricing_reserve_packet_hash || body.reserve_packet_hash || body.nav_reserve_packet_hash,
            'pricing_reserve_packet_hash',
        );
        const fields = {
            route_family: routeFamily,
            purchase_kind: routeFamily,
            route_trust_class: trustClass,
            supply_effect: 'mints_new_native_navcoin_supply',
            pricing_source: 'finalized_pre_inflow_nav_snapshot',
            settlement_reserve_effect: 'added_after_primary_fill',
            uniswap_supply_effect: 'not_uniswap_supply',
            mint_amount_atoms: String(mintAmountAtoms),
            pricing_nav_epoch: String(pricingNavEpoch),
            primary_nav_price_atoms: String(primaryNavPriceAtoms),
            pricing_reserve_packet_hash: reservePacketHash,
            nav_epoch: String(pricingNavEpoch),
            nav_per_unit: String(primaryNavPriceAtoms),
            reserve_packet_hash: reservePacketHash,
            nav_reserve_packet_hash: reservePacketHash,
        };
        if (routeFamily === 'composite_primary_mint_to_ethereum_venue') {
            fields.bridge_packet_effect = 'minted_navcoin_exported_or_claimed';
            fields.ethereum_supply_effect = 'mints_wrapped_venue_token_from_pftl_packet';
        }
        return fields;
    }

    function navswapFreshnessPayload({ navStatus = {}, marketStatus = {} } = {}) {
        const generatedAtMs = Date.now();
        const expiresAtMs = generatedAtMs + Math.max(1000, NAVSWAP_QUOTE_FRESHNESS_TTL_MS);
        return {
            quote_generated_at_ms: String(generatedAtMs),
            quote_expires_at_ms: String(expiresAtMs),
            proof_status: marketStatus.market_operations_status || 'active',
            market_ops_status: marketStatus.market_operations_status || null,
            market_ops_envelope_epoch: marketStatus.envelope_epoch === undefined || marketStatus.envelope_epoch === null
                ? null
                : String(marketStatus.envelope_epoch),
            reserve_packet_fresh: marketStatus.reserve_packet_fresh !== false,
            supply_packet_fresh: marketStatus.supply_packet_fresh !== false,
            nav_epoch: navStatus.finalized_epoch === undefined || navStatus.finalized_epoch === null
                ? null
                : String(navStatus.finalized_epoch),
            reserve_packet_hash: navStatus.finalized_reserve_packet_hash || null,
            nav_reserve_packet_hash: navStatus.finalized_reserve_packet_hash || null,
        };
    }

    async function prepareNavswapWalletAction(body = {}, rpcRequest = rpcTcpRequest) {
        const route = navswapRouteFromBody(body);
        if (route === 'uniswap_atomic_handoff') {
            return {
                ok: false,
                schema: NAVSWAP_WALLET_ACTION_PREPARE_SCHEMA,
                route,
                code: 'pftl_uniswap_requires_batch_prepare',
                message: 'PFTL-Uniswap source signing requires the primary subscribe and export debit actions in one reviewed batch.',
                supported_endpoint: '/api/navswap/actions/prepare-batch',
                supported_stages: ['pftl_uniswap_primary_subscribe', 'pftl_uniswap_export_debit'],
            };
        }
        if (route !== 'transparent_navswap') {
            return {
                ok: false,
                schema: NAVSWAP_WALLET_ACTION_PREPARE_SCHEMA,
                route,
                code: 'unsupported_navswap_action_route',
                message: 'Wallet action preparation is currently available only for transparent_navswap.',
            };
        }
        const stage = String(body.stage || body.action || '').trim();
        if (stage === 'trust_set') {
            return {
                ok: false,
                schema: NAVSWAP_WALLET_ACTION_PREPARE_SCHEMA,
                route,
                stage,
                code: 'transparent_navswap_trust_set_not_supported',
                message: 'Transparent NAVSwap no longer prepares trust_set actions; incoming issued credits are implicit.',
                rejected_stage: 'trust_set',
                supported_stages: ['nav_subscription_allocate', 'nav_redeem_at_nav'],
            };
        }
        if (stage === 'nav_subscription_allocate' || stage === 'vault_bridge_nav_subscription_allocate') {
            return prepareNavswapWalletNavSubscriptionAllocateAction(body, route, 'nav_subscription_allocate', rpcRequest);
        }
        if (stage === 'nav_redeem_at_nav') {
            return prepareNavswapWalletNavRedeemAtNavAction(body, route, stage, rpcRequest);
        }
        return {
            ok: false,
            schema: NAVSWAP_WALLET_ACTION_PREPARE_SCHEMA,
            route,
            stage,
            code: 'unsupported_navswap_wallet_action_stage',
            message: 'Only nav_subscription_allocate and nav_redeem_at_nav action preparation are wired.',
            supported_stages: ['nav_subscription_allocate', 'nav_redeem_at_nav'],
        };
    }

    function navswapWalletActionBatchItems(body = {}) {
        const items = Array.isArray(body.actions)
            ? body.actions
            : Array.isArray(body.prepared_actions)
                ? body.prepared_actions
                : Array.isArray(body.stages)
                    ? body.stages
                    : null;
        if (!items || items.length === 0) {
            const err = new Error('NAVSwap wallet action batch preparation requires a non-empty actions array.');
            err.code = 'navswap_wallet_action_batch_empty';
            throw err;
        }
        if (items.length > 16) {
            const err = new Error('NAVSwap wallet action batch preparation supports at most 16 actions.');
            err.code = 'navswap_wallet_action_batch_too_large';
            throw err;
        }
        return items;
    }

    async function prepareNavswapWalletActionBatch(body = {}, rpcRequest = rpcTcpRequest) {
        const route = navswapRouteFromBody(body);
        if (route === 'uniswap_atomic_handoff') {
            return preparePftlUniswapWalletActionBatch(body, rpcRequest);
        }
        if (route !== 'transparent_navswap') {
            return {
                ok: false,
                schema: NAVSWAP_WALLET_ACTION_BATCH_PREPARE_SCHEMA,
                route,
                code: 'unsupported_navswap_action_route',
                message: 'Wallet action batch preparation is currently available only for transparent_navswap.',
            };
        }
        let items;
        try {
            items = navswapWalletActionBatchItems(body);
        } catch (error) {
            return {
                ok: false,
                schema: NAVSWAP_WALLET_ACTION_BATCH_PREPARE_SCHEMA,
                route,
                code: error.code || 'navswap_wallet_action_batch_invalid',
                message: error.message,
            };
        }

        const base = {
            route,
            wallet_address: body.wallet_address || body.owner || body.source,
            owner: body.owner,
            source: body.source,
            from_asset: body.from_asset,
            to_asset: body.to_asset,
            nav_asset_id: body.nav_asset_id,
            settlement_asset_id: body.settlement_asset_id,
        };
        const actions = [];
        const preparations = [];
        for (let index = 0; index < items.length; index += 1) {
            const item = items[index];
            if (!item || typeof item !== 'object' || Array.isArray(item)) {
                return {
                    ok: false,
                    schema: NAVSWAP_WALLET_ACTION_BATCH_PREPARE_SCHEMA,
                    route,
                    code: 'navswap_wallet_action_batch_item_invalid',
                    message: `NAVSwap wallet action batch item ${index} must be an object.`,
                    failed_index: index,
                };
            }
            const prepared = await prepareNavswapWalletAction({ ...base, ...item, route }, rpcRequest);
            preparations.push(prepared);
            if (prepared.ok !== true || !prepared.action) {
                return {
                    ok: false,
                    schema: NAVSWAP_WALLET_ACTION_BATCH_PREPARE_SCHEMA,
                    route,
                    code: prepared.code || 'navswap_wallet_action_batch_prepare_failed',
                    message: prepared.message || `NAVSwap wallet action batch item ${index} failed to prepare.`,
                    failed_index: index,
                    failed_stage: item.stage || item.action || null,
                    failure: prepared,
                    prepared_count: actions.length,
                };
            }
            actions.push(prepared.action);
        }

        return {
            ok: true,
            schema: NAVSWAP_WALLET_ACTION_BATCH_PREPARE_SCHEMA,
            route,
            action_schema: NAVSWAP_WALLET_ACTION_SCHEMA,
            action_count: actions.length,
            stages: actions.map(action => action.stage),
            actions,
            preparations,
        };
    }

    async function navswapRpcRead(method, params = {}, rpcRequest = rpcTcpRequest) {
        const rpcResponse = await rpcRequest(RPC_HOST, RPC_PORT, {
            version: 'postfiat-local-rpc-v1',
            id: `navswap-${method}-${Date.now()}`,
            method,
            params,
        });
        if (rpcResponse.ok !== true) {
            const err = new Error(rpcResponse.error?.message || `${method} RPC failed while planning NAVSwap inputs.`);
            err.code = rpcResponse.error?.code || `navswap_${method}_failed`;
            err.rpc_error = rpcResponse.error || null;
            if (
                method === 'market_ops_status'
                && typeof err.message === 'string'
                && err.message.includes('missing finalized market ops envelope')
            ) {
                err.code = 'transparent_navswap_market_ops_envelope_missing';
                err.asset_id = params?.asset_id || null;
            }
            throw err;
        }
        return rpcResponse.result;
    }

    function navswapPlannerError(error, route = 'transparent_navswap', extra = {}) {
        return {
            ok: false,
            schema: NAVSWAP_TRANSPARENT_PLANNER_INPUTS_SCHEMA,
            route,
            code: error.code || 'transparent_navswap_planner_input_selection_failed',
            message: error.message || 'Transparent NAVSwap planner input selection failed.',
            ...(error.rpc_error ? { rpc_error: error.rpc_error } : {}),
            ...extra,
        };
    }

    function navswapActionAutoPlanRequested(body = {}) {
        return body.auto_plan === true
            || body.auto_plan === 'true'
            || body.planner === 'auto'
            || body.planner_mode === 'auto'
            || body.plan === 'auto';
    }

    function navswapPlannerNumber(value, field) {
        const parsed = Number.parseInt(String(value ?? ''), 10);
        if (!Number.isSafeInteger(parsed) || parsed < 0) {
            const err = new Error(`${field} must be a safe non-negative integer`);
            err.code = 'invalid_navswap_planner_integer';
            throw err;
        }
        return parsed;
    }

    function navswapPlannerPositiveNumber(value, field) {
        const parsed = navswapPlannerNumber(value, field);
        if (parsed <= 0) {
            const err = new Error(`${field} must be positive`);
            err.code = 'invalid_navswap_planner_integer';
            throw err;
        }
        return parsed;
    }

    function navswapAccountAssetItems(accountAssets) {
        if (Array.isArray(accountAssets)) return accountAssets;
        if (Array.isArray(accountAssets?.assets)) return accountAssets.assets;
        return [];
    }

    function navswapAccountBalanceAtoms(accountAssets, assetId) {
        let total = 0n;
        for (const item of navswapAccountAssetItems(accountAssets)) {
            if ((item?.asset_id || item?.id) !== assetId) continue;
            total += BigInt(String(item.balance ?? item.amount ?? 0));
        }
        return total;
    }

    function navswapNativeAccountBalanceAtoms(accountResult) {
        const account = accountResult?.account || accountResult || {};
        return BigInt(String(account.balance ?? account.pft_balance ?? account.native_balance ?? 0));
    }

    function navswapAssetInfoAsset(result) {
        return result?.asset || result?.asset_definition || result?.definition || result || null;
    }

    function navswapAssetPrecision(assetInfoResult, field = 'asset.precision') {
        const asset = navswapAssetInfoAsset(assetInfoResult);
        const precision = navswapPlannerNumber(asset?.precision, field);
        if (precision > 18) {
            const err = new Error(`${field} exceeds supported precision`);
            err.code = 'invalid_navswap_asset_precision';
            throw err;
        }
        return precision;
    }

    function navswapDecimalAmountToAtoms(value, precision, field) {
        const text = String(value ?? '').trim();
        if (!/^(?:[0-9]+(?:\.[0-9]*)?|\.[0-9]+)$/.test(text)) {
            const err = new Error(`${field} must be a positive decimal amount`);
            err.code = 'invalid_navswap_decimal_amount';
            throw err;
        }
        const [wholeRaw, fracRaw = ''] = text.split('.');
        const whole = wholeRaw === '' ? '0' : wholeRaw;
        if (fracRaw.length > precision) {
            const err = new Error(`${field} has more than ${precision} decimal places`);
            err.code = 'invalid_navswap_decimal_precision';
            throw err;
        }
        const scale = 10n ** BigInt(precision);
        const frac = fracRaw.padEnd(precision, '0');
        const atoms = (BigInt(whole) * scale) + BigInt(frac || '0');
        if (atoms <= 0n) {
            const err = new Error(`${field} must be positive`);
            err.code = 'invalid_navswap_decimal_amount';
            throw err;
        }
        return navswapSafeU64Number(atoms, field);
    }

    function parseNavswapDisplayOrAtomAmount(body, atomFields, displayFields, precision, field) {
        for (const atomField of atomFields) {
            const value = body[atomField];
            if (value !== undefined && value !== null && String(value).trim() !== '') {
                return parseNavswapActionInteger(value, atomField);
            }
        }
        for (const displayField of displayFields) {
            const value = body[displayField];
            if (value !== undefined && value !== null && String(value).trim() !== '') {
                return navswapDecimalAmountToAtoms(value, precision, field);
            }
        }
        return parseNavswapActionInteger(undefined, field);
    }

    function navswapValuationUnitScale(valuationUnit, settlementAssetPrecision) {
        const unit = String(valuationUnit || '').trim().toLowerCase();
        const usdScale = unit.match(/^usd_1e([0-9]+)$/);
        if (usdScale) {
            const exponent = Number.parseInt(usdScale[1], 10);
            if (!Number.isSafeInteger(exponent) || exponent > 38) return null;
            return 10n ** BigInt(exponent);
        }
        if (unit === 'usdc' || unit === 'usd_1e6' || unit === 'micro_usd') {
            return 10n ** BigInt(settlementAssetPrecision);
        }
        return null;
    }

    function navswapSafeU64Number(value, field) {
        if (value > BigInt(Number.MAX_SAFE_INTEGER)) {
            const err = new Error(`${field} exceeds the wallet adapter safe integer range`);
            err.code = 'invalid_navswap_planner_integer';
            throw err;
        }
        return Number(value);
    }

    function navswapRequiredVaultBridgeSettlementAtoms({
        mintAmount,
        navAssetPrecision = 0,
        navPerUnit,
        navValuationUnit,
        settlementValuationUnit,
        settlementAssetPrecision,
    }) {
        const navAssetScale = 10n ** BigInt(navswapPlannerNumber(navAssetPrecision, 'nav_asset.precision'));
        const raw = BigInt(navswapPlannerPositiveNumber(mintAmount, 'mint_amount'))
            * BigInt(navswapPlannerPositiveNumber(navPerUnit, 'nav_per_unit'));
        const navScale = navswapValuationUnitScale(navValuationUnit, settlementAssetPrecision);
        const settlementScale = navswapValuationUnitScale(settlementValuationUnit, settlementAssetPrecision);
        let numerator = raw;
        let denominator = navAssetScale;
        if (navScale !== null && settlementScale !== null && navScale !== settlementScale) {
            numerator = raw * settlementScale;
            denominator = navScale * navAssetScale;
        }
        const required = (numerator + denominator - 1n) / denominator;
        return navswapSafeU64Number(required, 'settlement_amount_atoms');
    }

    function navswapPlannerRemainingAtoms(allocation) {
        if (allocation.remaining_atoms !== undefined && allocation.remaining_atoms !== null) {
            return navswapPlannerNumber(allocation.remaining_atoms, 'allocation.remaining_atoms');
        }
        const amount = navswapPlannerNumber(allocation.amount_atoms, 'allocation.amount_atoms');
        const released = navswapPlannerNumber(allocation.released_atoms || 0, 'allocation.released_atoms');
        if (released > amount) {
            const err = new Error(`allocation ${allocation.allocation_id || ''} released atoms exceed amount`);
            err.code = 'navswap_planner_allocation_capacity_invalid';
            throw err;
        }
        return amount - released;
    }

    function navswapSettlementReceiptFreshnessConfig(body = {}) {
        const rawMax = body.max_snapshot_age_blocks
            ?? body.settlement_max_snapshot_age_blocks
            ?? NAVSWAP_SETTLEMENT_RECEIPT_MAX_SNAPSHOT_AGE_BLOCKS;
        const rawSafety = body.receipt_safety_blocks
            ?? body.settlement_receipt_safety_blocks
            ?? NAVSWAP_SETTLEMENT_RECEIPT_SAFETY_BLOCKS;
        const maxSnapshotAgeBlocks = navswapPlannerNumber(rawMax, 'settlement_receipt.max_snapshot_age_blocks');
        const safetyBlocks = navswapPlannerNumber(rawSafety, 'settlement_receipt.safety_blocks');
        return {
            max_snapshot_age_blocks: maxSnapshotAgeBlocks,
            safety_blocks: safetyBlocks,
        };
    }

    function navswapReceiptFreshness(receipt, currentHeight, config) {
        const createdAtHeight = navswapPlannerNumber(receipt.created_at_height || 0, 'receipt.created_at_height');
        const maxSnapshotAgeBlocks = config.max_snapshot_age_blocks;
        const safetyBlocks = config.safety_blocks;
        if (!currentHeight || createdAtHeight === 0 || maxSnapshotAgeBlocks === 0) {
            return {
                checked: false,
                fresh: true,
                created_at_height: createdAtHeight || null,
                current_height: currentHeight || null,
                max_snapshot_age_blocks: maxSnapshotAgeBlocks,
                safety_blocks: safetyBlocks,
                age_blocks: null,
                usable_until_height: maxSnapshotAgeBlocks === 0 || createdAtHeight === 0
                    ? null
                    : createdAtHeight + maxSnapshotAgeBlocks,
            };
        }
        const usableUntilHeight = createdAtHeight + maxSnapshotAgeBlocks;
        const ageBlocks = currentHeight >= createdAtHeight ? currentHeight - createdAtHeight : 0;
        return {
            checked: true,
            fresh: currentHeight + safetyBlocks <= usableUntilHeight,
            created_at_height: createdAtHeight,
            current_height: currentHeight,
            max_snapshot_age_blocks: maxSnapshotAgeBlocks,
            safety_blocks: safetyBlocks,
            age_blocks: ageBlocks,
            usable_until_height: usableUntilHeight,
        };
    }

    async function navswapPlannerCurrentHeight(body = {}, rpcRequest = rpcTcpRequest) {
        const explicit = body.current_height ?? body.block_height ?? body.pftl_height;
        if (explicit !== undefined && explicit !== null && explicit !== '') {
            return navswapPlannerNumber(explicit, 'current_height');
        }
        try {
            const serverInfo = await navswapRpcRead('server_info', {}, rpcRequest);
            const height = serverInfo?.ledger?.height
                ?? serverInfo?.block_height
                ?? serverInfo?.height
                ?? null;
            if (height === null || height === undefined || height === '') return null;
            return navswapPlannerNumber(height, 'server_info.ledger.height');
        } catch (_) {
            return null;
        }
    }

    function selectNavswapIssuedSettlementSource(status, amountAtoms, body = {}) {
        const explicitReceiptId = body.settlement_receipt_id || body.receipt_id || null;
        const explicitAllocationId = body.consume_supply_allocation_id || body.supply_allocation_id || null;
        const currentHeight = body.current_height === undefined || body.current_height === null
            ? null
            : navswapPlannerNumber(body.current_height, 'current_height');
        const freshnessConfig = navswapSettlementReceiptFreshnessConfig(body);
        const activeBuckets = new Set((status?.buckets || [])
            .filter(bucket => bucket?.status === VAULT_BRIDGE_BUCKET_STATUS_ACTIVE)
            .map(bucket => bucket.bucket_id));
        const receipts = (status?.receipts || []).filter(receipt => (
            receipt?.status === VAULT_BRIDGE_RECEIPT_STATUS_COUNTED
            && activeBuckets.has(receipt.bucket_id)
            && (!explicitReceiptId || receipt.receipt_id === explicitReceiptId)
        ));
        let staleCandidateCount = 0;
        let freshestRejected = null;
        const candidates = [];
        for (const receipt of receipts) {
            const freshness = navswapReceiptFreshness(receipt, currentHeight, freshnessConfig);
            for (const allocation of (status?.allocations || [])) {
                if (
                    allocation?.receipt_id !== receipt.receipt_id
                    || allocation?.bucket_id !== receipt.bucket_id
                    || allocation?.purpose !== VAULT_BRIDGE_ALLOCATION_PURPOSE_SUPPLY
                    || navswapPlannerNumber(allocation.retired_at_height || 0, 'allocation.retired_at_height') !== 0
                    || (explicitAllocationId && allocation.allocation_id !== explicitAllocationId)
                ) {
                    continue;
                }
                const remainingAtoms = navswapPlannerRemainingAtoms(allocation);
                if (remainingAtoms >= amountAtoms) {
                    if (!freshness.fresh) {
                        staleCandidateCount += 1;
                        if (
                            !freshestRejected
                            || navswapPlannerNumber(receipt.created_at_height || 0, 'receipt.created_at_height')
                                > navswapPlannerNumber(freshestRejected.receipt.created_at_height || 0, 'receipt.created_at_height')
                        ) {
                            freshestRejected = { receipt, allocation, remaining_atoms: remainingAtoms, freshness };
                        }
                        continue;
                    }
                    candidates.push({ receipt, allocation, remaining_atoms: remainingAtoms, freshness });
                }
            }
        }
        candidates.sort((left, right) => (
            navswapPlannerNumber(right.receipt.counted_at_height || 0, 'receipt.counted_at_height')
            - navswapPlannerNumber(left.receipt.counted_at_height || 0, 'receipt.counted_at_height')
        ) || (
            navswapPlannerNumber(right.allocation.created_at_height || 0, 'allocation.created_at_height')
            - navswapPlannerNumber(left.allocation.created_at_height || 0, 'allocation.created_at_height')
        ) || (right.remaining_atoms - left.remaining_atoms)
            || String(left.allocation.allocation_id).localeCompare(String(right.allocation.allocation_id)));
        if (candidates.length === 0) {
            const staleOnly = staleCandidateCount > 0;
            const err = new Error(staleOnly
                ? `No fresh counted active settlement receipt with a live supply allocation can cover ${amountAtoms} atoms. Bridge fresh pfUSDC before swapping.`
                : `No counted active settlement receipt with a live supply allocation can cover ${amountAtoms} atoms.`);
            err.code = staleOnly
                ? 'transparent_navswap_no_fresh_settlement_source'
                : 'transparent_navswap_no_settlement_source';
            err.settlement_status = {
                asset_id: status?.asset_id || null,
                bucket_count: status?.bucket_count || 0,
                receipt_count: status?.receipt_count || 0,
                allocation_count: status?.allocation_count || 0,
                stale_candidate_count: staleCandidateCount,
                current_height: currentHeight,
                max_snapshot_age_blocks: freshnessConfig.max_snapshot_age_blocks,
                safety_blocks: freshnessConfig.safety_blocks,
                freshest_rejected_receipt: freshestRejected ? {
                    receipt_id: freshestRejected.receipt.receipt_id,
                    allocation_id: freshestRejected.allocation.allocation_id,
                    remaining_atoms: String(freshestRejected.remaining_atoms),
                    freshness: freshestRejected.freshness,
                } : null,
            };
            throw err;
        }
        return candidates[0];
    }

    function validateNavswapPlannerMarketStatus(marketStatus, amountAtoms, { requireMintCap = false } = {}) {
        if (!marketStatus || marketStatus.market_operations_status !== 'active') {
            const err = new Error('Transparent NAVSwap planner requires active market operations status.');
            err.code = 'transparent_navswap_market_ops_not_active';
            throw err;
        }
        if (marketStatus.reserve_packet_fresh === false || marketStatus.supply_packet_fresh === false) {
            const err = new Error('Transparent NAVSwap planner requires fresh reserve and supply packets.');
            err.code = 'transparent_navswap_market_ops_stale';
            throw err;
        }
        if (requireMintCap) {
            const currentMintCap = navswapPlannerNumber(marketStatus.current_mint_cap_atoms || 0, 'market_ops_status.current_mint_cap_atoms');
            if (currentMintCap < amountAtoms) {
                const err = new Error(`Transparent NAVSwap planner mint cap ${currentMintCap} is below requested ${amountAtoms} atoms.`);
                err.code = 'transparent_navswap_market_ops_mint_cap_insufficient';
                throw err;
            }
        }
    }

    async function planTransparentNavswapWalletActions(body = {}, rpcRequest = rpcTcpRequest) {
        const route = navswapRouteFromBody(body);
        if (route !== 'transparent_navswap') {
            return navswapPlannerError({
                code: 'unsupported_navswap_planner_route',
                message: 'Transparent NAVSwap planner input discovery is available only for transparent_navswap.',
            }, route);
        }
        try {
            const walletAddress = parseNavswapWalletAddress(body.wallet_address || body.owner || body.source);
            const fromAsset = assetIdForNavswapSymbol(body.from_asset || body.from_asset_id || body.from || '');
            const toAsset = assetIdForNavswapSymbol(body.to_asset || body.to_asset_id || body.to || '');
            const explicitDirection = String(body.direction || '').trim();
            const inferredDirection = (() => {
                if (body.nav_asset_id && assetIdForNavswapSymbol(body.nav_asset_id) === fromAsset) return 'redeem';
                if (fromAsset === PFUSDC_ASSET_ID && toAsset !== PFUSDC_ASSET_ID) return 'subscribe';
                if (toAsset === PFUSDC_ASSET_ID && fromAsset !== PFUSDC_ASSET_ID) return 'redeem';
                return 'subscribe';
            })();
            const direction = explicitDirection || inferredDirection;
            if (direction !== 'subscribe' && direction !== 'redeem') {
                const err = new Error('Transparent NAVSwap planner direction must be subscribe or redeem.');
                err.code = 'transparent_navswap_planner_direction_invalid';
                throw err;
            }
            const navAssetId = assetIdForNavswapSymbol(body.nav_asset || body.nav_asset_id || (direction === 'subscribe' ? toAsset : fromAsset));
            const settlementAssetId = assetIdForNavswapSymbol(body.settlement_asset || body.settlement_asset_id || (direction === 'subscribe' ? fromAsset : toAsset));
            if (!isIssuedAsset(navAssetId) || !isIssuedAsset(settlementAssetId) || navAssetId === settlementAssetId) {
                const err = new Error('Transparent NAVSwap planner requires distinct issued NAV and settlement asset ids.');
                err.code = 'transparent_navswap_planner_asset_pair_invalid';
                throw err;
            }
            let mintAmountAtoms = null;
            let amountAtoms = null;

            const actions = [];
            const planner = {
                direction,
                wallet_address: walletAddress,
                nav_asset_id: navAssetId,
                settlement_asset_id: settlementAssetId,
            };

            if (direction === 'subscribe') {
                const [settlementStatus, marketStatus, navStatus, settlementAssetInfo, navAssetInfo] = await Promise.all([
                    navswapRpcRead('vault_bridge_status', { asset_id: settlementAssetId }, rpcRequest),
                    navswapRpcRead('market_ops_status', { asset_id: navAssetId }, rpcRequest),
                    navswapRpcRead('vault_bridge_status', { asset_id: navAssetId }, rpcRequest),
                    navswapRpcRead('asset_info', { asset_id: settlementAssetId }, rpcRequest),
                    navswapRpcRead('asset_info', { asset_id: navAssetId }, rpcRequest),
                ]);
                const settlementAssetPrecision = navswapAssetPrecision(settlementAssetInfo, 'settlement_asset.precision');
                const navAssetPrecision = navswapAssetPrecision(navAssetInfo, 'nav_asset.precision');
                mintAmountAtoms = parseNavswapDisplayOrAtomAmount(
                    body,
                    ['mint_amount_atoms', 'nav_amount_atoms', 'expected_output_atoms'],
                    ['mint_amount', 'nav_amount', 'expected_output', 'amount'],
                    navAssetPrecision,
                    'mint_amount',
                );
                amountAtoms = body.settlement_amount_atoms || body.amount_atoms
                    ? parseNavswapActionInteger(
                        body.settlement_amount_atoms || body.amount_atoms,
                        'settlement_amount_atoms',
                    )
                    : null;
                const requiredSettlementAtoms = navswapRequiredVaultBridgeSettlementAtoms({
                    mintAmount: mintAmountAtoms,
                    navAssetPrecision,
                    navPerUnit: navStatus.nav_per_unit,
                    navValuationUnit: navStatus.valuation_unit,
                    settlementValuationUnit: settlementStatus.valuation_unit,
                    settlementAssetPrecision,
                });
                if (amountAtoms === null) {
                    amountAtoms = requiredSettlementAtoms;
                } else if (amountAtoms !== requiredSettlementAtoms) {
                    const err = new Error(`Transparent NAVSwap settlement amount ${amountAtoms} does not match required settlement ${requiredSettlementAtoms} for mint amount ${mintAmountAtoms}.`);
                    err.code = 'transparent_navswap_settlement_amount_mismatch';
                    err.required_settlement_amount_atoms = String(requiredSettlementAtoms);
                    err.mint_amount_atoms = String(mintAmountAtoms);
                    throw err;
                }
                planner.amount_atoms = amountAtoms;
                planner.settlement_amount_atoms = amountAtoms;
                planner.mint_amount_atoms = mintAmountAtoms;
                planner.nav_per_unit = String(navStatus.nav_per_unit);
                planner.nav_epoch = String(navStatus.finalized_epoch);
                planner.reserve_packet_hash = navStatus.finalized_reserve_packet_hash;
                planner.nav_valuation_unit = navStatus.valuation_unit;
                planner.settlement_valuation_unit = settlementStatus.valuation_unit;
                planner.nav_asset_precision = navAssetPrecision;
                planner.settlement_asset_precision = settlementAssetPrecision;
                validateNavswapPlannerMarketStatus(marketStatus, mintAmountAtoms, { requireMintCap: true });
                const quoteFreshness = navswapFreshnessPayload({ navStatus, marketStatus });
                const plannerHeight = await navswapPlannerCurrentHeight(body, rpcRequest);
                const source = selectNavswapIssuedSettlementSource(settlementStatus, amountAtoms, {
                    ...body,
                    current_height: plannerHeight,
                });
                const subscriptionId = navswapSubscriptionId(
                    body.subscription_id || body.client_order_id || body.order_id,
                );
                actions.push({
                    stage: 'nav_subscription_allocate',
                    settlement_bucket_id: source.receipt.bucket_id,
                    settlement_receipt_id: source.receipt.receipt_id,
                    settlement_amount_atoms: String(amountAtoms),
                    mint_amount_atoms: String(mintAmountAtoms),
                    pricing_nav_epoch: String(navStatus.finalized_epoch),
                    primary_nav_price_atoms: String(navStatus.nav_per_unit),
                    pricing_reserve_packet_hash: navStatus.finalized_reserve_packet_hash,
                    consume_supply_allocation_id: source.allocation.allocation_id,
                    subscription_id: subscriptionId,
                    navswap_freshness: quoteFreshness,
                });
                return {
                    ok: true,
                    schema: NAVSWAP_TRANSPARENT_PLANNER_INPUTS_SCHEMA,
                    route,
                    planner,
                    actions,
                    quote_freshness: quoteFreshness,
                    operator_completion: {
                        stage: 'nav_mint_at_nav',
                        requires_operator_signature: true,
                        status: 'awaiting_wallet_allocation',
                        operation_template: {
                            operation: 'nav_mint_at_nav',
                            issuer: navStatus.issuer || marketStatus.issuer || null,
                            to: walletAddress,
                            asset_id: navAssetId,
                            amount: mintAmountAtoms,
                            epoch: navStatus.finalized_epoch,
                            reserve_packet_hash: navStatus.finalized_reserve_packet_hash,
                            settlement_asset_id: settlementAssetId,
                            settlement_bucket_id: source.receipt.bucket_id,
                            settlement_allocation_id: null,
                            settlement_amount_atoms: amountAtoms,
                        },
                        allocation_lookup: {
                            purpose: VAULT_BRIDGE_ALLOCATION_PURPOSE_NAV_SUBSCRIPTION,
                            consumer_id: `nav_subscription:${navAssetId}:${walletAddress}:${subscriptionId}`,
                            fallback_consumer_id: `nav_subscription:${navAssetId}`,
                            legacy_consumer_id: `nav_subscription:${navAssetId}:${walletAddress}`,
                            subscription_id: subscriptionId,
                            settlement_bucket_id: source.receipt.bucket_id,
                            settlement_receipt_id: source.receipt.receipt_id,
                            settlement_amount_atoms: String(amountAtoms),
                        },
                    },
                    selected: {
                        settlement_bucket_id: source.receipt.bucket_id,
                        settlement_receipt_id: source.receipt.receipt_id,
                        consume_supply_allocation_id: source.allocation.allocation_id,
                        supply_allocation_remaining_atoms: String(source.remaining_atoms),
                        receipt_unallocated_value_atoms: String(source.receipt.unallocated_value_atoms || 0),
                        receipt_freshness: source.freshness || null,
                    },
                    market_ops_status: {
                        asset_id: marketStatus.asset_id,
                        status: marketStatus.market_operations_status,
                        envelope_epoch: marketStatus.envelope_epoch,
                        reserve_packet_fresh: marketStatus.reserve_packet_fresh,
                        supply_packet_fresh: marketStatus.supply_packet_fresh,
                        current_mint_cap_atoms: String(marketStatus.current_mint_cap_atoms || 0),
                    },
                    vault_bridge_status: {
                        asset_id: settlementStatus.asset_id,
                        finalized_epoch: settlementStatus.finalized_epoch,
                        valuation_unit: settlementStatus.valuation_unit,
                        bucket_count: settlementStatus.bucket_count,
                        receipt_count: settlementStatus.receipt_count,
                        allocation_count: settlementStatus.allocation_count,
                    },
                };
            }

            const [navStatus, marketStatus, settlementStatus, settlementAssetInfo, navAssetInfo] = await Promise.all([
                navswapRpcRead('vault_bridge_status', { asset_id: navAssetId }, rpcRequest),
                navswapRpcRead('market_ops_status', { asset_id: navAssetId }, rpcRequest),
                navswapRpcRead('vault_bridge_status', { asset_id: settlementAssetId }, rpcRequest),
                navswapRpcRead('asset_info', { asset_id: settlementAssetId }, rpcRequest),
                navswapRpcRead('asset_info', { asset_id: navAssetId }, rpcRequest),
            ]);
            const navAssetPrecision = navswapAssetPrecision(navAssetInfo, 'nav_asset.precision');
            amountAtoms = parseNavswapDisplayOrAtomAmount(
                body,
                ['redeem_amount_atoms', 'nav_amount_atoms', 'amount_atoms'],
                ['redeem_amount', 'nav_amount', 'amount'],
                navAssetPrecision,
                'redeem_amount',
            );
            planner.amount_atoms = amountAtoms;
            validateNavswapPlannerMarketStatus(marketStatus, amountAtoms);
            const settlementAssetPrecision = navswapAssetPrecision(settlementAssetInfo, 'settlement_asset.precision');
            const requiredSettlementAtoms = navswapRequiredVaultBridgeSettlementAtoms({
                mintAmount: amountAtoms,
                navAssetPrecision,
                navPerUnit: navStatus.nav_per_unit,
                navValuationUnit: navStatus.valuation_unit,
                settlementValuationUnit: settlementStatus.valuation_unit,
                settlementAssetPrecision,
            });
            const backingAllocation = selectTransparentRedeemSettlementAllocation(settlementStatus, {
                navAssetId,
                owner: walletAddress,
                requiredSettlementAtoms,
                settlementAssetId,
            });
            const quoteFreshness = navswapFreshnessPayload({ navStatus, marketStatus });
            const reservePacketHash = parseNavswapHexId(
                body.reserve_packet_hash || body.nav_reserve_packet_hash || navStatus.finalized_reserve_packet_hash,
                'reserve_packet_hash',
            );
            const navEpoch = parseNavswapActionInteger(body.nav_epoch || body.epoch || navStatus.finalized_epoch, 'nav_epoch');
            planner.settlement_amount_atoms = requiredSettlementAtoms;
            planner.nav_per_unit = String(navStatus.nav_per_unit);
            planner.nav_epoch = String(navEpoch);
            planner.reserve_packet_hash = reservePacketHash;
            planner.nav_valuation_unit = navStatus.valuation_unit;
            planner.settlement_valuation_unit = settlementStatus.valuation_unit;
            planner.nav_asset_precision = navAssetPrecision;
            planner.settlement_asset_precision = settlementAssetPrecision;
            actions.push({
                stage: 'nav_redeem_at_nav',
                redeem_amount_atoms: String(amountAtoms),
                nav_epoch: String(navEpoch),
                reserve_packet_hash: reservePacketHash,
                navswap_freshness: {
                    ...quoteFreshness,
                    nav_epoch: String(navEpoch),
                    reserve_packet_hash: reservePacketHash,
                    nav_reserve_packet_hash: reservePacketHash,
                },
            });
            return {
                ok: true,
                schema: NAVSWAP_TRANSPARENT_PLANNER_INPUTS_SCHEMA,
                route,
                    planner,
                    actions,
                    quote_freshness: quoteFreshness,
                    operator_completion: {
                        stage: 'nav_redeem_settle',
                        requires_operator_signature: true,
                        status: 'awaiting_wallet_redeem',
                        operation_template: {
                            operation: 'nav_redeem_settle',
                            issuer: navStatus.issuer || marketStatus.issuer || null,
                            asset_id: navAssetId,
                            redemption_id: null,
                            settlement_receipt_hash: null,
                            settlement_asset_id: settlementAssetId,
                            settlement_bucket_id: backingAllocation.bucket_id,
                            settlement_allocation_id: backingAllocation.allocation_id,
                            settlement_amount_atoms: requiredSettlementAtoms,
                        },
                        allocation_lookup: {
                            purpose: VAULT_BRIDGE_ALLOCATION_PURPOSE_NAV_SUBSCRIPTION,
                            nav_asset_id: navAssetId,
                            settlement_asset_id: settlementAssetId,
                            owner: walletAddress,
                            settlement_amount_atoms: String(requiredSettlementAtoms),
                            settlement_bucket_id: backingAllocation.bucket_id,
                            settlement_allocation_id: backingAllocation.allocation_id,
                        },
                    },
                    selected: {
                        nav_epoch: String(navEpoch),
                        reserve_packet_hash: reservePacketHash,
                        settlement_bucket_id: backingAllocation.bucket_id,
                        settlement_allocation_id: backingAllocation.allocation_id,
                        backing_allocation_remaining_atoms: String(navswapAllocationRemainingAtoms(backingAllocation)),
                    },
                market_ops_status: {
                    asset_id: marketStatus.asset_id,
                    status: marketStatus.market_operations_status,
                    envelope_epoch: marketStatus.envelope_epoch,
                    reserve_packet_fresh: marketStatus.reserve_packet_fresh,
                    supply_packet_fresh: marketStatus.supply_packet_fresh,
                },
                    vault_bridge_status: {
                        asset_id: navStatus.asset_id,
                        finalized_epoch: navStatus.finalized_epoch,
                        finalized_reserve_packet_hash: navStatus.finalized_reserve_packet_hash,
                    },
                    settlement_vault_bridge_status: {
                        asset_id: settlementStatus.asset_id,
                        finalized_epoch: settlementStatus.finalized_epoch,
                        valuation_unit: settlementStatus.valuation_unit,
                        bucket_count: settlementStatus.bucket_count,
                        receipt_count: settlementStatus.receipt_count,
                        allocation_count: settlementStatus.allocation_count,
                    },
                };
        } catch (error) {
            return navswapPlannerError(error, route, error.settlement_status ? { settlement_status: error.settlement_status } : {});
        }
    }

    async function prepareNavswapWalletNavSubscriptionAllocateAction(body, route, stage, rpcRequest) {
        try {
            const walletAddress = parseNavswapWalletAddress(body.wallet_address || body.owner || body.source);
            const navAssetId = assetIdForNavswapSymbol(body.nav_asset_id || body.to_asset || body.asset_id || 'a651');
            const settlementAssetId = assetIdForNavswapSymbol(body.settlement_asset_id || body.from_asset || 'pfUSDC');
            if (!isIssuedAsset(navAssetId) || !isIssuedAsset(settlementAssetId) || navAssetId === settlementAssetId) {
                return {
                    ok: false,
                    schema: NAVSWAP_WALLET_ACTION_PREPARE_SCHEMA,
                    route,
                    stage,
                    code: 'navswap_allocate_requires_distinct_issued_assets',
                    message: 'NAV subscription allocation preparation requires distinct issued NAV and settlement asset ids.',
                    nav_asset_id: navAssetId,
                    settlement_asset_id: settlementAssetId,
                };
            }
            const settlementAmountAtoms = parseNavswapActionInteger(
                body.settlement_amount_atoms || body.amount_atoms || body.amount,
                'settlement_amount_atoms',
            );
            const settlementBucketId = parseNavswapHexId(body.settlement_bucket_id || body.bucket_id, 'settlement_bucket_id');
            const settlementReceiptId = parseNavswapHexId(body.settlement_receipt_id || body.receipt_id, 'settlement_receipt_id');
            const consumeSupplyAllocationId = parseNavswapHexId(
                body.consume_supply_allocation_id || body.supply_allocation_id,
                'consume_supply_allocation_id',
            );
            const subscriptionId = navswapSubscriptionId(
                body.subscription_id || body.client_order_id || body.order_id,
            );
            const primaryMintIntent = navswapPrimaryMintIntentFields(body);
            const { issuer, asset_info: navAssetInfo } = await navswapAssetIssuer(navAssetId, rpcRequest);
            const operation = {
                operation: 'vault_bridge_nav_subscription_allocate',
                operator: issuer,
                nav_asset_id: navAssetId,
                settlement_asset_id: settlementAssetId,
                settlement_bucket_id: settlementBucketId,
                settlement_receipt_id: settlementReceiptId,
                settlement_amount_atoms: settlementAmountAtoms,
                consume_supply_owner: walletAddress,
                consume_supply_allocation_id: consumeSupplyAllocationId,
                nav_recipient: walletAddress,
                subscription_id: subscriptionId,
            };
            const actionId = navswapWalletActionId({
                route,
                stage,
                wallet_address: walletAddress,
                nav_asset_id: navAssetId,
                settlement_asset_id: settlementAssetId,
                settlement_bucket_id: settlementBucketId,
                settlement_receipt_id: settlementReceiptId,
                settlement_amount_atoms: settlementAmountAtoms,
                consume_supply_allocation_id: consumeSupplyAllocationId,
                subscription_id: subscriptionId,
                operator: issuer,
            });
            return {
                ok: true,
                schema: NAVSWAP_WALLET_ACTION_PREPARE_SCHEMA,
                route,
                stage,
                action: {
                    schema: NAVSWAP_WALLET_ACTION_SCHEMA,
                    route,
                    action_id: actionId,
                    stage,
                    source: walletAddress,
                    wallet_address: walletAddress,
                    user_intent: {
                        wallet_address: walletAddress,
                        route,
                        from_asset_id: settlementAssetId,
                        to_asset_id: navAssetId,
                        nav_asset_id: navAssetId,
                        settlement_asset_id: settlementAssetId,
                        max_settlement_amount_atoms: String(settlementAmountAtoms),
                        subscription_id: subscriptionId,
                        operator: issuer,
                        ...primaryMintIntent,
                        ...navswapFreshnessFromBody(body),
                    },
                    operation,
                },
                asset_info: navAssetInfo,
            };
        } catch (error) {
            return navswapActionPrepareError(error, route, stage);
        }
    }

    async function prepareNavswapWalletNavRedeemAtNavAction(body, route, stage, rpcRequest) {
        try {
            const walletAddress = parseNavswapWalletAddress(body.wallet_address || body.owner || body.source);
            const navAssetId = assetIdForNavswapSymbol(body.nav_asset_id || body.from_asset || body.asset_id || body.asset || 'a651');
            if (!isIssuedAsset(navAssetId)) {
                return {
                    ok: false,
                    schema: NAVSWAP_WALLET_ACTION_PREPARE_SCHEMA,
                    route,
                    stage,
                    code: 'navswap_redeem_requires_issued_nav_asset',
                    message: 'NAV redeem preparation requires a 96-hex issued NAV asset id.',
                    nav_asset_id: navAssetId,
                };
            }
            const redeemAmountAtoms = parseNavswapActionInteger(
                body.redeem_amount_atoms || body.amount_atoms || body.amount,
                'redeem_amount_atoms',
            );
            const navEpoch = parseNavswapActionInteger(body.nav_epoch || body.epoch, 'nav_epoch');
            const reservePacketHash = parseNavswapHexId(
                body.reserve_packet_hash || body.nav_reserve_packet_hash,
                'reserve_packet_hash',
            );
            const { issuer, asset_info: navAssetInfo } = await navswapAssetIssuer(navAssetId, rpcRequest);
            const operation = {
                operation: 'nav_redeem_at_nav',
                owner: walletAddress,
                issuer,
                asset_id: navAssetId,
                amount: redeemAmountAtoms,
                epoch: navEpoch,
                reserve_packet_hash: reservePacketHash,
            };
            const actionId = navswapWalletActionId({
                route,
                stage,
                wallet_address: walletAddress,
                nav_asset_id: navAssetId,
                redeem_amount_atoms: redeemAmountAtoms,
                nav_epoch: navEpoch,
                reserve_packet_hash: reservePacketHash,
                issuer,
            });
            return {
                ok: true,
                schema: NAVSWAP_WALLET_ACTION_PREPARE_SCHEMA,
                route,
                stage,
                action: {
                    schema: NAVSWAP_WALLET_ACTION_SCHEMA,
                    route,
                    action_id: actionId,
                    stage,
                    source: walletAddress,
                    wallet_address: walletAddress,
                    user_intent: {
                        wallet_address: walletAddress,
                        route,
                        from_asset_id: navAssetId,
                        nav_asset_id: navAssetId,
                        amount_atoms: String(redeemAmountAtoms),
                        max_redeem_amount_atoms: String(redeemAmountAtoms),
                        nav_epoch: String(navEpoch),
                        reserve_packet_hash: reservePacketHash,
                        nav_reserve_packet_hash: reservePacketHash,
                        issuer,
                        ...navswapFreshnessFromBody(body),
                    },
                    operation,
                },
                asset_info: navAssetInfo,
            };
        } catch (error) {
            return navswapActionPrepareError(error, route, stage);
        }
    }

    function navswapPftlUniswapDefaultEthereumRecipient(body = {}, bridge = {}) {
        return body.ethereum_recipient
            || body.recipient
            || body.destination
            || presentEnv('NAVSWAP_UNISWAP_DEFAULT_RECIPIENT')
            || presentEnv('NAVSWAP_DEFAULT_ETHEREUM_RECIPIENT')
            || bridge.default_ethereum_recipient
            || bridge.lp_recipient
            || null;
    }

    function navswapPftlUniswapDefaultDeadlineSeconds(body = {}) {
        const explicit = body.destination_deadline_seconds
            || body.deadline_seconds
            || body.deadline
            || body.expiry;
        if (explicit !== undefined && explicit !== null && String(explicit).trim() !== '') {
            return parseNavswapActionInteger(explicit, 'destination_deadline_seconds');
        }
        const configured = presentPositiveSafeIntegerEnv('NAVSWAP_UNISWAP_DEFAULT_DEADLINE_SECONDS');
        if (configured) return configured;
        return Math.floor(Date.now() / 1000) + 3600;
    }

    function navswapPftlUniswapDefaultRefundDelayBlocks(body = {}) {
        const explicit = body.refund_delay_blocks || body.refund_after_blocks || body.refund_delay;
        if (explicit !== undefined && explicit !== null && String(explicit).trim() !== '') {
            return parseNavswapActionInteger(explicit, 'refund_delay_blocks');
        }
        return presentPositiveSafeIntegerEnv('NAVSWAP_UNISWAP_REFUND_DELAY_BLOCKS') || 5;
    }

    function navswapPftlUniswapRouteRow(routesStatus, routeId) {
        const routes = Array.isArray(routesStatus?.routes) ? routesStatus.routes : [];
        return routes.find(row => row?.route_id === routeId) || null;
    }

    function navswapPftlUniswapPacketHash(fields) {
        return navswapHashHexDomain(
            'postfiat.pftl_uniswap.export_packet.wallet.v1',
            navswapStableJson(fields),
        );
    }

    async function loadPftlUniswapWalletActionContext(body, bridge, rpcRequest) {
        const walletAddress = parseNavswapWalletAddress(body.wallet_address || body.owner || body.source);
        const [routesStatus, supplyStatus, navStatus, settlementStatus, nativeInfo, settlementInfo, serverInfo] = await Promise.all([
            navswapRpcRead('navcoin_bridge_routes', {}, rpcRequest),
            navswapRpcRead('navcoin_bridge_supply_status', { route_id: bridge.route_id }, rpcRequest),
            navswapRpcRead('vault_bridge_status', { asset_id: bridge.native_nav_asset_id }, rpcRequest),
            navswapRpcRead('vault_bridge_status', { asset_id: bridge.settlement_asset_id }, rpcRequest),
            navswapRpcRead('asset_info', { asset_id: bridge.native_nav_asset_id }, rpcRequest),
            navswapRpcRead('asset_info', { asset_id: bridge.settlement_asset_id }, rpcRequest),
            navswapRpcRead('server_info', {}, rpcRequest),
        ]);
        const routeRow = navswapPftlUniswapRouteRow(routesStatus, bridge.route_id);
        if (!routeRow) {
            const err = new Error(`PFTL-Uniswap route ${bridge.route_id} is not registered on the node`);
            err.code = 'pftl_uniswap_route_missing';
            throw err;
        }
        if (routeRow.route_config_digest !== bridge.route_config_digest) {
            const err = new Error('PFTL-Uniswap node route digest does not match the wallet proxy route digest');
            err.code = 'pftl_uniswap_route_digest_mismatch';
            err.node_route_config_digest = routeRow.route_config_digest;
            err.wallet_route_config_digest = bridge.route_config_digest;
            throw err;
        }
        if (routeRow.route_live !== true || routeRow.paused === true) {
            const err = new Error('PFTL-Uniswap route is paused or not live');
            err.code = 'pftl_uniswap_route_not_live';
            throw err;
        }
        if (routeRow.route_trust_class !== 'CONTROLLED' || bridge.route_trust_class !== 'CONTROLLED') {
            const err = new Error('PFTL-Uniswap wallet beta requires CONTROLLED route trust class');
            err.code = 'pftl_uniswap_route_trust_class_invalid';
            throw err;
        }

        const nativePrecision = navswapAssetPrecision(nativeInfo, 'native_nav_asset.precision');
        const settlementPrecision = navswapAssetPrecision(settlementInfo, 'settlement_asset.precision');
        const priceAtoms = navswapRequiredVaultBridgeSettlementAtoms({
            mintAmount: 1,
            navAssetPrecision: nativePrecision,
            navPerUnit: navStatus.nav_per_unit,
            navValuationUnit: navStatus.valuation_unit,
            settlementValuationUnit: settlementStatus.valuation_unit,
            settlementAssetPrecision: settlementPrecision,
        });
        const mintAmountAtoms = parseNavswapDisplayOrAtomAmount(
            body,
            ['mint_amount_atoms', 'nav_amount_atoms', 'amount_atoms'],
            ['amount', 'mint_amount', 'nav_amount'],
            nativePrecision,
            'mint_amount',
        );
        const settlementValue = BigInt(mintAmountAtoms) * BigInt(priceAtoms);
        const settlementValueAtoms = navswapSafeU64Number(settlementValue, 'settlement_value_atoms');
        const packetCapAtoms = navswapPlannerPositiveNumber(routeRow.packet_notional_cap_atoms, 'route.packet_notional_cap_atoms');
        const capRemainingAtoms = navswapPlannerNumber(routeRow.supply_cap_remaining_atoms, 'route.supply_cap_remaining_atoms');
        if (mintAmountAtoms > packetCapAtoms) {
            const err = new Error('PFTL-Uniswap mint amount exceeds the route packet cap');
            err.code = 'pftl_uniswap_packet_cap_exceeded';
            throw err;
        }
        if (mintAmountAtoms > capRemainingAtoms) {
            const err = new Error('PFTL-Uniswap mint amount exceeds the remaining route supply cap');
            err.code = 'pftl_uniswap_route_cap_exceeded';
            throw err;
        }
        const finalizedEpoch = navswapPlannerPositiveNumber(navStatus.finalized_epoch, 'vault_bridge_status.finalized_epoch');
        const reservePacketHash = parseNavswapHexId(
            navStatus.finalized_reserve_packet_hash,
            'vault_bridge_status.finalized_reserve_packet_hash',
        );
        const generatedAtMs = Date.now();
        const quoteFreshness = {
            quote_generated_at_ms: String(generatedAtMs),
            quote_expires_at_ms: String(generatedAtMs + Math.max(1000, NAVSWAP_QUOTE_FRESHNESS_TTL_MS)),
            proof_status: 'active',
            reserve_packet_fresh: true,
            supply_packet_fresh: true,
            nav_epoch: String(finalizedEpoch),
            reserve_packet_hash: reservePacketHash,
            nav_reserve_packet_hash: reservePacketHash,
        };
        return {
            walletAddress,
            routeRow,
            routesStatus,
            supplyStatus,
            navStatus,
            settlementStatus,
            nativeInfo,
            settlementInfo,
            serverInfo,
            currentHeight: serverInfo?.ledger?.height ?? null,
            nativePrecision,
            settlementPrecision,
            priceAtoms,
            mintAmountAtoms,
            settlementValueAtoms,
            finalizedEpoch,
            reservePacketHash,
            quoteFreshness,
        };
    }

    async function preparePftlUniswapWalletActionBatch(body = {}, rpcRequest = rpcTcpRequest) {
        const route = 'uniswap_atomic_handoff';
        const bridge = navswapBridgeConfig();
        const beta = navswapUniswapBetaRouteState(bridge);
        if (!beta.quote_enabled) {
            return {
                ok: false,
                schema: NAVSWAP_WALLET_ACTION_BATCH_PREPARE_SCHEMA,
                route,
                code: 'uniswap_handoff_beta_not_enabled',
                message: 'PFTL-Uniswap handoff requires explicit CONTROLLED beta enablement, caps, unpaused state, and public routing disabled before wallet actions can be prepared.',
                blockers: beta.blockers,
                config: bridge,
            };
        }
        try {
            const context = await loadPftlUniswapWalletActionContext(body, bridge, rpcRequest);
            const subscriptionNonce = parseNavswapHexId(
                body.subscription_nonce || navswapRandomHex(32),
                'subscription_nonce',
                64,
            );
            const exportNonce = parseNavswapHexId(
                body.export_nonce || navswapRandomHex(32),
                'export_nonce',
                64,
            );
            const ethereumRecipient = parseNavswapEvmAddress(
                navswapPftlUniswapDefaultEthereumRecipient(body, bridge),
                'ethereum_recipient',
            );
            const destinationDeadlineSeconds = navswapPftlUniswapDefaultDeadlineSeconds(body);
            const refundDelayBlocks = navswapPftlUniswapDefaultRefundDelayBlocks(body);
            const packetHash = parseNavswapHexId(
                body.packet_hash || navswapPftlUniswapPacketHash({
                    route_id: bridge.route_id,
                    owner: context.walletAddress,
                    export_nonce: exportNonce,
                    ethereum_recipient: ethereumRecipient.toLowerCase(),
                    amount_atoms: String(context.mintAmountAtoms),
                    destination_deadline_seconds: String(destinationDeadlineSeconds),
                    refund_delay_blocks: String(refundDelayBlocks),
                }),
                'packet_hash',
            );
            const baseIntent = {
                wallet_address: context.walletAddress,
                route,
                route_id: bridge.route_id,
                route_family: 'composite_primary_mint_to_ethereum_venue',
                purchase_kind: 'composite_primary_mint_to_ethereum_venue',
                route_trust_class: bridge.route_trust_class,
                route_config_digest: bridge.route_config_digest,
                launch_config_digest: bridge.launch_config_digest,
                release_stage: 'explicit_beta',
                public_routing_enabled: false,
                route_paused: false,
                source_chain: 'PFTL',
                destination_chain: 'ethereum',
                from_asset_id: bridge.settlement_asset_id,
                settlement_asset_id: bridge.settlement_asset_id,
                to_asset_id: bridge.native_nav_asset_id,
                nav_asset_id: bridge.native_nav_asset_id,
                native_nav_asset_id: bridge.native_nav_asset_id,
                wrapped_navcoin_token: bridge.wrapped_navcoin_token,
                handoff_controller: bridge.handoff_controller,
                settlement_adapter: bridge.settlement_adapter,
                uniswap_pool_id: bridge.uniswap_pool_id_or_path,
                uniswap_pool_id_or_path: bridge.uniswap_pool_id_or_path,
                router: bridge.router,
                route_supply_cap_atoms: String(context.routeRow.route_supply_cap_atoms),
                supply_cap_remaining_atoms: String(context.routeRow.supply_cap_remaining_atoms),
                packet_notional_cap_atoms: String(context.routeRow.packet_notional_cap_atoms),
                supply_effect: 'mints_new_native_navcoin_supply',
                pricing_source: 'finalized_pre_inflow_nav_snapshot',
                settlement_reserve_effect: 'added_after_primary_fill',
                uniswap_supply_effect: 'not_uniswap_supply',
                bridge_packet_effect: 'minted_navcoin_exported_or_claimed',
                ethereum_supply_effect: 'mints_wrapped_venue_token_from_pftl_packet',
                mint_amount_atoms: String(context.mintAmountAtoms),
                export_amount_atoms: String(context.mintAmountAtoms),
                settlement_value_atoms: String(context.settlementValueAtoms),
                max_settlement_amount_atoms: String(context.settlementValueAtoms),
                pricing_nav_epoch: String(context.finalizedEpoch),
                primary_nav_price_atoms: String(context.priceAtoms),
                nav_price_settlement_atoms_per_nav_atom: String(context.priceAtoms),
                pricing_reserve_packet_hash: context.reservePacketHash,
                nav_epoch: String(context.finalizedEpoch),
                nav_per_unit: String(context.priceAtoms),
                reserve_packet_hash: context.reservePacketHash,
                nav_reserve_packet_hash: context.reservePacketHash,
                subscription_nonce: subscriptionNonce,
                export_nonce: exportNonce,
                packet_hash: packetHash,
                ethereum_recipient: ethereumRecipient,
                destination_deadline_seconds: String(destinationDeadlineSeconds),
                refund_delay_blocks: String(refundDelayBlocks),
                bridge_verifier_mode: bridge.verifier_mode,
                route_trust_label: 'CONTROLLED beta, operator-attested destination events',
                custody_boundary: 'wallet-local-signing-for-source-actions',
                operator_attestation: 'destination consume, return import, and refund are operator-attested until Gate 5 verifier work lands',
                ...context.quoteFreshness,
            };
            const primaryOperation = {
                operation: 'pftl_uniswap_primary_subscribe',
                subscriber: context.walletAddress,
                route_id: bridge.route_id,
                settlement_asset_id: bridge.settlement_asset_id,
                subscription_nonce: subscriptionNonce,
                settlement_value_atoms: context.settlementValueAtoms,
                nav_price_settlement_atoms_per_nav_atom: context.priceAtoms,
                pricing_nav_epoch: context.finalizedEpoch,
                pricing_reserve_packet_hash: context.reservePacketHash,
            };
            const exportOperation = {
                operation: 'pftl_uniswap_export_debit',
                owner: context.walletAddress,
                route_id: bridge.route_id,
                packet_hash: packetHash,
                export_nonce: exportNonce,
                ethereum_recipient: ethereumRecipient,
                amount_atoms: context.mintAmountAtoms,
                destination_deadline_seconds: destinationDeadlineSeconds,
                refund_delay_blocks: refundDelayBlocks,
            };
            const actions = [
                {
                    schema: NAVSWAP_WALLET_ACTION_SCHEMA,
                    route,
                    action_id: navswapWalletActionId({
                        route,
                        stage: 'pftl_uniswap_primary_subscribe',
                        wallet_address: context.walletAddress,
                        route_id: bridge.route_id,
                        subscription_nonce: subscriptionNonce,
                        settlement_value_atoms: context.settlementValueAtoms,
                        nav_price_settlement_atoms_per_nav_atom: context.priceAtoms,
                        pricing_nav_epoch: context.finalizedEpoch,
                        pricing_reserve_packet_hash: context.reservePacketHash,
                    }),
                    stage: 'pftl_uniswap_primary_subscribe',
                    source: context.walletAddress,
                    wallet_address: context.walletAddress,
                    user_intent: baseIntent,
                    operation: primaryOperation,
                },
                {
                    schema: NAVSWAP_WALLET_ACTION_SCHEMA,
                    route,
                    action_id: navswapWalletActionId({
                        route,
                        stage: 'pftl_uniswap_export_debit',
                        wallet_address: context.walletAddress,
                        route_id: bridge.route_id,
                        packet_hash: packetHash,
                        export_nonce: exportNonce,
                        ethereum_recipient: ethereumRecipient,
                        amount_atoms: context.mintAmountAtoms,
                        destination_deadline_seconds: destinationDeadlineSeconds,
                        refund_delay_blocks: refundDelayBlocks,
                    }),
                    stage: 'pftl_uniswap_export_debit',
                    source: context.walletAddress,
                    wallet_address: context.walletAddress,
                    user_intent: baseIntent,
                    operation: exportOperation,
                },
            ];
            return {
                ok: true,
                schema: NAVSWAP_WALLET_ACTION_BATCH_PREPARE_SCHEMA,
                route,
                route_id: bridge.route_id,
                action_schema: NAVSWAP_WALLET_ACTION_SCHEMA,
                action_count: actions.length,
                stages: actions.map(action => action.stage),
                actions,
                quote_freshness: context.quoteFreshness,
                selected: {
                    nav_epoch: String(context.finalizedEpoch),
                    reserve_packet_hash: context.reservePacketHash,
                    route_config_digest: bridge.route_config_digest,
                    route_ledger_hash: context.routeRow.ledger_hash || null,
                    supply_ledger_hash: context.supplyStatus?.ledger_hash || null,
                    current_height: context.currentHeight === null || context.currentHeight === undefined
                        ? null
                        : String(context.currentHeight),
                },
                pricing: {
                    nav_price_settlement_atoms_per_nav_atom: String(context.priceAtoms),
                    nav_asset_precision: context.nativePrecision,
                    settlement_asset_precision: context.settlementPrecision,
                    finalized_epoch: String(context.finalizedEpoch),
                    finalized_reserve_packet_hash: context.reservePacketHash,
                },
                bridge: {
                    route_config_digest: bridge.route_config_digest,
                    launch_config_digest: bridge.launch_config_digest,
                    route_trust_class: bridge.route_trust_class,
                    public_routing_enabled: false,
                    operator_attested_destination_events: true,
                    wrapped_navcoin_token: bridge.wrapped_navcoin_token,
                    handoff_controller: bridge.handoff_controller,
                    settlement_adapter: bridge.settlement_adapter,
                    uniswap_pool_id_or_path: bridge.uniswap_pool_id_or_path,
                    router: bridge.router,
                },
            };
        } catch (error) {
            return navswapActionPrepareError(error, route, 'pftl_uniswap_source_batch');
        }
    }

    async function executePftlUniswapWalletQuote(body = {}, rpcRequest = rpcTcpRequest) {
        const prepared = await preparePftlUniswapWalletActionBatch(body, rpcRequest);
        if (prepared.ok !== true) {
            return {
                ok: false,
                schema: NAVSWAP_QUOTE_SCHEMA,
                route: 'uniswap_atomic_handoff',
                code: prepared.code || 'pftl_uniswap_wallet_action_prepare_failed',
                message: prepared.message || 'PFTL-Uniswap source wallet actions could not be prepared.',
                action_batch_prepare: prepared,
            };
        }
        const actions = prepared.actions || [];
        const primary = actions.find(action => action.stage === 'pftl_uniswap_primary_subscribe');
        const intent = primary?.user_intent || {};
        const feePreflight = await preflightNavswapPreparedActionFees(actions, rpcRequest);
        const accountState = await navswapRpcRead('account', { address: prepared.actions[0].source }, rpcRequest)
            .catch(() => null);
        return {
            ok: true,
            schema: NAVSWAP_QUOTE_SCHEMA,
            route: 'uniswap_atomic_handoff',
            status: 'prepared_actions_ready',
            can_run: false,
            requires_wallet_submit: true,
            custody_boundary: 'wallet-local-signing',
            route_family: 'composite_primary_mint_to_ethereum_venue',
            route_trust_class: intent.route_trust_class || 'CONTROLLED',
            release_stage: 'explicit_beta',
            public_routing_enabled: false,
            paused: false,
            route_config_digest: intent.route_config_digest,
            launch_config_digest: intent.launch_config_digest,
            from_asset: intent.settlement_asset_id,
            to_asset: intent.native_nav_asset_id,
            direction: 'subscribe_export',
            amount: body.amount === undefined || body.amount === null ? null : String(body.amount),
            input_amount_atoms: intent.settlement_value_atoms,
            settlement_amount_atoms: intent.settlement_value_atoms,
            mint_amount_atoms: intent.mint_amount_atoms,
            export_amount_atoms: intent.export_amount_atoms,
            expected_output: intent.mint_amount_atoms,
            expected_output_asset: intent.native_nav_asset_id,
            navswap_freshness: prepared.quote_freshness,
            quote_freshness: prepared.quote_freshness,
            pricing: prepared.pricing,
            bridge: prepared.bridge,
            operator_completion: {
                stage: 'pftl_uniswap_destination_consume',
                trust_class: 'CONTROLLED',
                attestation: 'operator-attested until Gate 5 verifier work lands',
                packet_hash: intent.packet_hash,
                ethereum_recipient: intent.ethereum_recipient,
                destination_deadline_seconds: intent.destination_deadline_seconds,
            },
            wallet_pft: {
                balance_atoms: navswapNativeAccountBalanceAtoms(accountState).toString(),
                fee_preflight: feePreflight,
                sufficient_for_prepared_actions: feePreflight.ok === true,
            },
            prepared_action_batch: prepared,
            next_step: 'wallet_verify_sign_submit_prepared_source_actions',
            message: 'Prepared wallet-owned PFTL-Uniswap source actions. The wallet signs primary mint and export locally; destination consume is operator-attested CONTROLLED beta.',
        };
    }

    function isPftAsset(value) {
        return String(value || '').toUpperCase() === 'PFT';
    }

    function isIssuedAsset(value) {
        return typeof value === 'string' && /^[0-9a-f]{96}$/i.test(value);
    }

    function parseAtomicInteger(value, field, { allowZero = false } = {}) {
        if (value === undefined || value === null || value === '') return value;
        const text = String(value).trim();
        const pattern = allowZero ? /^(0|[1-9][0-9]*)$/ : /^[1-9][0-9]*$/;
        if (!pattern.test(text)) {
            const err = new Error(`${field} must be a ${allowZero ? 'non-negative' : 'positive'} whole number`);
            err.code = 'invalid_atomic_template_integer';
            throw err;
        }
        const parsed = Number.parseInt(text, 10);
        if (!Number.isSafeInteger(parsed)) {
            const err = new Error(`${field} exceeds the wallet adapter safe integer range`);
            err.code = 'invalid_atomic_template_integer';
            throw err;
        }
        return parsed;
    }

    function navswapRouteFromBody(body) {
        const route = String(body?.route || body?.route_id || 'transparent_navswap').trim();
        if (route === 'transparent') return 'transparent_navswap';
        if (route === 'private') return 'shielded_navswap';
        if (route === 'otc') return 'pftl_atomic_settlement';
        return route;
    }

    function parseStakehubTransparentAmount(amount) {
        const value = String(amount ?? '').trim();
        if (!/^(?:[0-9]+(?:\.[0-9]*)?|\.[0-9]+)$/.test(value)) return null;
        const parsed = Number.parseFloat(value);
        if (!Number.isFinite(parsed) || parsed <= 0) return null;
        return value.replace(/^0+(?=\d)/, '') || '0';
    }

    function stakehubTransparentAmountError(route, amount) {
        return {
            ok: false,
            schema: NAVSWAP_QUOTE_SCHEMA,
            route,
            code: 'stakehub_transparent_amount_invalid',
            message: 'StakeHub transparent roundtrip amount must be a positive a651 decimal.',
            amount,
        };
    }

    function buildUrl(baseUrl, pathName, params = {}) {
        const url = new URL(pathName, baseUrl);
        for (const [key, value] of Object.entries(params)) {
            if (value !== undefined && value !== null && String(value) !== '') {
                url.searchParams.set(key, String(value));
            }
        }
        return url.toString();
    }

    async function fetchJsonWithTimeout(url, timeoutMs) {
        const controller = new AbortController();
        const timer = setTimeout(() => controller.abort(), timeoutMs);
        try {
            const response = await fetch(url, {
                method: 'GET',
                headers: { 'Accept': 'application/json' },
                signal: controller.signal,
            });
            const payload = await response.json();
            if (!response.ok) {
                const err = new Error(payload?.message || payload?.error || `HTTP ${response.status}`);
                err.status = response.status;
                err.payload = payload;
                throw err;
            }
            return payload;
        } finally {
            clearTimeout(timer);
        }
    }

    function navswapNavProofStub(assetId, phase, message) {
        return {
            ok: true,
            schema: NAVSWAP_NAV_PROOF_SCHEMA,
            asset_id: assetId,
            phase,
            proof_available: false,
            source: 'wallet-proxy',
            message: message || 'NAV proof passthrough is stubbed on the wallet proxy; routes requiring fresh NAV proof remain disabled until a proof source is configured.',
        };
    }

    async function buildNavswapNavProofResponse(searchParams = new URLSearchParams()) {
        const assetId = assetIdForNavswapSymbol(searchParams.get('asset_id') || searchParams.get('asset') || 'a651');
        const phase = searchParams.get('phase') || 'current';
        const config = navswapStakehubTransparentConfig();
        if (!config.configured) {
            return navswapNavProofStub(assetId, phase, 'Set NAVSWAP_STAKEHUB_BASE_URL to read the StakeHub NAV proof snapshot.');
        }

        let navcoinUrl;
        let statusUrl;
        try {
            navcoinUrl = buildUrl(config.base_url, config.navcoin_path);
            statusUrl = buildUrl(config.base_url, config.navcoin_status_path, { asset_id: assetId });
        } catch (_) {
            return {
                ok: false,
                schema: NAVSWAP_NAV_PROOF_SCHEMA,
                asset_id: assetId,
                phase,
                proof_available: false,
                code: 'stakehub_nav_proof_invalid_url',
                message: 'NAVSWAP_STAKEHUB_BASE_URL is not a valid HTTP(S) URL.',
            };
        }

        try {
            const [navcoin, status] = await Promise.all([
                fetchJsonWithTimeout(navcoinUrl, config.read_timeout_ms),
                fetchJsonWithTimeout(statusUrl, config.read_timeout_ms).catch((error) => ({
                    available: false,
                    error: error?.message || String(error),
                })),
            ]);
            const proof = navcoin?.proof || {};
            const token = navcoin?.token || {};
            const pftl = navcoin?.pftl || {};
            const proofAvailable = Boolean(proof && Object.keys(proof).length > 0 && proof.stale !== true);
            return {
                ok: true,
                schema: NAVSWAP_NAV_PROOF_SCHEMA,
                asset_id: assetId,
                phase,
                proof_available: proofAvailable,
                source: 'stakehub:/api/navcoin',
                chain_id: pftl.chain_id || proof.chain_id || status.chain_id || null,
                current_pftl_height: pftl.current_height || pftl.height || proof.current_height || status.current_height || null,
                nav_epoch: proof.envelope_epoch || proof.epoch || status.envelope_epoch || null,
                reserve_packet_hash: proof.reserve_packet_hash || proof.packet_hash || status.accepted_policy_hash || null,
                freshness_deadline_height: proof.freshness_deadline_height || null,
                nav_per_unit: proof.nav_per_unit ?? token.nav_per_unit ?? null,
                supply: token.supply ?? proof.supply ?? null,
                proof_status: proof.proof_status || status.market_operations_status || (proofAvailable ? 'available' : 'missing'),
                stale: proof.stale === true,
                source_receipt_hashes: proof.source_receipt_hashes || proof.receipt_hashes || [],
                proof,
                token,
                status,
            };
        } catch (error) {
            return {
                ok: false,
                schema: NAVSWAP_NAV_PROOF_SCHEMA,
                asset_id: assetId,
                phase,
                proof_available: false,
                code: error?.name === 'AbortError' ? 'stakehub_nav_proof_timeout' : 'stakehub_nav_proof_unavailable',
                message: error?.name === 'AbortError'
                    ? `StakeHub NAV proof read timed out after ${config.read_timeout_ms} ms.`
                    : (error?.message || 'StakeHub NAV proof read failed.'),
                source: 'stakehub:/api/navcoin',
            };
        }
    }

    async function buildStakehubTransparentPreflight() {
        const config = navswapStakehubTransparentConfig();
        if (!config.configured) {
            return {
                ok: false,
                code: 'stakehub_transparent_operator_not_configured',
                message: 'Set NAVSWAP_STAKEHUB_BASE_URL before reading StakeHub transparent preflight state.',
            };
        }

        let balancesUrl;
        let statusUrl;
        try {
            balancesUrl = buildUrl(config.base_url, config.balances_path);
            statusUrl = buildUrl(config.base_url, config.swap_status_path);
        } catch (_) {
            return {
                ok: false,
                code: 'stakehub_transparent_invalid_url',
                message: 'NAVSWAP_STAKEHUB_BASE_URL is not a valid HTTP(S) URL.',
            };
        }

        try {
            const [balances, swapStatus] = await Promise.all([
                fetchJsonWithTimeout(balancesUrl, config.read_timeout_ms),
                fetchJsonWithTimeout(statusUrl, config.read_timeout_ms).catch((error) => ({
                    ok: false,
                    error: error?.message || String(error),
                })),
            ]);
            const errors = Array.isArray(balances?.errors) ? balances.errors.filter(Boolean) : [];
            if (errors.length > 0) {
                return {
                    ok: false,
                    code: 'stakehub_transparent_balances_unavailable',
                    message: `StakeHub transparent balances returned errors: ${errors.join('; ')}`,
                    balances,
                    swap_status: swapStatus,
                };
            }
            const transparentRoundtrip = swapStatus?.transparent_roundtrip || swapStatus?.transparentRoundtrip || null;
            if (
                transparentRoundtrip?.finality_recovery_required === true
                || transparentRoundtrip?.status === 'needs_timeout_certificate'
            ) {
                return {
                    ok: false,
                    code: 'stakehub_transparent_finality_recovery_required',
                    message: transparentRoundtrip?.message
                        || 'StakeHub transparent roundtrip requires PFTL finality recovery before another live run.',
                    balances,
                    swap_status: swapStatus,
                };
            }
            if (
                transparentRoundtrip?.transport_recovery_required === true
                || transparentRoundtrip?.status === 'transport_recovery_required'
            ) {
                return {
                    ok: false,
                    code: 'stakehub_transparent_transport_recovery_required',
                    message: transparentRoundtrip?.message
                        || 'StakeHub transparent roundtrip transport recovery is required before another live run.',
                    balances,
                    swap_status: swapStatus,
                };
            }
            return {
                ok: true,
                balances: {
                    address: balances?.address || null,
                    pfusdc: balances?.pfusdc || null,
                    a651: balances?.a651 || null,
                },
                swap_status: {
                    status: swapStatus?.status || null,
                    ok: swapStatus?.ok ?? null,
                    run_dir: swapStatus?.run_dir || null,
                    error: swapStatus?.error || null,
                    transparent_roundtrip: transparentRoundtrip,
                },
            };
        } catch (error) {
            return {
                ok: false,
                code: error?.name === 'AbortError' ? 'stakehub_transparent_preflight_timeout' : 'stakehub_transparent_preflight_unavailable',
                message: error?.name === 'AbortError'
                    ? `StakeHub transparent preflight timed out after ${config.read_timeout_ms} ms.`
                    : (error?.message || 'StakeHub transparent preflight read failed.'),
            };
        }
    }

    function buildNavswapQuoteResponse(body = {}) {
        const route = navswapRouteFromBody(body);
        const fromAsset = assetIdForNavswapSymbol(body.from_asset || body.from || '');
        const toAsset = assetIdForNavswapSymbol(body.to_asset || body.to || '');
        const amount = body.amount;

        if (route === 'uniswap_atomic_handoff') {
            const bridge = navswapBridgeConfig();
            const requestedPool = body.pool_id || body.pool_id_or_path || body.pool_path || bridge.uniswap_pool_id_or_path;
            if (bridge.legacy_pool_selected || lower(requestedPool) === lower(LEGACY_A651_UNISWAP_POOL_ID)) {
                return {
                    ok: false,
                    schema: NAVSWAP_QUOTE_SCHEMA,
                    route,
                    code: 'legacy_pool_rejected',
                    message: 'The legacy a651/USDC pool cannot be used as the active PFTL-to-Uniswap handoff route.',
                    legacy_pool_id: LEGACY_A651_UNISWAP_POOL_ID,
                    legacy_token: LEGACY_A651_ETH_TOKEN,
                };
            }
            if (!bridge.configured) {
                return {
                    ok: false,
                    schema: NAVSWAP_QUOTE_SCHEMA,
                    route,
                    code: 'bridge_aware_pool_not_configured',
                    message: 'Bridge-aware wrapped NAVCoin token, handoff controller, verifier mode, router, and new Uniswap pool are required before quoting this route.',
                    missing: bridge.missing,
                };
            }
            const beta = navswapUniswapBetaRouteState(bridge);
            if (!beta.quote_enabled) {
                return {
                    ok: false,
                    schema: NAVSWAP_QUOTE_SCHEMA,
                    route,
                    code: 'uniswap_handoff_beta_not_enabled',
                    message: 'PFTL-Uniswap handoff requires explicit CONTROLLED beta enablement, caps, unpaused state, and public routing disabled before quoting.',
                    blockers: beta.blockers,
                    config: bridge,
                };
            }
            const handoff = buildUniswapHandoffQuoteBinding({
                body,
                bridge,
                fromAsset,
                toAsset,
                amount,
                requestedPool,
            });
            if (handoff.ok !== true) {
                return {
                    ok: false,
                    schema: NAVSWAP_QUOTE_SCHEMA,
                    route,
                    code: handoff.code || 'uniswap_handoff_quote_invalid',
                    message: handoff.message || 'Uniswap handoff quote fields are invalid.',
                    missing: handoff.missing || [],
                    config: bridge,
                };
            }
            return {
                ok: true,
                schema: NAVSWAP_QUOTE_SCHEMA,
                route,
                status: beta.status,
                can_run: beta.run_enabled,
                route_family: 'composite_primary_mint_to_ethereum_venue',
                route_trust_class: bridge.route_trust_class,
                release_stage: 'explicit_beta',
                public_routing_enabled: bridge.public_routing_enabled,
                paused: bridge.paused,
                route_supply_cap_atoms: bridge.route_supply_cap_atoms,
                supply_cap_remaining_atoms: bridge.supply_cap_remaining_atoms,
                packet_notional_cap_atoms: bridge.packet_notional_cap_atoms,
                route_config_digest: bridge.route_config_digest,
                from_asset: fromAsset,
                to_asset: toAsset,
                amount,
                mint_and_swap_uniswap: handoff.binding,
                quote_binding_hash: handoff.binding_hash,
                message: beta.run_enabled
                    ? 'Controlled beta handoff quote is ready; run endpoint returns the bounded execution packet.'
                    : 'Controlled beta handoff quote is ready; run execution remains disabled until NAVSWAP_ENABLE_UNISWAP_BETA_RUNS=true.',
                config: bridge,
            };
        }

        if (route === 'stakehub_transparent_roundtrip') {
            const config = navswapStakehubTransparentConfig();
            const parsedAmount = parseStakehubTransparentAmount(amount);
            if (parsedAmount === null) return stakehubTransparentAmountError(route, amount);
            if (Number(parsedAmount) > config.max_whole_a651_amount) {
                return {
                    ok: false,
                    schema: NAVSWAP_QUOTE_SCHEMA,
                    route,
                    code: 'stakehub_transparent_amount_exceeds_limit',
                    message: `StakeHub transparent roundtrip amount exceeds the configured ${config.max_whole_a651_amount} a651 smoke-test limit.`,
                    amount,
                    max_whole_a651_amount: config.max_whole_a651_amount,
                };
            }
            if (!config.configured) {
                return {
                    ok: false,
                    schema: NAVSWAP_QUOTE_SCHEMA,
                    route,
                    code: 'stakehub_transparent_operator_not_configured',
                    message: 'Set NAVSWAP_STAKEHUB_BASE_URL before quoting the existing StakeHub transparent roundtrip.',
                    config,
                };
            }
            return {
                ok: true,
                schema: NAVSWAP_QUOTE_SCHEMA,
                route,
                status: config.runs_enabled ? 'operator_run_enabled' : 'operator_quote_only',
                can_run: config.runs_enabled,
                from_asset: fromAsset,
                to_asset: toAsset,
                amount: parsedAmount,
                expected_output: parsedAmount,
                expected_output_asset: 'a651',
                custody_boundary: config.custody_boundary,
                message: config.runs_enabled
                    ? 'StakeHub transparent roundtrip is configured for operator-backed execution.'
                    : 'StakeHub transparent roundtrip is configured for adapter preflight only; set NAVSWAP_ENABLE_STAKEHUB_TRANSPARENT_RUNS=true to allow live runs.',
                config,
            };
        }

        if (route === 'pftl_atomic_settlement') {
            if (fromAsset && toAsset && !isPftAsset(fromAsset) && !isPftAsset(toAsset)) {
                return {
                    ok: false,
                    schema: NAVSWAP_QUOTE_SCHEMA,
                    route,
                    code: 'issued_to_issued_requires_pft_intermediary',
                    message: 'ESCROW-009 currently supports PFT<->issued-asset templates. Choose PFT as one leg or route through an explicit PFT intermediary.',
                    from_asset: fromAsset,
                    to_asset: toAsset,
                };
            }
            return {
                ok: true,
                schema: NAVSWAP_QUOTE_SCHEMA,
                route,
                status: 'template_ready',
                can_run: false,
                from_asset: fromAsset,
                to_asset: toAsset,
                amount,
                next_endpoint: '/api/navswap/atomic-templates',
                message: 'Atomic settlement template generation is available; each wallet must still sign and submit its own escrow-create leg.',
            };
        }

        if (route === 'transparent_navswap') {
            return {
                ok: false,
                schema: NAVSWAP_QUOTE_SCHEMA,
                route,
                code: 'transparent_navswap_planner_inputs_required',
                message: 'Transparent NAVSwap requires planner-supplied wallet action inputs; refusing to quote instead of falling back to a self-transfer placeholder.',
                next_endpoint: '/api/navswap/actions/prepare-batch',
                required_planner_fields: [
                    'actions[]',
                    'wallet_address',
                    'planner-selected settlement_bucket_id',
                    'planner-selected settlement_receipt_id',
                    'planner-selected consume_supply_allocation_id',
                    'settlement_amount_atoms',
                    'nav_epoch and reserve_packet_hash for redeem actions',
                ],
            };
        }

        if (route === 'shielded_navswap') {
            return {
                ok: false,
                schema: NAVSWAP_QUOTE_SCHEMA,
                route,
                code: 'shielded_navswap_operator_demo_only',
                message: 'Shielded NAVSwap is currently an operator-demo StakeHub flow, not a wallet-safe route.',
            };
        }

        return {
            ok: false,
            schema: NAVSWAP_QUOTE_SCHEMA,
            route,
            code: 'unsupported_navswap_route',
            message: `Unsupported NAVSwap route: ${route}`,
        };
    }

    function navswapProofIsFresh(proof) {
        if (!proof || proof.ok !== true || proof.proof_available !== true) return false;
        if (proof.stale === true) return false;
        const status = lower(proof.proof_status || '');
        return status === '' || status === 'fresh' || status === 'active' || status === 'available';
    }

    async function executeTransparentNavswapQuote(body = {}, rpcRequest = rpcTcpRequest) {
        const route = navswapRouteFromBody(body);
        let quoteBody = body;
        let plannerInputs = null;
        let plannerItems = Array.isArray(quoteBody.actions)
            ? quoteBody.actions
            : Array.isArray(quoteBody.prepared_actions)
                ? quoteBody.prepared_actions
                : Array.isArray(quoteBody.stages)
                    ? quoteBody.stages
                    : null;
        if (!plannerItems || plannerItems.length === 0) {
            if (!navswapActionAutoPlanRequested(body)) {
                return buildNavswapQuoteResponse(body);
            }
            plannerInputs = await planTransparentNavswapWalletActions(body, rpcRequest);
            if (plannerInputs.ok !== true) {
                return {
                    ok: false,
                    schema: NAVSWAP_QUOTE_SCHEMA,
                    route,
                    code: plannerInputs.code || 'transparent_navswap_auto_plan_failed',
                    message: plannerInputs.message || 'Transparent NAVSwap automatic planner input selection failed.',
                    planner_inputs: plannerInputs,
                };
            }
            quoteBody = {
                ...body,
                nav_asset_id: plannerInputs.planner.nav_asset_id,
                settlement_asset_id: plannerInputs.planner.settlement_asset_id,
                actions: plannerInputs.actions,
            };
            plannerItems = plannerInputs.actions;
        }
        const trustSetItem = plannerItems.find(action => action?.stage === 'trust_set') || null;
        if (trustSetItem) {
            return {
                ok: false,
                schema: NAVSWAP_QUOTE_SCHEMA,
                route,
                code: 'transparent_navswap_trust_set_not_supported',
                message: 'Transparent NAVSwap no longer accepts trust_set planner actions.',
                rejected_stage: 'trust_set',
            };
        }
        const prepared = await prepareNavswapWalletActionBatch(quoteBody, rpcRequest);
        if (prepared.ok !== true) {
            return {
                ok: false,
                schema: NAVSWAP_QUOTE_SCHEMA,
                route,
                code: prepared.code || 'transparent_navswap_action_batch_prepare_failed',
                message: prepared.message || 'Transparent NAVSwap action batch preparation failed.',
                action_batch_prepare: prepared,
            };
        }
        const planner = plannerInputs?.planner || null;
        const subscriptionAction = plannerItems.find(action => action?.stage === 'nav_subscription_allocate') || null;
        const redeemAction = plannerItems.find(action => action?.stage === 'nav_redeem_at_nav') || null;
        const direction = planner?.direction || (redeemAction ? 'redeem' : 'subscribe');
        const fromAsset = assetIdForNavswapSymbol(
            quoteBody.from_asset
            || quoteBody.from_asset_id
            || quoteBody.from
            || quoteBody.settlement_asset_id
            || planner?.settlement_asset_id
            || '',
        );
        const toAsset = assetIdForNavswapSymbol(
            quoteBody.to_asset
            || quoteBody.to_asset_id
            || quoteBody.to
            || quoteBody.nav_asset_id
            || planner?.nav_asset_id
            || '',
        );
        const inputAmountAtoms = direction === 'redeem'
            ? (quoteBody.redeem_amount_atoms
                || redeemAction?.redeem_amount_atoms
                || planner?.amount_atoms
                || quoteBody.amount_atoms
                || quoteBody.amount
                || null)
            : (quoteBody.settlement_amount_atoms
                || subscriptionAction?.settlement_amount_atoms
                || planner?.amount_atoms
                || quoteBody.amount_atoms
                || quoteBody.amount
                || null);
        const settlementAmountAtoms = quoteBody.settlement_amount_atoms
            || subscriptionAction?.settlement_amount_atoms
            || planner?.settlement_amount_atoms
            || (planner?.direction === 'subscribe' ? planner.amount_atoms : null);
        const redeemAmountAtoms = quoteBody.redeem_amount_atoms
            || redeemAction?.redeem_amount_atoms
            || (planner?.direction === 'redeem' ? planner.amount_atoms : null)
            || (direction === 'redeem' ? quoteBody.amount_atoms : null);
        const mintAmountAtoms = quoteBody.mint_amount_atoms
            || quoteBody.nav_amount_atoms
            || quoteBody.expected_output_atoms
            || planner?.mint_amount_atoms
            || null;
        const defaultExpectedOutput = direction === 'redeem'
            ? (settlementAmountAtoms === null ? null : String(settlementAmountAtoms))
            : (mintAmountAtoms === null ? null : String(mintAmountAtoms));
        const expectedOutput = quoteBody.expected_output === undefined
            || quoteBody.expected_output === null
            || String(quoteBody.expected_output).trim() === ''
            ? defaultExpectedOutput
            : String(quoteBody.expected_output);
        return {
            ok: true,
            schema: NAVSWAP_QUOTE_SCHEMA,
            route,
            status: 'prepared_actions_ready',
            can_run: false,
            route_family: direction === 'subscribe' ? NAVSWAP_PRIMARY_MINT_ROUTE_FAMILY : 'primary_pftl_redeem',
            route_trust_class: 'CONTROLLED',
            pricing_source: direction === 'subscribe' ? 'finalized_pre_inflow_nav_snapshot' : 'finalized_nav_snapshot',
            supply_effect: direction === 'subscribe'
                ? 'mints_new_native_navcoin_supply'
                : 'burns_or_retires_native_navcoin_supply',
            uniswap_supply_effect: 'not_uniswap_supply',
            requires_wallet_submit: true,
            custody_boundary: 'wallet-local-signing',
            from_asset: fromAsset,
            to_asset: toAsset,
            direction,
            amount: quoteBody.amount === undefined || quoteBody.amount === null ? null : String(quoteBody.amount),
            input_amount_atoms: inputAmountAtoms === null ? null : String(inputAmountAtoms),
            settlement_amount_atoms: settlementAmountAtoms === null ? null : String(settlementAmountAtoms),
            redeem_amount_atoms: redeemAmountAtoms === null ? null : String(redeemAmountAtoms),
            mint_amount_atoms: mintAmountAtoms === null ? null : String(mintAmountAtoms),
            expected_output: expectedOutput,
            expected_output_asset: expectedOutput === null ? null : (toAsset || null),
            ...(expectedOutput === null ? {
                expected_output_unavailable_reason: 'operator_nav_mint_at_nav_not_prepared',
            } : {}),
            ...(plannerInputs?.operator_completion ? { operator_completion: plannerInputs.operator_completion } : {}),
            next_step: 'wallet_verify_sign_submit_prepared_actions',
            message: 'Transparent NAVSwap planner supplied a canonical wallet action batch. The wallet must verify, sign, and submit the actions locally.',
            ...(plannerInputs ? { planner_inputs: plannerInputs } : {}),
            prepared_action_batch: prepared,
        };
    }

    async function preflightNavswapPreparedActionFees(preparedActions, rpcRequest = rpcTcpRequest) {
        if (!Array.isArray(preparedActions) || preparedActions.length === 0) {
            return {
                ok: true,
                status: 'no_prepared_actions',
                action_count: 0,
                total_minimum_fee_atoms: '0',
                actions: [],
                failed_action: null,
            };
        }

        const actions = [];
        let totalMinimumFeeAtoms = 0n;
        for (const action of preparedActions) {
            const entry = {
                action_id: action?.action_id || null,
                stage: action?.stage || null,
                operation: action?.operation?.operation || null,
                source: action?.source || null,
                ok: false,
            };
            try {
                if (!action?.source || !action?.operation) {
                    throw new Error('prepared action missing source or operation');
                }
                const feeQuote = await navswapRpcRead('asset_fee_quote', {
                    source: action.source,
                    operation_json: JSON.stringify(action.operation),
                }, rpcRequest);
                const minimumFee = BigInt(String(feeQuote.minimum_fee ?? 0));
                totalMinimumFeeAtoms += minimumFee;
                const sourceMatches = !feeQuote.source || feeQuote.source === action.source;
                const operationMatches = !feeQuote.operation
                    || navswapStableJson(feeQuote.operation) === navswapStableJson(action.operation);
                const senderMeetsReserveAfterFee = feeQuote.sender_meets_reserve_after_fee !== false;
                const senderMeetsReserveAndReserve = feeQuote.sender_meets_reserve_after_fee_and_reserve !== false;
                entry.ok = sourceMatches
                    && operationMatches
                    && senderMeetsReserveAfterFee
                    && senderMeetsReserveAndReserve;
                entry.minimum_fee_atoms = minimumFee.toString();
                entry.account_reserve_atoms = feeQuote.account_reserve === undefined || feeQuote.account_reserve === null
                    ? null
                    : String(feeQuote.account_reserve);
                entry.sender_meets_reserve_after_fee = feeQuote.sender_meets_reserve_after_fee ?? null;
                entry.sender_meets_reserve_after_fee_and_reserve = feeQuote.sender_meets_reserve_after_fee_and_reserve ?? null;
                entry.source_matches = sourceMatches;
                entry.operation_matches = operationMatches;
                entry.quote = feeQuote;
            } catch (error) {
                entry.ok = false;
                entry.code = error.code || 'navswap_action_fee_quote_failed';
                entry.message = error.message || 'NAVSwap action fee quote failed.';
            }
            actions.push(entry);
        }

        const failed = actions.find(action => action.ok !== true) || null;
        return {
            ok: !failed,
            status: failed ? 'fee_preflight_failed' : 'fee_preflight_ready',
            action_count: actions.length,
            total_minimum_fee_atoms: totalMinimumFeeAtoms.toString(),
            actions,
            failed_action: failed
                ? {
                    action_id: failed.action_id,
                    stage: failed.stage,
                    code: failed.code || 'navswap_action_fee_preflight_failed',
                    message: failed.message || 'NAVSwap wallet action fee/reserve preflight failed.',
                }
                : null,
        };
    }

    async function executeTransparentNavswapReadiness(body = {}, rpcRequest = rpcTcpRequest) {
        const route = navswapRouteFromBody(body);
        if (route !== 'transparent_navswap') {
            return {
                ok: false,
                schema: NAVSWAP_READINESS_SCHEMA,
                route,
                code: 'unsupported_navswap_readiness_route',
                message: 'NAVSwap readiness is currently available only for transparent_navswap.',
            };
        }

        const walletAddress = parseNavswapWalletAddress(body.wallet_address || body.owner || body.source);
        const fromAsset = assetIdForNavswapSymbol(body.from_asset || body.from_asset_id || body.from || 'pfUSDC');
        const toAsset = assetIdForNavswapSymbol(body.to_asset || body.to_asset_id || body.to || 'a651');
        const capabilities = navswapCapabilities();
        const routeCapability = capabilities.routes.transparent_navswap;
        const quote = await executeTransparentNavswapQuote({
            ...body,
            route,
            from_asset: fromAsset,
            to_asset: toAsset,
            wallet_address: walletAddress,
            auto_plan: body.auto_plan ?? true,
        }, rpcRequest).catch((error) => ({
            ok: false,
            schema: NAVSWAP_QUOTE_SCHEMA,
            route,
            code: error.code || 'transparent_navswap_quote_failed',
            message: error.message || 'Transparent NAVSwap quote failed.',
        }));

        const quoteDirection = quote?.direction
            || quote?.planner_inputs?.planner?.direction
            || (fromAsset === PFUSDC_ASSET_ID ? 'subscribe' : 'redeem');
        const requiredSettlementAtoms = quote?.settlement_amount_atoms === undefined || quote?.settlement_amount_atoms === null
            ? '0'
            : String(quote.settlement_amount_atoms);
        const requiredWalletSpendAtoms = quoteDirection === 'redeem'
            ? (quote?.redeem_amount_atoms === undefined || quote?.redeem_amount_atoms === null
                ? '0'
                : String(quote.redeem_amount_atoms))
            : requiredSettlementAtoms;
        const preparedActions = Array.isArray(quote?.prepared_action_batch?.actions)
            ? quote.prepared_action_batch.actions
            : [];
        const walletActionFeePreflight = await preflightNavswapPreparedActionFees(preparedActions, rpcRequest);
        let accountAssets = null;
        let accountState = null;
        let settlementAssetInfo = null;
        let settlementIssuer = null;
        let nativeBalanceAtoms = '0';
        let settlementBalanceAtoms = '0';
        const warnings = [];

        try {
            [accountState, accountAssets, settlementAssetInfo] = await Promise.all([
                navswapRpcRead('account', { address: walletAddress }, rpcRequest),
                navswapRpcRead('account_assets', { account: walletAddress }, rpcRequest),
                isIssuedAsset(fromAsset)
                    ? navswapRpcRead('asset_info', { asset_id: fromAsset }, rpcRequest)
                    : Promise.resolve(null),
            ]);
            nativeBalanceAtoms = navswapNativeAccountBalanceAtoms(accountState).toString();
            settlementIssuer = settlementAssetInfo ? navswapAssetInfoIssuer(settlementAssetInfo) : null;
            settlementBalanceAtoms = navswapAccountBalanceAtoms(accountAssets, fromAsset).toString();
        } catch (error) {
            warnings.push({
                code: error.code || 'transparent_navswap_readiness_state_unavailable',
                message: error.message || 'Transparent NAVSwap account state is unavailable.',
            });
        }

        const requiredWalletSpendBig = BigInt(requiredWalletSpendAtoms);
        const settlementBalanceBig = BigInt(settlementBalanceAtoms);
        const settlementSufficient = settlementBalanceBig >= requiredWalletSpendBig;
        const settlementShortfallAtoms = settlementSufficient
            ? 0n
            : requiredWalletSpendBig - settlementBalanceBig;
        const fundingConfig = navswapDevnetPfusdcFundingConfig();
        const fundingIssuerKeyAddress = readNavswapKeyFileAddress(fundingConfig.issuer_key_file);
        const fundingIssuerMatchesAsset = fundingIssuerKeyAddress ? fundingIssuerKeyAddress === settlementIssuer : null;
        const fundingMaxAtoms = BigInt(fundingConfig.max_amount_atoms);
        const fundingUsage = navswapDevnetFundingUsageSnapshot(walletAddress, fundingConfig);
        const fundingWindowRemainingAtoms = BigInt(fundingUsage.remaining_atoms);
        const fundingAvailable = fundingConfig.enabled
            && fundingConfig.signing_configured
            && quoteDirection === 'subscribe'
            && fromAsset === PFUSDC_ASSET_ID
            && quote.ok === true
            && !settlementSufficient
            && settlementShortfallAtoms > 0n
            && settlementShortfallAtoms <= fundingMaxAtoms
            && settlementShortfallAtoms <= fundingWindowRemainingAtoms
            && fundingIssuerMatchesAsset === true;
        const nextSteps = [];
        if (routeCapability.can_run !== true) {
            nextSteps.push(...routeCapability.required_next);
        }
        if (quote.ok !== true) {
            nextSteps.push(quote.message || 'get a transparent NAVSwap quote');
        }
        if (quote.ok === true && preparedActions.length > 0 && walletActionFeePreflight.ok !== true) {
            nextSteps.push('fund the wallet with PFT for NAVSwap fees/reserves');
        }
        if (quote.ok === true && !settlementSufficient) {
            nextSteps.push(fundingAvailable
                ? 'request guarded devnet pfUSDC funding'
                : 'fund the wallet with the required settlement asset');
        }
        if (quote.ok === true && settlementSufficient && preparedActions.length > 0 && routeCapability.can_run === true) {
            nextSteps.push('submit the prepared wallet-owned actions');
        }

        const canExecute = routeCapability.can_run === true
            && quote.ok === true
            && walletActionFeePreflight.ok === true
            && settlementSufficient
            && preparedActions.length > 0;

        return {
            ok: true,
            schema: NAVSWAP_READINESS_SCHEMA,
            route,
            wallet_address: walletAddress,
            from_asset: fromAsset,
            to_asset: toAsset,
            direction: quoteDirection,
            status: canExecute ? 'ready_to_submit_wallet_actions' : 'not_ready',
            can_execute: canExecute,
            capabilities: {
                status: routeCapability.status,
                can_quote: routeCapability.can_quote,
                can_run: routeCapability.can_run,
                required_next: routeCapability.required_next,
            },
            quote,
            wallet_pft: {
                balance_atoms: nativeBalanceAtoms,
                fee_preflight: walletActionFeePreflight,
                sufficient_for_prepared_actions: walletActionFeePreflight.ok === true,
            },
            required_settlement_atoms: requiredSettlementAtoms,
            required_wallet_spend_atoms: requiredWalletSpendAtoms,
            wallet_spend_asset: {
                asset_id: fromAsset,
                balance_atoms: settlementBalanceAtoms,
                sufficient: settlementSufficient,
                shortfall_atoms: settlementShortfallAtoms.toString(),
            },
            settlement_asset: {
                asset_id: fromAsset,
                issuer: settlementIssuer,
                balance_atoms: settlementBalanceAtoms,
                sufficient: settlementSufficient,
                shortfall_atoms: settlementShortfallAtoms.toString(),
            },
            funding: {
                enabled: fundingConfig.enabled,
                signing_configured: fundingConfig.signing_configured,
                available: fundingAvailable,
                endpoint: fundingConfig.endpoint,
                asset_id: fundingConfig.asset_id,
                amount_atoms: settlementShortfallAtoms.toString(),
                max_amount_atoms: fundingConfig.max_amount_atoms,
                max_recipient_window_atoms: fundingConfig.max_recipient_window_atoms,
                recipient_window_ms: fundingConfig.recipient_window_ms,
                recipient_window_used_atoms: fundingUsage.used_atoms,
                recipient_window_remaining_atoms: fundingUsage.remaining_atoms,
                recipient_window_reset_at_ms: fundingUsage.reset_at_ms,
                issuer_key_matches_asset: fundingIssuerMatchesAsset,
                unavailable_reason: fundingAvailable
                    ? null
                    : !fundingConfig.enabled
                        ? 'devnet_funding_disabled'
                        : !fundingConfig.signing_configured
                            ? 'issuer_key_not_configured'
                            : fromAsset !== PFUSDC_ASSET_ID
                                ? 'settlement_asset_not_pfusdc'
                                : settlementSufficient
                                    ? 'settlement_already_sufficient'
                                    : settlementShortfallAtoms > fundingMaxAtoms
                                        ? 'shortfall_exceeds_cap'
                                        : settlementShortfallAtoms > fundingWindowRemainingAtoms
                                            ? 'recipient_window_cap_exceeded'
                                            : fundingIssuerMatchesAsset !== true
                                                ? 'issuer_key_mismatch'
                                                : 'funding_unavailable',
            },
            prepared_action_count: preparedActions.length,
            prepared_stages: preparedActions.map(action => action?.stage).filter(Boolean),
            next_steps: [...new Set(nextSteps.filter(Boolean))],
            warnings,
        };
    }

    async function executeNavswapDevnetPfusdcFunding(body = {}, rpcRequest = rpcTcpRequest) {
        const config = navswapDevnetPfusdcFundingConfig();
        const route = navswapRouteFromBody(body);
        if (route !== 'transparent_navswap') {
            return {
                ok: false,
                schema: NAVSWAP_DEVNET_FUNDING_SCHEMA,
                route,
                code: 'unsupported_navswap_funding_route',
                message: 'Devnet pfUSDC settlement funding is available only for transparent_navswap.',
            };
        }
        if (!config.enabled) {
            return {
                ok: false,
                schema: NAVSWAP_DEVNET_FUNDING_SCHEMA,
                route,
                code: 'devnet_pfusdc_funding_disabled',
                message: 'Devnet pfUSDC settlement funding is disabled. Set NAVSWAP_ENABLE_DEVNET_PFUSDC_FUNDING=true to enable this capped helper.',
            };
        }
        if (!config.signing_configured) {
            return {
                ok: false,
                schema: NAVSWAP_DEVNET_FUNDING_SCHEMA,
                route,
                code: 'devnet_pfusdc_funding_key_missing',
                message: 'Devnet pfUSDC settlement funding requires NAVSWAP_PFUSDC_ISSUER_KEY_FILE or NAVSWAP_OPERATOR_ISSUER_KEY_FILE.',
            };
        }

        try {
            const walletAddress = parseNavswapWalletAddress(body.wallet_address || body.recipient || body.owner || body.source);
            const {
                amount_atoms: _fundingAmountAtoms,
                funding_amount_atoms: _explicitFundingAmountAtoms,
                ...readinessBody
            } = body;
            const readiness = await executeTransparentNavswapReadiness({
                ...readinessBody,
                route,
                wallet_address: walletAddress,
                from_asset: body.from_asset || body.from_asset_id || 'pfUSDC',
                to_asset: body.to_asset || body.to_asset_id || 'a651',
                auto_plan: body.auto_plan ?? true,
            }, rpcRequest);
            if (readiness.ok !== true || readiness.quote?.ok !== true) {
                return {
                    ok: false,
                    schema: NAVSWAP_DEVNET_FUNDING_SCHEMA,
                    route,
                    code: 'devnet_pfusdc_funding_quote_unavailable',
                    message: readiness.message || readiness.quote?.message || 'Transparent NAVSwap readiness did not return a usable quote.',
                    readiness,
                };
            }
            if (readiness.settlement_asset?.asset_id !== PFUSDC_ASSET_ID) {
                return {
                    ok: false,
                    schema: NAVSWAP_DEVNET_FUNDING_SCHEMA,
                    route,
                    code: 'devnet_pfusdc_funding_asset_mismatch',
                    message: 'Devnet funding helper only funds canonical pfUSDC settlement routes.',
                    readiness,
                };
            }
            if (readiness.settlement_asset?.sufficient === true) {
                return {
                    ok: false,
                    schema: NAVSWAP_DEVNET_FUNDING_SCHEMA,
                    route,
                    code: 'devnet_pfusdc_funding_not_needed',
                    message: 'Wallet already has enough pfUSDC for this transparent NAVSwap quote.',
                    readiness,
                };
            }
            const shortfallAtoms = BigInt(String(readiness.settlement_asset?.shortfall_atoms || readiness.funding?.amount_atoms || '0'));
            const requestedAtoms = body.amount_atoms || body.funding_amount_atoms
                ? BigInt(String(body.amount_atoms || body.funding_amount_atoms))
                : shortfallAtoms;
            if (requestedAtoms <= 0n) {
                return {
                    ok: false,
                    schema: NAVSWAP_DEVNET_FUNDING_SCHEMA,
                    route,
                    code: 'devnet_pfusdc_funding_amount_invalid',
                    message: 'Devnet pfUSDC funding amount must be positive.',
                    readiness,
                };
            }
            if (requestedAtoms > shortfallAtoms) {
                return {
                    ok: false,
                    schema: NAVSWAP_DEVNET_FUNDING_SCHEMA,
                    route,
                    code: 'devnet_pfusdc_funding_overfund_refused',
                    message: `Requested pfUSDC funding ${requestedAtoms} exceeds the current route shortfall ${shortfallAtoms}.`,
                    readiness,
                };
            }
            const maxAtoms = BigInt(config.max_amount_atoms);
            if (requestedAtoms > maxAtoms) {
                return {
                    ok: false,
                    schema: NAVSWAP_DEVNET_FUNDING_SCHEMA,
                    route,
                    code: 'devnet_pfusdc_funding_cap_exceeded',
                    message: `Requested pfUSDC funding ${requestedAtoms} exceeds cap ${config.max_amount_atoms}.`,
                    readiness,
                };
            }
            if (requestedAtoms > BigInt(Number.MAX_SAFE_INTEGER)) {
                return {
                    ok: false,
                    schema: NAVSWAP_DEVNET_FUNDING_SCHEMA,
                    route,
                    code: 'devnet_pfusdc_funding_amount_unsafe',
                    message: 'Requested pfUSDC funding amount exceeds the asset operation safe integer range.',
                    readiness,
                };
            }
            const issuer = readiness.settlement_asset.issuer;
            const issuerKeyAddress = readNavswapKeyFileAddress(config.issuer_key_file);
            if (!issuer || issuerKeyAddress !== issuer) {
                return {
                    ok: false,
                    schema: NAVSWAP_DEVNET_FUNDING_SCHEMA,
                    route,
                    code: 'devnet_pfusdc_funding_issuer_key_mismatch',
                    message: `Configured pfUSDC issuer key does not match canonical pfUSDC issuer ${issuer || 'unknown'}.`,
                    readiness,
                    issuer_key_address: issuerKeyAddress,
                };
            }

            const operation = {
                operation: 'issued_payment',
                from: issuer,
                to: walletAddress,
                issuer,
                asset_id: PFUSDC_ASSET_ID,
                amount: Number(requestedAtoms),
            };
            const quote = await navswapRpcRead('asset_fee_quote', {
                source: issuer,
                operation_json: JSON.stringify(operation),
            }, rpcRequest);
            if (quote.source && quote.source !== issuer) {
                return {
                    ok: false,
                    schema: NAVSWAP_DEVNET_FUNDING_SCHEMA,
                    route,
                    code: 'devnet_pfusdc_funding_quote_source_mismatch',
                    message: 'pfUSDC funding fee quote source does not match the issuer.',
                    readiness,
                    operation,
                    quote,
                };
            }
            if (quote.operation && navswapStableJson(quote.operation) !== navswapStableJson(operation)) {
                return {
                    ok: false,
                    schema: NAVSWAP_DEVNET_FUNDING_SCHEMA,
                    route,
                    code: 'devnet_pfusdc_funding_quote_operation_mismatch',
                    message: 'pfUSDC funding fee quote operation does not match the reviewed funding operation.',
                    readiness,
                    operation,
                    quote,
                };
            }
            if (quote.sender_meets_reserve_after_fee === false) {
                return {
                    ok: false,
                    schema: NAVSWAP_DEVNET_FUNDING_SCHEMA,
                    route,
                    code: 'devnet_pfusdc_funding_issuer_fee_balance_insufficient',
                    message: 'Configured pfUSDC issuer account does not have enough PFT for the funding transaction fee.',
                    readiness,
                    operation,
                    quote,
                };
            }
            const reservation = reserveNavswapDevnetFundingUsage(walletAddress, requestedAtoms, config);
            if (reservation.ok !== true) {
                return {
                    ok: false,
                    schema: NAVSWAP_DEVNET_FUNDING_SCHEMA,
                    route,
                    code: 'devnet_pfusdc_funding_recipient_window_exceeded',
                    message: `Requested pfUSDC funding ${requestedAtoms} exceeds the remaining recipient funding window ${reservation.snapshot.remaining_atoms}.`,
                    readiness,
                    operation,
                    quote,
                    recipient_window: reservation.snapshot,
                };
            }
            let fundingSubmitted = false;

            const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'postfiat-navswap-funding-'));
            const quoteFile = path.join(dir, 'quote.json');
            try {
                fs.writeFileSync(quoteFile, JSON.stringify(quote, null, 2), { mode: 0o600 });
                const { stdout } = await execFileAsync(
                    config.node_bin,
                    [
                        'wallet-sign-asset-transaction',
                        '--key-file',
                        config.issuer_key_file,
                        '--quote-file',
                        quoteFile,
                    ],
                    {
                        timeout: config.timeout_ms,
                        maxBuffer: 2 * 1024 * 1024,
                    },
                );
                const signed = JSON.parse(stdout);
                const submitMethod = 'mempool_submit_signed_asset_transaction_finality';
                const submitRequest = {
                    version: 'postfiat-local-rpc-v1',
                    id: `navswap-devnet-funding-submit-${Date.now()}`,
                    method: submitMethod,
                    params: {
                        signed_asset_transaction_json: JSON.stringify(signed),
                    },
                };
                const target = rpcRequest === rpcTcpRequest
                    ? await resolveRpcTarget(submitMethod)
                    : {
                        endpoint: { validatorId: 'test', host: RPC_HOST, port: RPC_PORT },
                        route: null,
                    };
                const outbound = requestWithProxyReadiness(submitRequest, target.route);
                const submitResponse = await rpcRequest(target.endpoint.host, target.endpoint.port, outbound);
                if (submitResponse.ok === true && target.route) {
                    const line = JSON.stringify(submitResponse);
                    rememberFinalizedReadEndpoint(line, target);
                    primeNextProposerRouteCacheFromResponse(line, target.route, {
                        warmReadiness: true,
                    });
                }
                if (submitResponse.ok !== true) {
                    return {
                        ok: false,
                        schema: NAVSWAP_DEVNET_FUNDING_SCHEMA,
                        route,
                        code: submitResponse.error?.code || 'devnet_pfusdc_funding_submit_failed',
                        message: submitResponse.error?.message || 'Devnet pfUSDC funding submit failed.',
                        readiness,
                        operation,
                        quote,
                        rpc_error: submitResponse.error || null,
                    };
                }
                fundingSubmitted = true;
                return {
                    ok: true,
                    schema: NAVSWAP_DEVNET_FUNDING_SCHEMA,
                    route,
                    status: 'funding_submitted',
                    message: 'Devnet pfUSDC settlement funding submitted.',
                    recipient: walletAddress,
                    asset_id: PFUSDC_ASSET_ID,
                    issuer,
                    amount_atoms: requestedAtoms.toString(),
                    before_balance_atoms: String(readiness.settlement_asset.balance_atoms || '0'),
                    required_settlement_atoms: String(readiness.required_settlement_atoms || '0'),
                    tx_id: submitResponse.result?.tx_id || null,
                    submit_result: submitResponse.result,
                    recipient_window: navswapDevnetFundingUsageSnapshot(walletAddress, config),
                    readiness,
                    operation,
                    quote,
                };
            } finally {
                if (!fundingSubmitted) {
                    releaseNavswapDevnetFundingUsage(walletAddress, requestedAtoms, config);
                }
                try { fs.rmSync(quoteFile, { force: true }); } catch (_) {}
                try { fs.rmdirSync(dir); } catch (_) {}
            }
        } catch (error) {
            return {
                ok: false,
                schema: NAVSWAP_DEVNET_FUNDING_SCHEMA,
                route,
                code: error.code || 'devnet_pfusdc_funding_failed',
                message: error.message || 'Devnet pfUSDC funding failed.',
            };
        }
    }

    function transparentCompletionError(code, message, extra = {}) {
        const err = new Error(message);
        err.code = code;
        Object.assign(err, extra);
        return err;
    }

    function transparentCompletionQuote(body = {}) {
        const quote = body.quote || body.route_quote || body.navswap_quote || null;
        if (!quote || typeof quote !== 'object' || quote.ok !== true) {
            throw transparentCompletionError(
                'transparent_navswap_quote_required',
                'Transparent NAVSwap completion requires the prepared quote returned by /api/navswap/quotes.',
            );
        }
        if (!quote.operator_completion || typeof quote.operator_completion !== 'object') {
            throw transparentCompletionError(
                'transparent_navswap_operator_completion_missing',
                'Transparent NAVSwap quote does not include an operator completion template.',
            );
        }
        return quote;
    }

    function transparentCompletionWalletResult(body = {}) {
        const result = body.wallet_action_result
            || body.wallet_submission
            || body.action_submit_result
            || body.submission_result
            || null;
        if (!result || typeof result !== 'object') {
            throw transparentCompletionError(
                'transparent_navswap_wallet_result_required',
                'Transparent NAVSwap completion requires the wallet-owned action submit result.',
            );
        }
        return result;
    }

    function transparentCompletionPreparedAction(quote, stage) {
        const actions = Array.isArray(quote?.prepared_action_batch?.actions)
            ? quote.prepared_action_batch.actions
            : [];
        return actions.find((action) => action?.stage === stage) || null;
    }

    function transparentCompletionSubmission(result, stage) {
        const submissions = Array.isArray(result?.submissions)
            ? result.submissions
            : Array.isArray(result?.results)
                ? result.results
                : [];
        return submissions.find((submission) => submission?.navswap_action?.stage === stage) || null;
    }

    function transparentCompletionStage(quote) {
        if (transparentCompletionPreparedAction(quote, 'nav_subscription_allocate')) {
            return 'nav_subscription_allocate';
        }
        if (transparentCompletionPreparedAction(quote, 'nav_redeem_at_nav')) {
            return 'nav_redeem_at_nav';
        }
        throw transparentCompletionError(
            'transparent_navswap_wallet_action_missing',
            'Transparent NAVSwap quote does not contain a supported prepared wallet action.',
        );
    }

    function verifyTransparentWalletCompletionInput(body = {}) {
        const quote = transparentCompletionQuote(body);
        const result = transparentCompletionWalletResult(body);
        const stage = transparentCompletionStage(quote);
        const prepared = transparentCompletionPreparedAction(quote, stage);
        const submission = transparentCompletionSubmission(result, stage);
        if (!prepared?.operation) {
            throw transparentCompletionError(
                'transparent_navswap_wallet_action_missing',
                `Transparent NAVSwap quote does not contain a prepared ${stage} action.`,
            );
        }
        if (!submission?.navswap_action?.operation) {
            throw transparentCompletionError(
                'transparent_navswap_wallet_submission_missing',
                `Transparent NAVSwap wallet result does not contain the submitted ${stage} action.`,
            );
        }
        if (submission.receipt?.accepted === false) {
            throw transparentCompletionError(
                'transparent_navswap_wallet_action_rejected',
                `The wallet-owned ${stage} transaction was rejected.`,
                { receipt: submission.receipt },
            );
        }
        if (navswapStableJson(prepared.operation) !== navswapStableJson(submission.navswap_action.operation)) {
            throw transparentCompletionError(
                'transparent_navswap_wallet_operation_mismatch',
                `Submitted ${stage} operation does not match the prepared quote action.`,
            );
        }
        return { quote, result, prepared, submission, stage };
    }

    function navswapCompletionOperationTemplate(completion = {}) {
        const template = completion.operation_template || completion.operation || null;
        if (!template || typeof template !== 'object') {
            throw transparentCompletionError(
                'transparent_navswap_operator_template_missing',
                'Transparent NAVSwap operator completion is missing nav_mint_at_nav operation_template.',
            );
        }
        if (template.operation !== 'nav_mint_at_nav') {
            throw transparentCompletionError(
                'transparent_navswap_operator_template_invalid',
                'Transparent NAVSwap operator completion template must be nav_mint_at_nav.',
            );
        }
        return template;
    }

    function navswapCompletionConsumerIds(template, lookup = {}) {
        const navAssetId = parseNavswapHexId(template.asset_id, 'operator_completion.asset_id');
        const to = parseNavswapWalletAddress(template.to);
        return new Set([
            lookup.consumer_id,
            lookup.fallback_consumer_id,
            lookup.legacy_consumer_id,
            `nav_subscription:${navAssetId}:${to}`,
            `nav_subscription:${navAssetId}`,
        ].filter(Boolean));
    }

    async function verifyTransparentNavSubscriptionAllocation(completion = {}, rpcRequest = rpcTcpRequest) {
        const template = navswapCompletionOperationTemplate(completion);
        const lookup = completion.allocation_lookup || {};
        const settlementAssetId = parseNavswapHexId(template.settlement_asset_id, 'operator_completion.settlement_asset_id');
        const settlementBucketId = parseNavswapHexId(
            template.settlement_bucket_id || lookup.settlement_bucket_id,
            'operator_completion.settlement_bucket_id',
        );
        const settlementReceiptId = parseNavswapHexId(
            lookup.settlement_receipt_id || template.settlement_receipt_id,
            'operator_completion.settlement_receipt_id',
        );
        const settlementAmountAtoms = parseNavswapActionInteger(
            template.settlement_amount_atoms || lookup.settlement_amount_atoms,
            'operator_completion.settlement_amount_atoms',
        );
        const consumerIds = navswapCompletionConsumerIds(template, lookup);
        const status = await navswapRpcRead('vault_bridge_status', { asset_id: settlementAssetId }, rpcRequest);
        const allocations = Array.isArray(status?.allocations) ? status.allocations : [];
        const candidates = allocations
            .filter((allocation) => (
                allocation?.purpose === VAULT_BRIDGE_ALLOCATION_PURPOSE_NAV_SUBSCRIPTION
                && allocation.bucket_id === settlementBucketId
                && allocation.receipt_id === settlementReceiptId
                && consumerIds.has(allocation.consumer_id)
                && navswapPlannerNumber(allocation.amount_atoms, 'allocation.amount_atoms') === settlementAmountAtoms
                && navswapPlannerNumber(allocation.retired_at_height || 0, 'allocation.retired_at_height') === 0
            ))
            .sort((left, right) => (
                navswapPlannerNumber(right.created_at_height || 0, 'allocation.created_at_height')
                - navswapPlannerNumber(left.created_at_height || 0, 'allocation.created_at_height')
            ) || String(left.allocation_id).localeCompare(String(right.allocation_id)));
        if (candidates.length === 0) {
            throw transparentCompletionError(
                'transparent_navswap_subscription_allocation_missing',
                'No live nav_subscription allocation matching the wallet submission is visible in vault_bridge_status yet.',
                {
                    lookup: {
                        settlement_asset_id: settlementAssetId,
                        settlement_bucket_id: settlementBucketId,
                        settlement_receipt_id: settlementReceiptId,
                        settlement_amount_atoms: String(settlementAmountAtoms),
                        consumer_ids: Array.from(consumerIds),
                    },
                    vault_bridge_status: {
                        asset_id: status?.asset_id || settlementAssetId,
                        allocation_count: status?.allocation_count ?? allocations.length,
                    },
                },
            );
        }
        const allocation = candidates[0];
        return {
            allocation,
            status,
            operation: {
                ...template,
                settlement_bucket_id: settlementBucketId,
                settlement_allocation_id: allocation.allocation_id,
                settlement_amount_atoms: settlementAmountAtoms,
            },
        };
    }

    function buildTransparentNavswapReceiptVerification({
        quote,
        walletResult,
        subscriptionSubmission,
        verifiedAllocation,
        operatorResult = null,
    } = {}) {
        const prepared = transparentCompletionPreparedAction(quote, 'nav_subscription_allocate');
        const submittedOperation = subscriptionSubmission?.navswap_action?.operation || null;
        const preparedOperation = prepared?.operation || null;
        const walletReceipt = subscriptionSubmission?.receipt || null;
        const allocation = verifiedAllocation?.allocation || null;
        const verifiedOperation = verifiedAllocation?.operation || null;
        const operatorOperation = operatorResult?.operation || null;
        const operatorQuoteOperation = operatorResult?.quote?.operation || null;
        const txIds = Array.isArray(walletResult?.submissions)
            ? walletResult.submissions.map(item => item?.txId).filter(Boolean)
            : [];
        const checks = {
            prepared_allocation_action_present: Boolean(preparedOperation),
            wallet_submission_present: Boolean(submittedOperation),
            wallet_submission_matches_prepared: Boolean(
                preparedOperation
                && submittedOperation
                && navswapStableJson(preparedOperation) === navswapStableJson(submittedOperation),
            ),
            wallet_submission_receipt_accepted: walletReceipt?.accepted === true,
            live_allocation_visible: Boolean(allocation?.allocation_id),
            live_allocation_matches_quote: Boolean(
                allocation
                && verifiedOperation
                && allocation.allocation_id === verifiedOperation.settlement_allocation_id
                && String(allocation.amount_atoms) === String(verifiedOperation.settlement_amount_atoms)
                && allocation.bucket_id === verifiedOperation.settlement_bucket_id
            ),
            operator_operation_matches_live_allocation: Boolean(
                operatorOperation
                && verifiedOperation
                && navswapStableJson(operatorOperation) === navswapStableJson(verifiedOperation),
            ),
            operator_fee_quote_matches_operation: Boolean(
                operatorOperation
                && operatorQuoteOperation
                && navswapStableJson(operatorOperation) === navswapStableJson(operatorQuoteOperation),
            ),
            operator_submit_accepted: operatorResult?.ok === true,
            operator_tx_id_present: Boolean(operatorResult?.tx_id),
        };
        const finalChecks = [
            checks.prepared_allocation_action_present,
            checks.wallet_submission_present,
            checks.wallet_submission_matches_prepared,
            checks.wallet_submission_receipt_accepted,
            checks.live_allocation_visible,
            checks.live_allocation_matches_quote,
            checks.operator_operation_matches_live_allocation,
            checks.operator_fee_quote_matches_operation,
            checks.operator_submit_accepted,
            checks.operator_tx_id_present,
        ];
        return {
            schema: 'postfiat-navswap-receipt-verification-v1',
            route: 'transparent_navswap',
            ok: finalChecks.every(Boolean),
            status: operatorResult?.status || 'operator_not_submitted',
            quote_status: quote?.status || null,
            wallet_action_count: Array.isArray(walletResult?.submissions) ? walletResult.submissions.length : null,
            wallet_tx_ids: txIds,
            nav_subscription_tx_id: subscriptionSubmission?.txId || null,
            operator_tx_id: operatorResult?.tx_id || null,
            allocation_id: allocation?.allocation_id || null,
            settlement_receipt_id: allocation?.receipt_id || null,
            settlement_bucket_id: allocation?.bucket_id || null,
            settlement_amount_atoms: allocation?.amount_atoms === undefined || allocation?.amount_atoms === null
                ? null
                : String(allocation.amount_atoms),
            checks,
        };
    }

    function navswapCompletionSubmittedSequence(submission) {
        const candidates = [
            submission?.quote?.sequence,
            submission?.signed?.unsigned?.sequence,
            submission?.signed?.unsigned_transaction?.sequence,
            submission?.signed?.transaction?.unsigned?.sequence,
            submission?.finality?.sequence,
        ];
        for (const candidate of candidates) {
            if (candidate !== undefined && candidate !== null && candidate !== '') {
                return parseNavswapActionInteger(candidate, 'wallet_submission.sequence');
            }
        }
        throw transparentCompletionError(
            'transparent_navswap_redeem_sequence_missing',
            'Submitted nav_redeem_at_nav result is missing the owner sequence needed to derive the NAV redemption id.',
        );
    }

    function navswapCompletionSubmittedChainId(submission) {
        const candidates = [
            submission?.quote?.chain_id,
            submission?.signed?.unsigned?.chain_id,
            submission?.signed?.unsigned_transaction?.chain_id,
            submission?.signed?.transaction?.unsigned?.chain_id,
            submission?.finality?.chain_id,
        ];
        for (const candidate of candidates) {
            const text = String(candidate || '').trim();
            if (text) return text;
        }
        throw transparentCompletionError(
            'transparent_navswap_redeem_chain_id_missing',
            'Submitted nav_redeem_at_nav result is missing the chain id needed to derive the NAV redemption id.',
        );
    }

    function navswapConsumerMatchesRecipient(consumerId, navAssetId, recipient) {
        const value = String(consumerId || '');
        return value === `nav_subscription:${navAssetId}`
            || value === `nav_subscription:${navAssetId}:${recipient}`
            || value.startsWith(`nav_subscription:${navAssetId}:${recipient}:`);
    }

    function navswapAllocationRemainingAtoms(allocation) {
        if (allocation?.remaining_atoms !== undefined && allocation?.remaining_atoms !== null) {
            return navswapPlannerNumber(allocation.remaining_atoms, 'allocation.remaining_atoms');
        }
        const amount = navswapPlannerNumber(allocation?.amount_atoms, 'allocation.amount_atoms');
        const released = navswapPlannerNumber(allocation?.released_atoms || 0, 'allocation.released_atoms');
        return Math.max(0, amount - released);
    }

    function selectTransparentRedeemSettlementAllocation(status, {
        navAssetId,
        owner,
        requiredSettlementAtoms,
        settlementAssetId,
    } = {}) {
        const required = navswapPlannerNumber(requiredSettlementAtoms, 'operator_completion.settlement_amount_atoms');
        const allocations = Array.isArray(status?.allocations) ? status.allocations : [];
        const candidates = allocations
            .filter((allocation) => (
                allocation?.asset_id === undefined || allocation.asset_id === settlementAssetId
            ))
            .filter((allocation) => (
                allocation?.purpose === VAULT_BRIDGE_ALLOCATION_PURPOSE_NAV_SUBSCRIPTION
                && navswapConsumerMatchesRecipient(allocation.consumer_id, navAssetId, owner)
                && navswapPlannerNumber(allocation.retired_at_height || 0, 'allocation.retired_at_height') > 0
                && navswapAllocationRemainingAtoms(allocation) >= required
            ))
            .sort((left, right) => {
                return (
                    navswapPlannerNumber(right.retired_at_height || 0, 'allocation.retired_at_height')
                    - navswapPlannerNumber(left.retired_at_height || 0, 'allocation.retired_at_height')
                ) || (
                    navswapPlannerNumber(right.created_at_height || 0, 'allocation.created_at_height')
                    - navswapPlannerNumber(left.created_at_height || 0, 'allocation.created_at_height')
                ) || String(left.allocation_id).localeCompare(String(right.allocation_id));
            });
        if (candidates.length === 0) {
            throw transparentCompletionError(
                'transparent_navswap_redeem_backing_allocation_missing',
                'No retired bridge-backed nav_subscription allocation can fully settle this NAV redemption into pfUSDC.',
                {
                    lookup: {
                        settlement_asset_id: settlementAssetId,
                        nav_asset_id: navAssetId,
                        owner,
                        required_settlement_amount_atoms: String(required),
                    },
                    vault_bridge_status: {
                        asset_id: status?.asset_id || settlementAssetId,
                        allocation_count: status?.allocation_count ?? allocations.length,
                    },
                },
            );
        }
        return candidates[0];
    }

    function navswapRedeemCompletionOperationTemplate(completion = {}) {
        const template = completion.operation_template || completion.operation || null;
        if (!template || typeof template !== 'object') {
            throw transparentCompletionError(
                'transparent_navswap_redeem_operator_template_missing',
                'Transparent NAVSwap operator completion is missing nav_redeem_settle operation_template.',
            );
        }
        if (template.operation !== 'nav_redeem_settle') {
            throw transparentCompletionError(
                'transparent_navswap_redeem_operator_template_invalid',
                'Transparent NAVSwap redeem operator completion template must be nav_redeem_settle.',
            );
        }
        return template;
    }

    async function verifyTransparentNavRedeemSettlement(completion = {}, submission = {}, rpcRequest = rpcTcpRequest) {
        const template = navswapRedeemCompletionOperationTemplate(completion);
        const redeemOperation = submission?.navswap_action?.operation || {};
        if (redeemOperation.operation !== 'nav_redeem_at_nav') {
            throw transparentCompletionError(
                'transparent_navswap_redeem_submission_invalid',
                'Transparent NAVSwap redeem completion requires a submitted nav_redeem_at_nav operation.',
            );
        }
        const owner = parseNavswapWalletAddress(redeemOperation.owner);
        const navAssetId = parseNavswapHexId(redeemOperation.asset_id, 'nav_redeem_at_nav.asset_id');
        const settlementAssetId = parseNavswapHexId(template.settlement_asset_id, 'operator_completion.settlement_asset_id');
        const settlementAmountAtoms = parseNavswapActionInteger(
            template.settlement_amount_atoms || completion.allocation_lookup?.settlement_amount_atoms,
            'operator_completion.settlement_amount_atoms',
        );
        const chainId = navswapCompletionSubmittedChainId(submission);
        const ownerSequence = navswapCompletionSubmittedSequence(submission);
        const redemptionId = navswapNavRedemptionId(chainId, owner, navAssetId, ownerSequence);
        const status = await navswapRpcRead('vault_bridge_status', { asset_id: settlementAssetId }, rpcRequest);
        const allocation = selectTransparentRedeemSettlementAllocation(status, {
            navAssetId,
            owner,
            requiredSettlementAtoms: settlementAmountAtoms,
            settlementAssetId,
        });
        const operation = {
            ...template,
            issuer: template.issuer || redeemOperation.issuer,
            asset_id: navAssetId,
            redemption_id: redemptionId,
            settlement_receipt_hash: template.settlement_receipt_hash
                || navswapSettlementReceiptHash({
                    redemption_id: redemptionId,
                    owner,
                    nav_asset_id: navAssetId,
                    settlement_asset_id: settlementAssetId,
                    settlement_allocation_id: allocation.allocation_id,
                    settlement_amount_atoms: settlementAmountAtoms,
                }),
            settlement_asset_id: settlementAssetId,
            settlement_bucket_id: allocation.bucket_id,
            settlement_allocation_id: allocation.allocation_id,
            settlement_amount_atoms: settlementAmountAtoms,
        };
        return {
            operation,
            allocation,
            redemption_id: redemptionId,
            owner_sequence: ownerSequence,
            status,
        };
    }

    function buildTransparentNavswapRedeemReceiptVerification({
        quote,
        walletResult,
        redeemSubmission,
        verifiedSettlement,
        operatorResult = null,
    } = {}) {
        const prepared = transparentCompletionPreparedAction(quote, 'nav_redeem_at_nav');
        const submittedOperation = redeemSubmission?.navswap_action?.operation || null;
        const preparedOperation = prepared?.operation || null;
        const walletReceipt = redeemSubmission?.receipt || null;
        const operatorOperation = operatorResult?.operation || null;
        const operatorQuoteOperation = operatorResult?.quote?.operation || null;
        const txIds = Array.isArray(walletResult?.submissions)
            ? walletResult.submissions.map(item => item?.txId).filter(Boolean)
            : [];
        const checks = {
            prepared_redeem_action_present: Boolean(preparedOperation),
            wallet_submission_present: Boolean(submittedOperation),
            wallet_submission_matches_prepared: Boolean(
                preparedOperation
                && submittedOperation
                && navswapStableJson(preparedOperation) === navswapStableJson(submittedOperation),
            ),
            wallet_submission_receipt_accepted: walletReceipt?.accepted === true,
            redemption_id_derived: Boolean(verifiedSettlement?.redemption_id),
            backing_allocation_selected: Boolean(verifiedSettlement?.allocation?.allocation_id),
            operator_operation_matches_redeem_settlement: Boolean(
                operatorOperation
                && verifiedSettlement?.operation
                && navswapStableJson(operatorOperation) === navswapStableJson(verifiedSettlement.operation),
            ),
            operator_fee_quote_matches_operation: Boolean(
                operatorOperation
                && operatorQuoteOperation
                && navswapStableJson(operatorOperation) === navswapStableJson(operatorQuoteOperation),
            ),
            operator_submit_accepted: operatorResult?.ok === true,
            operator_tx_id_present: Boolean(operatorResult?.tx_id),
        };
        const finalChecks = [
            checks.prepared_redeem_action_present,
            checks.wallet_submission_present,
            checks.wallet_submission_matches_prepared,
            checks.wallet_submission_receipt_accepted,
            checks.redemption_id_derived,
            checks.backing_allocation_selected,
            checks.operator_operation_matches_redeem_settlement,
            checks.operator_fee_quote_matches_operation,
            checks.operator_submit_accepted,
            checks.operator_tx_id_present,
        ];
        return {
            schema: 'postfiat-navswap-redeem-receipt-verification-v1',
            route: 'transparent_navswap',
            ok: finalChecks.every(Boolean),
            status: operatorResult?.status || 'operator_not_submitted',
            quote_status: quote?.status || null,
            wallet_action_count: Array.isArray(walletResult?.submissions) ? walletResult.submissions.length : null,
            wallet_tx_ids: txIds,
            nav_redeem_tx_id: redeemSubmission?.txId || null,
            operator_tx_id: operatorResult?.tx_id || null,
            redemption_id: verifiedSettlement?.redemption_id || null,
            settlement_allocation_id: verifiedSettlement?.allocation?.allocation_id || null,
            settlement_amount_atoms: verifiedSettlement?.operation?.settlement_amount_atoms === undefined
                ? null
                : String(verifiedSettlement.operation.settlement_amount_atoms),
            checks,
        };
    }

    async function signAndSubmitNavswapOperatorAssetTransaction(operation, source, rpcRequest = rpcTcpRequest) {
        const config = navswapTransparentOperatorConfig();
        const operationKind = operation?.operation || 'asset_transaction';
        const operationLabel = operationKind;
        if (!config.signing_configured) {
            return {
                ok: false,
                status: 'awaiting_operator_signature',
                code: 'navswap_operator_key_not_configured',
                message: `Set NAVSWAP_OPERATOR_ISSUER_KEY_FILE to let the wallet proxy sign the operator-owned ${operationLabel} leg.`,
                operation,
                config: {
                    signing_configured: false,
                    custody_boundary: config.custody_boundary,
                },
            };
        }
        if (!fs.existsSync(config.issuer_key_file)) {
            return {
                ok: false,
                status: 'failed',
                code: 'navswap_operator_key_file_missing',
                message: `NAVSwap operator issuer key file not found at ${config.issuer_key_file}.`,
                operation,
            };
        }
        if (!fs.existsSync(config.node_bin)) {
            return {
                ok: false,
                status: 'failed',
                code: 'navswap_operator_node_bin_missing',
                message: `postfiat-node binary not found at ${config.node_bin}.`,
                operation,
            };
        }

        const quote = await navswapRpcRead('asset_fee_quote', {
            source,
            operation_json: JSON.stringify(operation),
        }, rpcRequest);
            if (quote.source && quote.source !== source) {
                return {
                    ok: false,
                    status: 'failed',
                    code: 'navswap_operator_quote_source_mismatch',
                    message: `Operator asset fee quote source does not match the ${operationLabel} signer.`,
                    operation,
                    quote,
                };
            }
            if (quote.operation && navswapStableJson(quote.operation) !== navswapStableJson(operation)) {
            return {
                    ok: false,
                    status: 'failed',
                    code: 'navswap_operator_quote_operation_mismatch',
                    message: `Operator asset fee quote operation does not match the reviewed ${operationLabel} operation.`,
                    operation,
                    quote,
                };
            }
        if (quote.sender_meets_reserve_after_fee === false) {
            return {
                    ok: false,
                    status: 'failed',
                    code: 'navswap_operator_insufficient_fee_balance',
                    message: `Operator account does not have enough PFT for the ${operationLabel} fee.`,
                    operation,
                    quote,
                };
            }

        const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'postfiat-navswap-operator-'));
        const quoteFile = path.join(dir, 'quote.json');
        try {
            fs.writeFileSync(quoteFile, JSON.stringify(quote, null, 2), { mode: 0o600 });
            const { stdout } = await execFileAsync(
                config.node_bin,
                [
                    'wallet-sign-asset-transaction',
                    '--key-file',
                    config.issuer_key_file,
                    '--quote-file',
                    quoteFile,
                ],
                {
                    timeout: config.timeout_ms,
                    maxBuffer: 2 * 1024 * 1024,
                },
            );
            const signed = JSON.parse(stdout);
            const submitMethod = 'mempool_submit_signed_asset_transaction_finality';
            const submitRequest = {
                version: 'postfiat-local-rpc-v1',
                id: `navswap-operator-submit-${Date.now()}`,
                method: submitMethod,
                params: {
                    signed_asset_transaction_json: JSON.stringify(signed),
                },
            };
            const target = rpcRequest === rpcTcpRequest
                ? await resolveRpcTarget(submitMethod)
                : {
                    endpoint: { validatorId: 'test', host: RPC_HOST, port: RPC_PORT },
                    route: null,
                };
            const outbound = requestWithProxyReadiness(submitRequest, target.route);
            const submitResponse = await rpcRequest(
                target.endpoint.host,
                target.endpoint.port,
                outbound,
            );
            if (submitResponse.ok === true && target.route) {
                const line = JSON.stringify(submitResponse);
                rememberFinalizedReadEndpoint(line, target);
                primeNextProposerRouteCacheFromResponse(line, target.route, {
                    warmReadiness: true,
                });
            }
            if (submitResponse.ok !== true) {
                return {
                    ok: false,
                    status: 'failed',
                    code: submitResponse.error?.code || 'navswap_operator_submit_failed',
                    message: submitResponse.error?.message || `Operator ${operationLabel} submit failed.`,
                    operation,
                    quote,
                    signed,
                    rpc_error: submitResponse.error || null,
                };
            }
            return {
                ok: true,
                status: operationKind === 'nav_redeem_settle'
                    ? 'operator_redeem_settle_submitted'
                    : operationKind === 'pftl_uniswap_destination_consume'
                        ? 'destination_consume_submitted'
                        : 'operator_mint_submitted',
                message: `Operator ${operationLabel} transaction submitted.`,
                operation,
                quote,
                signed,
                submit_result: submitResponse.result,
                tx_id: submitResponse.result?.tx_id || null,
                proxy_route: target.route || null,
            };
        } catch (error) {
            return {
                ok: false,
                status: 'failed',
                code: 'navswap_operator_sign_failed',
                message: error?.stderr?.trim() || error?.message || `Operator ${operationLabel} signing failed.`,
                operation,
                quote,
            };
        } finally {
            try { fs.rmSync(quoteFile, { force: true }); } catch (_) {}
            try { fs.rmdirSync(dir); } catch (_) {}
        }
    }

    async function completeTransparentNavswapRun(run, body, rpcRequest = rpcTcpRequest) {
        try {
            const { quote, result, submission, stage } = verifyTransparentWalletCompletionInput(body);
            const completion = quote.operator_completion;
            recordNavswapRunEvent(run, 'wallet_batch_verified', 'Wallet-owned NAVSwap action batch verified after local submit.', {
                submitted_count: Array.isArray(result.submissions) ? result.submissions.length : null,
                stage,
                wallet_tx_id: submission.txId || null,
            });
            if (stage === 'nav_redeem_at_nav') {
                const verifiedSettlement = await verifyTransparentNavRedeemSettlement(completion, submission, rpcRequest);
                recordNavswapRunEvent(run, 'nav_redemption_verified', 'Submitted NAV redemption was mapped to bridge-backed settlement collateral.', {
                    redemption_id: verifiedSettlement.redemption_id,
                    settlement_allocation_id: verifiedSettlement.allocation.allocation_id,
                    settlement_amount_atoms: String(verifiedSettlement.operation.settlement_amount_atoms),
                });
                const allocationVerification = buildTransparentNavswapRedeemReceiptVerification({
                    quote,
                    walletResult: result,
                    redeemSubmission: submission,
                    verifiedSettlement,
                });
                const issuer = verifiedSettlement.operation.issuer
                    || completion.operation_template?.issuer
                    || null;
                if (!issuer) {
                    finishNavswapRun(run, {
                        ok: false,
                        status: 'failed',
                        code: 'transparent_navswap_operator_issuer_missing',
                        message: 'Operator completion template is missing the NAV asset issuer.',
                        receipt_type: 'transparent_navswap_operator_completion',
                        result: {
                            receipt_verification: allocationVerification,
                            operation: verifiedSettlement.operation,
                            allocation: verifiedSettlement.allocation,
                        },
                    });
                    return navswapRunPublic(run);
                }
                const operatorResult = await signAndSubmitNavswapOperatorAssetTransaction(
                    verifiedSettlement.operation,
                    issuer,
                    rpcRequest,
                );
                const receiptVerification = buildTransparentNavswapRedeemReceiptVerification({
                    quote,
                    walletResult: result,
                    redeemSubmission: submission,
                    verifiedSettlement,
                    operatorResult,
                });
                const ok = operatorResult.ok === true;
                finishNavswapRun(run, {
                    ok,
                    status: operatorResult.status || (ok ? 'operator_redeem_settle_submitted' : 'failed'),
                    code: ok ? undefined : operatorResult.code,
                    message: operatorResult.message,
                    receipt_type: 'transparent_navswap_operator_completion',
                    result: {
                        receipt_verification: receiptVerification,
                        allocation: verifiedSettlement.allocation,
                        redemption_id: verifiedSettlement.redemption_id,
                        operator_completion: operatorResult,
                    },
                });
                return navswapRunPublic(run);
            }
            const verifiedAllocation = await verifyTransparentNavSubscriptionAllocation(completion, rpcRequest);
            recordNavswapRunEvent(run, 'subscription_allocation_verified', 'Matching nav_subscription allocation is visible on public vault status.', {
                allocation_id: verifiedAllocation.allocation.allocation_id,
                settlement_amount_atoms: String(verifiedAllocation.allocation.amount_atoms),
                consumer_id: verifiedAllocation.allocation.consumer_id,
            });
            const allocationVerification = buildTransparentNavswapReceiptVerification({
                quote,
                walletResult: result,
                subscriptionSubmission: submission,
                verifiedAllocation,
            });
            const issuer = verifiedAllocation.operation.issuer
                || completion.operation_template?.issuer
                || null;
            if (!issuer) {
                finishNavswapRun(run, {
                    ok: false,
                    status: 'failed',
                    code: 'transparent_navswap_operator_issuer_missing',
                    message: 'Operator completion template is missing the NAV asset issuer.',
                    receipt_type: 'transparent_navswap_operator_completion',
                    result: {
                        receipt_verification: allocationVerification,
                        operation: verifiedAllocation.operation,
                        allocation: verifiedAllocation.allocation,
                    },
                });
                return navswapRunPublic(run);
            }
            const operatorResult = await signAndSubmitNavswapOperatorAssetTransaction(
                verifiedAllocation.operation,
                issuer,
                rpcRequest,
            );
            const receiptVerification = buildTransparentNavswapReceiptVerification({
                quote,
                walletResult: result,
                subscriptionSubmission: submission,
                verifiedAllocation,
                operatorResult,
            });
            const ok = operatorResult.ok === true;
            finishNavswapRun(run, {
                ok,
                status: operatorResult.status || (ok ? 'operator_mint_submitted' : 'failed'),
                code: ok ? undefined : operatorResult.code,
                message: operatorResult.message,
                receipt_type: 'transparent_navswap_operator_completion',
                result: {
                    receipt_verification: receiptVerification,
                    allocation: verifiedAllocation.allocation,
                    operator_completion: operatorResult,
                },
            });
            return navswapRunPublic(run);
        } catch (error) {
            finishNavswapRun(run, {
                ok: false,
                status: 'failed',
                code: error.code || 'transparent_navswap_completion_failed',
                message: error.message || 'Transparent NAVSwap completion failed.',
                receipt_type: 'transparent_navswap_operator_completion',
                result: {
                    code: error.code || 'transparent_navswap_completion_failed',
                    message: error.message || 'Transparent NAVSwap completion failed.',
                    ...(error.receipt ? { receipt: error.receipt } : {}),
                    ...(error.lookup ? { lookup: error.lookup } : {}),
                    ...(error.vault_bridge_status ? { vault_bridge_status: error.vault_bridge_status } : {}),
                },
            });
            return navswapRunPublic(run);
        }
    }

    async function executeTransparentNavswapRun(body = {}, rpcRequest = rpcTcpRequest) {
        const route = navswapRouteFromBody(body);
        let quote;
        try {
            quote = transparentCompletionQuote(body);
            transparentCompletionWalletResult(body);
        } catch (error) {
            return {
                ok: false,
                schema: NAVSWAP_RUN_SCHEMA,
                route,
                code: error.code || 'transparent_navswap_completion_request_invalid',
                message: error.message || 'Transparent NAVSwap completion request is invalid.',
            };
        }
        const run = createNavswapRun(route, body, quote);
        recordNavswapRunEvent(run, 'operator_completion_requested', 'Transparent NAVSwap operator completion requested.', {
            stage: quote.operator_completion?.stage || null,
            status: quote.operator_completion?.status || null,
        });

        if (navswapAsyncRunRequested(body)) {
            recordNavswapRunEvent(run, 'async_run_accepted', 'Transparent NAVSwap operator completion accepted for background execution.', {
                route,
                custody_boundary: 'wallet-local-signing-plus-operator-issuer',
            });
            void completeTransparentNavswapRun(run, body, rpcRequest).catch((error) => {
                finishNavswapRun(run, {
                    ok: false,
                    status: 'failed',
                    code: 'transparent_navswap_background_completion_failed',
                    message: error?.message || 'Transparent NAVSwap background completion failed.',
                    receipt_type: 'transparent_navswap_operator_completion',
                });
            });
            return {
                ok: true,
                schema: NAVSWAP_RUN_SCHEMA,
                route,
                status: 'running',
                message: 'Transparent NAVSwap operator completion accepted. Poll or stream the run for progress.',
                run_id: run.run_id,
                status_endpoint: `/api/navswap/runs/${run.run_id}`,
                events_endpoint: `/api/navswap/runs/${run.run_id}/events`,
                stream_endpoint: `/api/navswap/runs/${run.run_id}/stream`,
                receipts_endpoint: `/api/navswap/runs/${run.run_id}/receipts`,
                custody_boundary: 'wallet-local-signing-plus-operator-issuer',
                quote,
            };
        }

        const finalRun = await completeTransparentNavswapRun(run, body, rpcRequest);
        return {
            ok: finalRun.ok,
            schema: NAVSWAP_RUN_SCHEMA,
            route,
            status: finalRun.status,
            code: finalRun.code || undefined,
            message: finalRun.message,
            run_id: run.run_id,
            status_endpoint: `/api/navswap/runs/${run.run_id}`,
            events_endpoint: finalRun.events_endpoint,
            stream_endpoint: finalRun.stream_endpoint,
            receipts_endpoint: finalRun.receipts_endpoint,
            quote,
            result: finalRun.result,
        };
    }

    function pftlUniswapCompletionError(code, message, extra = {}) {
        const err = new Error(message);
        err.code = code;
        Object.assign(err, extra);
        return err;
    }

    function pftlUniswapCompletionQuote(body = {}) {
        const quote = body.quote || body.route_quote || body.navswap_quote || null;
        if (!quote || typeof quote !== 'object' || quote.ok !== true || quote.route !== 'uniswap_atomic_handoff') {
            throw pftlUniswapCompletionError(
                'pftl_uniswap_quote_required',
                'PFTL-Uniswap completion requires the prepared quote returned by /api/navswap/quotes.',
            );
        }
        if (!quote.operator_completion || typeof quote.operator_completion !== 'object') {
            throw pftlUniswapCompletionError(
                'pftl_uniswap_operator_completion_missing',
                'PFTL-Uniswap quote does not include operator completion metadata.',
            );
        }
        return quote;
    }

    function pftlUniswapPreparedAction(quote, stage) {
        const actions = Array.isArray(quote?.prepared_action_batch?.actions)
            ? quote.prepared_action_batch.actions
            : [];
        return actions.find((action) => action?.stage === stage) || null;
    }

    function verifyPftlUniswapWalletCompletionInput(body = {}) {
        const quote = pftlUniswapCompletionQuote(body);
        const result = transparentCompletionWalletResult(body);
        const primaryPrepared = pftlUniswapPreparedAction(quote, 'pftl_uniswap_primary_subscribe');
        const exportPrepared = pftlUniswapPreparedAction(quote, 'pftl_uniswap_export_debit');
        const primarySubmission = transparentCompletionSubmission(result, 'pftl_uniswap_primary_subscribe');
        const exportSubmission = transparentCompletionSubmission(result, 'pftl_uniswap_export_debit');
        if (!primaryPrepared?.operation || !exportPrepared?.operation) {
            throw pftlUniswapCompletionError(
                'pftl_uniswap_wallet_actions_missing',
                'PFTL-Uniswap quote must contain primary subscribe and export debit source actions.',
            );
        }
        if (!primarySubmission?.navswap_action?.operation || !exportSubmission?.navswap_action?.operation) {
            throw pftlUniswapCompletionError(
                'pftl_uniswap_wallet_submission_missing',
                'PFTL-Uniswap wallet result must contain submitted primary subscribe and export debit actions.',
            );
        }
        if (primarySubmission.receipt?.accepted === false || exportSubmission.receipt?.accepted === false) {
            throw pftlUniswapCompletionError(
                'pftl_uniswap_wallet_action_rejected',
                'A PFTL-Uniswap source action was rejected by the mempool.',
                { receipt: primarySubmission.receipt?.accepted === false ? primarySubmission.receipt : exportSubmission.receipt },
            );
        }
        if (navswapStableJson(primaryPrepared.operation) !== navswapStableJson(primarySubmission.navswap_action.operation)) {
            throw pftlUniswapCompletionError(
                'pftl_uniswap_primary_operation_mismatch',
                'Submitted PFTL-Uniswap primary subscribe operation does not match the prepared quote action.',
            );
        }
        if (navswapStableJson(exportPrepared.operation) !== navswapStableJson(exportSubmission.navswap_action.operation)) {
            throw pftlUniswapCompletionError(
                'pftl_uniswap_export_operation_mismatch',
                'Submitted PFTL-Uniswap export operation does not match the prepared quote action.',
            );
        }
        return { quote, result, primaryPrepared, exportPrepared, primarySubmission, exportSubmission };
    }

    function navswapPftlUniswapControlledAttestationTxHash({ packet, quote }) {
        return crypto.createHash('sha256')
            .update('postfiat.pftl_uniswap.controlled_destination_attestation.v1')
            .update(Buffer.from([0]))
            .update(navswapStableJson({
                route_id: quote?.route_id || quote?.prepared_action_batch?.route_id || null,
                route_config_digest: quote?.route_config_digest || null,
                packet_hash: packet?.packet_hash || null,
                source_wallet: packet?.source_wallet || null,
                amount_atoms: packet?.amount_atoms === undefined ? null : String(packet.amount_atoms),
                ethereum_recipient: packet?.ethereum_recipient || null,
            }))
            .digest('hex');
    }

    function navswapPftlUniswapDestinationHeights(body = {}, quote = {}) {
        const completion = quote.operator_completion || {};
        const consumedHeight = parseNavswapActionInteger(
            body.ethereum_consumed_height
                || body.consumed_height
                || completion.ethereum_consumed_height
                || completion.consumed_height
                || 1,
            'ethereum_consumed_height',
        );
        const finalityBlocks = parseNavswapActionInteger(
            body.return_finality_blocks
                || completion.return_finality_blocks
                || process.env.NAVSWAP_UNISWAP_RETURN_FINALITY_BLOCKS
                || '64',
            'return_finality_blocks',
        );
        const explicitFinalized = body.ethereum_finalized_height
            || body.finalized_height
            || completion.ethereum_finalized_height
            || completion.finalized_height
            || null;
        const finalizedHeight = explicitFinalized
            ? parseNavswapActionInteger(explicitFinalized, 'ethereum_finalized_height')
            : consumedHeight + finalityBlocks;
        return { consumedHeight, finalizedHeight, finalityBlocks };
    }

    function normalizePftlUniswapPacketStatus(value) {
        const text = String(value || '').trim();
        if (text === 'SourceDebited') return 'source_debited';
        if (text === 'DestinationConsumed') return 'destination_consumed';
        if (text === 'SourceRefunded') return 'source_refunded';
        return text.toLowerCase();
    }

    async function verifyPftlUniswapExportPacket(quote, exportOperation, rpcRequest = rpcTcpRequest) {
        const routeId = quote?.prepared_action_batch?.route_id
            || quote?.route_id
            || exportOperation.route_id;
        const packetHash = parseNavswapHexId(
            exportOperation.packet_hash || quote?.operator_completion?.packet_hash,
            'pftl_uniswap.packet_hash',
        );
        const status = await navswapRpcRead('navcoin_bridge_packet', {
            route_id: routeId,
            packet_hash: packetHash,
        }, rpcRequest);
        const packet = status?.packet || {};
        const packetStatus = normalizePftlUniswapPacketStatus(packet.status);
        const checks = {
            packet_hash_matches: status?.packet_hash === packetHash && packet.packet_hash === packetHash,
            route_digest_matches: status?.route_config_digest === quote.route_config_digest,
            packet_source_debited: packetStatus === 'source_debited',
            claim_class_outstanding: packet.claim_class === 'outstanding_bridge_claim',
            source_wallet_matches: packet.source_wallet === exportOperation.owner,
            ethereum_recipient_matches: String(packet.ethereum_recipient || '').toLowerCase()
                === String(exportOperation.ethereum_recipient || '').toLowerCase(),
            amount_matches: String(packet.amount_atoms) === String(exportOperation.amount_atoms),
            nonce_matches: packet.nonce === exportOperation.export_nonce,
            deadline_matches: String(packet.destination_deadline_seconds)
                === String(exportOperation.destination_deadline_seconds),
            refund_height_present: Number(packet.refund_not_before_height || 0) > 0,
            ledger_hash_present: /^[0-9a-f]{96}$/i.test(String(status?.ledger_hash || '')),
        };
        const failed = Object.entries(checks)
            .filter(([, ok]) => !ok)
            .map(([name]) => name);
        if (failed.length > 0) {
            throw pftlUniswapCompletionError(
                'pftl_uniswap_export_packet_verification_failed',
                `PFTL-Uniswap exported packet did not match the wallet source action: ${failed.join(', ')}.`,
                { packet_status: status, checks },
            );
        }
        return { status, packet, checks };
    }

    function buildPftlUniswapReceiptVerification({
        quote,
        walletResult,
        primarySubmission,
        exportSubmission,
        packetVerification,
        operatorResult,
        destinationAttestation,
    } = {}) {
        const txIds = Array.isArray(walletResult?.submissions)
            ? walletResult.submissions.map(item => item?.txId).filter(Boolean)
            : [];
        const packet = packetVerification?.packet || null;
        const operatorOperation = operatorResult?.operation || null;
        const operatorQuoteOperation = operatorResult?.quote?.operation || null;
        const checks = {
            wallet_primary_submitted: Boolean(primarySubmission?.txId),
            wallet_export_submitted: Boolean(exportSubmission?.txId),
            wallet_receipts_accepted: primarySubmission?.receipt?.accepted !== false
                && exportSubmission?.receipt?.accepted !== false,
            source_packet_visible: Boolean(packet?.packet_hash),
            source_packet_matches_wallet_export: packetVerification?.checks
                ? Object.values(packetVerification.checks).every(Boolean)
                : false,
            operator_destination_operation_prepared: operatorOperation?.operation === 'pftl_uniswap_destination_consume',
            operator_fee_quote_matches_operation: Boolean(
                operatorOperation
                && operatorQuoteOperation
                && navswapStableJson(operatorOperation) === navswapStableJson(operatorQuoteOperation),
            ),
            operator_submit_accepted: operatorResult?.ok === true,
            operator_tx_id_present: Boolean(operatorResult?.tx_id),
        };
        return {
            schema: 'postfiat-pftl-uniswap-controlled-destination-verification-v1',
            route: 'uniswap_atomic_handoff',
            ok: Object.values(checks).every(Boolean),
            trust_class: 'CONTROLLED',
            operator_attested_destination_events: true,
            wallet_action_count: Array.isArray(walletResult?.submissions) ? walletResult.submissions.length : null,
            wallet_tx_ids: txIds,
            primary_tx_id: primarySubmission?.txId || null,
            export_tx_id: exportSubmission?.txId || null,
            packet_hash: packet?.packet_hash || quote?.operator_completion?.packet_hash || null,
            route_config_digest: quote?.route_config_digest || null,
            launch_config_digest: quote?.launch_config_digest || null,
            destination_attestation: destinationAttestation,
            operator_tx_id: operatorResult?.tx_id || null,
            checks,
        };
    }

    async function completePftlUniswapHandoffRun(run, body, rpcRequest = rpcTcpRequest) {
        try {
            const { quote, result, primarySubmission, exportSubmission } = verifyPftlUniswapWalletCompletionInput(body);
            const exportOperation = exportSubmission.navswap_action.operation;
            recordNavswapRunEvent(run, 'wallet_batch_verified', 'Wallet-owned PFTL-Uniswap source action batch verified after local submit.', {
                submitted_count: Array.isArray(result.submissions) ? result.submissions.length : null,
                primary_tx_id: primarySubmission.txId || null,
                export_tx_id: exportSubmission.txId || null,
                packet_hash: exportOperation.packet_hash || null,
            });
            const packetVerification = await verifyPftlUniswapExportPacket(quote, exportOperation, rpcRequest);
            recordNavswapRunEvent(run, 'source_export_packet_verified', 'Consensus PFTL-Uniswap export packet is visible and matches the wallet export.', {
                packet_hash: packetVerification.packet.packet_hash,
                ledger_hash: packetVerification.status.ledger_hash,
                claim_class: packetVerification.packet.claim_class,
                amount_atoms: String(packetVerification.packet.amount_atoms),
            });

            const config = navswapTransparentOperatorConfig();
            const operator = readNavswapKeyFileAddress(config.issuer_key_file);
            if (!operator) {
                throw pftlUniswapCompletionError(
                    'pftl_uniswap_operator_key_not_configured',
                    'Set NAVSWAP_OPERATOR_ISSUER_KEY_FILE so the controlled operator can attest destination consume.',
                );
            }
            const ethereumConsumeTxHash = parseNavswapHexId(
                body.ethereum_consume_tx_hash
                    || body.destination_consume_tx_hash
                    || quote.operator_completion?.ethereum_consume_tx_hash
                    || navswapPftlUniswapControlledAttestationTxHash({
                        packet: packetVerification.packet,
                        quote,
                    }),
                'ethereum_consume_tx_hash',
                64,
            );
            const heights = navswapPftlUniswapDestinationHeights(body, quote);
            const operation = {
                operation: 'pftl_uniswap_destination_consume',
                operator,
                route_id: exportOperation.route_id,
                packet_hash: exportOperation.packet_hash,
                ethereum_consume_tx_hash: ethereumConsumeTxHash,
                consumed_height: heights.consumedHeight,
                finalized_height: heights.finalizedHeight,
            };
            recordNavswapRunEvent(run, 'destination_consume_attestation_prepared', 'CONTROLLED operator destination consume attestation prepared.', {
                packet_hash: operation.packet_hash,
                ethereum_consume_tx_hash: operation.ethereum_consume_tx_hash,
                consumed_height: String(operation.consumed_height),
                finalized_height: String(operation.finalized_height),
                attestation: body.ethereum_consume_tx_hash || body.destination_consume_tx_hash
                    ? 'operator_supplied_ethereum_tx_hash'
                    : 'controlled_operator_attestation_hash',
            });
            const operatorResult = await signAndSubmitNavswapOperatorAssetTransaction(
                operation,
                operator,
                rpcRequest,
            );
            const destinationAttestation = {
                trust_class: 'CONTROLLED',
                operator_attested: true,
                ethereum_consume_tx_hash: ethereumConsumeTxHash,
                consumed_height: String(heights.consumedHeight),
                finalized_height: String(heights.finalizedHeight),
                finality_blocks: String(heights.finalityBlocks),
                source: body.ethereum_consume_tx_hash || body.destination_consume_tx_hash
                    ? 'operator_supplied_ethereum_tx_hash'
                    : 'controlled_operator_attestation_hash',
            };
            const receiptVerification = buildPftlUniswapReceiptVerification({
                quote,
                walletResult: result,
                primarySubmission,
                exportSubmission,
                packetVerification,
                operatorResult,
                destinationAttestation,
            });
            const ok = operatorResult.ok === true && receiptVerification.ok === true;
            finishNavswapRun(run, {
                ok,
                status: operatorResult.status || (ok ? 'destination_consume_submitted' : 'failed'),
                code: ok ? undefined : (operatorResult.code || 'pftl_uniswap_destination_consume_failed'),
                message: ok
                    ? 'PFTL-Uniswap source actions verified; controlled operator destination consume submitted.'
                    : (operatorResult.message || 'PFTL-Uniswap destination consume failed.'),
                receipt_type: 'pftl_uniswap_controlled_destination_completion',
                result: {
                    receipt_verification: receiptVerification,
                    packet_status: packetVerification.status,
                    destination_attestation: destinationAttestation,
                    operator_completion: operatorResult,
                },
            });
            return navswapRunPublic(run);
        } catch (error) {
            finishNavswapRun(run, {
                ok: false,
                status: 'failed',
                code: error.code || 'pftl_uniswap_completion_failed',
                message: error.message || 'PFTL-Uniswap controlled completion failed.',
                receipt_type: 'pftl_uniswap_controlled_destination_completion',
                result: {
                    code: error.code || 'pftl_uniswap_completion_failed',
                    message: error.message || 'PFTL-Uniswap controlled completion failed.',
                    ...(error.receipt ? { receipt: error.receipt } : {}),
                    ...(error.packet_status ? { packet_status: error.packet_status } : {}),
                    ...(error.checks ? { checks: error.checks } : {}),
                },
            });
            return navswapRunPublic(run);
        }
    }

    async function executePftlUniswapHandoffRun(body = {}, rpcRequest = rpcTcpRequest) {
        const route = navswapRouteFromBody(body);
        let quote;
        try {
            quote = pftlUniswapCompletionQuote(body);
            transparentCompletionWalletResult(body);
        } catch (error) {
            return {
                ok: false,
                schema: NAVSWAP_RUN_SCHEMA,
                route,
                code: error.code || 'pftl_uniswap_completion_request_invalid',
                message: error.message || 'PFTL-Uniswap completion request is invalid.',
            };
        }
        const run = createNavswapRun(route, body, quote);
        recordNavswapRunEvent(run, 'operator_completion_requested', 'PFTL-Uniswap controlled operator completion requested.', {
            stage: quote.operator_completion?.stage || null,
            trust_class: quote.operator_completion?.trust_class || 'CONTROLLED',
            packet_hash: quote.operator_completion?.packet_hash || null,
        });

        if (navswapAsyncRunRequested(body)) {
            recordNavswapRunEvent(run, 'async_run_accepted', 'PFTL-Uniswap controlled operator completion accepted for background execution.', {
                route,
                custody_boundary: 'wallet-local-source-signing-plus-controlled-operator-attestation',
            });
            void completePftlUniswapHandoffRun(run, body, rpcRequest).catch((error) => {
                finishNavswapRun(run, {
                    ok: false,
                    status: 'failed',
                    code: 'pftl_uniswap_background_completion_failed',
                    message: error?.message || 'PFTL-Uniswap background completion failed.',
                    receipt_type: 'pftl_uniswap_controlled_destination_completion',
                });
            });
            return {
                ok: true,
                schema: NAVSWAP_RUN_SCHEMA,
                route,
                status: 'running',
                message: 'PFTL-Uniswap source actions submitted. Controlled operator destination completion started.',
                run_id: run.run_id,
                status_endpoint: `/api/navswap/runs/${run.run_id}`,
                events_endpoint: `/api/navswap/runs/${run.run_id}/events`,
                stream_endpoint: `/api/navswap/runs/${run.run_id}/stream`,
                receipts_endpoint: `/api/navswap/runs/${run.run_id}/receipts`,
                custody_boundary: 'wallet-local-source-signing-plus-controlled-operator-attestation',
                quote,
            };
        }

        const finalRun = await completePftlUniswapHandoffRun(run, body, rpcRequest);
        return {
            ok: finalRun.ok,
            schema: NAVSWAP_RUN_SCHEMA,
            route,
            status: finalRun.status,
            code: finalRun.code || undefined,
            message: finalRun.message,
            run_id: run.run_id,
            status_endpoint: `/api/navswap/runs/${run.run_id}`,
            events_endpoint: finalRun.events_endpoint,
            stream_endpoint: finalRun.stream_endpoint,
            receipts_endpoint: finalRun.receipts_endpoint,
            quote,
            result: finalRun.result,
        };
    }

    async function executeNavswapQuote(body = {}, rpcRequest = rpcTcpRequest) {
        const route = navswapRouteFromBody(body);
        if (route === 'transparent_navswap') {
            return executeTransparentNavswapQuote(body, rpcRequest);
        }
        if (route === 'uniswap_atomic_handoff') {
            return executePftlUniswapWalletQuote(body, rpcRequest);
        }
        if (route === 'shielded_navswap') {
            return {
                ok: false,
                schema: NAVSWAP_QUOTE_SCHEMA,
                route,
                code: 'shielded_navswap_use_shielded_quote_endpoint',
                message: 'Use /api/shielded-nav-swap/quote for private NAVSwap quote preview. Generic NAVSwap quote/run remains disabled for shielded value movement.',
            };
        }

        const quote = buildNavswapQuoteResponse(body);
        if (quote.ok !== true || route !== 'stakehub_transparent_roundtrip') return quote;

        const proof = await buildNavswapNavProofResponse(new URLSearchParams({
            asset_id: 'a651',
            phase: body.phase || 'current',
        }));
        if (!navswapProofIsFresh(proof)) {
            return {
                ok: false,
                schema: NAVSWAP_QUOTE_SCHEMA,
                route,
                code: proof.ok === false ? (proof.code || 'stakehub_nav_proof_unavailable') : 'stakehub_nav_proof_not_fresh',
                message: proof.message || 'StakeHub transparent roundtrip requires a fresh NAV proof before quote/run.',
                nav_proof: proof,
            };
        }
        const preflight = await buildStakehubTransparentPreflight();
        if (preflight.ok !== true) {
            return {
                ok: false,
                schema: NAVSWAP_QUOTE_SCHEMA,
                route,
                code: preflight.code || 'stakehub_transparent_preflight_unavailable',
                message: preflight.message || 'StakeHub transparent preflight is unavailable.',
                nav_proof: proof,
                stakehub_preflight: preflight,
            };
        }

        return {
            ...quote,
            nav_proof: {
                asset_id: proof.asset_id,
                chain_id: proof.chain_id,
                current_pftl_height: proof.current_pftl_height,
                nav_epoch: proof.nav_epoch,
                reserve_packet_hash: proof.reserve_packet_hash,
                freshness_deadline_height: proof.freshness_deadline_height,
                nav_per_unit: proof.nav_per_unit,
                supply: proof.supply,
                proof_status: proof.proof_status,
                stale: proof.stale,
                source_receipt_hashes: proof.source_receipt_hashes,
            },
            stakehub_preflight: preflight,
        };
    }


    return { assetIdForNavswapSymbol,buildNavswapNavProofResponse,buildNavswapQuoteResponse,buildPftlUniswapReceiptVerification,buildStakehubTransparentPreflight,buildTransparentNavswapReceiptVerification,buildTransparentNavswapRedeemReceiptVerification,buildUrl,completePftlUniswapHandoffRun,completeTransparentNavswapRun,executeNavswapDevnetPfusdcFunding,executeNavswapQuote,executePftlUniswapHandoffRun,executePftlUniswapWalletQuote,executeTransparentNavswapQuote,executeTransparentNavswapReadiness,executeTransparentNavswapRun,fetchJsonWithTimeout,isIssuedAsset,isPftAsset,loadPftlUniswapWalletActionContext,navswapAccountAssetItems,navswapAccountBalanceAtoms,navswapActionAutoPlanRequested,navswapActionPrepareError,navswapAllocationRemainingAtoms,navswapAssetInfoAsset,navswapAssetInfoIssuer,navswapAssetIssuer,navswapAssetPrecision,navswapCompletionConsumerIds,navswapCompletionOperationTemplate,navswapCompletionSubmittedChainId,navswapCompletionSubmittedSequence,navswapConsumerMatchesRecipient,navswapDecimalAmountToAtoms,navswapFreshnessFromBody,navswapFreshnessPayload,navswapHashHexDomain,navswapNativeAccountBalanceAtoms,navswapNavProofStub,navswapNavRedemptionId,navswapPftlUniswapControlledAttestationTxHash,navswapPftlUniswapDefaultDeadlineSeconds,navswapPftlUniswapDefaultEthereumRecipient,navswapPftlUniswapDefaultRefundDelayBlocks,navswapPftlUniswapDestinationHeights,navswapPftlUniswapPacketHash,navswapPftlUniswapRouteRow,navswapPlannerCurrentHeight,navswapPlannerError,navswapPlannerNumber,navswapPlannerPositiveNumber,navswapPlannerRemainingAtoms,navswapPrimaryMintIntentFields,navswapProofIsFresh,navswapRandomHex,navswapReceiptFreshness,navswapRedeemCompletionOperationTemplate,navswapRequiredVaultBridgeSettlementAtoms,navswapRouteFromBody,navswapRpcRead,navswapSafeU64Number,navswapSettlementReceiptFreshnessConfig,navswapSettlementReceiptHash,navswapSubscriptionId,navswapValuationUnitScale,navswapWalletActionBatchItems,navswapWalletActionId,normalizePftlUniswapPacketStatus,parseAtomicInteger,parseNavswapActionInteger,parseNavswapDisplayOrAtomAmount,parseNavswapEvmAddress,parseNavswapHexId,parseNavswapWalletAddress,parseStakehubTransparentAmount,pftlUniswapCompletionError,pftlUniswapCompletionQuote,pftlUniswapPreparedAction,planTransparentNavswapWalletActions,preflightNavswapPreparedActionFees,prepareNavswapWalletAction,prepareNavswapWalletActionBatch,prepareNavswapWalletNavRedeemAtNavAction,prepareNavswapWalletNavSubscriptionAllocateAction,preparePftlUniswapWalletActionBatch,selectNavswapIssuedSettlementSource,selectTransparentRedeemSettlementAllocation,signAndSubmitNavswapOperatorAssetTransaction,stakehubTransparentAmountError,transparentCompletionError,transparentCompletionPreparedAction,transparentCompletionQuote,transparentCompletionStage,transparentCompletionSubmission,transparentCompletionWalletResult,validateNavswapPlannerMarketStatus,verifyPftlUniswapExportPacket,verifyPftlUniswapWalletCompletionInput,verifyTransparentNavRedeemSettlement,verifyTransparentNavSubscriptionAllocation,verifyTransparentWalletCompletionInput };
}

module.exports = { create };
