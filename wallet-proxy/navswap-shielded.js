'use strict';

function create(runtime) {
    const { execFileSync, spawn } = runtime;
    const { A651_ASSET_ID,A652_ASSET_ID,ALLOWED_ORIGINS,ASSET_ORCHARD_INGRESS_FILE_SCHEMA,ASSET_ORCHARD_POOL_ID,ASSET_ORCHARD_PRIVATE_EGRESS_ACTION_SCHEMA,ASSET_ORCHARD_PRIVATE_EGRESS_FILE_SCHEMA,ASSET_ORCHARD_SWAP_ACTION_SCHEMA,DEFAULT_ASSET_ORCHARD_LOCAL_SERVICE_READINESS_TIMEOUT_MS,DEFAULT_ASSET_ORCHARD_LOCAL_SERVICE_URL,DEFAULT_RPC_FLEET,ENABLE_FINALITY_RESPONDER_READ_CACHE,ENABLE_FIRST_READY_SEQUENCED_READ,ENABLE_NATIVE_WALLET_SIGNER,ENABLE_PROPOSER_ROUTING,ENABLE_RPC_PARENT_WAIT_FINALITY_ROUTE,ENABLE_RPC_PARENT_WAIT_SEQUENCED_READ_ROUTE,ENABLE_UPSTREAM_KEEPALIVE,ETHEREUM_USDC_TOKEN,FASTPAY_BROADCAST_METHODS,FASTPAY_FLEET_STATUS_CACHE_MS,FASTPAY_OWNED_OBJECT_LOOKUP_LIMIT,FASTPAY_REQUIRE_PRIMARY_SUCCESS,FASTPAY_ROUTE_RETRY_MS,FASTPAY_ROUTE_TIMEOUT_MS,FINALITY_METHODS,FIRST_READY_SEQUENCED_READ_PROPOSERS,INJECT_RPC_CAPS,LEGACY_A651_ETH_TOKEN,LEGACY_A651_UNISWAP_POOL_ID,LISTEN_PORT,MAX_TCP_PER_WS,MAX_WS_MESSAGE_BYTES,NATIVE_WALLET_SIGNER_BIN,NATIVE_WALLET_SIGNER_TIMEOUT_MS,NAVSWAP_CAPABILITIES_SCHEMA,NAVSWAP_DEVNET_FUNDING_SCHEMA,NAVSWAP_IDEMPOTENCY_STORE_DEFAULT_PATH,NAVSWAP_IDEMPOTENCY_STORE_SCHEMA,NAVSWAP_IDEMPOTENCY_TTL_MS,NAVSWAP_MAX_LIVE_USD,NAVSWAP_NAV_PROOF_SCHEMA,NAVSWAP_PRIMARY_MINT_ROUTE_FAMILY,NAVSWAP_QUOTE_FRESHNESS_TTL_MS,NAVSWAP_QUOTE_SCHEMA,NAVSWAP_READINESS_SCHEMA,NAVSWAP_ROUTE_TRUST_CLASSES,NAVSWAP_RUN_EVENTS_SCHEMA,NAVSWAP_RUN_LIST_SCHEMA,NAVSWAP_RUN_RECEIPTS_SCHEMA,NAVSWAP_RUN_SCHEMA,NAVSWAP_RUN_STATUS_SCHEMA,NAVSWAP_RUN_STORE_DEFAULT_PATH,NAVSWAP_RUN_STORE_SCHEMA,NAVSWAP_RUN_STREAM_EVENT_SCHEMA,NAVSWAP_RUN_STREAM_SCHEMA,NAVSWAP_SETTLEMENT_RECEIPT_MAX_SNAPSHOT_AGE_BLOCKS,NAVSWAP_SETTLEMENT_RECEIPT_SAFETY_BLOCKS,NAVSWAP_STAKEHUB_TRANSPARENT_ACTION,NAVSWAP_TRANSPARENT_PLANNER_INPUTS_SCHEMA,NAVSWAP_WALLET_ACTION_BATCH_PREPARE_SCHEMA,NAVSWAP_WALLET_ACTION_PREPARE_SCHEMA,NAVSWAP_WALLET_ACTION_SCHEMA,OPTIMISTIC_CACHED_FINALITY_ROUTE,PFUSDC_ASSET_ID,PREFERRED_SEQUENCED_READ_VALIDATORS,PROPOSER_READY_RETRY_MS,PROPOSER_ROUTE_CACHE_MS,PROPOSER_ROUTE_RETRY_MS,PROPOSER_ROUTE_TIMEOUT_MS,RPC_CAPS,RPC_FLEET,RPC_HOST,RPC_PORT,SEQUENCED_ACCOUNT_METHODS,SHIELDED_NAVSWAP_EGRESS_POLICY_ID,SHIELDED_NAVSWAP_EGRESS_SCHEMA,SHIELDED_NAVSWAP_INGRESS_SCHEMA,SHIELDED_NAVSWAP_LIQUIDITY_MODES,SHIELDED_NAVSWAP_PREFLIGHT_SCHEMA,SHIELDED_NAVSWAP_QUOTE_SCHEMA,SHIELDED_NAVSWAP_STATUS_SCHEMA,SHIELDED_NAVSWAP_SWAP_SCHEMA,SHIELDED_PRIVATE_KEY_PATTERNS,SHIELDED_ROUND_TIMEOUT_DEFAULT_MS,TCP_TIMEOUT_MS,UpstreamRpcConnection,VAULT_BRIDGE_ALLOCATION_PURPOSE_NAV_SUBSCRIPTION,VAULT_BRIDGE_ALLOCATION_PURPOSE_SUPPLY,VAULT_BRIDGE_BUCKET_STATUS_ACTIVE,VAULT_BRIDGE_RECEIPT_STATUS_COUNTED,VAULT_BRIDGE_RECIPIENT_SPONSOR_AMOUNT,VAULT_BRIDGE_RECIPIENT_SPONSOR_MIN_AMOUNT_ATOMS,VAULT_BRIDGE_RELAY_DEFAULT_ACCOUNT,VAULT_BRIDGE_RELAY_EXPIRES_AT_HEIGHT,VAULT_BRIDGE_RELAY_POLICY_HASH,VAULT_BRIDGE_RELAY_SCHEMA,VAULT_BRIDGE_RELAY_SOURCE_CHAIN_ID,VAULT_BRIDGE_RELAY_SOURCE_RPC_URL,VAULT_BRIDGE_RELAY_TOKEN_ADDRESS,VAULT_BRIDGE_RELAY_VAULT_ADDRESS,WALLET_SUBSCRIPTION_INTERVAL_MS,WALLET_SUBSCRIPTION_MIN_INTERVAL_MS,WALLET_SUBSCRIPTION_READ_TIMEOUT_MS,crypto,execFileAsync,fs,http,navswapDevnetFundingUsage,navswapIdempotencyRecords,navswapRunStreams,navswapRuns,net,os,path,server,upstreamRpcConnections,wss } = runtime;
    let { fastpayFleetStatusCache,fastpayFleetStatusInFlight,latestFinalizedReadCache,preferredSequencedReadIndex,proposerRouteCache } = runtime;
    const addProxyRouteEvent = (...args) => runtime.addProxyRouteEvent(...args);
    const annotateNavswapIdempotency = (...args) => runtime.annotateNavswapIdempotency(...args);
    const assertNoShieldedPrivateMaterial = (...args) => runtime.assertNoShieldedPrivateMaterial(...args);
    const assertVaultBridgeEvidenceMatches = (...args) => runtime.assertVaultBridgeEvidenceMatches(...args);
    const assetIdForNavswapSymbol = (...args) => runtime.assetIdForNavswapSymbol(...args);
    const assetOrchardLocalServiceConfig = (...args) => runtime.assetOrchardLocalServiceConfig(...args);
    const bftQuorumThreshold = (...args) => runtime.bftQuorumThreshold(...args);
    const broadcastFastpayMutation = (...args) => runtime.broadcastFastpayMutation(...args);
    const buildNavswapNavProofResponse = (...args) => runtime.buildNavswapNavProofResponse(...args);
    const buildNavswapQuoteResponse = (...args) => runtime.buildNavswapQuoteResponse(...args);
    const buildNavswapRunResponse = (...args) => runtime.buildNavswapRunResponse(...args);
    const buildPftlUniswapReceiptVerification = (...args) => runtime.buildPftlUniswapReceiptVerification(...args);
    const buildStakehubTransparentPreflight = (...args) => runtime.buildStakehubTransparentPreflight(...args);
    const buildTransparentNavswapReceiptVerification = (...args) => runtime.buildTransparentNavswapReceiptVerification(...args);
    const buildTransparentNavswapRedeemReceiptVerification = (...args) => runtime.buildTransparentNavswapRedeemReceiptVerification(...args);
    const buildUniswapHandoffQuoteBinding = (...args) => runtime.buildUniswapHandoffQuoteBinding(...args);
    const buildUrl = (...args) => runtime.buildUrl(...args);
    const buildVaultBridgeRelayBundle = (...args) => runtime.buildVaultBridgeRelayBundle(...args);
    const cachedSelection = (...args) => runtime.cachedSelection(...args);
    const canonicalReadResult = (...args) => runtime.canonicalReadResult(...args);
    const chooseOwnedVoteEndpoint = (...args) => runtime.chooseOwnedVoteEndpoint(...args);
    const chooseProposerEndpointCached = (...args) => runtime.chooseProposerEndpointCached(...args);
    const chooseProposerEndpointFromStatuses = (...args) => runtime.chooseProposerEndpointFromStatuses(...args);
    const chooseProposerEndpointWithRetry = (...args) => runtime.chooseProposerEndpointWithRetry(...args);
    const chooseSequencedAccountReadEndpoint = (...args) => runtime.chooseSequencedAccountReadEndpoint(...args);
    const clearFastpayFleetStatusCache = (...args) => runtime.clearFastpayFleetStatusCache(...args);
    const clearNavswapDevnetFundingUsageForTest = (...args) => runtime.clearNavswapDevnetFundingUsageForTest(...args);
    const clearNavswapIdempotencyForTest = (...args) => runtime.clearNavswapIdempotencyForTest(...args);
    const clearNavswapRunsForTest = (...args) => runtime.clearNavswapRunsForTest(...args);
    const closeUpstreamRpcConnections = (...args) => runtime.closeUpstreamRpcConnections(...args);
    const collectFastpayFleetStatuses = (...args) => runtime.collectFastpayFleetStatuses(...args);
    const collectFleetStatuses = (...args) => runtime.collectFleetStatuses(...args);
    const compareNavswapRunsNewestFirst = (...args) => runtime.compareNavswapRunsNewestFirst(...args);
    const completePftlUniswapHandoffRun = (...args) => runtime.completePftlUniswapHandoffRun(...args);
    const completeTransparentNavswapRun = (...args) => runtime.completeTransparentNavswapRun(...args);
    const conciseRpcError = (...args) => runtime.conciseRpcError(...args);
    const convergedFleetGroup = (...args) => runtime.convergedFleetGroup(...args);
    const createNavswapRun = (...args) => runtime.createNavswapRun(...args);
    const currentA652AssetId = (...args) => runtime.currentA652AssetId(...args);
    const deterministicProposer = (...args) => runtime.deterministicProposer(...args);
    const endpointStatusMeetsRoute = (...args) => runtime.endpointStatusMeetsRoute(...args);
    const endpointStatusMeetsSequencedReadRoute = (...args) => runtime.endpointStatusMeetsSequencedReadRoute(...args);
    const ensureVaultBridgeRecipientAccount = (...args) => runtime.ensureVaultBridgeRecipientAccount(...args);
    const executeNavswapAtomicTemplate = (...args) => runtime.executeNavswapAtomicTemplate(...args);
    const executeNavswapCapabilities = (...args) => runtime.executeNavswapCapabilities(...args);
    const executeNavswapDevnetPfusdcFunding = (...args) => runtime.executeNavswapDevnetPfusdcFunding(...args);
    const executeNavswapIdempotentRequest = (...args) => runtime.executeNavswapIdempotentRequest(...args);
    const executeNavswapQuote = (...args) => runtime.executeNavswapQuote(...args);
    const executeNavswapRun = (...args) => runtime.executeNavswapRun(...args);
    const executePftlUniswapHandoffRun = (...args) => runtime.executePftlUniswapHandoffRun(...args);
    const executePftlUniswapWalletQuote = (...args) => runtime.executePftlUniswapWalletQuote(...args);
    const executeTransparentNavswapQuote = (...args) => runtime.executeTransparentNavswapQuote(...args);
    const executeTransparentNavswapReadiness = (...args) => runtime.executeTransparentNavswapReadiness(...args);
    const executeTransparentNavswapRun = (...args) => runtime.executeTransparentNavswapRun(...args);
    const executeVaultBridgeRelay = (...args) => runtime.executeVaultBridgeRelay(...args);
    const fetchJsonWithTimeout = (...args) => runtime.fetchJsonWithTimeout(...args);
    const fetchWalletSnapshot = (...args) => runtime.fetchWalletSnapshot(...args);
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
    const isIssuedAsset = (...args) => runtime.isIssuedAsset(...args);
    const isNativeWalletSignMethod = (...args) => runtime.isNativeWalletSignMethod(...args);
    const isPftAsset = (...args) => runtime.isPftAsset(...args);
    const isReplayableVaultBridgeRelayDuplicate = (...args) => runtime.isReplayableVaultBridgeRelayDuplicate(...args);
    const isSequencedAccountMethod = (...args) => runtime.isSequencedAccountMethod(...args);
    const jsonHeaders = (...args) => runtime.jsonHeaders(...args);
    const loadNavswapIdempotencyStore = (...args) => runtime.loadNavswapIdempotencyStore(...args);
    const loadNavswapRunStore = (...args) => runtime.loadNavswapRunStore(...args);
    const loadPftlUniswapWalletActionContext = (...args) => runtime.loadPftlUniswapWalletActionContext(...args);
    const lower = (...args) => runtime.lower(...args);
    const markStoredNavswapRunInterrupted = (...args) => runtime.markStoredNavswapRunInterrupted(...args);
    const navswapAccountAssetItems = (...args) => runtime.navswapAccountAssetItems(...args);
    const navswapAccountBalanceAtoms = (...args) => runtime.navswapAccountBalanceAtoms(...args);
    const navswapActionAutoPlanRequested = (...args) => runtime.navswapActionAutoPlanRequested(...args);
    const navswapActionPrepareError = (...args) => runtime.navswapActionPrepareError(...args);
    const navswapAllocationRemainingAtoms = (...args) => runtime.navswapAllocationRemainingAtoms(...args);
    const navswapAssetInfoAsset = (...args) => runtime.navswapAssetInfoAsset(...args);
    const navswapAssetInfoIssuer = (...args) => runtime.navswapAssetInfoIssuer(...args);
    const navswapAssetIssuer = (...args) => runtime.navswapAssetIssuer(...args);
    const navswapAssetPrecision = (...args) => runtime.navswapAssetPrecision(...args);
    const navswapAsyncRunRequested = (...args) => runtime.navswapAsyncRunRequested(...args);
    const navswapBridgeConfig = (...args) => runtime.navswapBridgeConfig(...args);
    const navswapCapabilities = (...args) => runtime.navswapCapabilities(...args);
    const navswapCompletionConsumerIds = (...args) => runtime.navswapCompletionConsumerIds(...args);
    const navswapCompletionOperationTemplate = (...args) => runtime.navswapCompletionOperationTemplate(...args);
    const navswapCompletionSubmittedChainId = (...args) => runtime.navswapCompletionSubmittedChainId(...args);
    const navswapCompletionSubmittedSequence = (...args) => runtime.navswapCompletionSubmittedSequence(...args);
    const navswapConsumerMatchesRecipient = (...args) => runtime.navswapConsumerMatchesRecipient(...args);
    const navswapDecimalAmountToAtoms = (...args) => runtime.navswapDecimalAmountToAtoms(...args);
    const navswapDevnetFundingUsageSnapshot = (...args) => runtime.navswapDevnetFundingUsageSnapshot(...args);
    const navswapDevnetFundingWindowUsage = (...args) => runtime.navswapDevnetFundingWindowUsage(...args);
    const navswapDevnetPfusdcFundingConfig = (...args) => runtime.navswapDevnetPfusdcFundingConfig(...args);
    const navswapFreshnessFromBody = (...args) => runtime.navswapFreshnessFromBody(...args);
    const navswapFreshnessPayload = (...args) => runtime.navswapFreshnessPayload(...args);
    const navswapHashHexDomain = (...args) => runtime.navswapHashHexDomain(...args);
    const navswapIdempotencyHashBody = (...args) => runtime.navswapIdempotencyHashBody(...args);
    const navswapIdempotencyStoreSnapshot = (...args) => runtime.navswapIdempotencyStoreSnapshot(...args);
    const navswapInferTrustClass = (...args) => runtime.navswapInferTrustClass(...args);
    const navswapListLimit = (...args) => runtime.navswapListLimit(...args);
    const navswapNativeAccountBalanceAtoms = (...args) => runtime.navswapNativeAccountBalanceAtoms(...args);
    const navswapNavProofStub = (...args) => runtime.navswapNavProofStub(...args);
    const navswapNavRedemptionId = (...args) => runtime.navswapNavRedemptionId(...args);
    const navswapNormalizeTrustClass = (...args) => runtime.navswapNormalizeTrustClass(...args);
    const navswapPftlUniswapControlledAttestationTxHash = (...args) => runtime.navswapPftlUniswapControlledAttestationTxHash(...args);
    const navswapPftlUniswapDefaultDeadlineSeconds = (...args) => runtime.navswapPftlUniswapDefaultDeadlineSeconds(...args);
    const navswapPftlUniswapDefaultEthereumRecipient = (...args) => runtime.navswapPftlUniswapDefaultEthereumRecipient(...args);
    const navswapPftlUniswapDefaultRefundDelayBlocks = (...args) => runtime.navswapPftlUniswapDefaultRefundDelayBlocks(...args);
    const navswapPftlUniswapDestinationHeights = (...args) => runtime.navswapPftlUniswapDestinationHeights(...args);
    const navswapPftlUniswapPacketHash = (...args) => runtime.navswapPftlUniswapPacketHash(...args);
    const navswapPftlUniswapRouteRow = (...args) => runtime.navswapPftlUniswapRouteRow(...args);
    const navswapPlannerCurrentHeight = (...args) => runtime.navswapPlannerCurrentHeight(...args);
    const navswapPlannerError = (...args) => runtime.navswapPlannerError(...args);
    const navswapPlannerNumber = (...args) => runtime.navswapPlannerNumber(...args);
    const navswapPlannerPositiveNumber = (...args) => runtime.navswapPlannerPositiveNumber(...args);
    const navswapPlannerRemainingAtoms = (...args) => runtime.navswapPlannerRemainingAtoms(...args);
    const navswapPrimaryMintIntentFields = (...args) => runtime.navswapPrimaryMintIntentFields(...args);
    const navswapProofIsFresh = (...args) => runtime.navswapProofIsFresh(...args);
    const navswapRandomHex = (...args) => runtime.navswapRandomHex(...args);
    const navswapReceiptFreshness = (...args) => runtime.navswapReceiptFreshness(...args);
    const navswapRedeemCompletionOperationTemplate = (...args) => runtime.navswapRedeemCompletionOperationTemplate(...args);
    const navswapRequiredVaultBridgeSettlementAtoms = (...args) => runtime.navswapRequiredVaultBridgeSettlementAtoms(...args);
    const navswapRouteFromBody = (...args) => runtime.navswapRouteFromBody(...args);
    const navswapRoutePrivacy = (...args) => runtime.navswapRoutePrivacy(...args);
    const navswapRpcRead = (...args) => runtime.navswapRpcRead(...args);
    const navswapRunEvents = (...args) => runtime.navswapRunEvents(...args);
    const navswapRunIsTerminal = (...args) => runtime.navswapRunIsTerminal(...args);
    const navswapRunList = (...args) => runtime.navswapRunList(...args);
    const navswapRunPublic = (...args) => runtime.navswapRunPublic(...args);
    const navswapRunReceipts = (...args) => runtime.navswapRunReceipts(...args);
    const navswapRunSortTime = (...args) => runtime.navswapRunSortTime(...args);
    const navswapRunStoreSnapshot = (...args) => runtime.navswapRunStoreSnapshot(...args);
    const navswapRunStreamSnapshot = (...args) => runtime.navswapRunStreamSnapshot(...args);
    const navswapSafeU64Number = (...args) => runtime.navswapSafeU64Number(...args);
    const navswapSettlementReceiptFreshnessConfig = (...args) => runtime.navswapSettlementReceiptFreshnessConfig(...args);
    const navswapSettlementReceiptHash = (...args) => runtime.navswapSettlementReceiptHash(...args);
    const navswapStakehubTransparentConfig = (...args) => runtime.navswapStakehubTransparentConfig(...args);
    const navswapSubscriptionId = (...args) => runtime.navswapSubscriptionId(...args);
    const navswapTransparentOperatorConfig = (...args) => runtime.navswapTransparentOperatorConfig(...args);
    const navswapTrustlessFinalityAgreement = (...args) => runtime.navswapTrustlessFinalityAgreement(...args);
    const navswapTruthyParam = (...args) => runtime.navswapTruthyParam(...args);
    const navswapUniswapBetaRouteState = (...args) => runtime.navswapUniswapBetaRouteState(...args);
    const navswapValuationUnitScale = (...args) => runtime.navswapValuationUnitScale(...args);
    const navswapWalletActionBatchItems = (...args) => runtime.navswapWalletActionBatchItems(...args);
    const navswapWalletActionId = (...args) => runtime.navswapWalletActionId(...args);
    const normalizeAtomicTemplateParams = (...args) => runtime.normalizeAtomicTemplateParams(...args);
    const normalizeFastpayBroadcastRequest = (...args) => runtime.normalizeFastpayBroadcastRequest(...args);
    const normalizePftlUniswapPacketStatus = (...args) => runtime.normalizePftlUniswapPacketStatus(...args);
    const normalizeShieldedKey = (...args) => runtime.normalizeShieldedKey(...args);
    const normalizeShieldedLiquidityMode = (...args) => runtime.normalizeShieldedLiquidityMode(...args);
    const normalizeStoredNavswapIdempotencyRecord = (...args) => runtime.normalizeStoredNavswapIdempotencyRecord(...args);
    const normalizeStoredNavswapRun = (...args) => runtime.normalizeStoredNavswapRun(...args);
    const normalizeVaultBridgeAddress = (...args) => runtime.normalizeVaultBridgeAddress(...args);
    const normalizeVaultBridgeBytes32 = (...args) => runtime.normalizeVaultBridgeBytes32(...args);
    const normalizeVaultBridgeTxHash = (...args) => runtime.normalizeVaultBridgeTxHash(...args);
    const normalizeWalletSubscriptionParams = (...args) => runtime.normalizeWalletSubscriptionParams(...args);
    const originAllowed = (...args) => runtime.originAllowed(...args);
    const parseAtomicInteger = (...args) => runtime.parseAtomicInteger(...args);
    const parseNavswapActionInteger = (...args) => runtime.parseNavswapActionInteger(...args);
    const parseNavswapDisplayOrAtomAmount = (...args) => runtime.parseNavswapDisplayOrAtomAmount(...args);
    const parseNavswapEvmAddress = (...args) => runtime.parseNavswapEvmAddress(...args);
    const parseNavswapHexId = (...args) => runtime.parseNavswapHexId(...args);
    const parseNavswapWalletAddress = (...args) => runtime.parseNavswapWalletAddress(...args);
    const parseRpcFleet = (...args) => runtime.parseRpcFleet(...args);
    const parseStakehubTransparentAmount = (...args) => runtime.parseStakehubTransparentAmount(...args);
    const parseUniswapHandoffBytes32 = (...args) => runtime.parseUniswapHandoffBytes32(...args);
    const parseUniswapHandoffPositiveInteger = (...args) => runtime.parseUniswapHandoffPositiveInteger(...args);
    const persistNavswapIdempotencyRecord = (...args) => runtime.persistNavswapIdempotencyRecord(...args);
    const persistNavswapRun = (...args) => runtime.persistNavswapRun(...args);
    const pftlUniswapCompletionError = (...args) => runtime.pftlUniswapCompletionError(...args);
    const pftlUniswapCompletionQuote = (...args) => runtime.pftlUniswapCompletionQuote(...args);
    const pftlUniswapPreparedAction = (...args) => runtime.pftlUniswapPreparedAction(...args);
    const planTransparentNavswapWalletActions = (...args) => runtime.planTransparentNavswapWalletActions(...args);
    const preferredSequencedReadEndpoint = (...args) => runtime.preferredSequencedReadEndpoint(...args);
    const preflightNavswapPreparedActionFees = (...args) => runtime.preflightNavswapPreparedActionFees(...args);
    const prepareNavswapWalletAction = (...args) => runtime.prepareNavswapWalletAction(...args);
    const prepareNavswapWalletActionBatch = (...args) => runtime.prepareNavswapWalletActionBatch(...args);
    const prepareNavswapWalletNavRedeemAtNavAction = (...args) => runtime.prepareNavswapWalletNavRedeemAtNavAction(...args);
    const prepareNavswapWalletNavSubscriptionAllocateAction = (...args) => runtime.prepareNavswapWalletNavSubscriptionAllocateAction(...args);
    const preparePftlUniswapWalletActionBatch = (...args) => runtime.preparePftlUniswapWalletActionBatch(...args);
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
    const sanitizeNavswapRunRequest = (...args) => runtime.sanitizeNavswapRunRequest(...args);
    const selectNavswapIssuedSettlementSource = (...args) => runtime.selectNavswapIssuedSettlementSource(...args);
    const selectTransparentRedeemSettlementAllocation = (...args) => runtime.selectTransparentRedeemSettlementAllocation(...args);
    const sendJson = (...args) => runtime.sendJson(...args);
    const sendNavswapRunStream = (...args) => runtime.sendNavswapRunStream(...args);
    const sendWalletNotification = (...args) => runtime.sendWalletNotification(...args);
    const shieldedLiquidityModeLabel = (...args) => runtime.shieldedLiquidityModeLabel(...args);
    const shieldedNavswapEgressConfig = (...args) => runtime.shieldedNavswapEgressConfig(...args);
    const shieldedNavswapIngressConfig = (...args) => runtime.shieldedNavswapIngressConfig(...args);
    const shieldedNavswapQuoteConfig = (...args) => runtime.shieldedNavswapQuoteConfig(...args);
    const shieldedNavswapSwapConfig = (...args) => runtime.shieldedNavswapSwapConfig(...args);
    const shieldedQuotePolicyHash = (...args) => runtime.shieldedQuotePolicyHash(...args);
    const shouldUseFirstReadySequencedRead = (...args) => runtime.shouldUseFirstReadySequencedRead(...args);
    const signAndSubmitNavswapOperatorAssetTransaction = (...args) => runtime.signAndSubmitNavswapOperatorAssetTransaction(...args);
    const signAndSubmitVaultBridgeRecipientSponsor = (...args) => runtime.signAndSubmitVaultBridgeRecipientSponsor(...args);
    const signAndSubmitVaultBridgeRelayOperation = (...args) => runtime.signAndSubmitVaultBridgeRelayOperation(...args);
    const signWalletOwnedOrder = (...args) => runtime.signWalletOwnedOrder(...args);
    const sleep = (...args) => runtime.sleep(...args);
    const sseHeaders = (...args) => runtime.sseHeaders(...args);
    const stakehubTransparentAmountError = (...args) => runtime.stakehubTransparentAmountError(...args);
    const startCachedSelectionReadinessProbe = (...args) => runtime.startCachedSelectionReadinessProbe(...args);
    const startWalletSubscription = (...args) => runtime.startWalletSubscription(...args);
    const stopWalletSubscription = (...args) => runtime.stopWalletSubscription(...args);
    const swapAtomicTemplateParams = (...args) => runtime.swapAtomicTemplateParams(...args);
    const transparentCompletionError = (...args) => runtime.transparentCompletionError(...args);
    const transparentCompletionPreparedAction = (...args) => runtime.transparentCompletionPreparedAction(...args);
    const transparentCompletionQuote = (...args) => runtime.transparentCompletionQuote(...args);
    const transparentCompletionStage = (...args) => runtime.transparentCompletionStage(...args);
    const transparentCompletionSubmission = (...args) => runtime.transparentCompletionSubmission(...args);
    const transparentCompletionWalletResult = (...args) => runtime.transparentCompletionWalletResult(...args);
    const upstreamEndpointKey = (...args) => runtime.upstreamEndpointKey(...args);
    const upstreamRpcConnection = (...args) => runtime.upstreamRpcConnection(...args);
    const validateNavswapPlannerMarketStatus = (...args) => runtime.validateNavswapPlannerMarketStatus(...args);
    const vaultBridgeAccountAssets = (...args) => runtime.vaultBridgeAccountAssets(...args);
    const vaultBridgeBodyTxHash = (...args) => runtime.vaultBridgeBodyTxHash(...args);
    const vaultBridgeEvidenceFromPlan = (...args) => runtime.vaultBridgeEvidenceFromPlan(...args);
    const vaultBridgeExpectedField = (...args) => runtime.vaultBridgeExpectedField(...args);
    const vaultBridgePftlAccountExists = (...args) => runtime.vaultBridgePftlAccountExists(...args);
    const vaultBridgeRelayConfig = (...args) => runtime.vaultBridgeRelayConfig(...args);
    const verifyAtomicTemplateResult = (...args) => runtime.verifyAtomicTemplateResult(...args);
    const verifyAtomicTemplateSymmetry = (...args) => runtime.verifyAtomicTemplateSymmetry(...args);
    const verifyPftlUniswapExportPacket = (...args) => runtime.verifyPftlUniswapExportPacket(...args);
    const verifyPftlUniswapWalletCompletionInput = (...args) => runtime.verifyPftlUniswapWalletCompletionInput(...args);
    const verifyTransparentNavRedeemSettlement = (...args) => runtime.verifyTransparentNavRedeemSettlement(...args);
    const verifyTransparentNavSubscriptionAllocation = (...args) => runtime.verifyTransparentNavSubscriptionAllocation(...args);
    const verifyTransparentWalletCompletionInput = (...args) => runtime.verifyTransparentWalletCompletionInput(...args);
    const waitForCachedSelectionReady = (...args) => runtime.waitForCachedSelectionReady(...args);
    const waitForFastpayConvergedGroup = (...args) => runtime.waitForFastpayConvergedGroup(...args);
    const walletSnapshotDigest = (...args) => runtime.walletSnapshotDigest(...args);
    const writeSseEvent = (...args) => runtime.writeSseEvent(...args);

    function newNavswapRunId() {
        return `navswap-${Date.now().toString(36)}-${crypto.randomBytes(4).toString('hex')}`;
    }

    function navswapRunStorePath() {
        const configured = process.env.NAVSWAP_RUN_STORE_PATH;
        if (configured !== undefined) {
            const trimmed = String(configured).trim();
            if (!trimmed || trimmed === 'off' || trimmed === 'false' || trimmed === '0') return null;
            return path.resolve(trimmed);
        }
        return require.main === module ? NAVSWAP_RUN_STORE_DEFAULT_PATH : null;
    }

    function navswapIdempotencyStorePath() {
        const configured = process.env.NAVSWAP_IDEMPOTENCY_STORE_PATH;
        if (configured !== undefined) {
            const trimmed = String(configured).trim();
            if (!trimmed || trimmed === 'off' || trimmed === 'false' || trimmed === '0') return null;
            return path.resolve(trimmed);
        }
        return require.main === module ? NAVSWAP_IDEMPOTENCY_STORE_DEFAULT_PATH : null;
    }

    function cloneJson(value) {
        if (value === undefined) return undefined;
        return JSON.parse(JSON.stringify(value));
    }

    function navswapStableJson(value) {
        if (value === null || typeof value !== 'object') return JSON.stringify(value);
        if (Array.isArray(value)) return `[${value.map(navswapStableJson).join(',')}]`;
        return `{${Object.keys(value).sort().map((key) => `${JSON.stringify(key)}:${navswapStableJson(value[key])}`).join(',')}}`;
    }

    function navswapIdempotencyKeyFromRequest(req, body = {}) {
        const headerValue = req?.headers?.['idempotency-key'];
        const raw = body.idempotency_key
            || body.idempotencyKey
            || (Array.isArray(headerValue) ? headerValue[0] : headerValue)
            || '';
        return String(raw || '').trim();
    }

    function navswapValidateIdempotencyKey(key) {
        if (!key) return null;
        if (!/^[A-Za-z0-9._:-]{8,160}$/.test(key)) {
            return {
                ok: false,
                schema: 'postfiat-navswap-idempotency-v1',
                code: 'navswap_idempotency_key_invalid',
                message: 'NAVSwap idempotency_key must be 8-160 characters of letters, numbers, dot, underscore, colon, or hyphen.',
                idempotency_key: key,
            };
        }
        return null;
    }

    function shieldedIngressSupportedAsset(config, assetId) {
        return config.supported_assets.find((asset) => asset.asset_id === assetId) || null;
    }

    function shieldedQuoteAssetByInput(value, config, field) {
        const text = String(value || '').trim().toLowerCase();
        if (!text) {
            const err = new Error(`${field} is required`);
            err.code = 'shielded_navswap_asset_required';
            throw err;
        }
        const asset = config.asset_registry.find((item) => (
            item.symbol === text || item.asset_id === text
        ));
        if (!asset) {
            const err = new Error(`${field} must be a651 or a652`);
            err.code = 'shielded_navswap_asset_unsupported';
            throw err;
        }
        return asset;
    }

    function shieldedQuotePairEnabled(fromAsset, toAsset, config) {
        return config.supported_pairs.some((pair) => (
            pair.enabled === true
            && pair.from_asset === fromAsset.symbol
            && pair.to_asset === toAsset.symbol
        ));
    }

    async function executeShieldedNavswapQuote(body = {}) {
        assertNoShieldedPrivateMaterial(body);
        const config = shieldedNavswapQuoteConfig();
        const swapConfig = shieldedNavswapSwapConfig();
        if (!config.configured) {
            return {
                ok: false,
                schema: SHIELDED_NAVSWAP_QUOTE_SCHEMA,
                route: 'shielded_navswap',
                code: 'shielded_navswap_liquidity_configuration_required',
                status: 'configuration_required',
                message: 'Private NAVSwap quote preview requires a configured a652 asset, issuer, and live liquidity commitment.',
                missing: config.missing,
                can_prove: false,
                can_run: false,
                submit_enabled: false,
                quote: {
                    endpoint: config.endpoint,
                    liquidity_mode: config.liquidity_mode,
                    liquidity_mode_label: config.liquidity_mode_label,
                    liquidity_commitment_status: config.liquidity_commitment_status,
                    policy_hash: config.policy_hash,
                    supported_pairs: config.supported_pairs,
                    asset_registry: config.asset_registry,
                },
            };
        }

        let walletAddress;
        let fromAsset;
        let toAsset;
        let amountAtoms;
        try {
            walletAddress = parseNavswapWalletAddress(body.wallet_address || body.owner || body.source);
            fromAsset = shieldedQuoteAssetByInput(
                body.from_asset_id || body.fromAssetId || body.from_asset || body.from || body.input_asset,
                config,
                'from_asset',
            );
            toAsset = shieldedQuoteAssetByInput(
                body.to_asset_id || body.toAssetId || body.to_asset || body.to || body.output_asset,
                config,
                'to_asset',
            );
            amountAtoms = parseNavswapActionInteger(
                body.amount_atoms || body.amountAtoms || body.amount,
                'amount_atoms',
            );
        } catch (error) {
            return {
                ok: false,
                schema: SHIELDED_NAVSWAP_QUOTE_SCHEMA,
                route: 'shielded_navswap',
                code: error.code || 'shielded_navswap_quote_request_invalid',
                status: 'invalid_request',
                message: error.message || 'Shielded NAVSwap quote request is invalid.',
                can_prove: false,
                can_run: false,
                submit_enabled: false,
            };
        }

        if (fromAsset.symbol === toAsset.symbol || !shieldedQuotePairEnabled(fromAsset, toAsset, config)) {
            return {
                ok: false,
                schema: SHIELDED_NAVSWAP_QUOTE_SCHEMA,
                route: 'shielded_navswap',
                code: 'shielded_navswap_pair_unsupported',
                status: 'pair_unsupported',
                message: 'Private NAVSwap quote preview currently supports configured a651 <-> a652 pairs only.',
                from_asset: fromAsset.symbol,
                to_asset: toAsset.symbol,
                supported_pairs: config.supported_pairs,
                can_prove: false,
                can_run: false,
                submit_enabled: false,
            };
        }

        const nowMs = Date.now();
        const expiresAtMs = nowMs + config.quote_ttl_ms;
        const outputAmountAtoms = String(amountAtoms);
        const expectedAssets = {
            input: {
                symbol: fromAsset.symbol,
                asset_id: fromAsset.asset_id,
                precision: fromAsset.precision,
            },
            output: {
                symbol: toAsset.symbol,
                asset_id: toAsset.asset_id,
                precision: toAsset.precision,
            },
        };
        const expectedValues = {
            input_amount_atoms: String(amountAtoms),
            output_amount_atoms: outputAmountAtoms,
            minimum_output_atoms: outputAmountAtoms,
            price_model: 'parity_nav_asset_atoms_1_to_1',
        };
        const outputRecipients = {
            type: 'wallet_local_private_note',
            wallet_address: walletAddress,
            custody_boundary: config.custody_boundary,
        };
        const binding = {
            schema: 'postfiat-shielded-navswap-quote-binding-v1',
            route: 'shielded_navswap',
            wallet_address: walletAddress,
            expected_assets: expectedAssets,
            expected_values: expectedValues,
            output_recipients: outputRecipients,
            liquidity_commitment: config.liquidity_commitment,
            liquidity_mode: config.liquidity_mode,
            policy_hash: config.policy_hash,
            quote_generated_at_ms: String(nowMs),
            quote_expires_at_ms: String(expiresAtMs),
            failure_mode: config.failure_mode,
            pool_id: config.pool_id,
            submit_gate: config.submit_gate,
        };
        const quoteBindingHash = crypto.createHash('sha256')
            .update(navswapStableJson(binding))
            .digest('hex');
        const submitEnabled = swapConfig.configured;
        return {
            ok: true,
            schema: SHIELDED_NAVSWAP_QUOTE_SCHEMA,
            route: 'shielded_navswap',
            status: submitEnabled ? 'quote_ready_submit_enabled' : 'quote_ready_submit_disabled',
            message: submitEnabled
                ? 'Private NAVSwap quote is bound to controlled pool liquidity and ready for Step 7 submit.'
                : 'Private NAVSwap quote preview is bound to live liquidity. Private proof and submit remain disabled until Step 7.',
            wallet_address: walletAddress,
            from_asset: fromAsset.symbol,
            to_asset: toAsset.symbol,
            from_asset_id: fromAsset.asset_id,
            to_asset_id: toAsset.asset_id,
            amount_atoms: String(amountAtoms),
            input_amount_atoms: String(amountAtoms),
            output_amount_atoms: outputAmountAtoms,
            expected_output: outputAmountAtoms,
            minimum_output_atoms: outputAmountAtoms,
            expected_assets: expectedAssets,
            expected_values: expectedValues,
            output_recipients: outputRecipients,
            price_model: 'parity_nav_asset_atoms_1_to_1',
            quote_generated_at_ms: String(nowMs),
            quote_expires_at_ms: String(expiresAtMs),
            quote_ttl_ms: String(config.quote_ttl_ms),
            quote_freshness: {
                quote_generated_at_ms: String(nowMs),
                quote_expires_at_ms: String(expiresAtMs),
                proof_status: 'fresh',
                reserve_packet_fresh: true,
                supply_packet_fresh: true,
                market_ops_status: 'controlled_pool_liquidity_commitment_live',
            },
            liquidity: {
                mode: config.liquidity_mode,
                mode_label: config.liquidity_mode_label,
                source_class: config.liquidity_mode,
                trust_class: config.trust_class,
                counterparty: config.liquidity_provider,
                commitment: config.liquidity_commitment,
                commitment_status: 'live',
                copy: config.copy,
            },
            policy_hash: config.policy_hash,
            quote_binding_hash: quoteBindingHash,
            quote_binding: binding,
            failure_mode: config.failure_mode,
            asset_registry: config.asset_registry,
            supported_pairs: config.supported_pairs,
            can_prove: submitEnabled,
            can_run: submitEnabled,
            submit_enabled: submitEnabled,
            next_gate: config.submit_gate,
            swap_endpoint: swapConfig.endpoint,
            swap_missing: swapConfig.missing,
            public_disclosure: [
                'wallet_address',
                'from_asset',
                'to_asset',
                'amount_atoms',
                'liquidity_mode',
                'liquidity_commitment',
                'policy_hash',
                'quote_binding_hash',
                'quote_expiry',
            ],
            custody_boundary: config.custody_boundary,
        };
    }

    async function executeShieldedNavswapStatus() {
        const config = shieldedNavswapIngressConfig();
        const quote = shieldedNavswapQuoteConfig();
        const swap = shieldedNavswapSwapConfig();
        const egress = shieldedNavswapEgressConfig();
        return {
            ok: true,
            schema: SHIELDED_NAVSWAP_STATUS_SCHEMA,
            route: 'shielded_navswap',
            status: egress.configured
                ? 'step9_egress_ready'
                : swap.configured
                ? 'step7_swap_ready'
                : quote.configured
                ? 'step6_quote_ready'
                : config.configured
                    ? 'step5_ingress_ready'
                    : 'configuration_required',
            message: egress.configured
                ? 'Private Asset-Orchard swap and explicit public exit are enabled for controlled Step 9 relay.'
                : swap.configured
                ? 'Private Asset-Orchard swap submit is enabled for controlled Step 7 relay.'
                : quote.configured
                ? quote.copy
                : config.configured
                    ? 'Public Asset-Orchard ingress relay is configured; private proof/swap submit remains disabled.'
                    : 'Public Asset-Orchard ingress relay is not configured.',
            quote: {
                enabled: quote.configured,
                endpoint: quote.endpoint,
                missing: quote.missing,
                liquidity_mode: quote.liquidity_mode,
                liquidity_mode_label: quote.liquidity_mode_label,
                liquidity_commitment_status: quote.liquidity_commitment_status,
                policy_hash: quote.policy_hash,
                failure_mode: quote.failure_mode,
                submit_gate: quote.submit_gate,
                asset_registry: quote.asset_registry,
                supported_pairs: quote.supported_pairs,
            },
            ingress: {
                enabled: config.configured,
                endpoint: config.endpoint,
                max_amount_atoms: config.max_amount_atoms,
                supported_assets: config.supported_assets,
                missing: config.missing,
            },
            swap: {
                enabled: swap.configured,
                endpoint: swap.endpoint,
                missing: swap.missing,
                trust_class: 'CONTROLLED',
                quote_binding_enforcement: 'proxy_checked_quote_freshness_and_liquidity_commitment_not_circuit_external_binding',
            },
            egress: {
                enabled: egress.configured,
                endpoint: egress.endpoint,
                missing: egress.missing,
                trust_class: 'CONTROLLED',
                policy_id: egress.policy_id,
                disclosure_required: true,
                bridge_out_requires_public_exit_receipt: true,
                public_disclosure: ['destination', 'asset_id', 'amount_atoms', 'receipt_timing'],
                private_fields: ['note_opening', 'spend_authority', 'wallet_local_note_file'],
            },
            can_quote: quote.configured,
            can_run: swap.configured,
            can_egress: egress.configured,
            custody_boundary: egress.configured ? egress.custody_boundary : swap.configured ? swap.custody_boundary : config.custody_boundary,
        };
    }

    async function executeShieldedNavswapNoteCapability() {
        const config = shieldedNavswapIngressConfig();
        const egress = shieldedNavswapEgressConfig();
        return {
            ok: true,
            schema: 'postfiat-shielded-navswap-note-capability-v1',
            route: 'shielded_navswap',
            local_vault_required: true,
            ingress_enabled: config.configured,
            egress_enabled: egress.configured,
            note_states: ['pending', 'spendable', 'locked_for_swap', 'locked_for_egress', 'spent', 'egressed', 'unknown'],
            custody_boundary: egress.configured ? egress.custody_boundary : config.custody_boundary,
        };
    }

    async function executeShieldedNavswapProverReadiness() {
        const config = assetOrchardLocalServiceConfig();
        if (!config.local_only || !config.readiness_endpoint) {
            return {
                ok: false,
                schema: 'postfiat-shielded-navswap-prover-readiness-v1',
                route: 'shielded_navswap',
                local_only: false,
                ready: false,
                status: 'local_service_configuration_invalid',
                message: 'Asset-Orchard local service must use an http(s) loopback URL.',
                local_service: {
                    url: config.url,
                    readiness_endpoint: config.readiness_endpoint,
                    missing: config.missing,
                },
            };
        }

        try {
            const service = await fetchJsonWithTimeout(config.readiness_endpoint, config.timeout_ms);
            const proverWarm = service?.prover_warm || null;
            return {
                ok: true,
                schema: 'postfiat-shielded-navswap-prover-readiness-v1',
                route: 'shielded_navswap',
                local_only: true,
                ready: Boolean(proverWarm?.ready),
                status: proverWarm?.status || (service?.ready ? 'service_ready_without_prover_state' : 'service_not_ready'),
                message: proverWarm?.ready
                    ? 'Asset-Orchard local prover service is warm.'
                    : 'Asset-Orchard local prover service is reachable; prover may still be warming.',
                pool_id: service?.pool_id || ASSET_ORCHARD_POOL_ID,
                circuit_id: service?.circuit_id || 'asset-orchard-swap-v1',
                k: service?.k || 15,
                params_hash: proverWarm?.circuits?.swap?.params_hash || null,
                vk_hash: proverWarm?.circuits?.swap?.vk_hash || null,
                prover_warm: proverWarm,
                service_readiness: service,
                local_service: {
                    url: config.url,
                    readiness_endpoint: config.readiness_endpoint,
                    timeout_ms: config.timeout_ms,
                },
            };
        } catch (error) {
            return {
                ok: true,
                schema: 'postfiat-shielded-navswap-prover-readiness-v1',
                route: 'shielded_navswap',
                local_only: true,
                ready: false,
                status: 'local_service_unavailable',
                message: 'Asset-Orchard local prover service is not reachable yet.',
                pool_id: ASSET_ORCHARD_POOL_ID,
                circuit_id: 'asset-orchard-swap-v1',
                k: 15,
                params_hash: null,
                vk_hash: null,
                error: error.message || String(error),
                local_service: {
                    url: config.url,
                    readiness_endpoint: config.readiness_endpoint,
                    timeout_ms: config.timeout_ms,
                },
            };
        }
    }

    async function executeShieldedNavswapBalances() {
        return {
            ok: true,
            schema: 'postfiat-shielded-navswap-balances-v1',
            route: 'shielded_navswap',
            public_balances_source: 'wallet_account_assets',
            private_balances_source: 'wallet_local_note_vault',
            message: 'The wallet computes public and private note balances locally; the adapter does not hold wallet note state.',
            balances: [],
        };
    }

    async function executeShieldedNavswapIngressPreflight(body = {}) {
        assertNoShieldedPrivateMaterial(body);
        const config = shieldedNavswapIngressConfig();
        const walletAddress = parseNavswapWalletAddress(body.wallet_address || body.owner || body.source);
        const assetId = parseNavswapHexId(body.asset_id || body.assetId, 'asset_id');
        const amountAtoms = parseNavswapActionInteger(
            body.amount_atoms || body.amountAtoms || body.amount,
            'amount_atoms',
        );
        const supported = shieldedIngressSupportedAsset(config, assetId);
        if (!supported) {
            return {
                ok: false,
                schema: SHIELDED_NAVSWAP_PREFLIGHT_SCHEMA,
                route: 'shielded_navswap',
                code: 'shielded_ingress_asset_not_supported',
                message: 'Shielded ingress currently supports configured a651/a652 assets only.',
                supported_assets: config.supported_assets,
            };
        }
        if (BigInt(String(amountAtoms)) > BigInt(config.max_amount_atoms)) {
            return {
                ok: false,
                schema: SHIELDED_NAVSWAP_PREFLIGHT_SCHEMA,
                route: 'shielded_navswap',
                code: 'shielded_ingress_amount_cap_exceeded',
                message: `Shielded ingress amount ${amountAtoms} exceeds cap ${config.max_amount_atoms}.`,
                max_amount_atoms: config.max_amount_atoms,
            };
        }
        let assetInfo = null;
        try {
            assetInfo = await navswapRpcRead('asset_info', { asset_id: assetId });
        } catch (error) {
            return {
                ok: false,
                schema: SHIELDED_NAVSWAP_PREFLIGHT_SCHEMA,
                route: 'shielded_navswap',
                code: error.code || 'shielded_ingress_asset_info_failed',
                message: error.message || 'Could not read asset issuer for shielded ingress.',
            };
        }
        const asset = assetInfo.asset || assetInfo;
        const issuer = asset.issuer || asset.issuer_address || null;
        if (!issuer) {
            return {
                ok: false,
                schema: SHIELDED_NAVSWAP_PREFLIGHT_SCHEMA,
                route: 'shielded_navswap',
                code: 'shielded_ingress_asset_issuer_missing',
                message: 'Asset info did not include an issuer for the ingress burn.',
                asset_info: assetInfo,
            };
        }
        const operation = {
            operation: 'asset_burn',
            owner: walletAddress,
            issuer,
            asset_id: assetId,
            amount: amountAtoms,
        };
        return {
            ok: config.configured,
            schema: SHIELDED_NAVSWAP_PREFLIGHT_SCHEMA,
            route: 'shielded_navswap',
            status: config.configured ? 'ready' : 'configuration_required',
            message: config.configured
                ? 'Review and sign this public asset burn locally, then submit the signed burn with the wallet-local ingress note payload.'
                : 'Ingress relay is not configured; wallet may prepare locally but cannot certify yet.',
            wallet_address: walletAddress,
            asset: supported,
            asset_info: assetInfo,
            amount_atoms: String(amountAtoms),
            operation,
            public_disclosure: ['wallet_address', 'asset_id', 'amount_atoms', 'burn_transaction', 'output_commitment'],
            ingress_endpoint: config.endpoint,
            relay_missing: config.missing,
        };
    }

    function validateShieldedIngressPayload(body, config) {
        const payload = body.ingress_payload || body.payload;
        if (!payload || typeof payload !== 'object' || Array.isArray(payload)) {
            const err = new Error('ingress_payload is required');
            err.code = 'shielded_ingress_payload_missing';
            throw err;
        }
        const walletAddress = parseNavswapWalletAddress(body.wallet_address || body.owner || body.source);
        const assetId = parseNavswapHexId(payload.asset_id, 'ingress_payload.asset_id');
        const amount = parseNavswapActionInteger(payload.amount, 'ingress_payload.amount');
        const supported = shieldedIngressSupportedAsset(config, assetId);
        if (!supported) {
            const err = new Error('Shielded ingress supports configured a651/a652 assets only');
            err.code = 'shielded_ingress_asset_not_supported';
            throw err;
        }
        if (BigInt(String(amount)) > BigInt(config.max_amount_atoms)) {
            const err = new Error(`Shielded ingress amount ${amount} exceeds cap ${config.max_amount_atoms}`);
            err.code = 'shielded_ingress_amount_cap_exceeded';
            throw err;
        }
        if (payload.pool_id !== ASSET_ORCHARD_POOL_ID) {
            const err = new Error(`ingress_payload.pool_id must be ${ASSET_ORCHARD_POOL_ID}`);
            err.code = 'shielded_ingress_pool_mismatch';
            throw err;
        }
        if (!/^[0-9a-f]{64}$/.test(String(payload.output_commitment || ''))) {
            const err = new Error('ingress_payload.output_commitment must be 64 lowercase hex characters');
            err.code = 'shielded_ingress_output_commitment_invalid';
            throw err;
        }
        const encryptedOutput = String(payload.encrypted_output || '');
        if (
            !/^[0-9a-f]+$/.test(encryptedOutput)
            || encryptedOutput.length % 2 !== 0
            || !encryptedOutput.startsWith('5046414f454e4331')
        ) {
            const err = new Error('ingress_payload.encrypted_output must be lowercase PFAOENC1 ciphertext hex');
            err.code = 'shielded_ingress_encrypted_output_invalid';
            throw err;
        }
        const burn = payload.burn_transaction;
        const unsigned = burn?.unsigned || {};
        const burnOperation = unsigned.asset_burn
            || unsigned.operation?.asset_burn
            || (unsigned.operation?.operation === 'asset_burn' ? unsigned.operation : null)
            || (unsigned.operation === 'asset_burn' ? unsigned : null)
            || burn?.asset_burn
            || (burn?.operation === 'asset_burn' ? burn : null);
        if (!burn || typeof burn !== 'object' || !burnOperation) {
            const err = new Error('ingress_payload.burn_transaction must be a signed asset_burn transaction');
            err.code = 'shielded_ingress_burn_missing';
            throw err;
        }
        if (unsigned.source !== walletAddress || burnOperation.owner !== walletAddress) {
            const err = new Error('ingress burn owner/source must match wallet_address');
            err.code = 'shielded_ingress_burn_owner_mismatch';
            throw err;
        }
        if (unsigned.transaction_kind !== 'asset_burn') {
            const err = new Error('ingress burn transaction_kind must be asset_burn');
            err.code = 'shielded_ingress_burn_kind_mismatch';
            throw err;
        }
        if (burnOperation.asset_id !== assetId || Number(burnOperation.amount) !== amount) {
            const err = new Error('ingress burn asset/amount must match ingress payload');
            err.code = 'shielded_ingress_burn_payload_mismatch';
            throw err;
        }
        return { walletAddress, payload, asset: supported, amount };
    }

    const ASSET_ORCHARD_ACTION_CLEAR_KEYS = new Set([
        'amount',
        'amount_atoms',
        'asset_id',
        'asset_tag',
        'asset_tag_hi',
        'asset_tag_lo',
        'diversifier',
        'full_viewing_key',
        'full_viewing_key_hex',
        'g_d',
        'input_note',
        'input_notes',
        'note',
        'note_opening',
        'note_openings',
        'nk',
        'output_note',
        'output_notes',
        'pk_d',
        'psi',
        'rcm',
        'rho',
        'rivk',
        'rseed',
        'spend_auth_signing_key',
        'spend_key',
        'spending_key',
    ]);

    function findAssetOrchardActionCleartext(value, pathLabel = '$', seen = new WeakSet()) {
        if (!value || typeof value !== 'object') return [];
        if (seen.has(value)) return [];
        seen.add(value);
        if (Array.isArray(value)) {
            return value.flatMap((item, index) => findAssetOrchardActionCleartext(item, `${pathLabel}[${index}]`, seen));
        }
        const hits = [];
        for (const [key, child] of Object.entries(value)) {
            const normalized = normalizeShieldedKey(key);
            const childPath = `${pathLabel}.${key}`;
            if (ASSET_ORCHARD_ACTION_CLEAR_KEYS.has(normalized)) hits.push(childPath);
            hits.push(...findAssetOrchardActionCleartext(child, childPath, seen));
        }
        return hits;
    }

    function shieldedQuoteFromSubmitBody(body = {}) {
        const quote = body.quote || body.shielded_quote || {};
        if (!quote || typeof quote !== 'object' || Array.isArray(quote)) {
            const err = new Error('quote is required');
            err.code = 'shielded_swap_quote_missing';
            throw err;
        }
        return quote;
    }

    function parseShieldedSwapActionJson(body = {}) {
        const raw = body.swap_action_json || body.action_json || body.swapActionJson;
        if (typeof raw !== 'string' || raw.length < 2) {
            const err = new Error('swap_action_json is required');
            err.code = 'shielded_swap_action_json_missing';
            throw err;
        }
        if (raw.length > 8 * 1024 * 1024) {
            const err = new Error('swap_action_json is too large');
            err.code = 'shielded_swap_action_json_too_large';
            throw err;
        }
        let action;
        try {
            action = JSON.parse(raw);
        } catch (_) {
            const err = new Error('swap_action_json must be valid JSON');
            err.code = 'shielded_swap_action_json_invalid';
            throw err;
        }
        return { raw, action };
    }

    function validateShieldedSwapAction(action) {
        if (!action || typeof action !== 'object' || Array.isArray(action)) {
            const err = new Error('swap action must be a JSON object');
            err.code = 'shielded_swap_action_invalid';
            throw err;
        }
        if (action.schema !== ASSET_ORCHARD_SWAP_ACTION_SCHEMA) {
            const err = new Error('swap action schema mismatch');
            err.code = 'shielded_swap_action_schema_mismatch';
            throw err;
        }
        if (action.pool_id !== ASSET_ORCHARD_POOL_ID) {
            const err = new Error(`swap action pool_id must be ${ASSET_ORCHARD_POOL_ID}`);
            err.code = 'shielded_swap_action_pool_mismatch';
            throw err;
        }
        const cleartext = findAssetOrchardActionCleartext(action);
        if (cleartext.length > 0) {
            const err = new Error(`swap action contains forbidden cleartext at ${cleartext[0]}`);
            err.code = 'shielded_swap_action_cleartext_rejected';
            throw err;
        }
        const nullifiers = Array.isArray(action.nullifiers) ? action.nullifiers : [];
        const outputCommitments = Array.isArray(action.output_commitments) ? action.output_commitments : [];
        const accountingInputs = Array.isArray(action.accounting_inputs) ? action.accounting_inputs : [];
        const accountingOutputs = Array.isArray(action.accounting_outputs) ? action.accounting_outputs : [];
        if (
            nullifiers.length !== 2
            || outputCommitments.length !== 2
            || accountingInputs.length !== 2
            || accountingOutputs.length !== 2
        ) {
            const err = new Error('swap action must contain two nullifiers, outputs, accounting inputs, and accounting outputs');
            err.code = 'shielded_swap_action_shape_mismatch';
            throw err;
        }
        return {
            schema: action.schema,
            pool_id: action.pool_id,
            swap_binding_hash: action.swap_binding_hash || null,
            nullifier_count: nullifiers.length,
            output_count: outputCommitments.length,
            accounting_input_count: accountingInputs.length,
            accounting_output_count: accountingOutputs.length,
        };
    }

    function validateShieldedSwapSubmit(body, config) {
        assertNoShieldedPrivateMaterial(body);
        const quote = shieldedQuoteFromSubmitBody(body);
        if (quote.ok !== true || quote.schema !== SHIELDED_NAVSWAP_QUOTE_SCHEMA) {
            const err = new Error('quote must be a successful shielded NAVSwap quote');
            err.code = 'shielded_swap_quote_invalid';
            throw err;
        }
        const walletAddress = parseNavswapWalletAddress(body.wallet_address || quote.wallet_address);
        const quoteBindingHash = parseNavswapHexId(
            body.quote_binding_hash || quote.quote_binding_hash,
            'quote_binding_hash',
            64,
        );
        if (quoteBindingHash !== String(quote.quote_binding_hash || '').trim().toLowerCase()) {
            const err = new Error('quote_binding_hash does not match quote');
            err.code = 'shielded_swap_quote_binding_mismatch';
            throw err;
        }
        const expiresAtMs = parseNavswapActionInteger(
            quote.quote_expires_at_ms || body.quote_expires_at_ms,
            'quote_expires_at_ms',
        );
        if (expiresAtMs <= Date.now()) {
            const err = new Error('shielded NAVSwap quote expired before submit');
            err.code = 'shielded_swap_quote_expired';
            throw err;
        }
        const liquidity = quote.liquidity || {};
        const liquidityCommitment = String(liquidity.commitment || quote.liquidity_commitment || '').trim().toLowerCase();
        if (!/^[0-9a-f]{64}$/.test(liquidityCommitment)) {
            const err = new Error('quote liquidity commitment must be a 32-byte pool note commitment');
            err.code = 'shielded_swap_liquidity_commitment_invalid';
            throw err;
        }
        if (quote.liquidity?.mode !== 'pool_managed_note' && quote.liquidity_mode !== 'pool_managed_note') {
            const err = new Error('Step 7 private swap submit requires pool_managed_note liquidity');
            err.code = 'shielded_swap_liquidity_mode_unsupported';
            throw err;
        }
        if (!config.configured) {
            const err = new Error('Shielded private swap relay is not configured.');
            err.code = 'shielded_swap_configuration_required';
            err.missing = config.missing;
            throw err;
        }
        const { raw, action } = parseShieldedSwapActionJson(body);
        const actionVerification = validateShieldedSwapAction(action);
        return {
            walletAddress,
            quote,
            quoteBindingHash,
            expiresAtMs,
            liquidityCommitment,
            rawAction: raw,
            action,
            actionVerification,
        };
    }

    function shieldedPrivateEgressDisclosureFields({
        walletAddress,
        to,
        assetId,
        amountAtoms,
        noteCommitment = '',
        policyId,
    } = {}) {
        return {
            schema: 'postfiat-shielded-navswap-private-egress-disclosure-v1',
            route: 'shielded_navswap',
            action: 'private_egress_public_exit',
            wallet_address: walletAddress,
            destination: to,
            asset_id: assetId,
            amount_atoms: String(amountAtoms),
            note_commitment: String(noteCommitment || ''),
            policy_id: policyId,
            visible_after_submit: ['destination', 'asset_id', 'amount_atoms', 'receipt_timing'],
            stays_private: ['note_opening', 'spend_authority', 'wallet_local_note_file'],
        };
    }

    function shieldedPrivateEgressDisclosureHash(fields) {
        return crypto.createHash('sha256')
            .update(navswapStableJson(fields))
            .digest('hex');
    }

    function parseShieldedPrivateEgressJson(body = {}) {
        const raw = body.egress_json || body.private_egress_json || body.egressFileJson;
        if (typeof raw !== 'string' || raw.length < 2) {
            const err = new Error('egress_json is required');
            err.code = 'shielded_egress_json_missing';
            throw err;
        }
        if (raw.length > 8 * 1024 * 1024) {
            const err = new Error('egress_json is too large');
            err.code = 'shielded_egress_json_too_large';
            throw err;
        }
        let file;
        try {
            file = JSON.parse(raw);
        } catch (_) {
            const err = new Error('egress_json must be valid JSON');
            err.code = 'shielded_egress_json_invalid';
            throw err;
        }
        return { raw, file };
    }

    function validateShieldedPrivateEgressFile(file, expected) {
        if (!file || typeof file !== 'object' || Array.isArray(file)) {
            const err = new Error('private egress file must be a JSON object');
            err.code = 'shielded_egress_file_invalid';
            throw err;
        }
        if (file.schema !== ASSET_ORCHARD_PRIVATE_EGRESS_FILE_SCHEMA) {
            const err = new Error('private egress file schema mismatch');
            err.code = 'shielded_egress_file_schema_mismatch';
            throw err;
        }
        const payload = file.payload;
        if (!payload || typeof payload !== 'object' || Array.isArray(payload)) {
            const err = new Error('private egress payload is required');
            err.code = 'shielded_egress_payload_missing';
            throw err;
        }
        if (payload.schema !== ASSET_ORCHARD_PRIVATE_EGRESS_ACTION_SCHEMA) {
            const err = new Error('private egress action schema mismatch');
            err.code = 'shielded_egress_action_schema_mismatch';
            throw err;
        }
        if (payload.pool_id !== ASSET_ORCHARD_POOL_ID) {
            const err = new Error(`private egress pool_id must be ${ASSET_ORCHARD_POOL_ID}`);
            err.code = 'shielded_egress_pool_mismatch';
            throw err;
        }
        const checks = [
            ['to', expected.to],
            ['asset_id', expected.assetId],
            ['policy_id', expected.policyId],
            ['disclosure_hash', expected.disclosureHash],
        ];
        for (const [field, value] of checks) {
            if (String(payload[field] || '').trim().toLowerCase() !== String(value || '').trim().toLowerCase()) {
                const err = new Error(`private egress ${field} does not match request`);
                err.code = `shielded_egress_${field}_mismatch`;
                throw err;
            }
        }
        if (String(payload.amount) !== String(expected.amountAtoms)) {
            const err = new Error('private egress amount does not match request');
            err.code = 'shielded_egress_amount_mismatch';
            throw err;
        }
        if (Number(payload.fee || 0) !== 0) {
            const err = new Error('private egress fee must be zero');
            err.code = 'shielded_egress_fee_mismatch';
            throw err;
        }
        for (const field of ['anchor', 'nullifier']) {
            if (!/^[0-9a-f]{64}$/.test(String(payload[field] || ''))) {
                const err = new Error(`private egress ${field} must be 32-byte lowercase hex`);
                err.code = `shielded_egress_${field}_invalid`;
                throw err;
            }
        }
        if (!/^[0-9a-f]{128}$/.test(String(payload.exit_binding_hash || ''))) {
            const err = new Error('private egress exit_binding_hash must be 64-byte lowercase hex');
            err.code = 'shielded_egress_exit_binding_hash_invalid';
            throw err;
        }
        if (!/^[0-9a-f]+$/.test(String(payload.proof || '')) || String(payload.proof || '').length % 2 !== 0) {
            const err = new Error('private egress proof must be even-length lowercase hex');
            err.code = 'shielded_egress_proof_invalid';
            throw err;
        }
        return {
            schema: file.schema,
            action_schema: payload.schema,
            pool_id: payload.pool_id,
            to: payload.to,
            asset_id: payload.asset_id,
            amount_atoms: String(payload.amount),
            fee: String(payload.fee || 0),
            policy_id: payload.policy_id,
            disclosure_hash: payload.disclosure_hash,
            anchor: payload.anchor,
            nullifier: payload.nullifier,
            exit_binding_hash: payload.exit_binding_hash,
            proof_bytes: Math.floor(String(payload.proof || '').length / 2),
        };
    }

    function validateShieldedEgressSubmit(body, config) {
        assertNoShieldedPrivateMaterial(body);
        if (body.disclosure_ack !== true) {
            const err = new Error('Private egress requires explicit disclosure_ack=true.');
            err.code = 'shielded_egress_disclosure_ack_required';
            throw err;
        }
        if (!config.configured) {
            const err = new Error('Shielded private egress relay is not configured.');
            err.code = 'shielded_egress_configuration_required';
            err.missing = config.missing;
            throw err;
        }
        const walletAddress = parseNavswapWalletAddress(body.wallet_address || body.owner || body.source);
        const to = parseNavswapWalletAddress(body.to || body.destination || body.recipient);
        const assetId = parseNavswapHexId(body.asset_id || body.assetId, 'asset_id');
        const amountAtoms = parseNavswapActionInteger(
            body.amount_atoms || body.amountAtoms || body.amount,
            'amount_atoms',
        );
        const policyId = String(body.policy_id || body.policyId || config.policy_id || '').trim();
        if (policyId !== config.policy_id) {
            const err = new Error('private egress policy_id does not match configured route policy');
            err.code = 'shielded_egress_policy_mismatch';
            throw err;
        }
        const noteCommitment = String(body.note_commitment || body.noteCommitment || '').trim().toLowerCase();
        if (noteCommitment && !/^[0-9a-f]{64}$/.test(noteCommitment)) {
            const err = new Error('note_commitment must be 32-byte lowercase hex');
            err.code = 'shielded_egress_note_commitment_invalid';
            throw err;
        }
        const disclosure = shieldedPrivateEgressDisclosureFields({
            walletAddress,
            to,
            assetId,
            amountAtoms,
            noteCommitment,
            policyId,
        });
        const expectedDisclosureHash = shieldedPrivateEgressDisclosureHash(disclosure);
        const disclosureHash = parseNavswapHexId(
            body.disclosure_hash || body.disclosureHash,
            'disclosure_hash',
            64,
        );
        if (disclosureHash !== expectedDisclosureHash) {
            const err = new Error('private egress disclosure_hash does not match the public-exit disclosure fields');
            err.code = 'shielded_egress_disclosure_hash_mismatch';
            err.expected_disclosure_hash = expectedDisclosureHash;
            throw err;
        }
        const { raw, file } = parseShieldedPrivateEgressJson(body);
        const actionVerification = validateShieldedPrivateEgressFile(file, {
            to,
            assetId,
            amountAtoms,
            policyId,
            disclosureHash,
        });
        return {
            walletAddress,
            to,
            assetId,
            amountAtoms,
            policyId,
            noteCommitment,
            disclosure,
            disclosureHash,
            rawEgress: raw,
            egressFile: file,
            actionVerification,
        };
    }

    function certifiedRoundReceipts(report) {
        const receipts = [];
        const hotFinality = Array.isArray(report?.local_hot_finality) ? report.local_hot_finality : [];
        for (const item of hotFinality) {
            if (item?.receipt) receipts.push(item.receipt);
        }
        return receipts;
    }

    function certifiedRoundHasQuorumCertificate(round) {
        const certification = round?.certification;
        if (!certification || typeof certification !== 'object') return false;
        if (certification.round_ok !== true) return false;
        const validators = Array.isArray(certification.validators) ? certification.validators.length : 0;
        const voteCount = Number.parseInt(certification.vote_count || '0', 10);
        const quorum = validators > 0 ? Math.floor((validators * 2) / 3) + 1 : 1;
        return voteCount >= quorum;
    }

    function certifiedRoundFailure(report, label, options = {}) {
        const round = report && typeof report.transport === 'object' ? report.transport : report;
        if (!round || typeof round !== 'object') return `${label} did not return a certified round report`;
        const receipts = certifiedRoundReceipts(round);
        const rejected = receipts.find((receipt) => receipt.accepted === false);
        if (rejected) return `${label} rejected with ${rejected.code || 'rejected'}: ${rejected.message || 'transaction receipt was rejected'}`;
        const localRejected = Number.parseInt(round.local_rejected_count || '0', 10);
        if (localRejected > 0) return `${label} produced ${localRejected} rejected local receipt(s)`;
        const localAccepted = Number.parseInt(round.local_accepted_count || '0', 10);
        if (localAccepted <= 0 && receipts.length === 0) return `${label} produced no accepted local receipt`;
        if (receipts.length && !receipts.every((receipt) => receipt.accepted === true)) {
            return `${label} produced a receipt without accepted=true`;
        }
        if (round.round_ok !== true) {
            const allowQuorumCertificate = options.allow_quorum_certificate === true;
            const localApplyVerified = round.local_apply_verified === true;
            if (!(allowQuorumCertificate && localApplyVerified && certifiedRoundHasQuorumCertificate(round))) {
                return `${label} did not complete cleanly: round_ok=${round.round_ok}`;
            }
        }
        return null;
    }

    function buildShieldedCertifiedRoundArgs(config, batchFile, artifactDir) {
        const args = [
            'transport-peer-certified-batch-round',
            '--data-dir', config.data_dir,
            '--topology', config.topology,
            '--batch-kind', 'shielded',
            '--batch-file', batchFile,
            '--key-file', config.key_file,
            '--artifact-dir', artifactDir,
            '--timeout-ms', String(config.timeout_ms),
            '--send-retries', '3',
            '--retry-backoff-ms', '1000',
            '--allow-existing-mempool',
        ];
        // Latency: early-quorum path (certify at 5/6, defer the laggard's certified send to a
        // background thread) recovers the ~14s round the StakeHub optimized runner achieves. The
        // dropped slow peer is reconciled afterward via rpc-catch-up (see runShieldedLaggardCatchUp),
        // so 6/6 convergence is preserved without full state-sync repair. Env-gated + reversible.
        if (process.env.SHIELDED_EARLY_QUORUM === 'true') {
            args.push('--quorum-early-full-propagation', '--local-apply-before-certified-send', '--defer-certified-sends');
        }
        if (config.proposal_key_file) {
            args.splice(11, 0, '--proposal-key-file', config.proposal_key_file);
        }
        return args;
    }

    function shieldedCertifiedRoundEnv(baseEnv = process.env) {
        const env = { ...baseEnv };
        const defaultIfUnset = (name, value) => {
            if (!Object.prototype.hasOwnProperty.call(env, name)) {
                env[name] = value;
            }
        };
        const roundPrewarmEnabled = ['1', 'true', 'yes'].includes(lower(
            env.NAVSWAP_SHIELDED_ROUND_PREWARM || env.POSTFIAT_SHIELDED_ROUND_PREWARM || ''
        ));
        if (roundPrewarmEnabled) {
            defaultIfUnset('POSTFIAT_PREWARM_SHIELDED_VERIFIER', '1');
            defaultIfUnset('POSTFIAT_PREWARM_ASSET_ORCHARD_SWAP_VERIFIER', '1');
            defaultIfUnset('POSTFIAT_PREWARM_ASSET_ORCHARD_PRIVATE_EGRESS_VERIFIER', '1');
        } else {
            env.POSTFIAT_PREWARM_SHIELDED_VERIFIER = '0';
            env.POSTFIAT_PREWARM_ASSET_ORCHARD_SWAP_VERIFIER = '0';
            env.POSTFIAT_PREWARM_ASSET_ORCHARD_PRIVATE_EGRESS_VERIFIER = '0';
        }
        return env;
    }

    let shieldedCertifierLoopState = null;

    function shieldedCertifierLoopStartHeight(config) {
        const configured = Number.parseInt(config.certifier_loop?.start_height || '', 10);
        if (Number.isInteger(configured) && configured > 0) return configured;
        const stdout = execFileSync(config.node_bin, ['status', '--data-dir', config.data_dir], {
            encoding: 'utf8',
            timeout: 10000,
            maxBuffer: 2 * 1024 * 1024,
        });
        const status = JSON.parse(stdout || '{}');
        const height = Number.parseInt(status.block_height || status.height || '0', 10);
        if (!Number.isInteger(height) || height < 0) {
            throw new Error('could not determine current chain height for shielded certifier loop');
        }
        return height + 1;
    }

    function startShieldedCertifierLoop(config) {
        const loop = config.certifier_loop;
        if (!loop?.enabled) return null;
        if (shieldedCertifierLoopState?.finished) {
            shieldedCertifierLoopState = null;
        }
        if (shieldedCertifierLoopState) return shieldedCertifierLoopState;

        for (const dir of [loop.batch_dir, loop.artifact_root, loop.processed_dir, path.dirname(loop.ready_file), path.dirname(loop.report_file)]) {
            fs.mkdirSync(dir, { recursive: true });
        }
        for (const file of [loop.ready_file, loop.report_file]) {
            try { fs.unlinkSync(file); } catch (_) {}
        }
        const startHeight = shieldedCertifierLoopStartHeight(config);
        const args = [
            'transport-peer-certified-batch-loop',
            '--data-dir', config.data_dir,
            '--topology', config.topology,
            '--batch-kind', 'shielded',
            '--batch-dir', loop.batch_dir,
            '--key-file', config.key_file,
            '--artifact-root', loop.artifact_root,
            '--processed-dir', loop.processed_dir,
            '--max-rounds', '1',
            '--start-height', String(startHeight),
            '--poll-ms', String(Number.isInteger(loop.poll_ms) && loop.poll_ms > 0 ? loop.poll_ms : 250),
            '--timeout-ms', String(config.timeout_ms),
            '--send-retries', '3',
            '--retry-backoff-ms', '1000',
        ];
        if (config.proposal_key_file) {
            args.splice(9, 0, '--proposal-key-file', config.proposal_key_file);
        }
        if (shieldedEarlyQuorumEnabled()) {
            args.push('--quorum-early-full-propagation', '--local-apply-before-certified-send', '--defer-certified-sends');
        }

        const certifierLoopPrewarm = ['1', 'true', 'yes', 'enabled'].includes(
            String(process.env.NAVSWAP_SHIELDED_ROUND_PREWARM || process.env.POSTFIAT_SHIELDED_ROUND_PREWARM || '')
                .trim()
                .toLowerCase(),
        );
        const childEnv = {
            ...process.env,
            POSTFIAT_CERTIFIED_BATCH_LOOP_READY_FILE: loop.ready_file,
            POSTFIAT_PREWARM_SHIELDED_VERIFIER: '0',
            POSTFIAT_PREWARM_ASSET_ORCHARD_SWAP_VERIFIER: certifierLoopPrewarm ? '1' : '0',
            POSTFIAT_PREWARM_ASSET_ORCHARD_PRIVATE_EGRESS_VERIFIER: '0',
        };
        const child = spawn(config.node_bin, args, {
            env: childEnv,
            stdio: ['ignore', 'pipe', 'pipe'],
        });
        const state = {
            child,
            start_height: startHeight,
            ready_file: loop.ready_file,
            report_file: loop.report_file,
            batch_dir: loop.batch_dir,
            artifact_root: loop.artifact_root,
            batch_submitted: false,
            finished: false,
        };
        let stdout = '';
        let stderr = '';
        child.stdout.on('data', (chunk) => { stdout += chunk.toString('utf8'); });
        child.stderr.on('data', (chunk) => { stderr += chunk.toString('utf8'); });
        const done = new Promise((resolve, reject) => {
            child.once('error', reject);
            child.once('exit', (code, signal) => {
                state.finished = true;
                if (shieldedCertifierLoopState === state) {
                    shieldedCertifierLoopState = null;
                }
                const report = {
                    schema: 'postfiat-wallet-proxy-shielded-certifier-loop-exit-v1',
                    code,
                    signal,
                    ok: code === 0,
                    start_height: startHeight,
                    batch_dir: loop.batch_dir,
                    artifact_root: loop.artifact_root,
                    stdout,
                    stderr,
                };
                try {
                    fs.writeFileSync(loop.report_file, `${JSON.stringify(report, null, 2)}\n`, { mode: 0o600 });
                } catch (_) {}
                if (code === 0) {
                    try {
                        resolve({ wrapper: report, loop_report: JSON.parse(stdout || '{}') });
                    } catch (error) {
                        reject(new Error(`shielded certifier loop report parse failed: ${error.message}`));
                    }
                } else {
                    const message = stderr.trim() || stdout.trim() || `shielded certifier loop exited with ${code ?? signal}`;
                    reject(new Error(message));
                }
            });
        });
        state.done = done;
        done.catch(() => {});
        shieldedCertifierLoopState = state;
        return shieldedCertifierLoopState;
    }

    async function certifyShieldedBatchViaWarmLoop(config, batchFile, stamp) {
        const loop = startShieldedCertifierLoop(config);
        if (!loop) return null;
        if (loop.finished) {
            throw new Error('shielded certifier loop is exhausted; submit requires a fresh loop');
        }
        if (loop.batch_submitted) {
            throw new Error('shielded certifier loop already has an in-flight batch');
        }
        const submittedBatch = JSON.parse(fs.readFileSync(batchFile, 'utf8') || '{}');
        const target = shieldedCertifierLoopBatchFile(loop, stamp);
        const tmp = `${target}.tmp-${process.pid}-${Date.now()}`;
        fs.copyFileSync(batchFile, tmp);
        fs.chmodSync(tmp, 0o600);
        loop.batch_submitted = true;
        fs.renameSync(tmp, target);
        const result = await loop.done;
        const round = Array.isArray(result.loop_report?.rounds) ? result.loop_report.rounds[0] : null;
        if (!round) {
            throw new Error('shielded certifier loop completed without a round report');
        }
        validateShieldedCertifierLoopReportForBatch(submittedBatch, round, result.loop_report);
        return {
            round,
            loop_report: result.loop_report,
            wrapper: result.wrapper,
            artifact_dir: path.join(loop.artifact_root, `round-${loop.start_height}`),
            batch_file: target,
        };
    }

    function shieldedCertifierLoopBatchFile(loop, stamp) {
        return path.join(loop.batch_dir, `${stamp}.batch.json`);
    }

    function shieldedBatchExplicitActionIds(batch) {
        const ids = [];
        for (const action of Array.isArray(batch?.actions) ? batch.actions : []) {
            for (const candidate of [
                action?.action_id,
                action?.id,
                action?.tx_id,
                action?.payload?.action_id,
                action?.payload?.id,
                action?.payload?.tx_id,
            ]) {
                if (typeof candidate === 'string' && candidate.trim()) ids.push(candidate.trim());
            }
        }
        return ids;
    }

    function shieldedRoundReceiptIds(round) {
        const ids = [];
        const push = (value) => {
            if (typeof value === 'string' && value.trim()) ids.push(value.trim());
        };
        for (const receiptId of Array.isArray(round?.receipt_ids) ? round.receipt_ids : []) push(receiptId);
        for (const receipt of Array.isArray(round?.receipts) ? round.receipts : []) {
            push(receipt?.tx_id);
            push(receipt?.receipt_id);
            push(receipt?.id);
        }
        for (const finality of Array.isArray(round?.local_hot_finality) ? round.local_hot_finality : []) {
            push(finality?.tx_id);
            push(finality?.receipt?.tx_id);
            push(finality?.receipt?.receipt_id);
            for (const receiptId of Array.isArray(finality?.block?.receipt_ids) ? finality.block.receipt_ids : []) {
                push(receiptId);
            }
        }
        return ids;
    }

    function shieldedRoundBatchIds(round, loopReport) {
        const ids = [];
        const push = (value) => {
            if (typeof value === 'string' && value.trim()) ids.push(value.trim());
        };
        push(round?.batch_id);
        push(round?.batch?.batch_id);
        push(round?.certification?.batch_id);
        push(round?.proposal?.batch_id);
        push(round?.block?.header?.batch_id);
        push(round?.block?.batch_id);
        for (const finality of Array.isArray(round?.local_hot_finality) ? round.local_hot_finality : []) {
            push(finality?.block?.header?.batch_id);
            push(finality?.block?.batch_id);
            push(finality?.receipt?.batch_id);
        }
        push(loopReport?.batch_id);
        return ids;
    }

    function validateShieldedCertifierLoopReportForBatch(batch, round, loopReport = null) {
        const expectedBatchId = typeof batch?.batch_id === 'string' ? batch.batch_id.trim() : '';
        if (expectedBatchId) {
            const reportBatchIds = shieldedRoundBatchIds(round, loopReport);
            if (reportBatchIds.length === 0) {
                throw new Error('shielded certifier loop report did not carry a certified batch id');
            }
            if (!reportBatchIds.includes(expectedBatchId)) {
                throw new Error('shielded certifier loop report batch id does not match submitted batch');
            }
        }

        const actionIds = shieldedBatchExplicitActionIds(batch);
        const receiptIds = shieldedRoundReceiptIds(round);
        if (actionIds.length > 0) {
            const missing = actionIds.filter((id) => !receiptIds.includes(id));
            if (missing.length > 0) {
                throw new Error(`shielded certifier loop report missing receipt for submitted batch action ${missing[0]}`);
            }
        } else if (expectedBatchId && receiptIds.length === 0) {
            throw new Error('shielded certifier loop report did not carry receipt ids for submitted batch');
        }
    }

    function shieldedEarlyQuorumEnabled() {
        return process.env.SHIELDED_EARLY_QUORUM === 'true';
    }

    function shieldedLaggardCatchUpConfig() {
        return {
            enabled: shieldedEarlyQuorumEnabled(),
            ssh_bin: presentEnv('SHIELDED_LAGGARD_CATCHUP_SSH_BIN') || 'ssh',
            ssh_key: presentEnv('SHIELDED_LAGGARD_CATCHUP_SSH_KEY') || path.join(os.homedir(), '.ssh', 'id_ed25519'),
            ssh_user: presentEnv('SHIELDED_LAGGARD_CATCHUP_SSH_USER') || 'root',
            remote_user: presentEnv('SHIELDED_LAGGARD_CATCHUP_REMOTE_USER') || 'postfiat',
            remote_node_bin: presentEnv('SHIELDED_LAGGARD_CATCHUP_REMOTE_NODE_BIN') || '/usr/local/bin/postfiat-node',
            data_dir_template: presentEnv('SHIELDED_LAGGARD_CATCHUP_DATA_DIR_TEMPLATE') || '/var/lib/postfiat/{validator}',
            work_dir_template: presentEnv('SHIELDED_LAGGARD_CATCHUP_WORK_DIR_TEMPLATE') || '{data_dir}/rpc-catch-up-work',
            preferred_sources: (presentEnv('SHIELDED_LAGGARD_CATCHUP_SOURCES') || 'validator-2,validator-5')
                .split(',')
                .map((value) => value.trim())
                .filter(Boolean),
            max_blocks: Number.parseInt(process.env.SHIELDED_LAGGARD_CATCHUP_MAX_BLOCKS || '64', 10),
            timeout_ms: Number.parseInt(process.env.SHIELDED_LAGGARD_CATCHUP_TIMEOUT_MS || '60000', 10),
            status_timeout_ms: Number.parseInt(process.env.SHIELDED_LAGGARD_CATCHUP_STATUS_TIMEOUT_MS || '8000', 10),
            source_wait_attempts: Number.parseInt(process.env.SHIELDED_LAGGARD_CATCHUP_SOURCE_WAIT_ATTEMPTS || '15', 10),
            source_wait_delay_ms: Number.parseInt(process.env.SHIELDED_LAGGARD_CATCHUP_SOURCE_WAIT_DELAY_MS || '1000', 10),
            recheck_attempts: Number.parseInt(process.env.SHIELDED_LAGGARD_CATCHUP_RECHECK_ATTEMPTS || '6', 10),
            recheck_delay_ms: Number.parseInt(process.env.SHIELDED_LAGGARD_CATCHUP_RECHECK_DELAY_MS || '1000', 10),
        };
    }

    function loadShieldedTopologyPeers(topologyFile) {
        const parsed = JSON.parse(fs.readFileSync(topologyFile, 'utf8'));
        const peers = Array.isArray(parsed.peers) ? parsed.peers : [];
        return peers.map((peer) => ({
            validatorId: String(peer.node_id || peer.validator_id || '').trim(),
            host: String(peer.host || '').trim(),
            port: Number.parseInt(peer.rpc_port || peer.port || '0', 10),
        })).filter((peer) => peer.validatorId && peer.host && Number.isInteger(peer.port) && peer.port > 0);
    }

    async function collectShieldedTopologyStatuses(peers, timeoutMs) {
        return collectFleetStatuses(peers.map((peer) => ({
            validatorId: peer.validatorId,
            host: peer.host,
            port: peer.port,
        })), { timeoutMs });
    }

    function certifiedRoundHeight(report) {
        const candidates = [
            report?.certification?.block_height,
            report?.local_state?.block_height,
            report?.transport?.certification?.block_height,
            report?.transport?.local_state?.block_height,
        ];
        for (const value of candidates) {
            const parsed = Number.parseInt(value, 10);
            if (Number.isInteger(parsed) && parsed >= 0) return parsed;
        }
        return null;
    }

    function majorityRootAtHeight(statusRows, height) {
        const counts = new Map();
        for (const row of statusRows) {
            const status = row?.status || {};
            if (Number.parseInt(status.block_height, 10) !== height) continue;
            const root = typeof status.state_root === 'string' ? status.state_root : '';
            if (!root) continue;
            counts.set(root, (counts.get(root) || 0) + 1);
        }
        let best = null;
        for (const [root, count] of counts.entries()) {
            if (!best || count > best.count) best = { root, count };
        }
        return best?.root || null;
    }

    function shieldedConvergenceSummary(peers, statusRows, minHeight) {
        const okRows = statusRows.filter((row) => row.ok && row.status);
        const heights = new Set(okRows.map((row) => Number.parseInt(row.status.block_height, 10)));
        const roots = new Set(okRows.map((row) => row.status.state_root).filter(Boolean));
        const tips = new Set(okRows.map((row) => row.status.block_tip_hash).filter(Boolean));
        const height = heights.size === 1 ? [...heights][0] : null;
        const root = roots.size === 1 ? [...roots][0] : null;
        const tip = tips.size === 1 ? [...tips][0] : null;
        return {
            validator_count: peers.length,
            ok_count: okRows.length,
            same_height: heights.size === 1,
            same_root: roots.size === 1,
            same_tip: tips.size === 1,
            height,
            root,
            tip,
            converged: okRows.length === peers.length
                && heights.size === 1
                && roots.size === 1
                && tips.size === 1
                && Number.isInteger(height)
                && height >= minHeight,
        };
    }

    function shieldedCatchUpLaggards(statusRows, certifiedHeight, expectedRoot) {
        return statusRows.filter((row) => {
            if (!row.ok || !row.status) return true;
            const height = Number.parseInt(row.status.block_height, 10);
            if (!Number.isInteger(height) || height < certifiedHeight) return true;
            if (height === certifiedHeight && expectedRoot && row.status.state_root !== expectedRoot) return true;
            return false;
        });
    }

    function shieldedCatchUpSourceCandidates(peers, statusRows, certifiedHeight, expectedRoot, preferredSources) {
        const rowById = new Map(statusRows.map((row) => [row.endpoint?.validatorId, row]));
        const peerById = new Map(peers.map((peer) => [peer.validatorId, peer]));
        const ordered = [...preferredSources, ...peers.map((peer) => peer.validatorId)];
        const seen = new Set();
        const candidates = [];
        for (const id of ordered) {
            if (!id || seen.has(id)) continue;
            seen.add(id);
            const peer = peerById.get(id);
            const row = rowById.get(id);
            if (!peer || !row?.ok || !row.status) continue;
            const height = Number.parseInt(row.status.block_height, 10);
            if (!Number.isInteger(height) || height < certifiedHeight) continue;
            if (expectedRoot && height === certifiedHeight && row.status.state_root !== expectedRoot) continue;
            candidates.push({ peer, row });
        }
        return candidates;
    }

    function chooseShieldedCatchUpSource(peers, statusRows, targetId, certifiedHeight, expectedRoot, preferredSources) {
        for (const candidate of shieldedCatchUpSourceCandidates(
            peers,
            statusRows,
            certifiedHeight,
            expectedRoot,
            preferredSources,
        )) {
            if (candidate.peer.validatorId !== targetId) return candidate.peer;
        }
        return null;
    }

    function shieldedRemoteDataDir(template, validatorId) {
        return String(template || '').replace(/\{validator\}/g, validatorId);
    }

    function shieldedRemoteWorkDir(template, validatorId, dataDir) {
        return String(template || '{data_dir}/rpc-catch-up-work')
            .replace(/\{validator\}/g, validatorId)
            .replace(/\{data_dir\}/g, dataDir);
    }

    function shellQuote(value) {
        return `'${String(value).replace(/'/g, `'\\''`)}'`;
    }

    async function runShieldedRpcCatchUp(config, targetPeer, sourcePeer, catchUpConfig) {
        const dataDir = shieldedRemoteDataDir(catchUpConfig.data_dir_template, targetPeer.validatorId);
        const workDir = shieldedRemoteWorkDir(catchUpConfig.work_dir_template, targetPeer.validatorId, dataDir);
        const remoteCommand = [
            [
                'sudo', '-n', '-u', shellQuote(catchUpConfig.remote_user),
                'mkdir', '-p', shellQuote(workDir),
            ].join(' '),
            [
                'sudo', '-n', '-u', shellQuote(catchUpConfig.remote_user),
                shellQuote(catchUpConfig.remote_node_bin),
                'rpc-catch-up',
                '--data-dir', shellQuote(dataDir),
                '--source-host', shellQuote(sourcePeer.host),
                '--source-rpc-port', shellQuote(String(sourcePeer.port)),
                '--work-dir', shellQuote(workDir),
                '--max-blocks', shellQuote(String(catchUpConfig.max_blocks)),
                '--timeout-ms', shellQuote(String(catchUpConfig.timeout_ms)),
            ].join(' '),
        ].join(' && ');
        const args = [
            '-i', catchUpConfig.ssh_key,
            '-o', 'BatchMode=yes',
            '-o', 'StrictHostKeyChecking=accept-new',
            '-o', 'ConnectTimeout=12',
            `${catchUpConfig.ssh_user}@${targetPeer.host}`,
            `sh -lc ${shellQuote(remoteCommand)}`,
        ];
        const startedAtMs = Date.now();
        try {
            const { stdout, stderr } = await execFileAsync(catchUpConfig.ssh_bin, args, {
                timeout: catchUpConfig.timeout_ms + 30000,
                maxBuffer: 16 * 1024 * 1024,
            });
            let report = null;
            try {
                report = JSON.parse(stdout || '{}');
            } catch (_) {
                report = { raw_stdout: stdout };
            }
            return {
                ok: true,
                target: targetPeer.validatorId,
                target_host: targetPeer.host,
                source: sourcePeer.validatorId,
                source_host: sourcePeer.host,
                source_rpc_port: sourcePeer.port,
                data_dir: dataDir,
                work_dir: workDir,
                elapsed_ms: Date.now() - startedAtMs,
                report,
                stderr: stderr || undefined,
            };
        } catch (error) {
            return {
                ok: false,
                target: targetPeer.validatorId,
                target_host: targetPeer.host,
                source: sourcePeer.validatorId,
                source_host: sourcePeer.host,
                source_rpc_port: sourcePeer.port,
                data_dir: dataDir,
                work_dir: workDir,
                elapsed_ms: Date.now() - startedAtMs,
                message: error.message || 'rpc-catch-up failed',
                stdout: error.stdout || undefined,
                stderr: error.stderr || undefined,
            };
        }
    }

    async function runShieldedLaggardCatchUp(config, report, artifactDir) {
        const catchUpConfig = shieldedLaggardCatchUpConfig();
        if (!catchUpConfig.enabled) {
            return { enabled: false, status: 'disabled' };
        }
        const startedAtMs = Date.now();
        const certifiedHeight = certifiedRoundHeight(report);
        if (!Number.isInteger(certifiedHeight)) {
            return {
                enabled: true,
                ok: false,
                status: 'missing_certified_height',
                message: 'Cannot run shielded laggard catch-up without certified round height.',
            };
        }
        let peers;
        try {
            peers = loadShieldedTopologyPeers(config.topology);
        } catch (error) {
            return {
                enabled: true,
                ok: false,
                status: 'topology_unavailable',
                message: error.message || 'Unable to read topology for laggard catch-up.',
            };
        }
        const before = await collectShieldedTopologyStatuses(peers, catchUpConfig.status_timeout_ms);
        let sourceReady = before;
        let expectedRoot = majorityRootAtHeight(sourceReady, certifiedHeight)
            || report?.local_state?.state_root
            || report?.transport?.local_state?.state_root
            || null;
        let laggards = shieldedCatchUpLaggards(sourceReady, certifiedHeight, expectedRoot);
        const sourceWaitAttempts = Number.isInteger(catchUpConfig.source_wait_attempts)
            && catchUpConfig.source_wait_attempts > 0
            ? catchUpConfig.source_wait_attempts
            : 15;
        const sourceWaitDelayMs = Number.isInteger(catchUpConfig.source_wait_delay_ms)
            && catchUpConfig.source_wait_delay_ms >= 0
            ? catchUpConfig.source_wait_delay_ms
            : 1000;
        const sourceWait = {
            attempts: 0,
            waited_ms: 0,
            max_attempts: sourceWaitAttempts,
            delay_ms: sourceWaitDelayMs,
            source_available: false,
            source_validator_ids: [],
            status: 'source_unavailable',
        };
        for (let attempt = 0; attempt < sourceWaitAttempts; attempt += 1) {
            if (attempt > 0) {
                await sleep(sourceWaitDelayMs);
                sourceWait.waited_ms += sourceWaitDelayMs;
                sourceReady = await collectShieldedTopologyStatuses(peers, catchUpConfig.status_timeout_ms);
                expectedRoot = expectedRoot || majorityRootAtHeight(sourceReady, certifiedHeight);
                laggards = shieldedCatchUpLaggards(sourceReady, certifiedHeight, expectedRoot);
            }
            sourceWait.attempts = attempt + 1;
            const sourceCandidates = shieldedCatchUpSourceCandidates(
                peers,
                sourceReady,
                certifiedHeight,
                expectedRoot,
                catchUpConfig.preferred_sources,
            );
            sourceWait.source_validator_ids = sourceCandidates.map((candidate) => candidate.peer.validatorId);
            sourceWait.source_available = sourceCandidates.length > 0;
            if (laggards.length === 0) {
                sourceWait.status = 'already_converged';
                break;
            }
            if (sourceWait.source_available) {
                sourceWait.status = 'source_available';
                break;
            }
        }
        const catchUps = [];
        for (const row of laggards) {
            const target = peers.find((peer) => peer.validatorId === row.endpoint?.validatorId);
            if (!target) {
                catchUps.push({
                    ok: false,
                    target: row.endpoint?.validatorId || 'unknown',
                    message: 'No topology peer found for laggard.',
                });
                continue;
            }
            const source = chooseShieldedCatchUpSource(
                peers,
                sourceReady,
                target.validatorId,
                certifiedHeight,
                expectedRoot,
                catchUpConfig.preferred_sources,
            );
            if (!source) {
                catchUps.push({
                    ok: false,
                    target: target.validatorId,
                    message: 'No caught-up source peer available for rpc-catch-up.',
                });
                continue;
            }
            catchUps.push(await runShieldedRpcCatchUp(config, target, source, catchUpConfig));
        }

        let after = before;
        let convergence = shieldedConvergenceSummary(peers, after, certifiedHeight);
        const attempts = Math.max(1, catchUpConfig.recheck_attempts);
        for (let attempt = 0; attempt < attempts; attempt += 1) {
            after = await collectShieldedTopologyStatuses(peers, catchUpConfig.status_timeout_ms);
            convergence = shieldedConvergenceSummary(peers, after, certifiedHeight);
            if (convergence.converged) break;
            if (attempt < attempts - 1) await sleep(catchUpConfig.recheck_delay_ms);
        }

        const finishedAtMs = Date.now();
        const result = {
            enabled: true,
            ok: convergence.converged && catchUps.every((item) => item.ok !== false),
            status: convergence.converged ? 'converged' : 'not_converged',
            certified_height: certifiedHeight,
            expected_root: expectedRoot,
            started_at_unix_ms: startedAtMs,
            finished_at_unix_ms: finishedAtMs,
            elapsed_ms: finishedAtMs - startedAtMs,
            laggard_count: laggards.length,
            source_wait: sourceWait,
            laggards: laggards.map((row) => ({
                validator_id: row.endpoint?.validatorId || null,
                ok: row.ok,
                height: row.status?.block_height ?? null,
                root: row.status?.state_root ?? null,
                error: row.error || null,
            })),
            catch_ups: catchUps,
            before,
            source_ready: sourceReady,
            after,
            convergence,
        };
        if (artifactDir) {
            try {
                fs.writeFileSync(
                    path.join(artifactDir, 'laggard-catch-up.json'),
                    `${JSON.stringify(result, null, 2)}\n`,
                    { mode: 0o600 },
                );
            } catch (_) {
                // The round response still carries the catch-up report if artifact writing fails.
            }
        }
        return result;
    }

    function fileMtimeUnixMs(file) {
        try {
            return fs.statSync(file).mtimeMs;
        } catch (_) {
            return null;
        }
    }

    function maxMtimeUnixMs(dir) {
        try {
            return fs.readdirSync(dir)
                .map((name) => fileMtimeUnixMs(path.join(dir, name)))
                .filter((value) => Number.isFinite(value))
                .reduce((best, value) => (best === null || value > best ? value : best), null);
        } catch (_) {
            return null;
        }
    }

    function msSpan(startMs, endMs) {
        if (!Number.isFinite(startMs) || !Number.isFinite(endMs) || endMs < startMs) return null;
        return Math.round(endMs - startMs);
    }

    function shieldedRoundPhaseTimings({
        artifactDir,
        batchReadyAtMs,
        roundStartedAtMs,
        roundFinishedAtMs,
        laggardCatchUp,
    }) {
        const proposalAtMs = artifactDir ? fileMtimeUnixMs(path.join(artifactDir, 'block-proposal.json')) : null;
        const certificateAtMs = artifactDir ? fileMtimeUnixMs(path.join(artifactDir, 'block-certificate.json')) : null;
        const deferredSendsDoneAtMs = artifactDir
            ? maxMtimeUnixMs(path.join(artifactDir, 'deferred-certified-sends'))
            : null;
        const catchUpStartedAtMs = Number.isFinite(laggardCatchUp?.started_at_unix_ms)
            ? laggardCatchUp.started_at_unix_ms
            : null;
        const catchUpFinishedAtMs = Number.isFinite(laggardCatchUp?.finished_at_unix_ms)
            ? laggardCatchUp.finished_at_unix_ms
            : null;
        return {
            schema: 'postfiat-wallet-proxy-shielded-round-phase-timings-v1',
            artifact_dir: artifactDir || null,
            batch_ready_at_unix_ms: batchReadyAtMs || null,
            round_started_at_unix_ms: roundStartedAtMs || null,
            proposal_at_unix_ms: proposalAtMs,
            certificate_at_unix_ms: certificateAtMs,
            deferred_sends_done_at_unix_ms: deferredSendsDoneAtMs,
            round_finished_at_unix_ms: roundFinishedAtMs || null,
            catch_up_started_at_unix_ms: catchUpStartedAtMs,
            catch_up_finished_at_unix_ms: catchUpFinishedAtMs,
            batch_ready_to_round_start_ms: msSpan(batchReadyAtMs, roundStartedAtMs),
            round_start_to_proposal_ms: msSpan(roundStartedAtMs, proposalAtMs),
            proposal_to_certificate_ms: msSpan(proposalAtMs, certificateAtMs),
            certificate_to_deferred_sends_done_ms: msSpan(certificateAtMs, deferredSendsDoneAtMs),
            deferred_sends_done_to_round_finished_ms: msSpan(deferredSendsDoneAtMs, roundFinishedAtMs),
            catch_up_ms: msSpan(catchUpStartedAtMs, catchUpFinishedAtMs),
            attribution_note: 'Proposal/certificate/deferred-send timestamps are artifact file mtimes emitted by the node round subprocess.',
        };
    }

    async function executeShieldedNavswapIngress(body = {}) {
        assertNoShieldedPrivateMaterial(body);
        const config = shieldedNavswapIngressConfig();
        if (!config.configured) {
            return {
                ok: false,
                schema: SHIELDED_NAVSWAP_INGRESS_SCHEMA,
                route: 'shielded_navswap',
                code: 'shielded_ingress_configuration_required',
                message: 'Shielded ingress relay is not configured.',
                missing: config.missing,
            };
        }
        const { walletAddress, payload, asset, amount } = validateShieldedIngressPayload(body, config);
        const stamp = new Date().toISOString().replace(/[-:.]/g, '').replace('T', '-').replace('Z', 'Z');
        const workDir = fs.mkdtempSync(path.join(os.tmpdir(), 'postfiat-shielded-ingress-'));
        const artifactDir = path.join(config.artifact_root, stamp);
        fs.mkdirSync(artifactDir, { recursive: true, mode: 0o700 });
        const ingressFile = path.join(workDir, 'ingress.json');
        const batchFile = path.join(workDir, 'batch.json');
        fs.writeFileSync(ingressFile, `${JSON.stringify({
            schema: ASSET_ORCHARD_INGRESS_FILE_SCHEMA,
            payload,
        }, null, 2)}\n`, { mode: 0o600 });

        const batchArgs = [
            'shield-batch-asset-orchard-ingress',
            '--data-dir', config.data_dir,
            '--ingress-file', ingressFile,
            '--batch-file', batchFile,
        ];
        let batch;
        try {
            const { stdout } = await execFileAsync(config.node_bin, batchArgs, {
                timeout: config.timeout_ms,
                maxBuffer: 8 * 1024 * 1024,
            });
            batch = JSON.parse(stdout || '{}');
            const batchArtifactFile = path.join(artifactDir, 'batch.json');
            fs.copyFileSync(batchFile, batchArtifactFile);
            fs.chmodSync(batchArtifactFile, 0o600);
        } catch (error) {
            return {
                ok: false,
                schema: SHIELDED_NAVSWAP_INGRESS_SCHEMA,
                route: 'shielded_navswap',
                code: 'shielded_ingress_batch_failed',
                message: error.message || 'shield-batch-asset-orchard-ingress failed',
                wallet_address: walletAddress,
                asset,
                amount_atoms: String(amount),
            };
        }

        const certifyArgs = buildShieldedCertifiedRoundArgs(config, batchFile, artifactDir);
        let report;
        let laggardCatchUp = null;
        try {
            const { stdout } = await execFileAsync(config.node_bin, certifyArgs, {
                timeout: config.timeout_ms,
                maxBuffer: 16 * 1024 * 1024,
                env: shieldedCertifiedRoundEnv(),
            });
            report = JSON.parse(stdout || '{}');
            laggardCatchUp = await runShieldedLaggardCatchUp(config, report, artifactDir);
        } catch (error) {
            return {
                ok: false,
                schema: SHIELDED_NAVSWAP_INGRESS_SCHEMA,
                route: 'shielded_navswap',
                code: 'shielded_ingress_certify_failed',
                message: error.message || 'transport-peer-certified-batch-round failed',
                wallet_address: walletAddress,
                asset,
                amount_atoms: String(amount),
                batch,
                artifact_dir: artifactDir,
            };
        }
        if (laggardCatchUp?.enabled && laggardCatchUp.ok !== true) {
            return {
                ok: false,
                schema: SHIELDED_NAVSWAP_INGRESS_SCHEMA,
                route: 'shielded_navswap',
                code: 'shielded_ingress_catch_up_failed',
                message: 'Shielded ingress certified, but early-quorum laggard catch-up did not prove 6/6 convergence.',
                wallet_address: walletAddress,
                asset,
                amount_atoms: String(amount),
                batch,
                report,
                laggard_catch_up: laggardCatchUp,
                artifact_dir: artifactDir,
            };
        }
        const failure = certifiedRoundFailure(report, 'Asset-Orchard ingress');
        if (failure) {
            return {
                ok: false,
                schema: SHIELDED_NAVSWAP_INGRESS_SCHEMA,
                route: 'shielded_navswap',
                code: 'shielded_ingress_rejected',
                message: failure,
                wallet_address: walletAddress,
                asset,
                amount_atoms: String(amount),
                batch,
                report,
                laggard_catch_up: laggardCatchUp || undefined,
                artifact_dir: artifactDir,
            };
        }
        return {
            ok: true,
            schema: SHIELDED_NAVSWAP_INGRESS_SCHEMA,
            route: 'shielded_navswap',
            status: 'ingress_certified',
            message: 'Public asset burn was certified into an Asset-Orchard ingress note.',
            wallet_address: walletAddress,
            asset,
            amount_atoms: String(amount),
            output_commitment: payload.output_commitment,
            batch,
            report,
            laggard_catch_up: laggardCatchUp || undefined,
            receipts: certifiedRoundReceipts(report?.transport || report),
            artifact_dir: artifactDir,
        };
    }

    async function createShieldedSwapBatchViaLocalService(config, rawAction, batchFile) {
        const service = config.asset_orchard_local_service || assetOrchardLocalServiceConfig();
        const endpoint = new URL('/asset-orchard/swap-batch', service.url).toString();
        const controller = new AbortController();
        const timer = setTimeout(() => controller.abort(), service.timeout_ms || config.timeout_ms);
        try {
            const response = await fetch(endpoint, {
                method: 'POST',
                headers: {
                    'Accept': 'application/json',
                    'Content-Type': 'application/json',
                },
                body: JSON.stringify({
                    route: 'shielded_navswap',
                    swap_action_json: rawAction,
                }),
                signal: controller.signal,
            });
            let payload = null;
            try {
                payload = await response.json();
            } catch (_) {
                payload = null;
            }
            if (!response.ok || payload?.ok !== true) {
                const err = new Error(payload?.message || payload?.error || `asset-orchard local service HTTP ${response.status}`);
                err.status = response.status;
                err.payload = payload;
                throw err;
            }
            const batchJson = typeof payload.batch_json === 'string'
                ? payload.batch_json
                : JSON.stringify(payload.batch, null, 2);
            if (!payload.batch || typeof batchJson !== 'string' || batchJson.length < 2) {
                const err = new Error('asset-orchard local service did not return a swap batch');
                err.payload = payload;
                throw err;
            }
            fs.writeFileSync(batchFile, `${batchJson.trim()}\n`, { mode: 0o600 });
            return payload.batch;
        } finally {
            clearTimeout(timer);
        }
    }

    function shieldedSwapProxyTimingReport(
        startedAtMs,
        batchStartedAtMs,
        batchFinishedAtMs,
        certifyStartedAtMs,
        certifyFinishedAtMs,
        batchRoute = 'resident_service',
        artifactDir = null,
        laggardCatchUp = null,
        roundReport = null,
    ) {
        const lastFinishedAtMs = certifyFinishedAtMs || batchFinishedAtMs || Date.now();
        const span = (start, end) => (
            Number.isFinite(start) && Number.isFinite(end) && end >= start ? end - start : null
        );
        const batchMs = span(batchStartedAtMs, batchFinishedAtMs);
        const roundTimings = roundReport && typeof roundReport === 'object' ? roundReport.timings : null;
        return {
            schema: 'postfiat-wallet-proxy-shielded-swap-timings-v1',
            timing_scope: 'wallet_proxy_submit_handler_wall_clock',
            non_overlapping: true,
            proof_only_ms: null,
            proof_stage_note: 'Proof construction is routed through the warm resident Asset-Orchard service; certified-round transport remains a subprocess.',
            batch_route: batchRoute,
            total_proxy_ms: span(startedAtMs, lastFinishedAtMs),
            batch_ms: batchMs,
            batch_subprocess_ms: batchRoute === 'subprocess' ? batchMs : null,
            batch_resident_service_ms: batchRoute === 'resident_service' ? batchMs : null,
            certified_round_ms: span(certifyStartedAtMs, certifyFinishedAtMs),
            started_at_unix_ms: startedAtMs,
            batch_started_at_unix_ms: batchStartedAtMs || null,
            batch_finished_at_unix_ms: batchFinishedAtMs || null,
            certify_started_at_unix_ms: certifyStartedAtMs || null,
            certify_finished_at_unix_ms: certifyFinishedAtMs || null,
            phase_timings: shieldedRoundPhaseTimings({
                artifactDir,
                batchReadyAtMs: batchFinishedAtMs,
                roundStartedAtMs: certifyStartedAtMs,
                roundFinishedAtMs: certifyFinishedAtMs,
                laggardCatchUp,
            }),
            node_round: {
                shielded_verifier_prewarm: roundReport?.shielded_verifier_prewarm || null,
                proposal_breakdown: roundTimings?.proposal_breakdown || null,
                local_apply_breakdown: roundTimings?.local_apply_breakdown || null,
            },
        };
    }

    async function executeShieldedNavswapSwap(body = {}) {
        const proxyStartedAtMs = Date.now();
        let batchStartedAtMs = null;
        let batchFinishedAtMs = null;
        let batchRoute = 'resident_service';
        let certifyStartedAtMs = null;
        let certifyFinishedAtMs = null;
        const config = shieldedNavswapSwapConfig();
        let validated;
        try {
            validated = validateShieldedSwapSubmit(body, config);
        } catch (error) {
            return {
                ok: false,
                schema: SHIELDED_NAVSWAP_SWAP_SCHEMA,
                route: 'shielded_navswap',
                code: error.code || 'shielded_swap_request_invalid',
                message: error.message || 'Shielded swap submit request is invalid.',
                missing: error.missing || undefined,
            };
        }

        const stamp = new Date().toISOString().replace(/[-:.]/g, '').replace('T', '-').replace('Z', 'Z');
        const workDir = fs.mkdtempSync(path.join(os.tmpdir(), 'postfiat-shielded-swap-'));
        const artifactDir = path.join(config.artifact_root, stamp);
        fs.mkdirSync(artifactDir, { recursive: true, mode: 0o700 });
        const swapFile = path.join(workDir, 'swap-action.json');
        const batchFile = path.join(workDir, 'batch.json');
        fs.writeFileSync(swapFile, `${validated.rawAction}\n`, { mode: 0o600 });

        let batch;
        try {
            batchStartedAtMs = Date.now();
            batch = await createShieldedSwapBatchViaLocalService(config, validated.rawAction, batchFile);
            batchFinishedAtMs = Date.now();
            const batchArtifactFile = path.join(artifactDir, 'batch.json');
            fs.copyFileSync(batchFile, batchArtifactFile);
            fs.chmodSync(batchArtifactFile, 0o600);
            const actionArtifactFile = path.join(artifactDir, 'swap-action.json');
            fs.copyFileSync(swapFile, actionArtifactFile);
            fs.chmodSync(actionArtifactFile, 0o600);
        } catch (error) {
            return {
                ok: false,
                schema: SHIELDED_NAVSWAP_SWAP_SCHEMA,
                route: 'shielded_navswap',
                code: 'shielded_swap_batch_failed',
                message: error.message || 'shield-batch-swap failed',
                wallet_address: validated.walletAddress,
                quote_binding_hash: validated.quoteBindingHash,
                action_verification: validated.actionVerification,
                timings_ms: shieldedSwapProxyTimingReport(proxyStartedAtMs, batchStartedAtMs, batchFinishedAtMs, certifyStartedAtMs, certifyFinishedAtMs, batchRoute),
            };
        }

        const certifyArgs = buildShieldedCertifiedRoundArgs(config, batchFile, artifactDir);
        let report;
        let loopCertification = null;
        let certifyArtifactDir = artifactDir;
        let laggardCatchUp = null;
        try {
            certifyStartedAtMs = Date.now();
            if (config.certifier_loop?.enabled) {
                loopCertification = await certifyShieldedBatchViaWarmLoop(config, batchFile, stamp);
                report = loopCertification.round;
                certifyArtifactDir = loopCertification.artifact_dir;
            } else {
                const { stdout } = await execFileAsync(config.node_bin, certifyArgs, {
                    timeout: config.timeout_ms,
                    maxBuffer: 16 * 1024 * 1024,
                    env: shieldedCertifiedRoundEnv(),
                });
                report = JSON.parse(stdout || '{}');
            }
            certifyFinishedAtMs = Date.now();
            laggardCatchUp = await runShieldedLaggardCatchUp(config, report, artifactDir);
        } catch (error) {
            return {
                ok: false,
                schema: SHIELDED_NAVSWAP_SWAP_SCHEMA,
                route: 'shielded_navswap',
                code: 'shielded_swap_certify_failed',
                message: error.message || 'transport-peer-certified-batch-round failed',
                wallet_address: validated.walletAddress,
                quote_binding_hash: validated.quoteBindingHash,
                batch,
                artifact_dir: artifactDir,
                certifier_loop: loopCertification || undefined,
                action_verification: validated.actionVerification,
                timings_ms: shieldedSwapProxyTimingReport(
                    proxyStartedAtMs,
                    batchStartedAtMs,
                    batchFinishedAtMs,
                    certifyStartedAtMs,
                    certifyFinishedAtMs,
                    batchRoute,
                    certifyArtifactDir,
                ),
            };
        }
        if (laggardCatchUp?.enabled && laggardCatchUp.ok !== true) {
            return {
                ok: false,
                schema: SHIELDED_NAVSWAP_SWAP_SCHEMA,
                route: 'shielded_navswap',
                code: 'shielded_swap_catch_up_failed',
                message: 'Private Asset-Orchard NAVSwap certified, but early-quorum laggard catch-up did not prove 6/6 convergence.',
                wallet_address: validated.walletAddress,
                quote_binding_hash: validated.quoteBindingHash,
                batch,
                report,
                certifier_loop: loopCertification || undefined,
                laggard_catch_up: laggardCatchUp,
                artifact_dir: artifactDir,
                certified_artifact_dir: certifyArtifactDir,
                action_verification: validated.actionVerification,
                timings_ms: shieldedSwapProxyTimingReport(
                    proxyStartedAtMs,
                    batchStartedAtMs,
                    batchFinishedAtMs,
                    certifyStartedAtMs,
                    certifyFinishedAtMs,
                    batchRoute,
                    certifyArtifactDir,
                    laggardCatchUp,
                    report,
                ),
            };
        }
        const failure = certifiedRoundFailure(report, 'Asset-Orchard swap', {
            allow_quorum_certificate: true,
        });
        if (failure) {
            return {
                ok: false,
                schema: SHIELDED_NAVSWAP_SWAP_SCHEMA,
                route: 'shielded_navswap',
                code: 'shielded_swap_rejected',
                message: failure,
                wallet_address: validated.walletAddress,
                quote_binding_hash: validated.quoteBindingHash,
                batch,
                report,
                certifier_loop: loopCertification || undefined,
                laggard_catch_up: laggardCatchUp || undefined,
                artifact_dir: artifactDir,
                certified_artifact_dir: certifyArtifactDir,
                action_verification: validated.actionVerification,
                timings_ms: shieldedSwapProxyTimingReport(
                    proxyStartedAtMs,
                    batchStartedAtMs,
                    batchFinishedAtMs,
                    certifyStartedAtMs,
                    certifyFinishedAtMs,
                    batchRoute,
                    certifyArtifactDir,
                    laggardCatchUp,
                    report,
                ),
            };
        }
        return {
            ok: true,
            schema: SHIELDED_NAVSWAP_SWAP_SCHEMA,
            route: 'shielded_navswap',
            status: 'swap_certified',
            message: 'Private Asset-Orchard NAVSwap was certified.',
            wallet_address: validated.walletAddress,
            quote_binding_hash: validated.quoteBindingHash,
            quote_expires_at_ms: String(validated.expiresAtMs),
            liquidity_commitment: validated.liquidityCommitment,
            action_verification: validated.actionVerification,
            batch,
            report,
            certifier_loop: loopCertification || undefined,
            laggard_catch_up: laggardCatchUp || undefined,
            receipts: certifiedRoundReceipts(report?.transport || report),
            artifact_dir: artifactDir,
            certified_artifact_dir: certifyArtifactDir,
            trust_class: 'CONTROLLED',
            certification_acceptance: (report?.transport || report)?.round_ok === true
                ? 'round_ok'
                : 'quorum_certificate_local_apply',
            quote_binding_enforcement: 'proxy_checked_quote_freshness_and_liquidity_commitment_not_circuit_external_binding',
            timings_ms: shieldedSwapProxyTimingReport(
                proxyStartedAtMs,
                batchStartedAtMs,
                batchFinishedAtMs,
                certifyStartedAtMs,
                certifyFinishedAtMs,
                batchRoute,
                certifyArtifactDir,
                laggardCatchUp,
                report,
            ),
        };
    }

    async function executeShieldedNavswapEgress(body = {}) {
        const config = shieldedNavswapEgressConfig();
        let validated;
        try {
            validated = validateShieldedEgressSubmit(body, config);
        } catch (error) {
            return {
                ok: false,
                schema: SHIELDED_NAVSWAP_EGRESS_SCHEMA,
                route: 'shielded_navswap',
                code: error.code || 'shielded_egress_request_invalid',
                message: error.message || 'Shielded private egress request is invalid.',
                missing: error.missing || undefined,
                expected_disclosure_hash: error.expected_disclosure_hash || undefined,
            };
        }

        const stamp = new Date().toISOString().replace(/[-:.]/g, '').replace('T', '-').replace('Z', 'Z');
        const workDir = fs.mkdtempSync(path.join(os.tmpdir(), 'postfiat-shielded-egress-'));
        const artifactDir = path.join(config.artifact_root, stamp);
        fs.mkdirSync(artifactDir, { recursive: true, mode: 0o700 });
        const egressFile = path.join(workDir, 'private-egress.json');
        const batchFile = path.join(workDir, 'batch.json');
        fs.writeFileSync(egressFile, `${validated.rawEgress.trim()}\n`, { mode: 0o600 });

        let batch;
        try {
            const { stdout } = await execFileAsync(config.node_bin, [
                'shield-batch-asset-orchard-private-egress',
                '--data-dir', config.data_dir,
                '--egress-file', egressFile,
                '--batch-file', batchFile,
            ], {
                timeout: config.timeout_ms,
                maxBuffer: 16 * 1024 * 1024,
            });
            batch = JSON.parse(stdout || '{}');
            const batchArtifactFile = path.join(artifactDir, 'batch.json');
            fs.copyFileSync(batchFile, batchArtifactFile);
            fs.chmodSync(batchArtifactFile, 0o600);
            const egressArtifactFile = path.join(artifactDir, 'private-egress.json');
            fs.copyFileSync(egressFile, egressArtifactFile);
            fs.chmodSync(egressArtifactFile, 0o600);
            fs.writeFileSync(path.join(artifactDir, 'public-exit-disclosure.json'), `${JSON.stringify({
                disclosure: validated.disclosure,
                disclosure_hash: validated.disclosureHash,
                acknowledgement: 'public destination, asset, amount, and timing become visible; note opening stays hidden',
            }, null, 2)}\n`, { mode: 0o600 });
        } catch (error) {
            return {
                ok: false,
                schema: SHIELDED_NAVSWAP_EGRESS_SCHEMA,
                route: 'shielded_navswap',
                code: 'shielded_egress_batch_failed',
                message: error.message || 'shield-batch-asset-orchard-private-egress failed',
                wallet_address: validated.walletAddress,
                disclosure_hash: validated.disclosureHash,
                action_verification: validated.actionVerification,
            };
        }

        const certifyArgs = buildShieldedCertifiedRoundArgs(config, batchFile, artifactDir);
        let report;
        let laggardCatchUp = null;
        try {
            const { stdout } = await execFileAsync(config.node_bin, certifyArgs, {
                timeout: config.timeout_ms,
                maxBuffer: 16 * 1024 * 1024,
                env: shieldedCertifiedRoundEnv(),
            });
            report = JSON.parse(stdout || '{}');
            laggardCatchUp = await runShieldedLaggardCatchUp(config, report, artifactDir);
        } catch (error) {
            return {
                ok: false,
                schema: SHIELDED_NAVSWAP_EGRESS_SCHEMA,
                route: 'shielded_navswap',
                code: 'shielded_egress_certify_failed',
                message: error.message || 'transport-peer-certified-batch-round failed',
                wallet_address: validated.walletAddress,
                disclosure_hash: validated.disclosureHash,
                batch,
                artifact_dir: artifactDir,
                action_verification: validated.actionVerification,
            };
        }
        if (laggardCatchUp?.enabled && laggardCatchUp.ok !== true) {
            return {
                ok: false,
                schema: SHIELDED_NAVSWAP_EGRESS_SCHEMA,
                route: 'shielded_navswap',
                code: 'shielded_egress_catch_up_failed',
                message: 'Private Asset-Orchard egress certified, but early-quorum laggard catch-up did not prove 6/6 convergence.',
                wallet_address: validated.walletAddress,
                disclosure_hash: validated.disclosureHash,
                batch,
                report,
                laggard_catch_up: laggardCatchUp,
                artifact_dir: artifactDir,
                action_verification: validated.actionVerification,
            };
        }
        const failure = certifiedRoundFailure(report, 'Asset-Orchard private egress', {
            allow_quorum_certificate: true,
        });
        if (failure) {
            return {
                ok: false,
                schema: SHIELDED_NAVSWAP_EGRESS_SCHEMA,
                route: 'shielded_navswap',
                code: 'shielded_egress_rejected',
                message: failure,
                wallet_address: validated.walletAddress,
                disclosure_hash: validated.disclosureHash,
                batch,
                report,
                laggard_catch_up: laggardCatchUp || undefined,
                artifact_dir: artifactDir,
                action_verification: validated.actionVerification,
            };
        }
        return {
            ok: true,
            schema: SHIELDED_NAVSWAP_EGRESS_SCHEMA,
            route: 'shielded_navswap',
            status: 'private_egress_certified_public_exit',
            message: 'Private Asset-Orchard egress was certified and credited public issued-asset balance.',
            wallet_address: validated.walletAddress,
            to: validated.to,
            asset_id: validated.assetId,
            amount_atoms: String(validated.amountAtoms),
            policy_id: validated.policyId,
            disclosure_hash: validated.disclosureHash,
            disclosure: validated.disclosure,
            note_commitment: validated.noteCommitment || null,
            public_exit_receipt_required_for_bridge_out: true,
            bridge_out_enabled: true,
            bridge_out_gate: 'public_exit_receipt_certified',
            action_verification: validated.actionVerification,
            batch,
            report,
            laggard_catch_up: laggardCatchUp || undefined,
            receipts: certifiedRoundReceipts(report?.transport || report),
            artifact_dir: artifactDir,
            trust_class: 'CONTROLLED',
            certification_acceptance: (report?.transport || report)?.round_ok === true
                ? 'round_ok'
                : 'quorum_certificate_local_apply',
        };
    }


    return { ASSET_ORCHARD_ACTION_CLEAR_KEYS,buildShieldedCertifiedRoundArgs,certifiedRoundFailure,certifiedRoundHasQuorumCertificate,certifiedRoundHeight,certifiedRoundReceipts,certifyShieldedBatchViaWarmLoop,chooseShieldedCatchUpSource,cloneJson,collectShieldedTopologyStatuses,createShieldedSwapBatchViaLocalService,executeShieldedNavswapBalances,executeShieldedNavswapEgress,executeShieldedNavswapIngress,executeShieldedNavswapIngressPreflight,executeShieldedNavswapNoteCapability,executeShieldedNavswapProverReadiness,executeShieldedNavswapQuote,executeShieldedNavswapStatus,executeShieldedNavswapSwap,fileMtimeUnixMs,findAssetOrchardActionCleartext,loadShieldedTopologyPeers,majorityRootAtHeight,maxMtimeUnixMs,msSpan,navswapIdempotencyKeyFromRequest,navswapIdempotencyStorePath,navswapRunStorePath,navswapStableJson,navswapValidateIdempotencyKey,newNavswapRunId,parseShieldedPrivateEgressJson,parseShieldedSwapActionJson,runShieldedLaggardCatchUp,runShieldedRpcCatchUp,shellQuote,shieldedBatchExplicitActionIds,shieldedCatchUpLaggards,shieldedCatchUpSourceCandidates,shieldedCertifiedRoundEnv,shieldedCertifierLoopBatchFile,shieldedCertifierLoopStartHeight,shieldedCertifierLoopState,shieldedConvergenceSummary,shieldedEarlyQuorumEnabled,shieldedIngressSupportedAsset,shieldedLaggardCatchUpConfig,shieldedPrivateEgressDisclosureFields,shieldedPrivateEgressDisclosureHash,shieldedQuoteAssetByInput,shieldedQuoteFromSubmitBody,shieldedQuotePairEnabled,shieldedRemoteDataDir,shieldedRemoteWorkDir,shieldedRoundBatchIds,shieldedRoundPhaseTimings,shieldedRoundReceiptIds,shieldedSwapProxyTimingReport,startShieldedCertifierLoop,validateShieldedCertifierLoopReportForBatch,validateShieldedEgressSubmit,validateShieldedIngressPayload,validateShieldedPrivateEgressFile,validateShieldedSwapAction,validateShieldedSwapSubmit };
}

module.exports = { create };
