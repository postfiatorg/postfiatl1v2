'use strict';

function create(runtime) {
    const { A651_ASSET_ID,A652_ASSET_ID,ALLOWED_ORIGINS,ASSET_ORCHARD_ACTION_CLEAR_KEYS,ASSET_ORCHARD_INGRESS_FILE_SCHEMA,ASSET_ORCHARD_POOL_ID,ASSET_ORCHARD_PRIVATE_EGRESS_ACTION_SCHEMA,ASSET_ORCHARD_PRIVATE_EGRESS_FILE_SCHEMA,ASSET_ORCHARD_SWAP_ACTION_SCHEMA,DEFAULT_ASSET_ORCHARD_LOCAL_SERVICE_READINESS_TIMEOUT_MS,DEFAULT_ASSET_ORCHARD_LOCAL_SERVICE_URL,DEFAULT_RPC_FLEET,ENABLE_FINALITY_RESPONDER_READ_CACHE,ENABLE_FIRST_READY_SEQUENCED_READ,ENABLE_NATIVE_WALLET_SIGNER,ENABLE_PROPOSER_ROUTING,ENABLE_RPC_PARENT_WAIT_FINALITY_ROUTE,ENABLE_RPC_PARENT_WAIT_SEQUENCED_READ_ROUTE,ENABLE_UPSTREAM_KEEPALIVE,ETHEREUM_USDC_TOKEN,FASTPAY_BROADCAST_METHODS,FASTPAY_FLEET_STATUS_CACHE_MS,FASTPAY_OWNED_OBJECT_LOOKUP_LIMIT,FASTPAY_REQUIRE_PRIMARY_SUCCESS,FASTPAY_ROUTE_RETRY_MS,FASTPAY_ROUTE_TIMEOUT_MS,FINALITY_METHODS,FIRST_READY_SEQUENCED_READ_PROPOSERS,INJECT_RPC_CAPS,LEGACY_A651_ETH_TOKEN,LEGACY_A651_UNISWAP_POOL_ID,LISTEN_PORT,MAX_TCP_PER_WS,MAX_WS_MESSAGE_BYTES,NATIVE_WALLET_SIGNER_BIN,NATIVE_WALLET_SIGNER_TIMEOUT_MS,NAVSWAP_CAPABILITIES_SCHEMA,NAVSWAP_DEVNET_FUNDING_SCHEMA,NAVSWAP_IDEMPOTENCY_STORE_DEFAULT_PATH,NAVSWAP_IDEMPOTENCY_STORE_SCHEMA,NAVSWAP_IDEMPOTENCY_TTL_MS,NAVSWAP_MAX_LIVE_USD,NAVSWAP_NAV_PROOF_SCHEMA,NAVSWAP_PRIMARY_MINT_ROUTE_FAMILY,NAVSWAP_QUOTE_FRESHNESS_TTL_MS,NAVSWAP_QUOTE_SCHEMA,NAVSWAP_READINESS_SCHEMA,NAVSWAP_ROUTE_TRUST_CLASSES,NAVSWAP_RUN_EVENTS_SCHEMA,NAVSWAP_RUN_LIST_SCHEMA,NAVSWAP_RUN_RECEIPTS_SCHEMA,NAVSWAP_RUN_SCHEMA,NAVSWAP_RUN_STATUS_SCHEMA,NAVSWAP_RUN_STORE_DEFAULT_PATH,NAVSWAP_RUN_STORE_SCHEMA,NAVSWAP_RUN_STREAM_EVENT_SCHEMA,NAVSWAP_RUN_STREAM_SCHEMA,NAVSWAP_SETTLEMENT_RECEIPT_MAX_SNAPSHOT_AGE_BLOCKS,NAVSWAP_SETTLEMENT_RECEIPT_SAFETY_BLOCKS,NAVSWAP_STAKEHUB_TRANSPARENT_ACTION,NAVSWAP_TRANSPARENT_PLANNER_INPUTS_SCHEMA,NAVSWAP_WALLET_ACTION_BATCH_PREPARE_SCHEMA,NAVSWAP_WALLET_ACTION_PREPARE_SCHEMA,NAVSWAP_WALLET_ACTION_SCHEMA,OPTIMISTIC_CACHED_FINALITY_ROUTE,PFUSDC_ASSET_ID,PREFERRED_SEQUENCED_READ_VALIDATORS,PROPOSER_READY_RETRY_MS,PROPOSER_ROUTE_CACHE_MS,PROPOSER_ROUTE_RETRY_MS,PROPOSER_ROUTE_TIMEOUT_MS,RPC_CAPS,RPC_FLEET,RPC_HOST,RPC_PORT,SEQUENCED_ACCOUNT_METHODS,SHIELDED_NAVSWAP_EGRESS_POLICY_ID,SHIELDED_NAVSWAP_EGRESS_SCHEMA,SHIELDED_NAVSWAP_INGRESS_SCHEMA,SHIELDED_NAVSWAP_LIQUIDITY_MODES,SHIELDED_NAVSWAP_PREFLIGHT_SCHEMA,SHIELDED_NAVSWAP_QUOTE_SCHEMA,SHIELDED_NAVSWAP_STATUS_SCHEMA,SHIELDED_NAVSWAP_SWAP_SCHEMA,SHIELDED_ROUND_TIMEOUT_DEFAULT_MS,TCP_TIMEOUT_MS,UpstreamRpcConnection,VAULT_BRIDGE_ALLOCATION_PURPOSE_NAV_SUBSCRIPTION,VAULT_BRIDGE_ALLOCATION_PURPOSE_SUPPLY,VAULT_BRIDGE_BUCKET_STATUS_ACTIVE,VAULT_BRIDGE_RECEIPT_STATUS_COUNTED,VAULT_BRIDGE_RECIPIENT_SPONSOR_AMOUNT,VAULT_BRIDGE_RECIPIENT_SPONSOR_MIN_AMOUNT_ATOMS,VAULT_BRIDGE_RELAY_DEFAULT_ACCOUNT,VAULT_BRIDGE_RELAY_EXPIRES_AT_HEIGHT,VAULT_BRIDGE_RELAY_POLICY_HASH,VAULT_BRIDGE_RELAY_SCHEMA,VAULT_BRIDGE_RELAY_SOURCE_CHAIN_ID,VAULT_BRIDGE_RELAY_SOURCE_RPC_URL,VAULT_BRIDGE_RELAY_TOKEN_ADDRESS,VAULT_BRIDGE_RELAY_VAULT_ADDRESS,WALLET_SUBSCRIPTION_INTERVAL_MS,WALLET_SUBSCRIPTION_MIN_INTERVAL_MS,WALLET_SUBSCRIPTION_READ_TIMEOUT_MS,crypto,execFileAsync,fs,http,navswapDevnetFundingUsage,navswapIdempotencyRecords,navswapRunStreams,navswapRuns,net,os,path,server,upstreamRpcConnections,wss } = runtime;
    let { fastpayFleetStatusCache,fastpayFleetStatusInFlight,latestFinalizedReadCache,preferredSequencedReadIndex,proposerRouteCache,shieldedCertifierLoopState } = runtime;
    const addProxyRouteEvent = (...args) => runtime.addProxyRouteEvent(...args);
    const annotateNavswapIdempotency = (...args) => runtime.annotateNavswapIdempotency(...args);
    const assetIdForNavswapSymbol = (...args) => runtime.assetIdForNavswapSymbol(...args);
    const bftQuorumThreshold = (...args) => runtime.bftQuorumThreshold(...args);
    const broadcastFastpayMutation = (...args) => runtime.broadcastFastpayMutation(...args);
    const buildNavswapNavProofResponse = (...args) => runtime.buildNavswapNavProofResponse(...args);
    const buildNavswapQuoteResponse = (...args) => runtime.buildNavswapQuoteResponse(...args);
    const buildNavswapRunResponse = (...args) => runtime.buildNavswapRunResponse(...args);
    const buildPftlUniswapReceiptVerification = (...args) => runtime.buildPftlUniswapReceiptVerification(...args);
    const buildShieldedCertifiedRoundArgs = (...args) => runtime.buildShieldedCertifiedRoundArgs(...args);
    const buildStakehubTransparentPreflight = (...args) => runtime.buildStakehubTransparentPreflight(...args);
    const buildTransparentNavswapReceiptVerification = (...args) => runtime.buildTransparentNavswapReceiptVerification(...args);
    const buildTransparentNavswapRedeemReceiptVerification = (...args) => runtime.buildTransparentNavswapRedeemReceiptVerification(...args);
    const buildUrl = (...args) => runtime.buildUrl(...args);
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
    const clearNavswapIdempotencyForTest = (...args) => runtime.clearNavswapIdempotencyForTest(...args);
    const clearNavswapRunsForTest = (...args) => runtime.clearNavswapRunsForTest(...args);
    const cloneJson = (...args) => runtime.cloneJson(...args);
    const closeUpstreamRpcConnections = (...args) => runtime.closeUpstreamRpcConnections(...args);
    const collectFastpayFleetStatuses = (...args) => runtime.collectFastpayFleetStatuses(...args);
    const collectFleetStatuses = (...args) => runtime.collectFleetStatuses(...args);
    const collectShieldedTopologyStatuses = (...args) => runtime.collectShieldedTopologyStatuses(...args);
    const compareNavswapRunsNewestFirst = (...args) => runtime.compareNavswapRunsNewestFirst(...args);
    const completePftlUniswapHandoffRun = (...args) => runtime.completePftlUniswapHandoffRun(...args);
    const completeTransparentNavswapRun = (...args) => runtime.completeTransparentNavswapRun(...args);
    const conciseRpcError = (...args) => runtime.conciseRpcError(...args);
    const convergedFleetGroup = (...args) => runtime.convergedFleetGroup(...args);
    const createNavswapRun = (...args) => runtime.createNavswapRun(...args);
    const createShieldedSwapBatchViaLocalService = (...args) => runtime.createShieldedSwapBatchViaLocalService(...args);
    const deterministicProposer = (...args) => runtime.deterministicProposer(...args);
    const endpointStatusMeetsRoute = (...args) => runtime.endpointStatusMeetsRoute(...args);
    const endpointStatusMeetsSequencedReadRoute = (...args) => runtime.endpointStatusMeetsSequencedReadRoute(...args);
    const executeNavswapAtomicTemplate = (...args) => runtime.executeNavswapAtomicTemplate(...args);
    const executeNavswapDevnetPfusdcFunding = (...args) => runtime.executeNavswapDevnetPfusdcFunding(...args);
    const executeNavswapIdempotentRequest = (...args) => runtime.executeNavswapIdempotentRequest(...args);
    const executeNavswapQuote = (...args) => runtime.executeNavswapQuote(...args);
    const executeNavswapRun = (...args) => runtime.executeNavswapRun(...args);
    const executePftlUniswapHandoffRun = (...args) => runtime.executePftlUniswapHandoffRun(...args);
    const executePftlUniswapWalletQuote = (...args) => runtime.executePftlUniswapWalletQuote(...args);
    const executeShieldedNavswapBalances = (...args) => runtime.executeShieldedNavswapBalances(...args);
    const executeShieldedNavswapEgress = (...args) => runtime.executeShieldedNavswapEgress(...args);
    const executeShieldedNavswapIngress = (...args) => runtime.executeShieldedNavswapIngress(...args);
    const executeShieldedNavswapIngressPreflight = (...args) => runtime.executeShieldedNavswapIngressPreflight(...args);
    const executeShieldedNavswapNoteCapability = (...args) => runtime.executeShieldedNavswapNoteCapability(...args);
    const executeShieldedNavswapProverReadiness = (...args) => runtime.executeShieldedNavswapProverReadiness(...args);
    const executeShieldedNavswapQuote = (...args) => runtime.executeShieldedNavswapQuote(...args);
    const executeShieldedNavswapStatus = (...args) => runtime.executeShieldedNavswapStatus(...args);
    const executeShieldedNavswapSwap = (...args) => runtime.executeShieldedNavswapSwap(...args);
    const executeTransparentNavswapQuote = (...args) => runtime.executeTransparentNavswapQuote(...args);
    const executeTransparentNavswapReadiness = (...args) => runtime.executeTransparentNavswapReadiness(...args);
    const executeTransparentNavswapRun = (...args) => runtime.executeTransparentNavswapRun(...args);
    const fetchJsonWithTimeout = (...args) => runtime.fetchJsonWithTimeout(...args);
    const fetchWalletSnapshot = (...args) => runtime.fetchWalletSnapshot(...args);
    const fileMtimeUnixMs = (...args) => runtime.fileMtimeUnixMs(...args);
    const findAssetOrchardActionCleartext = (...args) => runtime.findAssetOrchardActionCleartext(...args);
    const finishNavswapRun = (...args) => runtime.finishNavswapRun(...args);
    const firstReadyEndpointForRoute = (...args) => runtime.firstReadyEndpointForRoute(...args);
    const firstStructuredFastpayResult = (...args) => runtime.firstStructuredFastpayResult(...args);
    const forwardStakehubTransparentRun = (...args) => runtime.forwardStakehubTransparentRun(...args);
    const handleNavswapHttp = (...args) => runtime.handleNavswapHttp(...args);
    const invalidateProposerRouteCache = (...args) => runtime.invalidateProposerRouteCache(...args);
    const isFastpayBroadcastMethod = (...args) => runtime.isFastpayBroadcastMethod(...args);
    const isFinalityMethod = (...args) => runtime.isFinalityMethod(...args);
    const isIssuedAsset = (...args) => runtime.isIssuedAsset(...args);
    const isNativeWalletSignMethod = (...args) => runtime.isNativeWalletSignMethod(...args);
    const isPftAsset = (...args) => runtime.isPftAsset(...args);
    const isSequencedAccountMethod = (...args) => runtime.isSequencedAccountMethod(...args);
    const jsonHeaders = (...args) => runtime.jsonHeaders(...args);
    const loadNavswapIdempotencyStore = (...args) => runtime.loadNavswapIdempotencyStore(...args);
    const loadNavswapRunStore = (...args) => runtime.loadNavswapRunStore(...args);
    const loadPftlUniswapWalletActionContext = (...args) => runtime.loadPftlUniswapWalletActionContext(...args);
    const loadShieldedTopologyPeers = (...args) => runtime.loadShieldedTopologyPeers(...args);
    const majorityRootAtHeight = (...args) => runtime.majorityRootAtHeight(...args);
    const markStoredNavswapRunInterrupted = (...args) => runtime.markStoredNavswapRunInterrupted(...args);
    const maxMtimeUnixMs = (...args) => runtime.maxMtimeUnixMs(...args);
    const msSpan = (...args) => runtime.msSpan(...args);
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
    const navswapCompletionConsumerIds = (...args) => runtime.navswapCompletionConsumerIds(...args);
    const navswapCompletionOperationTemplate = (...args) => runtime.navswapCompletionOperationTemplate(...args);
    const navswapCompletionSubmittedChainId = (...args) => runtime.navswapCompletionSubmittedChainId(...args);
    const navswapCompletionSubmittedSequence = (...args) => runtime.navswapCompletionSubmittedSequence(...args);
    const navswapConsumerMatchesRecipient = (...args) => runtime.navswapConsumerMatchesRecipient(...args);
    const navswapDecimalAmountToAtoms = (...args) => runtime.navswapDecimalAmountToAtoms(...args);
    const navswapFreshnessFromBody = (...args) => runtime.navswapFreshnessFromBody(...args);
    const navswapFreshnessPayload = (...args) => runtime.navswapFreshnessPayload(...args);
    const navswapHashHexDomain = (...args) => runtime.navswapHashHexDomain(...args);
    const navswapIdempotencyHashBody = (...args) => runtime.navswapIdempotencyHashBody(...args);
    const navswapIdempotencyKeyFromRequest = (...args) => runtime.navswapIdempotencyKeyFromRequest(...args);
    const navswapIdempotencyStorePath = (...args) => runtime.navswapIdempotencyStorePath(...args);
    const navswapIdempotencyStoreSnapshot = (...args) => runtime.navswapIdempotencyStoreSnapshot(...args);
    const navswapListLimit = (...args) => runtime.navswapListLimit(...args);
    const navswapNativeAccountBalanceAtoms = (...args) => runtime.navswapNativeAccountBalanceAtoms(...args);
    const navswapNavProofStub = (...args) => runtime.navswapNavProofStub(...args);
    const navswapNavRedemptionId = (...args) => runtime.navswapNavRedemptionId(...args);
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
    const navswapRpcRead = (...args) => runtime.navswapRpcRead(...args);
    const navswapRunEvents = (...args) => runtime.navswapRunEvents(...args);
    const navswapRunIsTerminal = (...args) => runtime.navswapRunIsTerminal(...args);
    const navswapRunList = (...args) => runtime.navswapRunList(...args);
    const navswapRunPublic = (...args) => runtime.navswapRunPublic(...args);
    const navswapRunReceipts = (...args) => runtime.navswapRunReceipts(...args);
    const navswapRunSortTime = (...args) => runtime.navswapRunSortTime(...args);
    const navswapRunStorePath = (...args) => runtime.navswapRunStorePath(...args);
    const navswapRunStoreSnapshot = (...args) => runtime.navswapRunStoreSnapshot(...args);
    const navswapRunStreamSnapshot = (...args) => runtime.navswapRunStreamSnapshot(...args);
    const navswapSafeU64Number = (...args) => runtime.navswapSafeU64Number(...args);
    const navswapSettlementReceiptFreshnessConfig = (...args) => runtime.navswapSettlementReceiptFreshnessConfig(...args);
    const navswapSettlementReceiptHash = (...args) => runtime.navswapSettlementReceiptHash(...args);
    const navswapStableJson = (...args) => runtime.navswapStableJson(...args);
    const navswapSubscriptionId = (...args) => runtime.navswapSubscriptionId(...args);
    const navswapTruthyParam = (...args) => runtime.navswapTruthyParam(...args);
    const navswapValidateIdempotencyKey = (...args) => runtime.navswapValidateIdempotencyKey(...args);
    const navswapValuationUnitScale = (...args) => runtime.navswapValuationUnitScale(...args);
    const navswapWalletActionBatchItems = (...args) => runtime.navswapWalletActionBatchItems(...args);
    const navswapWalletActionId = (...args) => runtime.navswapWalletActionId(...args);
    const newNavswapRunId = (...args) => runtime.newNavswapRunId(...args);
    const normalizeAtomicTemplateParams = (...args) => runtime.normalizeAtomicTemplateParams(...args);
    const normalizeFastpayBroadcastRequest = (...args) => runtime.normalizeFastpayBroadcastRequest(...args);
    const normalizePftlUniswapPacketStatus = (...args) => runtime.normalizePftlUniswapPacketStatus(...args);
    const normalizeStoredNavswapIdempotencyRecord = (...args) => runtime.normalizeStoredNavswapIdempotencyRecord(...args);
    const normalizeStoredNavswapRun = (...args) => runtime.normalizeStoredNavswapRun(...args);
    const normalizeWalletSubscriptionParams = (...args) => runtime.normalizeWalletSubscriptionParams(...args);
    const originAllowed = (...args) => runtime.originAllowed(...args);
    const parseAtomicInteger = (...args) => runtime.parseAtomicInteger(...args);
    const parseNavswapActionInteger = (...args) => runtime.parseNavswapActionInteger(...args);
    const parseNavswapDisplayOrAtomAmount = (...args) => runtime.parseNavswapDisplayOrAtomAmount(...args);
    const parseNavswapEvmAddress = (...args) => runtime.parseNavswapEvmAddress(...args);
    const parseNavswapHexId = (...args) => runtime.parseNavswapHexId(...args);
    const parseNavswapWalletAddress = (...args) => runtime.parseNavswapWalletAddress(...args);
    const parseRpcFleet = (...args) => runtime.parseRpcFleet(...args);
    const parseShieldedPrivateEgressJson = (...args) => runtime.parseShieldedPrivateEgressJson(...args);
    const parseShieldedSwapActionJson = (...args) => runtime.parseShieldedSwapActionJson(...args);
    const parseStakehubTransparentAmount = (...args) => runtime.parseStakehubTransparentAmount(...args);
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
    const primeNextProposerRouteCache = (...args) => runtime.primeNextProposerRouteCache(...args);
    const primeNextProposerRouteCacheFromResponse = (...args) => runtime.primeNextProposerRouteCacheFromResponse(...args);
    const proposerEndpointForHeight = (...args) => runtime.proposerEndpointForHeight(...args);
    const pruneNavswapIdempotencyRecords = (...args) => runtime.pruneNavswapIdempotencyRecords(...args);
    const publishNavswapRunUpdate = (...args) => runtime.publishNavswapRunUpdate(...args);
    const readFleetRpcMajority = (...args) => runtime.readFleetRpcMajority(...args);
    const readGroupKey = (...args) => runtime.readGroupKey(...args);
    const readJsonBody = (...args) => runtime.readJsonBody(...args);
    const recordNavswapRunEvent = (...args) => runtime.recordNavswapRunEvent(...args);
    const rememberFinalizedReadEndpoint = (...args) => runtime.rememberFinalizedReadEndpoint(...args);
    const removeNavswapRunStreamSubscriber = (...args) => runtime.removeNavswapRunStreamSubscriber(...args);
    const requestWithProxyReadiness = (...args) => runtime.requestWithProxyReadiness(...args);
    const resolveRpcTarget = (...args) => runtime.resolveRpcTarget(...args);
    const responseEnvelope = (...args) => runtime.responseEnvelope(...args);
    const rpcTcpRequest = (...args) => runtime.rpcTcpRequest(...args);
    const rpcTcpRequestLine = (...args) => runtime.rpcTcpRequestLine(...args);
    const rpcTcpRequestOneShotLine = (...args) => runtime.rpcTcpRequestOneShotLine(...args);
    const runShieldedLaggardCatchUp = (...args) => runtime.runShieldedLaggardCatchUp(...args);
    const runShieldedRpcCatchUp = (...args) => runtime.runShieldedRpcCatchUp(...args);
    const sanitizeNavswapRunRequest = (...args) => runtime.sanitizeNavswapRunRequest(...args);
    const selectNavswapIssuedSettlementSource = (...args) => runtime.selectNavswapIssuedSettlementSource(...args);
    const selectTransparentRedeemSettlementAllocation = (...args) => runtime.selectTransparentRedeemSettlementAllocation(...args);
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
    const shieldedPrivateEgressDisclosureFields = (...args) => runtime.shieldedPrivateEgressDisclosureFields(...args);
    const shieldedPrivateEgressDisclosureHash = (...args) => runtime.shieldedPrivateEgressDisclosureHash(...args);
    const shieldedQuoteAssetByInput = (...args) => runtime.shieldedQuoteAssetByInput(...args);
    const shieldedQuoteFromSubmitBody = (...args) => runtime.shieldedQuoteFromSubmitBody(...args);
    const shieldedQuotePairEnabled = (...args) => runtime.shieldedQuotePairEnabled(...args);
    const shieldedRemoteDataDir = (...args) => runtime.shieldedRemoteDataDir(...args);
    const shieldedRemoteWorkDir = (...args) => runtime.shieldedRemoteWorkDir(...args);
    const shieldedRoundBatchIds = (...args) => runtime.shieldedRoundBatchIds(...args);
    const shieldedRoundPhaseTimings = (...args) => runtime.shieldedRoundPhaseTimings(...args);
    const shieldedRoundReceiptIds = (...args) => runtime.shieldedRoundReceiptIds(...args);
    const shieldedSwapProxyTimingReport = (...args) => runtime.shieldedSwapProxyTimingReport(...args);
    const shouldUseFirstReadySequencedRead = (...args) => runtime.shouldUseFirstReadySequencedRead(...args);
    const signAndSubmitNavswapOperatorAssetTransaction = (...args) => runtime.signAndSubmitNavswapOperatorAssetTransaction(...args);
    const signWalletOwnedOrder = (...args) => runtime.signWalletOwnedOrder(...args);
    const sleep = (...args) => runtime.sleep(...args);
    const sseHeaders = (...args) => runtime.sseHeaders(...args);
    const stakehubTransparentAmountError = (...args) => runtime.stakehubTransparentAmountError(...args);
    const startCachedSelectionReadinessProbe = (...args) => runtime.startCachedSelectionReadinessProbe(...args);
    const startShieldedCertifierLoop = (...args) => runtime.startShieldedCertifierLoop(...args);
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
    const validateShieldedCertifierLoopReportForBatch = (...args) => runtime.validateShieldedCertifierLoopReportForBatch(...args);
    const validateShieldedEgressSubmit = (...args) => runtime.validateShieldedEgressSubmit(...args);
    const validateShieldedIngressPayload = (...args) => runtime.validateShieldedIngressPayload(...args);
    const validateShieldedPrivateEgressFile = (...args) => runtime.validateShieldedPrivateEgressFile(...args);
    const validateShieldedSwapAction = (...args) => runtime.validateShieldedSwapAction(...args);
    const validateShieldedSwapSubmit = (...args) => runtime.validateShieldedSwapSubmit(...args);
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

    function lower(value) {
        return String(value || '').toLowerCase();
    }

    function presentEnv(name) {
        const value = process.env[name];
        if (!value || !String(value).trim()) return null;
        return String(value).trim();
    }

    function presentPositiveSafeIntegerEnv(name) {
        const value = presentEnv(name);
        if (!value || !/^[1-9][0-9]*$/.test(value)) return null;
        const parsed = Number.parseInt(value, 10);
        if (!Number.isSafeInteger(parsed) || parsed <= 0) return null;
        return parsed;
    }

    function navswapNormalizeTrustClass(value, fallback = 'CONTROLLED') {
        const text = String(value || fallback || '').trim().toUpperCase();
        if (NAVSWAP_ROUTE_TRUST_CLASSES.has(text)) return text;
        return fallback;
    }

    function navswapInferTrustClass(verifierMode) {
        const mode = lower(verifierMode);
        if (!mode) return 'DISABLED';
        if (mode.includes('trustless') || mode.includes('succinct') || mode.includes('direct-finality')) {
            return 'TRUSTLESS_FINALITY';
        }
        if (mode.includes('optimistic')) return 'OPTIMISTIC';
        return 'CONTROLLED';
    }

    function navswapTrustlessFinalityAgreement(requestedTrustClass) {
        const routeRegistry = navswapNormalizeTrustClass(
            presentEnv('NAVSWAP_ROUTE_REGISTRY_TRUST_CLASS'),
            'DISABLED',
        );
        const ethereumController = navswapNormalizeTrustClass(
            presentEnv('NAVSWAP_ETHEREUM_CONTROLLER_TRUST_CLASS')
                || presentEnv('NAVSWAP_CONTROLLER_TRUST_CLASS'),
            'DISABLED',
        );
        const configDigest = navswapNormalizeTrustClass(
            presentEnv('NAVSWAP_CONFIG_DIGEST_TRUST_CLASS'),
            'DISABLED',
        );
        const requestedFinality = requestedTrustClass === 'TRUSTLESS_FINALITY';
        const agreed = routeRegistry === 'TRUSTLESS_FINALITY'
            && ethereumController === 'TRUSTLESS_FINALITY'
            && configDigest === 'TRUSTLESS_FINALITY';
        return {
            required_components: ['pftl_route_registry', 'ethereum_controller', 'config_digest'],
            requested_finality: requestedFinality,
            display_allowed: !requestedFinality || agreed,
            status: requestedFinality ? (agreed ? 'agreed' : 'incomplete') : 'not_requested',
        };
    }

    function navswapBridgeConfig() {
        const routeId = presentEnv('NAVSWAP_ROUTE_ID') || 'pftl-navcoin-uniswap-v1';
        const nativeNavAssetId = presentEnv('NAVSWAP_NATIVE_NAV_ASSET_ID')
            || presentEnv('NAVSWAP_NAV_ASSET_ID');
        const settlementAssetId = presentEnv('NAVSWAP_SETTLEMENT_ASSET_ID') || PFUSDC_ASSET_ID;
        const wrappedToken = presentEnv('NAVSWAP_WRAPPED_NAVCOIN_TOKEN');
        const handoffController = presentEnv('NAVSWAP_HANDOFF_CONTROLLER');
        const verifierMode = presentEnv('NAVSWAP_VERIFIER_MODE');
        const requestedTrustClass = navswapNormalizeTrustClass(
            presentEnv('NAVSWAP_ROUTE_TRUST_CLASS'),
            navswapInferTrustClass(verifierMode),
        );
        const finalityAgreement = navswapTrustlessFinalityAgreement(requestedTrustClass);
        const trustClass = finalityAgreement.display_allowed ? requestedTrustClass : 'DISABLED';
        const displayedVerifierMode = finalityAgreement.display_allowed ? verifierMode : 'finality_pending';
        const poolIdOrPath = presentEnv('NAVSWAP_UNISWAP_POOL_ID') || presentEnv('NAVSWAP_UNISWAP_POOL_PATH');
        const settlementAdapter = presentEnv('NAVSWAP_SETTLEMENT_ADAPTER')
            || presentEnv('NAVSWAP_UNISWAP_SETTLEMENT_ADAPTER');
        const router = presentEnv('NAVSWAP_UNISWAP_ROUTER');
        const uniswapOutputToken = presentEnv('NAVSWAP_UNISWAP_OUTPUT_TOKEN') || ETHEREUM_USDC_TOKEN;
        const defaultEthereumRecipient = presentEnv('NAVSWAP_UNISWAP_DEFAULT_RECIPIENT')
            || presentEnv('NAVSWAP_DEFAULT_ETHEREUM_RECIPIENT')
            || null;
        const failureBehavior = presentEnv('NAVSWAP_FAILURE_BEHAVIOR') || 'refund_unconsumed_pftl_packet';
        const routeSupplyCapAtoms = presentPositiveSafeIntegerEnv('NAVSWAP_ROUTE_SUPPLY_CAP_ATOMS');
        const supplyCapRemainingAtoms = presentPositiveSafeIntegerEnv('NAVSWAP_SUPPLY_CAP_REMAINING_ATOMS')
            || routeSupplyCapAtoms;
        const packetNotionalCapAtoms = presentPositiveSafeIntegerEnv('NAVSWAP_PACKET_NOTIONAL_CAP_ATOMS');
        const seedNavEpoch = presentPositiveSafeIntegerEnv('NAVSWAP_SEED_NAV_EPOCH');
        const seedUsdcAtoms = presentPositiveSafeIntegerEnv('NAVSWAP_SEED_USDC_ATOMS');
        const seedWrappedNavcoinAtoms = presentPositiveSafeIntegerEnv('NAVSWAP_SEED_WRAPPED_NAVCOIN_ATOMS');
        const lpRecipient = presentEnv('NAVSWAP_LP_RECIPIENT');
        const lpCustodyPolicy = presentEnv('NAVSWAP_LP_CUSTODY_POLICY');
        const nodeRouteConfigDigest = presentEnv('NAVSWAP_ROUTE_CONFIG_DIGEST')
            || presentEnv('NAVSWAP_NODE_ROUTE_CONFIG_DIGEST');
        const nodeLaunchConfigDigest = presentEnv('NAVSWAP_LAUNCH_CONFIG_DIGEST')
            || presentEnv('NAVSWAP_NODE_LAUNCH_CONFIG_DIGEST');
        const legacyPoolSelected = lower(poolIdOrPath) === lower(LEGACY_A651_UNISWAP_POOL_ID)
            || lower(wrappedToken) === lower(LEGACY_A651_ETH_TOKEN);
        const explicitBeta = ['1', 'true', 'yes'].includes(lower(process.env.NAVSWAP_ENABLE_UNISWAP_BETA_ROUTE));
        const betaRunsEnabled = explicitBeta
            && ['1', 'true', 'yes'].includes(lower(process.env.NAVSWAP_ENABLE_UNISWAP_BETA_RUNS));
        const routePaused = ['1', 'true', 'yes'].includes(lower(process.env.NAVSWAP_UNISWAP_ROUTE_PAUSED));
        const publicRoutingEnabled = ['1', 'true', 'yes'].includes(lower(process.env.NAVSWAP_UNISWAP_PUBLIC_ROUTING_ENABLED));
        const missing = [];
        if (!nativeNavAssetId || !/^[0-9a-f]{96}$/i.test(nativeNavAssetId)) missing.push('NAVSWAP_NATIVE_NAV_ASSET_ID');
        if (!settlementAssetId || !/^[0-9a-f]{96}$/i.test(settlementAssetId)) missing.push('NAVSWAP_SETTLEMENT_ASSET_ID');
        if (!wrappedToken) missing.push('NAVSWAP_WRAPPED_NAVCOIN_TOKEN');
        if (!handoffController) missing.push('NAVSWAP_HANDOFF_CONTROLLER');
        if (!settlementAdapter) missing.push('NAVSWAP_SETTLEMENT_ADAPTER');
        if (!verifierMode) missing.push('NAVSWAP_VERIFIER_MODE');
        if (!poolIdOrPath) missing.push('NAVSWAP_UNISWAP_POOL_ID');
        if (!router) missing.push('NAVSWAP_UNISWAP_ROUTER');
        if (!routeSupplyCapAtoms) missing.push('NAVSWAP_ROUTE_SUPPLY_CAP_ATOMS');
        if (!packetNotionalCapAtoms) missing.push('NAVSWAP_PACKET_NOTIONAL_CAP_ATOMS');
        if (!seedNavEpoch) missing.push('NAVSWAP_SEED_NAV_EPOCH');
        if (!seedUsdcAtoms) missing.push('NAVSWAP_SEED_USDC_ATOMS');
        if (!seedWrappedNavcoinAtoms) missing.push('NAVSWAP_SEED_WRAPPED_NAVCOIN_ATOMS');
        if (!lpRecipient) missing.push('NAVSWAP_LP_RECIPIENT');
        if (!lpCustodyPolicy) missing.push('NAVSWAP_LP_CUSTODY_POLICY');
        if (!nodeRouteConfigDigest || !/^[0-9a-f]{96}$/i.test(nodeRouteConfigDigest)) {
            missing.push('NAVSWAP_ROUTE_CONFIG_DIGEST');
        }
        if (nodeLaunchConfigDigest && !/^[0-9a-f]{96}$/i.test(nodeLaunchConfigDigest)) {
            missing.push('NAVSWAP_LAUNCH_CONFIG_DIGEST');
        }
        if (legacyPoolSelected) missing.push('bridge-aware token/pool must not be the legacy a651/USDC pool');
        if (!finalityAgreement.display_allowed) {
            missing.push('finality agreement missing across registry, controller, and config digest');
        }
        const configured = missing.length === 0;
        const routeConfig = configured ? {
            schema: 'postfiat-pftl-uniswap-route-config-v1',
            route_id: routeId,
            route_family: 'primary_pftl_mint',
            native_nav_asset_id: nativeNavAssetId.toLowerCase(),
            settlement_asset_id: settlementAssetId.toLowerCase(),
            wrapped_navcoin_token: wrappedToken,
            handoff_controller: handoffController,
            settlement_adapter: settlementAdapter,
            verifier_mode: displayedVerifierMode,
            route_trust_class: trustClass,
            uniswap_pool_id_or_path: poolIdOrPath,
            router,
            failure_behavior: failureBehavior,
            route_supply_cap_atoms: routeSupplyCapAtoms,
            packet_notional_cap_atoms: packetNotionalCapAtoms,
            seed_nav_epoch: seedNavEpoch,
            seed_usdc_atoms: seedUsdcAtoms,
            seed_wrapped_navcoin_atoms: seedWrappedNavcoinAtoms,
            lp_recipient: lpRecipient,
            lp_custody_policy: lpCustodyPolicy,
        } : null;
        const routeConfigDigest = configured ? nodeRouteConfigDigest.toLowerCase() : null;
        const launchConfigDigest = nodeLaunchConfigDigest && /^[0-9a-f]{96}$/i.test(nodeLaunchConfigDigest)
            ? nodeLaunchConfigDigest.toLowerCase()
            : null;

        return {
            route_id: routeId,
            native_nav_asset_id: nativeNavAssetId || null,
            settlement_asset_id: settlementAssetId,
            wrapped_navcoin_token: wrappedToken,
            handoff_controller: handoffController,
            settlement_adapter: settlementAdapter,
            verifier_mode: displayedVerifierMode,
            route_trust_class: trustClass,
            finality_agreement: finalityAgreement,
            route_config: routeConfig,
            route_config_digest: routeConfigDigest,
            launch_config_digest: launchConfigDigest,
            route_config_digest_authority: routeConfigDigest ? 'node' : null,
            uniswap_pool_id_or_path: poolIdOrPath,
            router,
            uniswap_output_token: uniswapOutputToken,
            default_ethereum_recipient: defaultEthereumRecipient,
            failure_behavior: failureBehavior,
            route_supply_cap_atoms: routeSupplyCapAtoms,
            supply_cap_remaining_atoms: supplyCapRemainingAtoms,
            packet_notional_cap_atoms: packetNotionalCapAtoms,
            seed_nav_epoch: seedNavEpoch,
            seed_usdc_atoms: seedUsdcAtoms,
            seed_wrapped_navcoin_atoms: seedWrappedNavcoinAtoms,
            lp_recipient: lpRecipient,
            lp_custody_policy: lpCustodyPolicy,
            configured,
            explicit_beta: explicitBeta,
            beta_runs_enabled: betaRunsEnabled,
            paused: routePaused,
            public_routing_enabled: publicRoutingEnabled,
            missing,
            legacy_pool_rejected: true,
            legacy_pool_selected: legacyPoolSelected,
            legacy_a651_token: LEGACY_A651_ETH_TOKEN,
            legacy_a651_uniswap_pool_id: LEGACY_A651_UNISWAP_POOL_ID,
            ethereum_usdc_token: ETHEREUM_USDC_TOKEN,
            max_live_usd: Number.isFinite(NAVSWAP_MAX_LIVE_USD) ? NAVSWAP_MAX_LIVE_USD : 100,
        };
    }

    function navswapUniswapBetaRouteState(bridge) {
        const blockers = [];
        if (!bridge.configured) blockers.push('bridge-aware pool config missing');
        if (!bridge.explicit_beta) blockers.push('NAVSWAP_ENABLE_UNISWAP_BETA_ROUTE not enabled');
        if (bridge.route_trust_class !== 'CONTROLLED') blockers.push('route trust class must be CONTROLLED for beta');
        if (bridge.finality_agreement && bridge.finality_agreement.display_allowed === false) {
            blockers.push('finality agreement missing across registry, controller, and config digest');
        }
        if (bridge.paused) blockers.push('route is paused');
        if (bridge.public_routing_enabled) blockers.push('public routing flag must be false for beta');
        if (bridge.legacy_pool_selected) blockers.push('legacy a651 pool/token selected');
        if (!bridge.route_supply_cap_atoms) blockers.push('route supply cap missing');
        if (!bridge.supply_cap_remaining_atoms) blockers.push('remaining route cap missing');
        if (!bridge.packet_notional_cap_atoms) blockers.push('packet notional cap missing');
        const quoteEnabled = blockers.length === 0;
        return {
            quote_enabled: quoteEnabled,
            run_enabled: quoteEnabled && bridge.beta_runs_enabled,
            status: quoteEnabled
                ? (bridge.beta_runs_enabled ? 'controlled_beta_run_ready' : 'controlled_beta_quote_ready')
                : (bridge.configured ? 'configured_beta_disabled' : 'disabled_missing_bridge_aware_pool'),
            blockers,
        };
    }

    function parseUniswapHandoffPositiveInteger(value, field) {
        const text = String(value ?? '').trim();
        if (!/^[1-9][0-9]*$/.test(text)) {
            const err = new Error(`${field} must be a positive integer`);
            err.code = 'invalid_uniswap_handoff_integer';
            throw err;
        }
        return text;
    }

    function parseUniswapHandoffBytes32(value, field) {
        const text = String(value ?? '').trim();
        const unprefixed = text.toLowerCase().startsWith('0x') ? text.slice(2) : text;
        if (!/^[0-9a-f]{64}$/.test(unprefixed)) {
            const err = new Error(`${field} must be a 32-byte hex value`);
            err.code = 'invalid_uniswap_handoff_bytes32';
            throw err;
        }
        return unprefixed;
    }

    function buildUniswapHandoffQuoteBinding({ body, bridge, fromAsset, toAsset, amount, requestedPool }) {
        const missing = [];
        const recipient = body.ethereum_recipient || body.recipient || body.destination || null;
        const minOutput = body.minimum_output_atoms || body.minimum_output || body.min_output_atoms || body.min_output || null;
        const deadline = body.deadline || body.deadline_seconds || body.expiry || null;
        const swapPathHash = body.swap_path_hash || body.path_hash || body.router_data_hash || null;
        if (!recipient) missing.push('recipient');
        if (!minOutput) missing.push('minimum_output');
        if (!deadline) missing.push('deadline');
        if (!swapPathHash) missing.push('swap_path_hash');
        if (amount === undefined || amount === null || amount === '') missing.push('amount');
        if (missing.length > 0) {
            return {
                ok: false,
                code: 'uniswap_handoff_quote_fields_required',
                message: `Uniswap handoff quotes require ${missing.join(', ')} before any PFTL debit can be signed.`,
                missing,
            };
        }

        let amountIn;
        let minimumOutput;
        let deadlineValue;
        let normalizedSwapPathHash;
        try {
            amountIn = parseUniswapHandoffPositiveInteger(amount, 'amount');
            minimumOutput = parseUniswapHandoffPositiveInteger(minOutput, 'minimum_output');
            deadlineValue = parseUniswapHandoffPositiveInteger(deadline, 'deadline');
            normalizedSwapPathHash = parseUniswapHandoffBytes32(swapPathHash, 'swap_path_hash');
        } catch (error) {
            return {
                ok: false,
                code: error.code || 'invalid_uniswap_handoff_quote',
                message: error.message || 'invalid Uniswap handoff quote field',
            };
        }

        const failureBehavior = String(body.failure_behavior || body.failureBehavior || 'refund_unconsumed_pftl_packet');
        const tokenIn = bridge.wrapped_navcoin_token;
        const tokenOut = body.token_out || body.output_token || bridge.uniswap_output_token;
        const binding = {
            schema: 'postfiat-navswap-mint-and-swap-uniswap-quote-v1',
            operation: 'mint_and_swap_uniswap',
            route_family: 'composite_primary_mint_to_ethereum_venue',
            route_trust_class: bridge.route_trust_class,
            route_config_digest: bridge.route_config_digest,
            native_nav_asset_id: bridge.native_nav_asset_id,
            settlement_asset_id: bridge.settlement_asset_id,
            swap_path_hash: normalizedSwapPathHash,
            source_chain: 'PFTL',
            destination_chain: 'ethereum',
            handoff_controller: bridge.handoff_controller,
            settlement_adapter: bridge.settlement_adapter,
            verifier_mode: bridge.verifier_mode,
            pool_id_or_path: requestedPool,
            router: bridge.router,
            token_in: tokenIn,
            token_out: tokenOut,
            pftl_source_asset: fromAsset || null,
            pftl_destination_asset: toAsset || null,
            amount_in: amountIn,
            minimum_output: minimumOutput,
            recipient: String(recipient),
            deadline: deadlineValue,
            failure_behavior: failureBehavior,
            execution_enabled: false,
        };
        return {
            ok: true,
            binding,
            binding_hash: crypto.createHash('sha256').update(navswapStableJson(binding)).digest('hex'),
        };
    }

    function navswapStakehubTransparentConfig() {
        const baseUrl = presentEnv('NAVSWAP_STAKEHUB_BASE_URL') || presentEnv('NAVSWAP_STAKEHUB_URL');
        const actionPath = presentEnv('NAVSWAP_STAKEHUB_ACTION_PATH') || '/api/shielded-nav-swap/action';
        const navcoinPath = presentEnv('NAVSWAP_STAKEHUB_NAVCOIN_PATH') || '/api/navcoin';
        const navcoinStatusPath = presentEnv('NAVSWAP_STAKEHUB_NAVCOIN_STATUS_PATH') || '/api/navcoin/status';
        const balancesPath = presentEnv('NAVSWAP_STAKEHUB_BALANCES_PATH') || '/api/shielded-nav-swap/balances';
        const swapStatusPath = presentEnv('NAVSWAP_STAKEHUB_SWAP_STATUS_PATH') || '/api/shielded-nav-swap/status';
        const runsEnabled = ['1', 'true', 'yes'].includes(lower(process.env.NAVSWAP_ENABLE_STAKEHUB_TRANSPARENT_RUNS));
        const maxWholeA651 = Number.parseInt(process.env.NAVSWAP_STAKEHUB_MAX_A651_AMOUNT || '1', 10);
        const timeoutMs = Number.parseInt(process.env.NAVSWAP_STAKEHUB_TIMEOUT_MS || '600000', 10);
        const readTimeoutMs = Number.parseInt(process.env.NAVSWAP_STAKEHUB_READ_TIMEOUT_MS || '15000', 10);
        return {
            configured: Boolean(baseUrl),
            base_url: baseUrl,
            action_path: actionPath.startsWith('/') ? actionPath : `/${actionPath}`,
            navcoin_path: navcoinPath.startsWith('/') ? navcoinPath : `/${navcoinPath}`,
            navcoin_status_path: navcoinStatusPath.startsWith('/') ? navcoinStatusPath : `/${navcoinStatusPath}`,
            balances_path: balancesPath.startsWith('/') ? balancesPath : `/${balancesPath}`,
            swap_status_path: swapStatusPath.startsWith('/') ? swapStatusPath : `/${swapStatusPath}`,
            runs_enabled: runsEnabled,
            max_whole_a651_amount: Number.isInteger(maxWholeA651) && maxWholeA651 > 0 ? maxWholeA651 : 1,
            timeout_ms: Number.isInteger(timeoutMs) && timeoutMs > 0 ? timeoutMs : 600000,
            read_timeout_ms: Number.isInteger(readTimeoutMs) && readTimeoutMs > 0 ? readTimeoutMs : 15000,
            action: NAVSWAP_STAKEHUB_TRANSPARENT_ACTION,
            custody_boundary: 'stakehub-operator-wallet',
            amount_semantics: 'positive decimal a651 amount',
        };
    }

    function navswapTransparentOperatorConfig() {
        const issuerKeyFile = presentEnv('NAVSWAP_OPERATOR_ISSUER_KEY_FILE')
            || presentEnv('NAVSWAP_ISSUER_KEY_FILE')
            || presentEnv('ISSUER_KEY_FILE');
        const nodeBin = presentEnv('NAVSWAP_OPERATOR_NODE_BIN')
            || presentEnv('POSTFIAT_NODE_BIN')
            || path.resolve(__dirname, '..', 'target', 'release', 'postfiat-node');
        const timeoutMs = Number.parseInt(process.env.NAVSWAP_OPERATOR_SIGN_TIMEOUT_MS || '30000', 10);
        return {
            issuer_key_file: issuerKeyFile,
            node_bin: nodeBin,
            signing_configured: Boolean(issuerKeyFile),
            timeout_ms: Number.isInteger(timeoutMs) && timeoutMs > 0 ? timeoutMs : 30000,
            custody_boundary: 'operator-issuer-key-file',
        };
    }

    function navswapDevnetPfusdcFundingConfig() {
        const enabled = process.env.NAVSWAP_ENABLE_DEVNET_PFUSDC_FUNDING === 'true';
        const issuerKeyFile = presentEnv('NAVSWAP_PFUSDC_ISSUER_KEY_FILE')
            || presentEnv('NAVSWAP_OPERATOR_ISSUER_KEY_FILE')
            || presentEnv('NAVSWAP_ISSUER_KEY_FILE')
            || presentEnv('ISSUER_KEY_FILE');
        const nodeBin = presentEnv('NAVSWAP_OPERATOR_NODE_BIN')
            || presentEnv('POSTFIAT_NODE_BIN')
            || path.resolve(__dirname, '..', 'target', 'release', 'postfiat-node');
        const timeoutMs = Number.parseInt(process.env.NAVSWAP_FUNDING_SIGN_TIMEOUT_MS || '30000', 10);
        const maxAmountAtoms = String(process.env.NAVSWAP_DEVNET_PFUSDC_FUNDING_MAX_ATOMS || '10000000');
        const normalizedMaxAmountAtoms = /^[1-9][0-9]*$/.test(maxAmountAtoms) ? maxAmountAtoms : '10000000';
        const maxRecipientWindowAtoms = String(
            process.env.NAVSWAP_DEVNET_PFUSDC_FUNDING_RECIPIENT_WINDOW_MAX_ATOMS || normalizedMaxAmountAtoms,
        );
        const normalizedMaxRecipientWindowAtoms = /^[1-9][0-9]*$/.test(maxRecipientWindowAtoms)
            ? maxRecipientWindowAtoms
            : normalizedMaxAmountAtoms;
        const recipientWindowMs = Number.parseInt(process.env.NAVSWAP_DEVNET_PFUSDC_FUNDING_RECIPIENT_WINDOW_MS || '86400000', 10);
        return {
            enabled,
            issuer_key_file: issuerKeyFile,
            signing_configured: Boolean(issuerKeyFile),
            node_bin: nodeBin,
            timeout_ms: Number.isInteger(timeoutMs) && timeoutMs > 0 ? timeoutMs : 30000,
            max_amount_atoms: normalizedMaxAmountAtoms,
            max_recipient_window_atoms: normalizedMaxRecipientWindowAtoms,
            recipient_window_ms: Number.isInteger(recipientWindowMs) && recipientWindowMs > 0 ? recipientWindowMs : 86400000,
            endpoint: '/api/navswap/devnet-fund-pfusdc',
            asset_id: PFUSDC_ASSET_ID,
            custody_boundary: 'operator-issuer-key-file-devnet-funding',
        };
    }

    function shieldedNavswapIngressConfig() {
        const nodeBin = presentEnv('NAVSWAP_SHIELDED_INGRESS_NODE_BIN')
            || presentEnv('NAVSWAP_OPERATOR_NODE_BIN')
            || presentEnv('POSTFIAT_NODE_BIN')
            || path.resolve(__dirname, '..', 'target', 'release', 'postfiat-node');
        const dataDir = presentEnv('NAVSWAP_SHIELDED_INGRESS_DATA_DIR')
            || presentEnv('POSTFIAT_DATA_DIR')
            || presentEnv('PFTL_DATA_DIR');
        const topology = presentEnv('NAVSWAP_SHIELDED_INGRESS_TOPOLOGY')
            || presentEnv('POSTFIAT_TOPOLOGY')
            || presentEnv('PFTL_TOPOLOGY');
        const keyFile = presentEnv('NAVSWAP_SHIELDED_INGRESS_KEY_FILE')
            || presentEnv('NAVSWAP_VALIDATOR_KEY_FILE')
            || presentEnv('POSTFIAT_VALIDATOR_KEY_FILE');
        const proposalKeyFile = presentEnv('NAVSWAP_SHIELDED_INGRESS_PROPOSAL_KEY_FILE')
            || presentEnv('POSTFIAT_PROPOSAL_KEY_FILE');
        const artifactRoot = presentEnv('NAVSWAP_SHIELDED_INGRESS_ARTIFACT_ROOT')
            || path.join(os.homedir(), '.local', 'share', 'postfiat', 'wallet-proxy', 'shielded-ingress');
        const enabled = ['1', 'true', 'yes'].includes(lower(process.env.NAVSWAP_ENABLE_SHIELDED_INGRESS));
        const timeoutMs = Number.parseInt(
            process.env.NAVSWAP_SHIELDED_INGRESS_TIMEOUT_MS || String(SHIELDED_ROUND_TIMEOUT_DEFAULT_MS),
            10,
        );
        const maxAmountAtoms = String(process.env.NAVSWAP_SHIELDED_INGRESS_MAX_AMOUNT_ATOMS || '1000000');
        const a652AssetId = currentA652AssetId();
        const supportedAssets = [
            { symbol: 'a651', asset_id: A651_ASSET_ID, precision: 6, supported: true },
            ...(a652AssetId ? [{ symbol: 'a652', asset_id: a652AssetId, precision: 6, supported: true }] : []),
        ];
        const missing = [];
        if (!enabled) missing.push('NAVSWAP_ENABLE_SHIELDED_INGRESS=true');
        if (!dataDir) missing.push('NAVSWAP_SHIELDED_INGRESS_DATA_DIR or POSTFIAT_DATA_DIR');
        if (!topology) missing.push('NAVSWAP_SHIELDED_INGRESS_TOPOLOGY or POSTFIAT_TOPOLOGY');
        if (!keyFile) missing.push('NAVSWAP_SHIELDED_INGRESS_KEY_FILE or POSTFIAT_VALIDATOR_KEY_FILE');
        if (!fs.existsSync(nodeBin)) missing.push(`postfiat-node at ${nodeBin}`);
        if (keyFile && !fs.existsSync(keyFile)) missing.push(`key file at ${keyFile}`);
        if (proposalKeyFile && !fs.existsSync(proposalKeyFile)) missing.push(`proposal key file at ${proposalKeyFile}`);
        return {
            enabled,
            configured: missing.length === 0,
            missing,
            node_bin: nodeBin,
            data_dir: dataDir,
            topology,
            key_file: keyFile,
            proposal_key_file: proposalKeyFile,
            artifact_root: artifactRoot,
            timeout_ms: Number.isInteger(timeoutMs) && timeoutMs > 0 ? timeoutMs : SHIELDED_ROUND_TIMEOUT_DEFAULT_MS,
            max_amount_atoms: /^[1-9][0-9]*$/.test(maxAmountAtoms) ? maxAmountAtoms : '1000000',
            endpoint: '/api/shielded-nav-swap/ingress',
            custody_boundary: 'wallet-local-note-and-burn-signing',
            supported_assets: supportedAssets,
        };
    }

    function shieldedNavswapSwapConfig() {
        const ingress = shieldedNavswapIngressConfig();
        const localService = assetOrchardLocalServiceConfig();
        const enabled = ['1', 'true', 'yes'].includes(lower(process.env.NAVSWAP_ENABLE_SHIELDED_SWAPS));
        const artifactRoot = presentEnv('NAVSWAP_SHIELDED_SWAP_ARTIFACT_ROOT')
            || path.join(os.homedir(), '.local', 'share', 'postfiat', 'wallet-proxy', 'shielded-swaps');
        const certifierLoopEnabled = ['1', 'true', 'yes'].includes(lower(
            process.env.NAVSWAP_SHIELDED_CERTIFIER_LOOP
                || process.env.NAVSWAP_SHIELDED_SWAP_CERTIFIER_LOOP
                || '',
        ));
        const certifierLoopRoot = presentEnv('NAVSWAP_SHIELDED_CERTIFIER_LOOP_ROOT')
            || path.join(artifactRoot, 'certifier-loop');
        const certifierLoop = {
            enabled: certifierLoopEnabled,
            batch_dir: presentEnv('NAVSWAP_SHIELDED_CERTIFIER_LOOP_BATCH_DIR')
                || path.join(certifierLoopRoot, 'batches'),
            artifact_root: presentEnv('NAVSWAP_SHIELDED_CERTIFIER_LOOP_ARTIFACT_ROOT')
                || path.join(certifierLoopRoot, 'artifacts'),
            processed_dir: presentEnv('NAVSWAP_SHIELDED_CERTIFIER_LOOP_PROCESSED_DIR')
                || path.join(certifierLoopRoot, 'processed'),
            ready_file: presentEnv('NAVSWAP_SHIELDED_CERTIFIER_READY_FILE')
                || path.join(certifierLoopRoot, 'ready.json'),
            report_file: presentEnv('NAVSWAP_SHIELDED_CERTIFIER_REPORT_FILE')
                || path.join(certifierLoopRoot, 'loop-report.json'),
            start_height: presentEnv('NAVSWAP_SHIELDED_CERTIFIER_LOOP_START_HEIGHT'),
            poll_ms: Number.parseInt(process.env.NAVSWAP_SHIELDED_CERTIFIER_LOOP_POLL_MS || '250', 10),
        };
        const missing = [...ingress.missing];
        if (!enabled) missing.push('NAVSWAP_ENABLE_SHIELDED_SWAPS=true');
        missing.push(...localService.missing);
        return {
            ...ingress,
            asset_orchard_local_service: localService,
            enabled,
            configured: enabled && ingress.configured && localService.local_only,
            missing,
            artifact_root: artifactRoot,
            certifier_loop: certifierLoop,
            endpoint: '/api/shielded-nav-swap/swap',
            custody_boundary: 'wallet-local-swap-proof-proxy-certified-relay',
        };
    }

    function shieldedNavswapEgressConfig() {
        const ingress = shieldedNavswapIngressConfig();
        const enabled = ['1', 'true', 'yes'].includes(lower(
            process.env.NAVSWAP_ENABLE_SHIELDED_EGRESS
                || process.env.NAVSWAP_ENABLE_SHIELDED_SWAPS
                || '',
        ));
        const artifactRoot = presentEnv('NAVSWAP_SHIELDED_EGRESS_ARTIFACT_ROOT')
            || path.join(os.homedir(), '.local', 'share', 'postfiat', 'wallet-proxy', 'shielded-egress');
        const policyId = presentEnv('NAVSWAP_SHIELDED_EGRESS_POLICY_ID') || SHIELDED_NAVSWAP_EGRESS_POLICY_ID;
        const missing = [...ingress.missing];
        if (!enabled) missing.push('NAVSWAP_ENABLE_SHIELDED_EGRESS=true');
        if (!policyId) missing.push('NAVSWAP_SHIELDED_EGRESS_POLICY_ID');
        return {
            ...ingress,
            enabled,
            configured: enabled && ingress.configured && Boolean(policyId),
            missing,
            artifact_root: artifactRoot,
            endpoint: '/api/shielded-nav-swap/egress',
            custody_boundary: 'wallet-local-private-egress-proof-proxy-certified-public-exit',
            policy_id: policyId,
            disclosure_required: true,
            bridge_out_requires_public_exit_receipt: true,
        };
    }

    function currentA652AssetId() {
        return String(presentEnv('A652_ASSET_ID') || A652_ASSET_ID || '').trim().toLowerCase();
    }

    function assetOrchardLocalServiceConfig() {
        const serviceUrl = process.env.ASSET_ORCHARD_LOCAL_SERVICE_URL || DEFAULT_ASSET_ORCHARD_LOCAL_SERVICE_URL;
        const readinessTimeoutMs = Number.parseInt(
            process.env.ASSET_ORCHARD_LOCAL_SERVICE_READINESS_TIMEOUT_MS
                || String(DEFAULT_ASSET_ORCHARD_LOCAL_SERVICE_READINESS_TIMEOUT_MS),
            10,
        );
        let parsed = null;
        try {
            parsed = new URL(serviceUrl);
        } catch (_) {
            parsed = null;
        }
        const localHostnames = new Set(['127.0.0.1', 'localhost', '[::1]', '::1']);
        const localOnly = Boolean(
            parsed
            && ['http:', 'https:'].includes(parsed.protocol)
            && localHostnames.has(parsed.hostname),
        );
        return {
            url: serviceUrl,
            readiness_endpoint: parsed ? new URL('/asset-orchard/readiness', parsed).toString() : null,
            timeout_ms: Number.isInteger(readinessTimeoutMs) && readinessTimeoutMs > 0
                ? readinessTimeoutMs
                : DEFAULT_ASSET_ORCHARD_LOCAL_SERVICE_READINESS_TIMEOUT_MS,
            local_only: localOnly,
            missing: localOnly ? [] : ['ASSET_ORCHARD_LOCAL_SERVICE_URL must be http(s) loopback'],
        };
    }

    function normalizeShieldedLiquidityMode(value) {
        const mode = String(value || 'pool_managed_note')
            .trim()
            .toLowerCase()
            .replace(/[^a-z0-9]+/g, '_')
            .replace(/^_+|_+$/g, '');
        return mode || 'pool_managed_note';
    }

    function shieldedLiquidityModeLabel(mode) {
        if (mode === 'bilateral_rfq') return 'Bilateral RFQ';
        if (mode === 'operator_inventory') return 'Operator inventory';
        if (mode === 'pool_managed_note') return 'Controlled pool-managed liquidity note';
        if (mode === 'issuer_reserve_source') return 'Issuer/reserve source';
        return mode;
    }

    function shieldedQuotePolicyHash(fields) {
        return crypto.createHash('sha256')
            .update(navswapStableJson(fields))
            .digest('hex');
    }

    function shieldedNavswapQuoteConfig(nowMs = Date.now()) {
        const a652AssetId = currentA652AssetId();
        const enabled = ['1', 'true', 'yes'].includes(lower(
            process.env.NAVSWAP_ENABLE_SHIELDED_QUOTES
                || process.env.NAVSWAP_ENABLE_SHIELDED_NAVSWAP_QUOTES
                || 'true',
        ));
        const liquidityMode = normalizeShieldedLiquidityMode(process.env.NAVSWAP_SHIELDED_LIQUIDITY_MODE);
        const liquidityCommitment = String(
            presentEnv('NAVSWAP_SHIELDED_LIQUIDITY_COMMITMENT')
                || presentEnv('NAVSWAP_SHIELDED_POOL_NOTE_COMMITMENT')
                || '',
        ).trim().toLowerCase();
        const liquidityProvider = presentEnv('NAVSWAP_SHIELDED_LIQUIDITY_PROVIDER')
            || 'controlled_pool_operator';
        const assetIssuer = presentEnv('NAVSWAP_SHIELDED_ASSET_ISSUER')
            || presentEnv('NAVSWAP_OPERATOR_ACCOUNT')
            || presentEnv('NAVSWAP_SHIELDED_ISSUER')
            || '';
        const ttlMsParsed = Number.parseInt(
            process.env.NAVSWAP_SHIELDED_QUOTE_TTL_MS
                || process.env.NAVSWAP_QUOTE_TTL_MS
                || '300000',
            10,
        );
        const quoteTtlMs = Number.isSafeInteger(ttlMsParsed) && ttlMsParsed >= 1000 ? ttlMsParsed : 300000;
        const failureMode = presentEnv('NAVSWAP_SHIELDED_FAILURE_MODE')
            || 'quote_expires_before_private_proof_or_submit';
        const poolId = presentEnv('NAVSWAP_SHIELDED_POOL_ID') || ASSET_ORCHARD_POOL_ID;
        const policyFields = {
            schema: 'postfiat-shielded-navswap-policy-v1',
            route: 'shielded_navswap',
            pool_id: poolId,
            from_asset_id: A651_ASSET_ID,
            to_asset_id: a652AssetId,
            liquidity_mode: liquidityMode,
            liquidity_commitment: liquidityCommitment,
            liquidity_provider: liquidityProvider,
            failure_mode: failureMode,
            submit_gate: 'Step 7 private swap submit',
        };
        const configuredPolicyHash = String(presentEnv('NAVSWAP_SHIELDED_POLICY_HASH') || '')
            .trim()
            .toLowerCase();
        const policyHash = /^[0-9a-f]{64}$/.test(configuredPolicyHash)
            ? configuredPolicyHash
            : shieldedQuotePolicyHash(policyFields);
        const missing = [];
        if (!enabled) missing.push('NAVSWAP_ENABLE_SHIELDED_QUOTES=true');
        if (!/^[0-9a-f]{96}$/.test(a652AssetId)) missing.push('A652_ASSET_ID');
        if (!SHIELDED_NAVSWAP_LIQUIDITY_MODES.has(liquidityMode)) missing.push('NAVSWAP_SHIELDED_LIQUIDITY_MODE');
        if (!/^[0-9a-f]{64}$/.test(liquidityCommitment) && !/^[0-9a-f]{96}$/.test(liquidityCommitment)) {
            missing.push('NAVSWAP_SHIELDED_LIQUIDITY_COMMITMENT');
        }
        if (!assetIssuer) missing.push('NAVSWAP_SHIELDED_ASSET_ISSUER or NAVSWAP_OPERATOR_ACCOUNT');
        const configured = missing.length === 0;
        const assetRegistry = [
            {
                symbol: 'a651',
                asset_id: A651_ASSET_ID,
                precision: 6,
                issuer: assetIssuer,
                nav_source: 'finalized_nav_state',
                policy_hash: policyHash,
                supported: configured,
                display_only: !configured,
            },
            {
                symbol: 'a652',
                asset_id: a652AssetId,
                precision: 6,
                issuer: assetIssuer,
                nav_source: 'finalized_nav_state',
                policy_hash: policyHash,
                supported: configured,
                display_only: !configured,
            },
        ];
        const supportedPairs = [
            {
                from_asset: 'a651',
                to_asset: 'a652',
                enabled: configured,
                liquidity_mode: liquidityMode,
                liquidity_source_class: liquidityMode,
            },
            {
                from_asset: 'a652',
                to_asset: 'a651',
                enabled: configured,
                liquidity_mode: liquidityMode,
                liquidity_source_class: liquidityMode,
            },
        ];
        return {
            enabled,
            configured,
            missing,
            schema: SHIELDED_NAVSWAP_QUOTE_SCHEMA,
            endpoint: '/api/shielded-nav-swap/quote',
            quote_ttl_ms: quoteTtlMs,
            generated_at_ms: String(nowMs),
            liquidity_mode: liquidityMode,
            liquidity_mode_label: shieldedLiquidityModeLabel(liquidityMode),
            liquidity_provider: liquidityProvider,
            liquidity_commitment: liquidityCommitment,
            liquidity_commitment_status: configured ? 'live' : 'missing',
            trust_class: liquidityMode === 'pool_managed_note' ? 'CONTROLLED' : 'CONTROLLED',
            failure_mode: failureMode,
            pool_id: poolId,
            asset_issuer: assetIssuer,
            asset_registry: assetRegistry,
            supported_pairs: supportedPairs,
            policy_hash: policyHash,
            policy_fields: policyFields,
            submit_gate: 'Step 7 private swap submit',
            custody_boundary: 'wallet-local-note-keys-only',
            copy: configured
                ? `${shieldedLiquidityModeLabel(liquidityMode)} is configured. Quotes are preview-only; private proof and submit remain gated until Step 7.`
                : 'Private quote preview requires a configured a652 asset, issuer, and live liquidity commitment before the wallet will prepare a proof.',
        };
    }

    const SHIELDED_PRIVATE_KEY_PATTERNS = [
        /(^|_)backup(_json)?$/,
        /(^|_)decrypted_backup$/,
        /(^|_)key_file$/,
        /(^|_)mnemonic$/,
        /(^|_)note_file(s)?$/,
        /(^|_)note_opening(s)?$/,
        /(^|_)passphrase$/,
        /(^|_)private_key$/,
        /(^|_)secret_key$/,
        /(^|_)seed(_phrase|_hex)?$/,
        /(^|_)spend(_|$)/,
        /(^|_)spending_key$/,
        /^(diversifier|g_d|pk_d|rho|psi|rcm|nk|rivk|rseed|spend_auth_signing_key|full_viewing_key(_hex)?)$/,
    ];
    const MAX_SHIELDED_SERIALIZED_JSON_INSPECTION_BYTES = 1_048_576;

    function normalizeShieldedKey(key) {
        return String(key || '')
            .replace(/([a-z0-9])([A-Z])/g, '$1_$2')
            .replace(/[^A-Za-z0-9]+/g, '_')
            .replace(/^_+|_+$/g, '')
            .toLowerCase();
    }

    function findShieldedPrivateMaterialPaths(value, pathLabel = '$', seen = new WeakSet(), depth = 0) {
        if (depth > 32) return [`${pathLabel}.inspection_depth_exceeded`];
        if (typeof value === 'string') {
            const trimmed = value.trim();
            if (['{', '['].includes(trimmed[0])) {
                if (trimmed.length > MAX_SHIELDED_SERIALIZED_JSON_INSPECTION_BYTES) {
                    return [`${pathLabel}.serialized_json_inspection_limit_exceeded`];
                }
                try {
                    return findShieldedPrivateMaterialPaths(JSON.parse(trimmed), `${pathLabel}<json>`, seen, depth + 1);
                } catch (_) {
                    return [];
                }
            }
            return [];
        }
        if (!value || typeof value !== 'object') return [];
        if (seen.has(value)) return [];
        seen.add(value);
        if (Array.isArray(value)) {
            return value.flatMap((item, index) => findShieldedPrivateMaterialPaths(item, `${pathLabel}[${index}]`, seen, depth + 1));
        }
        const hits = [];
        for (const [key, child] of Object.entries(value)) {
            const normalized = normalizeShieldedKey(key);
            const childPath = `${pathLabel}.${key}`;
            const publicSpendAuthorization = normalized === 'spend_authorization_signature'
                || normalized === 'spend_authorization_signatures';
            if (!publicSpendAuthorization
                && SHIELDED_PRIVATE_KEY_PATTERNS.some((pattern) => pattern.test(normalized))) {
                hits.push(childPath);
            }
            hits.push(...findShieldedPrivateMaterialPaths(child, childPath, seen, depth + 1));
        }
        return hits;
    }

    function assertNoShieldedPrivateMaterial(body) {
        const hits = findShieldedPrivateMaterialPaths(body);
        if (hits.length > 0) {
            const err = new Error(`shielded NAVSwap request contains forbidden private wallet material at ${hits[0]}`);
            err.code = 'shielded_navswap_private_material_rejected';
            throw err;
        }
    }

    function readNavswapKeyFileAddress(keyFile) {
        if (!keyFile) return null;
        try {
            const parsed = JSON.parse(fs.readFileSync(keyFile, 'utf8'));
            return parsed.address || null;
        } catch (_) {
            return null;
        }
    }

    function vaultBridgeRelayConfig() {
        const relayKeyFile = presentEnv('VAULT_BRIDGE_RELAY_KEY_FILE')
            || presentEnv('NAVSWAP_OPERATOR_HOLDER_KEY_FILE')
            || presentEnv('HOLDER_KEY_FILE');
        const nodeBin = presentEnv('VAULT_BRIDGE_NODE_BIN')
            || presentEnv('NAVSWAP_OPERATOR_NODE_BIN')
            || presentEnv('POSTFIAT_NODE_BIN')
            || path.resolve(__dirname, '..', 'target', 'release', 'postfiat-node');
        const castBin = presentEnv('VAULT_BRIDGE_CAST_BIN') || 'cast';
        const signTimeoutMs = Number.parseInt(process.env.VAULT_BRIDGE_RELAY_SIGN_TIMEOUT_MS || '60000', 10);
        const bundleTimeoutMs = Number.parseInt(process.env.VAULT_BRIDGE_RELAY_BUNDLE_TIMEOUT_MS || '120000', 10);
        const relayAccount = presentEnv('VAULT_BRIDGE_RELAY_ACCOUNT')
            || readNavswapKeyFileAddress(relayKeyFile)
            || VAULT_BRIDGE_RELAY_DEFAULT_ACCOUNT;
        return {
            schema: VAULT_BRIDGE_RELAY_SCHEMA,
            source_rpc_url: VAULT_BRIDGE_RELAY_SOURCE_RPC_URL,
            asset_id: PFUSDC_ASSET_ID,
            relay_account: relayAccount,
            relay_key_file: relayKeyFile,
            signing_configured: Boolean(relayKeyFile),
            node_bin: nodeBin,
            cast_bin: castBin,
            expires_at_height: Number.isInteger(VAULT_BRIDGE_RELAY_EXPIRES_AT_HEIGHT)
                && VAULT_BRIDGE_RELAY_EXPIRES_AT_HEIGHT > 0
                ? VAULT_BRIDGE_RELAY_EXPIRES_AT_HEIGHT
                : 1000000,
            sign_timeout_ms: Number.isInteger(signTimeoutMs) && signTimeoutMs > 0 ? signTimeoutMs : 60000,
            bundle_timeout_ms: Number.isInteger(bundleTimeoutMs) && bundleTimeoutMs > 0 ? bundleTimeoutMs : 120000,
            sponsor_recipient_accounts: process.env.VAULT_BRIDGE_SPONSOR_RECIPIENT_ACCOUNTS !== 'false',
            recipient_sponsor_amount: Number.isInteger(VAULT_BRIDGE_RECIPIENT_SPONSOR_AMOUNT)
                && VAULT_BRIDGE_RECIPIENT_SPONSOR_AMOUNT > 0
                ? VAULT_BRIDGE_RECIPIENT_SPONSOR_AMOUNT
                : 10,
            recipient_sponsor_min_amount_atoms: VAULT_BRIDGE_RECIPIENT_SPONSOR_MIN_AMOUNT_ATOMS > 0n
                ? VAULT_BRIDGE_RECIPIENT_SPONSOR_MIN_AMOUNT_ATOMS
                : 1000000n,
            custody_boundary: 'operator-relay-key-file',
        };
    }

    async function governedVaultBridgeRelayConfig(config, rpcRequest = rpcTcpRequest) {
        const read = await routedRpcRead('vault_bridge_route', { asset_id: config.asset_id }, rpcRequest);
        const report = read.result;
        const profile = report?.profile;
        const canonicalAddress = (value) => normalizeVaultBridgeAddress(value);
        const runtimeHash = (value) => {
            const normalized = String(value || '').toLowerCase();
            return /^0x[0-9a-f]{64}$/.test(normalized) && !/^0x0{64}$/.test(normalized)
                ? normalized
                : '';
        };
        const profileHash = String(report?.profile_hash || '').toLowerCase();
        const routeBinding = normalizeVaultBridgeBytes32(report?.route_binding);
        const routeEpoch = Number(profile?.route_epoch);
        const sourceChainId = Number(profile?.source_chain_id);
        const currentHeight = Number(report?.current_height);
        const activationHeight = Number(profile?.activation_height);
        const expiresAtHeight = Number(profile?.expires_at_height);
        if (
            report?.schema !== 'postfiat.vault_bridge.route_report.v1'
            || report?.active !== true
            || profile?.schema !== 'postfiat.vault_bridge.route_profile.v1'
            || String(profile?.asset_id || '').toLowerCase() !== String(config.asset_id).toLowerCase()
            || !/^[0-9a-f]{96}$/.test(profileHash)
            || String(report?.nav_profile_policy_hash || '').toLowerCase() !== profileHash
            || !routeBinding
            || /^0{64}$/.test(routeBinding)
            || !Number.isSafeInteger(sourceChainId)
            || sourceChainId <= 0
            || !Number.isSafeInteger(routeEpoch)
            || routeEpoch <= 0
            || Number(report?.governance_route_epoch) !== routeEpoch
            || !Number.isSafeInteger(currentHeight)
            || !Number.isSafeInteger(activationHeight)
            || !Number.isSafeInteger(expiresAtHeight)
            || activationHeight > currentHeight
            || expiresAtHeight <= currentHeight
            || !canonicalAddress(profile?.vault_address)
            || !canonicalAddress(profile?.token_address)
            || !runtimeHash(profile?.vault_runtime_code_hash)
            || !runtimeHash(profile?.token_runtime_code_hash)
        ) {
            const err = new Error('Active governed vault bridge route RPC response is incomplete or inconsistent');
            err.code = 'vault_bridge_governed_route_invalid';
            throw err;
        }
        return {
            ...config,
            source_chain_id: sourceChainId,
            vault_address: canonicalAddress(profile.vault_address),
            vault_code_hash: runtimeHash(profile.vault_runtime_code_hash),
            token_address: canonicalAddress(profile.token_address),
            token_code_hash: runtimeHash(profile.token_runtime_code_hash),
            policy_hash: profileHash,
            route_epoch: routeEpoch,
            route_binding: routeBinding,
            route_report: report,
        };
    }

    function normalizeVaultBridgeTxHash(value, field = 'deposit_tx_hash') {
        const text = String(value || '').trim();
        const prefixed = text.toLowerCase().startsWith('0x') ? text : `0x${text}`;
        if (!/^0x[0-9a-fA-F]{64}$/.test(prefixed)) {
            const err = new Error(`${field} must be a 32-byte transaction hash`);
            err.code = 'vault_bridge_invalid_tx_hash';
            throw err;
        }
        return prefixed.toLowerCase();
    }

    function normalizeVaultBridgeBytes32(value) {
        const text = String(value || '').trim();
        if (!text) return '';
        const unprefixed = text.toLowerCase().startsWith('0x') ? text.slice(2) : text;
        return /^[0-9a-f]{64}$/.test(unprefixed) ? unprefixed : '';
    }

    function normalizeVaultBridgeAddress(value) {
        const text = String(value || '').trim();
        return /^0x[0-9a-fA-F]{40}$/.test(text) ? text.toLowerCase() : '';
    }

    function vaultBridgeBodyTxHash(body = {}) {
        return body.deposit_tx_hash
            || body.depositTxHash
            || body.deposit_tx
            || body.depositTx
            || body.tx_hash
            || body.txHash
            || '';
    }

    function vaultBridgeExpectedField(body = {}, snake, camel) {
        const value = body[snake] ?? body[camel];
        return value === undefined || value === null ? '' : String(value).trim();
    }

    function vaultBridgeEvidenceFromPlan(plan = {}) {
        return plan.evidence || plan.deposit || plan.bridge_deposit || plan;
    }

    function assertVaultBridgeEvidenceMatches(evidence, body, config, txHash) {
        const errors = [];
        if (Number(evidence.source_chain_id) !== Number(config.source_chain_id)) {
            errors.push(`source_chain_id ${evidence.source_chain_id} does not match ${config.source_chain_id}`);
        }
        if (normalizeVaultBridgeAddress(evidence.vault_address) !== normalizeVaultBridgeAddress(config.vault_address)) {
            errors.push('vault address does not match configured bridge vault');
        }
        if (normalizeVaultBridgeAddress(evidence.token_address) !== normalizeVaultBridgeAddress(config.token_address)) {
            errors.push('token address does not match configured Arbitrum USDC');
        }
        const evidenceTxHash = normalizeVaultBridgeTxHash(evidence.tx_hash || '', 'evidence.tx_hash');
        if (evidenceTxHash !== txHash) {
            errors.push('evidence tx_hash does not match requested deposit tx');
        }

        const expectedRecipient = vaultBridgeExpectedField(body, 'pftl_recipient', 'pftlRecipient');
        if (expectedRecipient && String(evidence.pftl_recipient || '') !== expectedRecipient) {
            errors.push('PFTL recipient does not match the wallet recipient');
        }
        const expectedDepositor = normalizeVaultBridgeAddress(
            vaultBridgeExpectedField(body, 'depositor', 'depositor')
                || vaultBridgeExpectedField(body, 'evm_address', 'evmAddress'),
        );
        if (expectedDepositor && normalizeVaultBridgeAddress(evidence.depositor) !== expectedDepositor) {
            errors.push('EVM depositor does not match the connected MetaMask account');
        }
        const expectedDepositId = normalizeVaultBridgeBytes32(
            vaultBridgeExpectedField(body, 'deposit_id', 'depositId'),
        );
        if (expectedDepositId && normalizeVaultBridgeBytes32(evidence.deposit_id) !== expectedDepositId) {
            errors.push('deposit_id does not match the vault event');
        }
        const expectedAmountAtoms = vaultBridgeExpectedField(body, 'amount_atoms', 'amountAtoms');
        if (expectedAmountAtoms && String(evidence.amount_atoms) !== expectedAmountAtoms) {
            errors.push('amount_atoms does not match the vault event');
        }
        const expectedProfileHash = vaultBridgeExpectedField(body, 'route_profile_hash', 'routeProfileHash').toLowerCase();
        if (!/^[0-9a-f]{96}$/.test(expectedProfileHash) || expectedProfileHash !== config.policy_hash) {
            errors.push('wallet route profile hash does not match the active governed route');
        }
        const expectedRouteEpoch = Number(vaultBridgeExpectedField(body, 'route_epoch', 'routeEpoch'));
        if (!Number.isSafeInteger(expectedRouteEpoch) || expectedRouteEpoch !== config.route_epoch) {
            errors.push('wallet route epoch does not match the active governed route');
        }
        const expectedRouteBinding = normalizeVaultBridgeBytes32(
            vaultBridgeExpectedField(body, 'route_binding', 'routeBinding'),
        );
        if (!expectedRouteBinding || expectedRouteBinding !== config.route_binding) {
            errors.push('wallet route binding does not match the active governed route');
        }
        if (normalizeVaultBridgeBytes32(evidence.route_binding) !== config.route_binding) {
            errors.push('vault event route binding does not match the active governed route');
        }
        if (errors.length > 0) {
            const err = new Error(`Bridge relay evidence validation failed: ${errors.join('; ')}`);
            err.code = 'vault_bridge_evidence_mismatch';
            err.evidence = evidence;
            throw err;
        }
    }

    async function buildVaultBridgeRelayBundle(body, config) {
        if (!fs.existsSync(config.node_bin)) {
            const err = new Error(`postfiat-node binary not found at ${config.node_bin}`);
            err.code = 'vault_bridge_node_bin_missing';
            throw err;
        }
        const { stdout: vaultCodeHashStdout } = await execFileAsync(
            config.cast_bin,
            ['codehash', config.vault_address, '--rpc-url', config.source_rpc_url],
            {
                timeout: config.bundle_timeout_ms,
                maxBuffer: 1024 * 1024,
                windowsHide: true,
            },
        );
        const observedVaultCodeHash = String(vaultCodeHashStdout || '').trim().toLowerCase();
        if (observedVaultCodeHash !== config.vault_code_hash) {
            const err = new Error(
                `Bridge vault code hash mismatch: expected ${config.vault_code_hash}, received ${observedVaultCodeHash || '<empty>'}`,
            );
            err.code = 'vault_bridge_code_hash_mismatch';
            throw err;
        }
        const { stdout: tokenCodeHashStdout } = await execFileAsync(
            config.cast_bin,
            ['codehash', config.token_address, '--rpc-url', config.source_rpc_url],
            {
                timeout: config.bundle_timeout_ms,
                maxBuffer: 1024 * 1024,
                windowsHide: true,
            },
        );
        const observedTokenCodeHash = String(tokenCodeHashStdout || '').trim().toLowerCase();
        if (observedTokenCodeHash !== config.token_code_hash) {
            const err = new Error(
                `Bridge token code hash mismatch: expected ${config.token_code_hash}, received ${observedTokenCodeHash || '<empty>'}`,
            );
            err.code = 'vault_bridge_code_hash_mismatch';
            throw err;
        }
        const txHash = normalizeVaultBridgeTxHash(vaultBridgeBodyTxHash(body));
        const workDir = fs.mkdtempSync(path.join(os.tmpdir(), 'postfiat-vault-bridge-relay-'));
        const bundleDir = path.join(workDir, 'bundle');
        const args = [
            'vault-bridge-deposit-relay-rpc-bundle',
            '--source-rpc-url',
            config.source_rpc_url,
            '--tx-hash',
            txHash,
            '--cast-bin',
            config.cast_bin,
            '--vault-address',
            config.vault_address,
            '--token-address',
            config.token_address,
            '--asset-id',
            config.asset_id,
            '--policy-hash',
            config.policy_hash,
            '--proposer',
            config.relay_account,
            '--attestor',
            config.relay_account,
            '--finalizer',
            config.relay_account,
            '--claimer',
            config.relay_account,
            '--expires-at-height',
            String(config.expires_at_height),
            '--bundle',
            bundleDir,
            '--overwrite',
        ];
        const { stdout } = await execFileAsync(config.node_bin, args, {
            timeout: config.bundle_timeout_ms,
            maxBuffer: 4 * 1024 * 1024,
        });
        const reportFile = path.join(workDir, 'bundle-report.json');
        fs.writeFileSync(reportFile, stdout || '{}', { mode: 0o600 });
        const plan = JSON.parse(fs.readFileSync(path.join(bundleDir, 'plan.json'), 'utf8'));
        const evidence = vaultBridgeEvidenceFromPlan(plan);
        assertVaultBridgeEvidenceMatches(evidence, body, config, txHash);
        return {
            work_dir: workDir,
            bundle_dir: bundleDir,
            report_file: reportFile,
            plan,
            evidence,
        };
    }

    async function routedRpcRead(method, params = {}, rpcRequest = rpcTcpRequest) {
        if (rpcRequest !== rpcTcpRequest) {
            return {
                result: await navswapRpcRead(method, params, rpcRequest),
                route: null,
            };
        }
        const target = await resolveRpcTarget(method);
        const request = {
            version: 'postfiat-local-rpc-v1',
            id: `proxy-${method}-${Date.now()}`,
            method,
            params,
        };
        const response = await rpcRequest(
            target.endpoint.host,
            target.endpoint.port,
            requestWithProxyReadiness(request, target.route),
        );
        if (response.ok !== true) {
            const err = new Error(response.error?.message || `${method} RPC failed.`);
            err.code = response.error?.code || `${method}_failed`;
            err.rpc_error = response.error || null;
            err.route = target.route || null;
            throw err;
        }
        return {
            result: response.result,
            route: target.route || null,
        };
    }

    function isBadSequenceSubmitResponse(response) {
        const code = String(response?.error?.code || '');
        const message = String(response?.error?.message || '');
        return code === 'bad_sequence' || /bad_sequence/i.test(message);
    }

    function isReplayableVaultBridgeRelayDuplicate(label, response) {
        const code = String(response?.error?.code || '');
        const message = String(response?.error?.message || '');
        return (
            (label === 'propose' && (code === 'duplicate_vault_bridge_deposit' || /duplicate_vault_bridge_deposit\b/.test(message)))
            || (label === 'attest' && (code === 'duplicate_vault_bridge_deposit_attestation' || /duplicate_vault_bridge_deposit_attestation\b/.test(message)))
            || (label === 'attest' && /vault bridge asset bridge deposit attestation requires pending evidence/i.test(message))
            || (label === 'finalize' && /vault bridge asset bridge deposit finalize requires pending evidence/i.test(message))
        );
    }

    async function signAndSubmitVaultBridgeRelayOperation(
        label,
        operation,
        config,
        rpcRequest = rpcTcpRequest,
        attempt = 1,
        priorFailures = [],
    ) {
        const quoteRead = await routedRpcRead('asset_fee_quote', {
            source: config.relay_account,
            operation_json: JSON.stringify(operation),
        }, rpcRequest);
        const quote = quoteRead.result;
        if (quote.source && quote.source !== config.relay_account) {
            return {
                ok: false,
                label,
                code: 'vault_bridge_quote_source_mismatch',
                message: 'Bridge relay fee quote source does not match the relay signer.',
                operation,
                quote,
                quote_route: quoteRead.route,
                attempt,
                prior_failures: priorFailures,
            };
        }
        if (quote.operation && navswapStableJson(quote.operation) !== navswapStableJson(operation)) {
            return {
                ok: false,
                label,
                code: 'vault_bridge_quote_operation_mismatch',
                message: 'Bridge relay fee quote operation does not match the verified relay operation.',
                operation,
                quote,
                quote_route: quoteRead.route,
                attempt,
                prior_failures: priorFailures,
            };
        }
        if (quote.sender_meets_reserve_after_fee === false) {
            return {
                ok: false,
                label,
                code: 'vault_bridge_relay_insufficient_fee_balance',
                message: 'Bridge relay account does not have enough PFT for the relay fee.',
                operation,
                quote,
                quote_route: quoteRead.route,
                attempt,
                prior_failures: priorFailures,
            };
        }

        const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'postfiat-vault-bridge-sign-'));
        const quoteFile = path.join(dir, `${label}.quote.json`);
        fs.writeFileSync(quoteFile, JSON.stringify(quote, null, 2), { mode: 0o600 });
        const { stdout } = await execFileAsync(
            config.node_bin,
            [
                'wallet-sign-asset-transaction',
                '--key-file',
                config.relay_key_file,
                '--quote-file',
                quoteFile,
            ],
            {
                timeout: config.sign_timeout_ms,
                maxBuffer: 2 * 1024 * 1024,
            },
        );
        const signed = JSON.parse(stdout);
        const submitMethod = 'mempool_submit_signed_asset_transaction_finality';
        const submitRequest = {
            version: 'postfiat-local-rpc-v1',
            id: `vault-bridge-relay-${label}-${Date.now()}`,
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
            const failure = {
                ok: false,
                label,
                code: submitResponse.error?.code || 'vault_bridge_relay_submit_failed',
                message: submitResponse.error?.message || `Bridge relay ${label} submit failed.`,
                operation,
                quote,
                quote_route: quoteRead.route,
                rpc_error: submitResponse.error || null,
                route: target.route,
                attempt,
                prior_failures: priorFailures,
            };
            if (isBadSequenceSubmitResponse(submitResponse) && attempt < 3) {
                await sleep(1000 * attempt);
                return signAndSubmitVaultBridgeRelayOperation(
                    label,
                    operation,
                    config,
                    rpcRequest,
                    attempt + 1,
                    [...priorFailures, failure],
                );
            }
            if (isReplayableVaultBridgeRelayDuplicate(label, submitResponse)) {
                return {
                    ok: true,
                    label,
                    tx_id: null,
                    height: null,
                    replayed_stage: true,
                    code: failure.code,
                    message: failure.message,
                    route: target.route,
                    operation,
                    quote,
                    quote_route: quoteRead.route,
                    submit_result: null,
                    attempt,
                    prior_failures: priorFailures,
                };
            }
            return failure;
        }
        return {
            ok: true,
            label,
            tx_id: submitResponse.result?.tx_id || null,
            height: submitResponse.result?.finality?.block?.header?.height || null,
            route: target.route,
            operation,
            quote,
            quote_route: quoteRead.route,
            submit_result: submitResponse.result,
            attempt,
            prior_failures: priorFailures,
        };
    }

    async function vaultBridgePftlAccountExists(account, rpcRequest = rpcTcpRequest, reserveAmount = 10) {
        try {
            const read = await routedRpcRead('account', { address: account }, rpcRequest);
            const balance = BigInt(read.result?.balance || 0);
            return balance >= BigInt(reserveAmount);
        } catch (error) {
            const message = String(error?.message || '');
            if (/account.*(not found|does not exist)|unknown account|missing account/i.test(message)) {
                return false;
            }
            throw error;
        }
    }

    async function signAndSubmitVaultBridgeRecipientSponsor(
        recipient,
        config,
        rpcRequest = rpcTcpRequest,
        attempt = 1,
        priorFailures = [],
    ) {
        const amount = config.recipient_sponsor_amount;
        const quoteRead = await routedRpcRead('transfer_fee_quote', {
            from: config.relay_account,
            to: recipient,
            amount,
        }, rpcRequest);
        const quote = quoteRead.result;
        const quoteErrors = [];
        if (quote.from && quote.from !== config.relay_account) {
            quoteErrors.push('transfer quote source does not match the relay signer');
        }
        if (quote.to && quote.to !== recipient) {
            quoteErrors.push('transfer quote recipient does not match the bridge recipient');
        }
        if (String(quote.amount) !== String(amount)) {
            quoteErrors.push('transfer quote amount does not match the sponsorship amount');
        }
        if (quote.sender_meets_reserve_after_transfer === false) {
            quoteErrors.push('relay account would fall below reserve after sponsorship');
        }
        if (quote.recipient_meets_reserve_after_transfer === false) {
            quoteErrors.push('recipient would still be below reserve after sponsorship');
        }
        if (quoteErrors.length > 0) {
            return {
                ok: false,
                label: 'sponsor_recipient',
                code: 'vault_bridge_recipient_sponsor_quote_invalid',
                message: quoteErrors.join('; '),
                recipient,
                quote,
                quote_route: quoteRead.route,
                attempt,
                prior_failures: priorFailures,
            };
        }

        const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'postfiat-vault-bridge-sponsor-'));
        const quoteFile = path.join(dir, 'sponsor-recipient.quote.json');
        fs.writeFileSync(quoteFile, JSON.stringify(quote, null, 2), { mode: 0o600 });
        const { stdout } = await execFileAsync(
            config.node_bin,
            [
                'wallet-sign-transfer',
                '--key-file',
                config.relay_key_file,
                '--quote-file',
                quoteFile,
            ],
            {
                timeout: config.sign_timeout_ms,
                maxBuffer: 2 * 1024 * 1024,
            },
        );
        const signed = JSON.parse(stdout);
        const submitMethod = 'mempool_submit_signed_transfer_finality';
        const submitRequest = {
            version: 'postfiat-local-rpc-v1',
            id: `vault-bridge-sponsor-recipient-${Date.now()}`,
            method: submitMethod,
            params: {
                signed_transfer_json: JSON.stringify(signed),
            },
        };
        const target = rpcRequest === rpcTcpRequest
            ? await resolveRpcTarget(submitMethod)
            : {
                endpoint: { validatorId: 'test', host: RPC_HOST, port: RPC_PORT },
                route: null,
            };
        const submitResponse = await rpcRequest(
            target.endpoint.host,
            target.endpoint.port,
            requestWithProxyReadiness(submitRequest, target.route),
        );
        if (submitResponse.ok === true && target.route) {
            const line = JSON.stringify(submitResponse);
            rememberFinalizedReadEndpoint(line, target);
            primeNextProposerRouteCacheFromResponse(line, target.route, {
                warmReadiness: true,
            });
        }
        if (submitResponse.ok !== true) {
            const failure = {
                ok: false,
                label: 'sponsor_recipient',
                code: submitResponse.error?.code || 'vault_bridge_recipient_sponsor_failed',
                message: submitResponse.error?.message || 'Bridge recipient account sponsorship failed.',
                recipient,
                quote,
                quote_route: quoteRead.route,
                rpc_error: submitResponse.error || null,
                route: target.route,
                attempt,
                prior_failures: priorFailures,
            };
            if (isBadSequenceSubmitResponse(submitResponse) && attempt < 3) {
                await sleep(1000 * attempt);
                return signAndSubmitVaultBridgeRecipientSponsor(
                    recipient,
                    config,
                    rpcRequest,
                    attempt + 1,
                    [...priorFailures, failure],
                );
            }
            return failure;
        }
        return {
            ok: true,
            label: 'sponsor_recipient',
            tx_id: submitResponse.result?.tx_id || null,
            height: submitResponse.result?.finality?.block?.header?.height || null,
            recipient,
            amount,
            route: target.route,
            quote,
            quote_route: quoteRead.route,
            submit_result: submitResponse.result,
            attempt,
            prior_failures: priorFailures,
        };
    }

    async function ensureVaultBridgeRecipientAccount(recipient, evidence, config, rpcRequest = rpcTcpRequest) {
        if (!config.sponsor_recipient_accounts) {
            return { ok: true, label: 'sponsor_recipient', skipped: true, reason: 'disabled' };
        }
        const amountAtoms = BigInt(evidence?.amount_atoms || 0);
        if (amountAtoms < config.recipient_sponsor_min_amount_atoms) {
            return { ok: true, label: 'sponsor_recipient', skipped: true, reason: 'deposit_below_sponsor_minimum' };
        }
        if (await vaultBridgePftlAccountExists(recipient, rpcRequest, config.recipient_sponsor_amount)) {
            return { ok: true, label: 'sponsor_recipient', skipped: true, reason: 'recipient_exists' };
        }
        return signAndSubmitVaultBridgeRecipientSponsor(recipient, config, rpcRequest);
    }

    async function vaultBridgeAccountAssets(account, rpcRequest = rpcTcpRequest, minimumBalanceAtoms = null) {
        const minimum = minimumBalanceAtoms === null || minimumBalanceAtoms === undefined
            ? null
            : BigInt(minimumBalanceAtoms);
        let last = null;
        for (let attempt = 1; attempt <= 8; attempt += 1) {
            const read = await routedRpcRead('account_assets', {
                account,
                asset_id: PFUSDC_ASSET_ID,
                limit: 10,
            }, rpcRequest);
            const assets = read.result;
            const asset = Array.isArray(assets.assets)
                ? assets.assets.find((item) => String(item.asset_id || '').toLowerCase() === PFUSDC_ASSET_ID.toLowerCase())
                : null;
            last = {
                result: assets,
                balance_atoms: String(asset?.balance ?? '0'),
                asset: asset || null,
                route: read.route,
            };
            if (minimum === null || BigInt(last.balance_atoms) >= minimum) {
                return last;
            }
            await sleep(750);
        }
        return last;
    }

    async function executeVaultBridgeRelay(body = {}, rpcRequest = rpcTcpRequest) {
        const baseConfig = vaultBridgeRelayConfig();
        if (!baseConfig.signing_configured) {
            return {
                ok: false,
                schema: VAULT_BRIDGE_RELAY_SCHEMA,
                code: 'vault_bridge_relay_key_not_configured',
                message: 'Set VAULT_BRIDGE_RELAY_KEY_FILE so the wallet proxy can relay vault deposits to PFTL.',
                custody_boundary: baseConfig.custody_boundary,
            };
        }
        if (!fs.existsSync(baseConfig.relay_key_file)) {
            return {
                ok: false,
                schema: VAULT_BRIDGE_RELAY_SCHEMA,
                code: 'vault_bridge_relay_key_file_missing',
                message: `Bridge relay key file not found at ${baseConfig.relay_key_file}.`,
                custody_boundary: baseConfig.custody_boundary,
            };
        }

        let config;
        let bundle;
        try {
            config = await governedVaultBridgeRelayConfig(baseConfig, rpcRequest);
            bundle = await buildVaultBridgeRelayBundle(body, config);
        } catch (error) {
            return {
                ok: false,
                schema: VAULT_BRIDGE_RELAY_SCHEMA,
                code: error.code || 'vault_bridge_bundle_failed',
                message: error.message || 'Bridge vault deposit relay bundle failed.',
                evidence: error.evidence || null,
            };
        }

        const recipient = bundle.evidence.pftl_recipient;
        const before = await vaultBridgeAccountAssets(recipient, rpcRequest);
        const submitted = [];
        let sponsorResult;
        try {
            sponsorResult = await ensureVaultBridgeRecipientAccount(recipient, bundle.evidence, config, rpcRequest);
        } catch (error) {
            return {
                ok: false,
                schema: VAULT_BRIDGE_RELAY_SCHEMA,
                code: error.code || 'vault_bridge_recipient_sponsor_failed',
                message: error.message || 'Bridge recipient account sponsorship failed.',
                status: 'failed',
                failed_stage: 'sponsor_recipient',
                evidence: bundle.evidence,
                before_balance_atoms: before.balance_atoms,
                submitted,
                work_dir: bundle.work_dir,
            };
        }
        if (sponsorResult && sponsorResult.skipped !== true) {
            submitted.push(sponsorResult);
            if (sponsorResult.ok !== true) {
                return {
                    ok: false,
                    schema: VAULT_BRIDGE_RELAY_SCHEMA,
                    code: sponsorResult.code || 'vault_bridge_recipient_sponsor_failed',
                    message: sponsorResult.message || 'Bridge recipient account sponsorship failed.',
                    status: 'failed',
                    failed_stage: 'sponsor_recipient',
                    evidence: bundle.evidence,
                    before_balance_atoms: before.balance_atoms,
                    submitted,
                    work_dir: bundle.work_dir,
                };
            }
        }
        for (const label of ['propose', 'attest', 'finalize', 'claim']) {
            const operation = JSON.parse(fs.readFileSync(path.join(bundle.bundle_dir, `${label}.operation.json`), 'utf8'));
            const result = await signAndSubmitVaultBridgeRelayOperation(label, operation, config, rpcRequest);
            submitted.push(result);
            if (result.ok !== true) {
                return {
                    ok: false,
                    schema: VAULT_BRIDGE_RELAY_SCHEMA,
                    code: result.code || 'vault_bridge_relay_submit_failed',
                    message: result.message || `Bridge relay ${label} failed.`,
                    status: 'failed',
                    failed_stage: label,
                    evidence: bundle.evidence,
                    before_balance_atoms: before.balance_atoms,
                    submitted,
                    work_dir: bundle.work_dir,
                };
            }
        }
        const expectedAfterBalance = BigInt(before.balance_atoms) + BigInt(bundle.evidence.amount_atoms || 0);
        const after = await vaultBridgeAccountAssets(recipient, rpcRequest, expectedAfterBalance);
        return {
            ok: true,
            schema: VAULT_BRIDGE_RELAY_SCHEMA,
            status: 'complete',
            message: 'Bridge vault deposit relayed to PFTL and claimed as pfUSDC.',
            asset_id: PFUSDC_ASSET_ID,
            pftl_recipient: recipient,
            before_balance_atoms: before.balance_atoms,
            after_balance_atoms: after.balance_atoms,
            minted_atoms: String(BigInt(after.balance_atoms) - BigInt(before.balance_atoms)),
            evidence: bundle.evidence,
            submitted: submitted.map((item) => ({
                label: item.label,
                tx_id: item.tx_id,
                height: item.height,
                amount: item.amount,
                skipped: item.skipped,
                reason: item.reason,
                replayed_stage: item.replayed_stage,
                route: item.route,
            })),
            account_asset: after.asset,
            account_assets: after.result,
            work_dir: bundle.work_dir,
        };
    }

    function navswapDevnetFundingWindowUsage(recipient, config, nowMs = Date.now()) {
        const key = String(recipient || '').trim();
        const current = navswapDevnetFundingUsage.get(key);
        if (!current || nowMs >= current.reset_at_ms) {
            const next = {
                used_atoms: 0n,
                reset_at_ms: nowMs + config.recipient_window_ms,
            };
            navswapDevnetFundingUsage.set(key, next);
            return next;
        }
        return current;
    }

    function navswapDevnetFundingUsageSnapshot(recipient, config, nowMs = Date.now()) {
        const usage = navswapDevnetFundingWindowUsage(recipient, config, nowMs);
        const maxAtoms = BigInt(config.max_recipient_window_atoms);
        const remainingAtoms = usage.used_atoms >= maxAtoms ? 0n : maxAtoms - usage.used_atoms;
        return {
            used_atoms: usage.used_atoms.toString(),
            remaining_atoms: remainingAtoms.toString(),
            max_atoms: config.max_recipient_window_atoms,
            reset_at_ms: usage.reset_at_ms,
        };
    }

    function reserveNavswapDevnetFundingUsage(recipient, amountAtoms, config, nowMs = Date.now()) {
        const amount = BigInt(amountAtoms);
        const usage = navswapDevnetFundingWindowUsage(recipient, config, nowMs);
        const maxAtoms = BigInt(config.max_recipient_window_atoms);
        const remainingAtoms = usage.used_atoms >= maxAtoms ? 0n : maxAtoms - usage.used_atoms;
        if (amount > remainingAtoms) {
            return {
                ok: false,
                snapshot: navswapDevnetFundingUsageSnapshot(recipient, config, nowMs),
            };
        }
        usage.used_atoms += amount;
        return {
            ok: true,
            snapshot: navswapDevnetFundingUsageSnapshot(recipient, config, nowMs),
        };
    }

    function releaseNavswapDevnetFundingUsage(recipient, amountAtoms, config, nowMs = Date.now()) {
        const amount = BigInt(amountAtoms);
        const usage = navswapDevnetFundingWindowUsage(recipient, config, nowMs);
        usage.used_atoms = usage.used_atoms > amount ? usage.used_atoms - amount : 0n;
        return navswapDevnetFundingUsageSnapshot(recipient, config, nowMs);
    }

    function clearNavswapDevnetFundingUsageForTest() {
        navswapDevnetFundingUsage.clear();
    }

    function navswapRoutePrivacy({
        mode,
        label,
        disclosureLabel,
        publicFields = [],
        disclosedFields = [],
        privateFields = [],
        warning = null,
    } = {}) {
        return {
            schema: 'postfiat-navswap-route-privacy-v1',
            mode,
            label,
            disclosure_label: disclosureLabel,
            public_fields: publicFields,
            disclosed_fields: disclosedFields,
            private_fields: privateFields,
            warning,
        };
    }

    function navswapCapabilities(now = new Date()) {
        const bridge = navswapBridgeConfig();
        const uniswapBeta = navswapUniswapBetaRouteState(bridge);
        const stakehubTransparent = navswapStakehubTransparentConfig();
        const transparentOperator = navswapTransparentOperatorConfig();
        const devnetFunding = navswapDevnetPfusdcFundingConfig();
        const shieldedIngress = shieldedNavswapIngressConfig();
        const shieldedQuote = shieldedNavswapQuoteConfig(now.getTime());
        const shieldedSwap = shieldedNavswapSwapConfig();
        const shieldedEgress = shieldedNavswapEgressConfig();
        const a652AssetId = currentA652AssetId();
        const transparentFinalityEnabled = RPC_CAPS.mempool_submit_asset_transaction_finality_enabled;
        const transparentCanRun = transparentOperator.signing_configured && transparentFinalityEnabled;
        const transparentStatus = !transparentOperator.signing_configured
            ? 'operator_key_required'
            : transparentFinalityEnabled
                ? 'quote_ready'
                : 'asset_finality_required';
        const transparentReason = !transparentOperator.signing_configured
            ? 'Transparent NAVSwap can quote wallet-owned actions, but operator mint completion requires NAVSWAP_OPERATOR_ISSUER_KEY_FILE before a full route can complete.'
            : transparentFinalityEnabled
                ? 'Transparent NAVSwap can quote wallet-owned actions and complete the issuer/operator mint after the wallet allocation lands.'
                : 'Transparent NAVSwap can quote wallet-owned actions, but browser-signed asset transactions require mempool_submit_signed_asset_transaction_finality before a full route can complete.';
        const transparentRequiredNext = [];
        if (!transparentOperator.signing_configured) {
            transparentRequiredNext.push('configure NAVSWAP_OPERATOR_ISSUER_KEY_FILE');
        }
        if (!transparentFinalityEnabled) {
            transparentRequiredNext.push('enable mempool_submit_signed_asset_transaction_finality');
        }
        transparentRequiredNext.push('manual browser UI click-through from the target user wallet');
        return {
            ok: true,
            schema: NAVSWAP_CAPABILITIES_SCHEMA,
            generated_at: now.toISOString(),
            custody_boundary: 'wallet-local-signing',
            live_transaction_policy: {
                max_usd_equivalent: bridge.max_live_usd,
                new_uniswap_pool_seed_allowed: false,
                large_lp_position_allowed: false,
                contract_deploy_allowed_above_cap: false,
            },
            assets: {
                PFT: { asset_id: 'PFT', lane: 'native' },
                pfUSDC: { asset_id: PFUSDC_ASSET_ID, lane: 'issued' },
                a651: { asset_id: A651_ASSET_ID, lane: 'issued' },
                ...(a652AssetId ? { a652: { asset_id: a652AssetId, lane: 'issued' } } : {}),
            },
            routes: {
                transparent_navswap: {
                    label: 'Transparent NAVSwap',
                    route_family: NAVSWAP_PRIMARY_MINT_ROUTE_FAMILY,
                    route_trust_class: 'CONTROLLED',
                    primary_supply_effect: 'mints_new_native_navcoin_supply',
                    pricing_source: 'finalized_pre_inflow_nav_snapshot',
                    status: transparentStatus,
                    enabled: true,
                    can_quote: true,
                    can_run: transparentCanRun,
                    reason: transparentReason,
                    privacy: navswapRoutePrivacy({
                        mode: 'public_wallet_signed',
                        label: 'Public',
                        disclosureLabel: 'Wallet, assets, amount, allocation, and receipts are public.',
                        publicFields: ['wallet_address', 'from_asset', 'to_asset', 'settlement_amount', 'nav_amount', 'allocation_id', 'operator_mint_tx'],
                        privateFields: ['wallet_seed', 'wallet_private_key', 'signing_material'],
                    }),
                    required_next: transparentRequiredNext,
                    supported_pairs: ['pfUSDC->a651', 'a651->pfUSDC'],
                    current_pair: {
                        from_asset: 'pfUSDC',
                        to_asset: 'a651',
                        amount_asset: 'a651',
                        settlement_asset: 'pfUSDC',
                        amount_semantics: 'display_nav_amount_decimal',
                        amount_precision: 6,
                    },
                    planner_fed_quote_supported: true,
                    quote_requires_planner_actions: true,
                    automatic_planner_input_selection: 'default_wallet_quote',
                    planner_inputs_endpoint: '/api/navswap/planner-inputs',
                    readiness_endpoint: '/api/navswap/readiness',
                    prepared_action_schema: NAVSWAP_WALLET_ACTION_SCHEMA,
                    prepare_action_endpoint: '/api/navswap/actions/prepare',
                    prepare_action_batch_endpoint: '/api/navswap/actions/prepare-batch',
                    prepared_action_batch_schema: NAVSWAP_WALLET_ACTION_BATCH_PREPARE_SCHEMA,
                    prepared_action_stages: ['nav_subscription_allocate', 'nav_redeem_at_nav'],
                    wallet_owned_actions: ['vault_bridge_nav_subscription_allocate', 'nav_redeem_at_nav'],
                    operator_owned_actions: ['nav_mint_at_nav', 'nav_redeem_settle'],
                    operator_completion: {
                        endpoint: '/api/navswap/runs',
                        async_supported: true,
                        signing_configured: transparentOperator.signing_configured,
                        submit_method: 'mempool_submit_signed_asset_transaction_finality',
                        asset_transaction_finality_enabled: transparentFinalityEnabled,
                        custody_boundary: transparentOperator.custody_boundary,
                    },
                    devnet_settlement_funding: {
                        enabled: devnetFunding.enabled,
                        signing_configured: devnetFunding.signing_configured,
                        endpoint: devnetFunding.endpoint,
                        asset_id: devnetFunding.asset_id,
                        max_amount_atoms: devnetFunding.max_amount_atoms,
                        max_recipient_window_atoms: devnetFunding.max_recipient_window_atoms,
                        recipient_window_ms: devnetFunding.recipient_window_ms,
                        submit_method: 'mempool_submit_signed_asset_transaction_finality',
                        custody_boundary: devnetFunding.custody_boundary,
                    },
                },
                shielded_navswap: {
                    label: 'Shielded NAVSwap',
                    status: shieldedEgress.configured
                        ? 'step9_egress_ready'
                        : shieldedSwap.configured
                        ? 'step7_swap_ready'
                        : shieldedQuote.configured
                        ? 'step6_quote_ready'
                        : shieldedIngress.configured
                            ? 'step5_ingress_ready'
                            : 'step6_quote_configuration_required',
                    enabled: shieldedIngress.configured || shieldedQuote.configured,
                    can_quote: shieldedQuote.configured,
                    can_run: shieldedSwap.configured,
                    can_ingress: shieldedIngress.configured,
                    can_egress: shieldedEgress.configured,
                    bridge_out_requires_public_exit_receipt: true,
                    custody_boundary: shieldedEgress.configured
                        ? shieldedEgress.custody_boundary
                        : shieldedSwap.configured
                        ? shieldedSwap.custody_boundary
                        : shieldedIngress.custody_boundary,
                    requires_local_prover: true,
                    requires_note_scan: true,
                    liquidity_mode: shieldedQuote.liquidity_mode,
                    liquidity_source_class: shieldedQuote.liquidity_mode,
                    reason: shieldedQuote.configured
                        ? (shieldedEgress.configured
                            ? 'Private swap and explicit public exit are enabled for the controlled Step 9 route.'
                            : shieldedSwap.configured
                            ? 'Private submit is enabled for the controlled Step 7 route; public exit remains separate.'
                            : shieldedQuote.copy)
                        : `Private quote preview is blocked until operator quote config is present: ${shieldedQuote.missing.join(', ') || 'unknown configuration missing'}.`,
                    privacy: navswapRoutePrivacy({
                        mode: shieldedEgress.configured ? 'wallet_local_private_swap_explicit_public_exit_boundary' : shieldedSwap.configured ? 'wallet_local_private_swap_submit_boundary' : 'wallet_local_quote_and_ingress_boundary',
                        label: shieldedEgress.configured ? 'Private swap with explicit public exit' : shieldedSwap.configured ? 'Private swap submit' : 'Private quote preview',
                        disclosureLabel: shieldedEgress.configured
                            ? 'Swap output stays private by default. Public exit reveals destination, asset, amount, and timing; note opening and spend authority stay wallet-local.'
                            : 'Quote request, liquidity commitment, and opaque swap action are visible; private notes and spend authority stay wallet-local.',
                        publicFields: ['wallet_address', 'from_asset', 'to_asset', 'amount_atoms', 'quote_binding_hash', 'public_ingress_burn', 'burn_transaction', 'egress_destination', 'egress_asset_id', 'egress_amount_atoms', 'egress_receipt_timing'],
                        disclosedFields: ['liquidity_mode', 'liquidity_commitment', 'policy_hash', 'quote_expiry', 'public_ingress_payload', 'output_commitment', 'swap_nullifiers', 'swap_output_commitments', 'egress_nullifier', 'egress_exit_binding_hash'],
                        privateFields: ['wallet_local_notes', 'wallet_spend_authorization'],
                        warning: shieldedEgress.configured
                            ? 'Step 9 is CONTROLLED: private exit is explicit and certified, while bridge-out must wait for a public-exit receipt.'
                            : shieldedSwap.configured
                            ? 'Step 7 is CONTROLLED: quote freshness and liquidity commitment are proxy-checked, not circuit-external-bound.'
                            : 'Quote preview and public ingress are enabled only when configured. Private proof/swap submit remains disabled until the Step 7 review gate.',
                    }),
                    quote: {
                        schema: shieldedQuote.schema,
                        enabled: shieldedQuote.configured,
                        endpoint: shieldedQuote.endpoint,
                        quote_ttl_ms: shieldedQuote.quote_ttl_ms,
                        liquidity_mode: shieldedQuote.liquidity_mode,
                        liquidity_mode_label: shieldedQuote.liquidity_mode_label,
                        liquidity_source_class: shieldedQuote.liquidity_mode,
                        liquidity_commitment: shieldedQuote.liquidity_commitment,
                        liquidity_provider: shieldedQuote.liquidity_provider,
                        liquidity_commitment_status: shieldedQuote.liquidity_commitment_status,
                        policy_hash: shieldedQuote.policy_hash,
                        failure_mode: shieldedQuote.failure_mode,
                        trust_class: shieldedQuote.trust_class,
                        submit_gate: shieldedQuote.submit_gate,
                        missing: shieldedQuote.missing,
                        liquidity: {
                            mode: shieldedQuote.liquidity_mode,
                            mode_label: shieldedQuote.liquidity_mode_label,
                            source_class: shieldedQuote.liquidity_mode,
                            trust_class: shieldedQuote.trust_class,
                            counterparty: shieldedQuote.liquidity_provider,
                            commitment: shieldedQuote.liquidity_commitment,
                            commitment_status: shieldedQuote.liquidity_commitment_status,
                            copy: shieldedQuote.copy,
                        },
                    },
                    asset_registry: shieldedQuote.asset_registry,
                    supported_pairs: shieldedQuote.supported_pairs,
                    ingress: {
                        enabled: shieldedIngress.configured,
                        endpoint: shieldedIngress.endpoint,
                        max_amount_atoms: shieldedIngress.max_amount_atoms,
                        supported_assets: shieldedIngress.supported_assets,
                        requires_browser_signed_burn: true,
                        adapter_custody: 'certify-only-no-wallet-secret-material',
                        missing: shieldedIngress.missing,
                    },
                    swap: {
                        enabled: shieldedSwap.configured,
                        endpoint: shieldedSwap.endpoint,
                        trust_class: 'CONTROLLED',
                        missing: shieldedSwap.missing,
                        quote_binding_enforcement: 'proxy_checked_quote_freshness_and_liquidity_commitment_not_circuit_external_binding',
                    },
                    egress: {
                        enabled: shieldedEgress.configured,
                        endpoint: shieldedEgress.endpoint,
                        trust_class: 'CONTROLLED',
                        missing: shieldedEgress.missing,
                        policy_id: shieldedEgress.policy_id,
                        disclosure_required: true,
                        bridge_out_requires_public_exit_receipt: true,
                        public_disclosure: ['destination', 'asset_id', 'amount_atoms', 'receipt_timing'],
                        private_fields: ['note_opening', 'spend_authority', 'wallet_local_note_file'],
                        custody_boundary: shieldedEgress.custody_boundary,
                    },
                    operator_demo_paths: ['/api/shielded-nav-swap/status', '/api/shielded-nav-swap/quote', '/api/shielded-nav-swap/swap', '/api/shielded-nav-swap/egress'],
                },
                stakehub_transparent_roundtrip: {
                    label: 'StakeHub transparent roundtrip',
                    status: stakehubTransparent.configured
                        ? (stakehubTransparent.runs_enabled ? 'operator_run_enabled' : 'operator_quote_only')
                        : 'operator_not_configured',
                    enabled: stakehubTransparent.configured,
                    can_quote: stakehubTransparent.configured,
                    can_run: stakehubTransparent.configured && stakehubTransparent.runs_enabled,
                    reason: stakehubTransparent.configured
                        ? 'Existing StakeHub transparent no-Orchard PFTL roundtrip is reachable through the adapter; it uses the StakeHub operator wallet, not browser-local signing.'
                        : 'Set NAVSWAP_STAKEHUB_BASE_URL to expose the existing StakeHub transparent no-Orchard PFTL roundtrip as an operator-backed smoke route.',
                    privacy: navswapRoutePrivacy({
                        mode: 'public_operator_backed',
                        label: 'Public operator route',
                        disclosureLabel: 'Operator-backed smoke route; wallet-local custody is not claimed.',
                        publicFields: ['wallet_address', 'amount', 'operator_run_status', 'receipts'],
                        disclosedFields: ['stakehub_operator_request'],
                        privateFields: ['wallet_seed', 'wallet_private_key'],
                    }),
                    config: stakehubTransparent,
                    supported_pairs: ['pfUSDC->a651 smoke amount'],
                },
                pftl_atomic_settlement: {
                    label: 'PFTL atomic settlement',
                    status: 'template_ready',
                    enabled: true,
                    can_quote: true,
                    can_run: false,
                    reason: 'ESCROW-009 template generation is exposed; execution still requires both wallets to sign their own escrow-create legs.',
                    privacy: navswapRoutePrivacy({
                        mode: 'public_atomic_template',
                        label: 'Public atomic',
                        disclosureLabel: 'Owners, recipients, assets, amounts, condition hash, and escrow receipts are public.',
                        publicFields: ['left_owner', 'right_owner', 'left_recipient', 'right_recipient', 'asset_ids', 'amounts', 'condition_hash', 'escrow_ids'],
                        privateFields: ['fulfillment_preimage_until_finish', 'wallet_private_keys'],
                    }),
                    endpoint: '/api/navswap/atomic-templates',
                    supported_pairs: ['PFT<->issued_asset'],
                    unsupported_pairs: ['issued_asset<->issued_asset without an explicit PFT intermediary'],
                },
                uniswap_atomic_handoff: {
                    label: 'PFTL to Uniswap atomic handoff',
                    route_family: 'composite_primary_mint_to_ethereum_venue',
                    route_trust_class: bridge.route_trust_class,
                    route_config_digest: bridge.route_config_digest,
                    release_stage: bridge.explicit_beta ? 'explicit_beta' : 'disabled',
                    explicit_beta: bridge.explicit_beta,
                    public_routing_enabled: bridge.public_routing_enabled,
                    paused: bridge.paused,
                    route_supply_cap_atoms: bridge.route_supply_cap_atoms,
                    supply_cap_remaining_atoms: bridge.supply_cap_remaining_atoms,
                    packet_notional_cap_atoms: bridge.packet_notional_cap_atoms,
                    status: bridge.legacy_pool_selected
                        ? 'disabled_legacy_pool_rejected'
                        : uniswapBeta.status,
                    enabled: uniswapBeta.quote_enabled,
                    can_quote: uniswapBeta.quote_enabled,
                    can_run: uniswapBeta.run_enabled,
                    reason: uniswapBeta.quote_enabled
                        ? 'Controlled beta PFTL-Uniswap handoff is explicitly enabled with route caps and CONTROLLED trust class.'
                        : 'Requires a bridge-aware wrapped NAVCoin token, handoff controller, verifier mode, router, and new Uniswap pool.',
                    required_next: uniswapBeta.blockers,
                    privacy: navswapRoutePrivacy({
                        mode: uniswapBeta.quote_enabled ? 'controlled_beta_public_handoff' : 'disabled_public_handoff',
                        label: uniswapBeta.quote_enabled ? 'CONTROLLED beta' : 'Disabled public handoff',
                        disclosureLabel: 'Would disclose PFTL debit, bridge packet, Ethereum path, recipient, deadline, and swap receipts.',
                        publicFields: ['pool', 'path', 'router', 'token_in', 'token_out', 'amount_in', 'min_output', 'recipient', 'deadline'],
                        privateFields: ['wallet_private_keys'],
                        warning: uniswapBeta.quote_enabled
                            ? 'Operator-controlled beta route. Public routing is disabled.'
                            : 'Disabled until a bridge-aware wrapped NAVCoin token, handoff controller, verifier mode, and new Uniswap pool are configured.',
                    }),
                    current_pair: {
                        from_asset: 'pfUSDC',
                        to_asset: 'a651',
                        amount_asset: 'a651',
                        settlement_asset: 'pfUSDC',
                        amount_semantics: 'display_nav_amount_decimal',
                        amount_precision: 6,
                    },
                    prepared_action_schema: NAVSWAP_WALLET_ACTION_SCHEMA,
                    prepare_action_batch_endpoint: '/api/navswap/actions/prepare-batch',
                    prepared_action_batch_schema: NAVSWAP_WALLET_ACTION_BATCH_PREPARE_SCHEMA,
                    prepared_action_stages: ['pftl_uniswap_primary_subscribe', 'pftl_uniswap_export_debit'],
                    wallet_owned_actions: ['pftl_uniswap_primary_subscribe', 'pftl_uniswap_export_debit'],
                    operator_owned_actions: ['pftl_uniswap_destination_consume', 'pftl_uniswap_return_import', 'pftl_uniswap_refund_source'],
                    operator_completion: {
                        custody_boundary: 'operator-attested-controlled-beta',
                        destination_consume: 'operator-attested until Gate 5 verifier work lands',
                        return_import: 'operator-attested return relay',
                    },
                    config: bridge,
                },
                legacy_a651_uniswap: {
                    label: 'Legacy a651/USDC pool',
                    status: 'inspection_only',
                    enabled: false,
                    can_quote: false,
                    can_run: false,
                    reason: 'The historical a651/USDC pool is secondary liquidity, not the active PFTL handoff route.',
                    privacy: navswapRoutePrivacy({
                        mode: 'legacy_public_inspection',
                        label: 'Public inspection',
                        disclosureLabel: 'Legacy Ethereum pool inspection only; not an active wallet route.',
                        publicFields: ['chain_id', 'token', 'pool_id', 'usdc'],
                        privateFields: [],
                    }),
                    chain_id: 1,
                    token: LEGACY_A651_ETH_TOKEN,
                    pool_id: LEGACY_A651_UNISWAP_POOL_ID,
                    usdc: ETHEREUM_USDC_TOKEN,
                },
            },
        };
    }

    async function executeNavswapCapabilities(now = new Date()) {
        const caps = navswapCapabilities(now);
        const route = caps.routes?.stakehub_transparent_roundtrip;
        if (!route?.enabled) return caps;

        const preflight = await buildStakehubTransparentPreflight();
        route.preflight = preflight;
        if (preflight.ok !== true) {
            route.status = preflight.code || 'preflight_unavailable';
            route.enabled = false;
            route.can_quote = false;
            route.can_run = false;
            route.reason = preflight.message || 'StakeHub transparent roundtrip preflight is unavailable.';
        }
        return caps;
    }


    return { SHIELDED_PRIVATE_KEY_PATTERNS,assertNoShieldedPrivateMaterial,assertVaultBridgeEvidenceMatches,assetOrchardLocalServiceConfig,buildUniswapHandoffQuoteBinding,buildVaultBridgeRelayBundle,clearNavswapDevnetFundingUsageForTest,currentA652AssetId,ensureVaultBridgeRecipientAccount,executeNavswapCapabilities,executeVaultBridgeRelay,findShieldedPrivateMaterialPaths,governedVaultBridgeRelayConfig,isBadSequenceSubmitResponse,isReplayableVaultBridgeRelayDuplicate,lower,navswapBridgeConfig,navswapCapabilities,navswapDevnetFundingUsageSnapshot,navswapDevnetFundingWindowUsage,navswapDevnetPfusdcFundingConfig,navswapInferTrustClass,navswapNormalizeTrustClass,navswapRoutePrivacy,navswapStakehubTransparentConfig,navswapTransparentOperatorConfig,navswapTrustlessFinalityAgreement,navswapUniswapBetaRouteState,normalizeShieldedKey,normalizeShieldedLiquidityMode,normalizeVaultBridgeAddress,normalizeVaultBridgeBytes32,normalizeVaultBridgeTxHash,parseUniswapHandoffBytes32,parseUniswapHandoffPositiveInteger,presentEnv,presentPositiveSafeIntegerEnv,readNavswapKeyFileAddress,releaseNavswapDevnetFundingUsage,reserveNavswapDevnetFundingUsage,routedRpcRead,shieldedLiquidityModeLabel,shieldedNavswapEgressConfig,shieldedNavswapIngressConfig,shieldedNavswapQuoteConfig,shieldedNavswapSwapConfig,shieldedQuotePolicyHash,signAndSubmitVaultBridgeRecipientSponsor,signAndSubmitVaultBridgeRelayOperation,vaultBridgeAccountAssets,vaultBridgeBodyTxHash,vaultBridgeEvidenceFromPlan,vaultBridgeExpectedField,vaultBridgePftlAccountExists,vaultBridgeRelayConfig };
}

module.exports = { create };
