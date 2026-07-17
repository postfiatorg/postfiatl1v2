'use strict';

function create(runtime) {
    const { A651_ASSET_ID,A652_ASSET_ID,ALLOWED_ORIGINS,ASSET_ORCHARD_ACTION_CLEAR_KEYS,ASSET_ORCHARD_INGRESS_FILE_SCHEMA,ASSET_ORCHARD_POOL_ID,ASSET_ORCHARD_PRIVATE_EGRESS_ACTION_SCHEMA,ASSET_ORCHARD_PRIVATE_EGRESS_FILE_SCHEMA,ASSET_ORCHARD_SWAP_ACTION_SCHEMA,DEFAULT_ASSET_ORCHARD_LOCAL_SERVICE_READINESS_TIMEOUT_MS,DEFAULT_ASSET_ORCHARD_LOCAL_SERVICE_URL,DEFAULT_RPC_FLEET,ENABLE_FINALITY_RESPONDER_READ_CACHE,ENABLE_FIRST_READY_SEQUENCED_READ,ENABLE_NATIVE_WALLET_SIGNER,ENABLE_PROPOSER_ROUTING,ENABLE_RPC_PARENT_WAIT_FINALITY_ROUTE,ENABLE_RPC_PARENT_WAIT_SEQUENCED_READ_ROUTE,ENABLE_UPSTREAM_KEEPALIVE,ETHEREUM_USDC_TOKEN,FASTPAY_BROADCAST_METHODS,FASTPAY_FLEET_STATUS_CACHE_MS,FASTPAY_OWNED_OBJECT_LOOKUP_LIMIT,FASTPAY_REQUIRE_PRIMARY_SUCCESS,FASTPAY_ROUTE_RETRY_MS,FASTPAY_ROUTE_TIMEOUT_MS,FINALITY_METHODS,FIRST_READY_SEQUENCED_READ_PROPOSERS,INJECT_RPC_CAPS,LEGACY_A651_ETH_TOKEN,LEGACY_A651_UNISWAP_POOL_ID,LISTEN_PORT,MAX_TCP_PER_WS,MAX_WS_MESSAGE_BYTES,NATIVE_WALLET_SIGNER_BIN,NATIVE_WALLET_SIGNER_TIMEOUT_MS,NAVSWAP_CAPABILITIES_SCHEMA,NAVSWAP_DEVNET_FUNDING_SCHEMA,NAVSWAP_IDEMPOTENCY_STORE_DEFAULT_PATH,NAVSWAP_IDEMPOTENCY_STORE_SCHEMA,NAVSWAP_IDEMPOTENCY_TTL_MS,NAVSWAP_MAX_LIVE_USD,NAVSWAP_NAV_PROOF_SCHEMA,NAVSWAP_PRIMARY_MINT_ROUTE_FAMILY,NAVSWAP_QUOTE_FRESHNESS_TTL_MS,NAVSWAP_QUOTE_SCHEMA,NAVSWAP_READINESS_SCHEMA,NAVSWAP_ROUTE_TRUST_CLASSES,NAVSWAP_RUN_EVENTS_SCHEMA,NAVSWAP_RUN_LIST_SCHEMA,NAVSWAP_RUN_RECEIPTS_SCHEMA,NAVSWAP_RUN_SCHEMA,NAVSWAP_RUN_STATUS_SCHEMA,NAVSWAP_RUN_STORE_DEFAULT_PATH,NAVSWAP_RUN_STORE_SCHEMA,NAVSWAP_RUN_STREAM_EVENT_SCHEMA,NAVSWAP_RUN_STREAM_SCHEMA,NAVSWAP_SETTLEMENT_RECEIPT_MAX_SNAPSHOT_AGE_BLOCKS,NAVSWAP_SETTLEMENT_RECEIPT_SAFETY_BLOCKS,NAVSWAP_STAKEHUB_TRANSPARENT_ACTION,NAVSWAP_TRANSPARENT_PLANNER_INPUTS_SCHEMA,NAVSWAP_WALLET_ACTION_BATCH_PREPARE_SCHEMA,NAVSWAP_WALLET_ACTION_PREPARE_SCHEMA,NAVSWAP_WALLET_ACTION_SCHEMA,OPTIMISTIC_CACHED_FINALITY_ROUTE,PFUSDC_ASSET_ID,PREFERRED_SEQUENCED_READ_VALIDATORS,PROPOSER_READY_RETRY_MS,PROPOSER_ROUTE_CACHE_MS,PROPOSER_ROUTE_RETRY_MS,PROPOSER_ROUTE_TIMEOUT_MS,RPC_CAPS,RPC_FLEET,RPC_HOST,RPC_PORT,SEQUENCED_ACCOUNT_METHODS,SHIELDED_NAVSWAP_EGRESS_POLICY_ID,SHIELDED_NAVSWAP_EGRESS_SCHEMA,SHIELDED_NAVSWAP_INGRESS_SCHEMA,SHIELDED_NAVSWAP_LIQUIDITY_MODES,SHIELDED_NAVSWAP_PREFLIGHT_SCHEMA,SHIELDED_NAVSWAP_QUOTE_SCHEMA,SHIELDED_NAVSWAP_STATUS_SCHEMA,SHIELDED_NAVSWAP_SWAP_SCHEMA,SHIELDED_PRIVATE_KEY_PATTERNS,SHIELDED_ROUND_TIMEOUT_DEFAULT_MS,TCP_TIMEOUT_MS,UpstreamRpcConnection,VAULT_BRIDGE_ALLOCATION_PURPOSE_NAV_SUBSCRIPTION,VAULT_BRIDGE_ALLOCATION_PURPOSE_SUPPLY,VAULT_BRIDGE_BUCKET_STATUS_ACTIVE,VAULT_BRIDGE_RECEIPT_STATUS_COUNTED,VAULT_BRIDGE_RECIPIENT_SPONSOR_AMOUNT,VAULT_BRIDGE_RECIPIENT_SPONSOR_MIN_AMOUNT_ATOMS,VAULT_BRIDGE_RELAY_DEFAULT_ACCOUNT,VAULT_BRIDGE_RELAY_EXPIRES_AT_HEIGHT,VAULT_BRIDGE_RELAY_POLICY_HASH,VAULT_BRIDGE_RELAY_SCHEMA,VAULT_BRIDGE_RELAY_SOURCE_CHAIN_ID,VAULT_BRIDGE_RELAY_SOURCE_RPC_URL,VAULT_BRIDGE_RELAY_TOKEN_ADDRESS,VAULT_BRIDGE_RELAY_VAULT_ADDRESS,WALLET_SUBSCRIPTION_INTERVAL_MS,WALLET_SUBSCRIPTION_MIN_INTERVAL_MS,WALLET_SUBSCRIPTION_READ_TIMEOUT_MS,crypto,execFileAsync,fs,http,navswapDevnetFundingUsage,navswapIdempotencyRecords,navswapRunStreams,navswapRuns,net,os,path,server,upstreamRpcConnections,wss } = runtime;
    let { fastpayFleetStatusCache,fastpayFleetStatusInFlight,latestFinalizedReadCache,preferredSequencedReadIndex,proposerRouteCache,shieldedCertifierLoopState } = runtime;
    const httpMutationAuthorized = (...args) => runtime.httpMutationAuthorized(...args);
    const httpMutationPrincipal = (...args) => runtime.httpMutationPrincipal(...args);
    const httpRequestRequiresAuth = (...args) => runtime.httpRequestRequiresAuth(...args);
    const acquireMutationAdmission = (...args) => runtime.acquireMutationAdmission(...args);
    const boundedHttpBodyLimit = (...args) => runtime.boundedHttpBodyLimit(...args);
    const addProxyRouteEvent = (...args) => runtime.addProxyRouteEvent(...args);
    const assertNoShieldedPrivateMaterial = (...args) => runtime.assertNoShieldedPrivateMaterial(...args);
    const assertVaultBridgeEvidenceMatches = (...args) => runtime.assertVaultBridgeEvidenceMatches(...args);
    const assetIdForNavswapSymbol = (...args) => runtime.assetIdForNavswapSymbol(...args);
    const assetOrchardLocalServiceConfig = (...args) => runtime.assetOrchardLocalServiceConfig(...args);
    const bftQuorumThreshold = (...args) => runtime.bftQuorumThreshold(...args);
    const broadcastFastpayMutation = (...args) => runtime.broadcastFastpayMutation(...args);
    const buildNavswapNavProofResponse = (...args) => runtime.buildNavswapNavProofResponse(...args);
    const buildNavswapQuoteResponse = (...args) => runtime.buildNavswapQuoteResponse(...args);
    const buildPftlUniswapReceiptVerification = (...args) => runtime.buildPftlUniswapReceiptVerification(...args);
    const buildShieldedCertifiedRoundArgs = (...args) => runtime.buildShieldedCertifiedRoundArgs(...args);
    const buildStakehubTransparentPreflight = (...args) => runtime.buildStakehubTransparentPreflight(...args);
    const buildTransparentNavswapReceiptVerification = (...args) => runtime.buildTransparentNavswapReceiptVerification(...args);
    const buildTransparentNavswapRedeemReceiptVerification = (...args) => runtime.buildTransparentNavswapRedeemReceiptVerification(...args);
    const buildUniswapHandoffQuoteBinding = (...args) => runtime.buildUniswapHandoffQuoteBinding(...args);
    const buildUrl = (...args) => runtime.buildUrl(...args);
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
    const cloneJson = (...args) => runtime.cloneJson(...args);
    const closeUpstreamRpcConnections = (...args) => runtime.closeUpstreamRpcConnections(...args);
    const collectFastpayFleetStatuses = (...args) => runtime.collectFastpayFleetStatuses(...args);
    const collectFleetStatuses = (...args) => runtime.collectFleetStatuses(...args);
    const collectShieldedTopologyStatuses = (...args) => runtime.collectShieldedTopologyStatuses(...args);
    const completePftlUniswapHandoffRun = (...args) => runtime.completePftlUniswapHandoffRun(...args);
    const completeTransparentNavswapRun = (...args) => runtime.completeTransparentNavswapRun(...args);
    const conciseRpcError = (...args) => runtime.conciseRpcError(...args);
    const convergedFleetGroup = (...args) => runtime.convergedFleetGroup(...args);
    const createShieldedSwapBatchViaLocalService = (...args) => runtime.createShieldedSwapBatchViaLocalService(...args);
    const currentA652AssetId = (...args) => runtime.currentA652AssetId(...args);
    const deterministicProposer = (...args) => runtime.deterministicProposer(...args);
    const endpointStatusMeetsRoute = (...args) => runtime.endpointStatusMeetsRoute(...args);
    const endpointStatusMeetsSequencedReadRoute = (...args) => runtime.endpointStatusMeetsSequencedReadRoute(...args);
    const ensureVaultBridgeRecipientAccount = (...args) => runtime.ensureVaultBridgeRecipientAccount(...args);
    const executeNavswapCapabilities = (...args) => runtime.executeNavswapCapabilities(...args);
    const executeNavswapDevnetPfusdcFunding = (...args) => runtime.executeNavswapDevnetPfusdcFunding(...args);
    const executeNavswapQuote = (...args) => runtime.executeNavswapQuote(...args);
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
    const executeVaultBridgeRelay = (...args) => runtime.executeVaultBridgeRelay(...args);
    const fetchJsonWithTimeout = (...args) => runtime.fetchJsonWithTimeout(...args);
    const fetchWalletSnapshot = (...args) => runtime.fetchWalletSnapshot(...args);
    const fileMtimeUnixMs = (...args) => runtime.fileMtimeUnixMs(...args);
    const findAssetOrchardActionCleartext = (...args) => runtime.findAssetOrchardActionCleartext(...args);
    const findShieldedPrivateMaterialPaths = (...args) => runtime.findShieldedPrivateMaterialPaths(...args);
    const firstReadyEndpointForRoute = (...args) => runtime.firstReadyEndpointForRoute(...args);
    const firstStructuredFastpayResult = (...args) => runtime.firstStructuredFastpayResult(...args);
    const invalidateProposerRouteCache = (...args) => runtime.invalidateProposerRouteCache(...args);
    const isBadSequenceSubmitResponse = (...args) => runtime.isBadSequenceSubmitResponse(...args);
    const isFastpayBroadcastMethod = (...args) => runtime.isFastpayBroadcastMethod(...args);
    const isFinalityMethod = (...args) => runtime.isFinalityMethod(...args);
    const isIssuedAsset = (...args) => runtime.isIssuedAsset(...args);
    const isNativeWalletSignMethod = (...args) => runtime.isNativeWalletSignMethod(...args);
    const isPftAsset = (...args) => runtime.isPftAsset(...args);
    const isReplayableVaultBridgeRelayDuplicate = (...args) => runtime.isReplayableVaultBridgeRelayDuplicate(...args);
    const isSequencedAccountMethod = (...args) => runtime.isSequencedAccountMethod(...args);
    const loadPftlUniswapWalletActionContext = (...args) => runtime.loadPftlUniswapWalletActionContext(...args);
    const loadShieldedTopologyPeers = (...args) => runtime.loadShieldedTopologyPeers(...args);
    const lower = (...args) => runtime.lower(...args);
    const majorityRootAtHeight = (...args) => runtime.majorityRootAtHeight(...args);
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
    const navswapIdempotencyKeyFromRequest = (...args) => runtime.navswapIdempotencyKeyFromRequest(...args);
    const navswapIdempotencyStorePath = (...args) => runtime.navswapIdempotencyStorePath(...args);
    const navswapInferTrustClass = (...args) => runtime.navswapInferTrustClass(...args);
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
    const navswapRunStorePath = (...args) => runtime.navswapRunStorePath(...args);
    const navswapSafeU64Number = (...args) => runtime.navswapSafeU64Number(...args);
    const navswapSettlementReceiptFreshnessConfig = (...args) => runtime.navswapSettlementReceiptFreshnessConfig(...args);
    const navswapSettlementReceiptHash = (...args) => runtime.navswapSettlementReceiptHash(...args);
    const navswapStableJson = (...args) => runtime.navswapStableJson(...args);
    const navswapStakehubTransparentConfig = (...args) => runtime.navswapStakehubTransparentConfig(...args);
    const navswapSubscriptionId = (...args) => runtime.navswapSubscriptionId(...args);
    const navswapTransparentOperatorConfig = (...args) => runtime.navswapTransparentOperatorConfig(...args);
    const navswapTrustlessFinalityAgreement = (...args) => runtime.navswapTrustlessFinalityAgreement(...args);
    const navswapUniswapBetaRouteState = (...args) => runtime.navswapUniswapBetaRouteState(...args);
    const navswapValidateIdempotencyKey = (...args) => runtime.navswapValidateIdempotencyKey(...args);
    const navswapValuationUnitScale = (...args) => runtime.navswapValuationUnitScale(...args);
    const navswapWalletActionBatchItems = (...args) => runtime.navswapWalletActionBatchItems(...args);
    const navswapWalletActionId = (...args) => runtime.navswapWalletActionId(...args);
    const newNavswapRunId = (...args) => runtime.newNavswapRunId(...args);
    const normalizeFastpayBroadcastRequest = (...args) => runtime.normalizeFastpayBroadcastRequest(...args);
    const normalizePftlUniswapPacketStatus = (...args) => runtime.normalizePftlUniswapPacketStatus(...args);
    const normalizeShieldedKey = (...args) => runtime.normalizeShieldedKey(...args);
    const normalizeShieldedLiquidityMode = (...args) => runtime.normalizeShieldedLiquidityMode(...args);
    const normalizeVaultBridgeAddress = (...args) => runtime.normalizeVaultBridgeAddress(...args);
    const normalizeVaultBridgeBytes32 = (...args) => runtime.normalizeVaultBridgeBytes32(...args);
    const normalizeVaultBridgeTxHash = (...args) => runtime.normalizeVaultBridgeTxHash(...args);
    const normalizeWalletSubscriptionParams = (...args) => runtime.normalizeWalletSubscriptionParams(...args);
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
    const parseUniswapHandoffBytes32 = (...args) => runtime.parseUniswapHandoffBytes32(...args);
    const parseUniswapHandoffPositiveInteger = (...args) => runtime.parseUniswapHandoffPositiveInteger(...args);
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
    const readFleetRpcMajority = (...args) => runtime.readFleetRpcMajority(...args);
    const readGroupKey = (...args) => runtime.readGroupKey(...args);
    const readNavswapKeyFileAddress = (...args) => runtime.readNavswapKeyFileAddress(...args);
    const releaseNavswapDevnetFundingUsage = (...args) => runtime.releaseNavswapDevnetFundingUsage(...args);
    const rememberFinalizedReadEndpoint = (...args) => runtime.rememberFinalizedReadEndpoint(...args);
    const requestWithProxyReadiness = (...args) => runtime.requestWithProxyReadiness(...args);
    const reserveNavswapDevnetFundingUsage = (...args) => runtime.reserveNavswapDevnetFundingUsage(...args);
    const resolveRpcTarget = (...args) => runtime.resolveRpcTarget(...args);
    const responseEnvelope = (...args) => runtime.responseEnvelope(...args);
    const routedRpcRead = (...args) => runtime.routedRpcRead(...args);
    const rpcTcpRequest = (...args) => runtime.rpcTcpRequest(...args);
    runtime.markAtomicProxyRoutableTransport(rpcTcpRequest);
    const rpcTcpRequestLine = (...args) => runtime.rpcTcpRequestLine(...args);
    const rpcTcpRequestOneShotLine = (...args) => runtime.rpcTcpRequestOneShotLine(...args);
    const runShieldedLaggardCatchUp = (...args) => runtime.runShieldedLaggardCatchUp(...args);
    const runShieldedRpcCatchUp = (...args) => runtime.runShieldedRpcCatchUp(...args);
    const selectNavswapIssuedSettlementSource = (...args) => runtime.selectNavswapIssuedSettlementSource(...args);
    const selectTransparentRedeemSettlementAllocation = (...args) => runtime.selectTransparentRedeemSettlementAllocation(...args);
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
    const signAndSubmitNavswapOperatorAssetTransaction = (...args) => runtime.signAndSubmitNavswapOperatorAssetTransaction(...args);
    const signAndSubmitVaultBridgeRecipientSponsor = (...args) => runtime.signAndSubmitVaultBridgeRecipientSponsor(...args);
    const signAndSubmitVaultBridgeRelayOperation = (...args) => runtime.signAndSubmitVaultBridgeRelayOperation(...args);
    const signWalletOwnedOrder = (...args) => runtime.signWalletOwnedOrder(...args);
    const sleep = (...args) => runtime.sleep(...args);
    const stakehubTransparentAmountError = (...args) => runtime.stakehubTransparentAmountError(...args);
    const startCachedSelectionReadinessProbe = (...args) => runtime.startCachedSelectionReadinessProbe(...args);
    const startShieldedCertifierLoop = (...args) => runtime.startShieldedCertifierLoop(...args);
    const startWalletSubscription = (...args) => runtime.startWalletSubscription(...args);
    const stopWalletSubscription = (...args) => runtime.stopWalletSubscription(...args);
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
    const vaultBridgeAccountAssets = (...args) => runtime.vaultBridgeAccountAssets(...args);
    const vaultBridgeBodyTxHash = (...args) => runtime.vaultBridgeBodyTxHash(...args);
    const vaultBridgeEvidenceFromPlan = (...args) => runtime.vaultBridgeEvidenceFromPlan(...args);
    const vaultBridgeExpectedField = (...args) => runtime.vaultBridgeExpectedField(...args);
    const vaultBridgePftlAccountExists = (...args) => runtime.vaultBridgePftlAccountExists(...args);
    const vaultBridgeRelayConfig = (...args) => runtime.vaultBridgeRelayConfig(...args);
    const verifyPftlUniswapExportPacket = (...args) => runtime.verifyPftlUniswapExportPacket(...args);
    const verifyPftlUniswapWalletCompletionInput = (...args) => runtime.verifyPftlUniswapWalletCompletionInput(...args);
    const verifyTransparentNavRedeemSettlement = (...args) => runtime.verifyTransparentNavRedeemSettlement(...args);
    const verifyTransparentNavSubscriptionAllocation = (...args) => runtime.verifyTransparentNavSubscriptionAllocation(...args);
    const verifyTransparentWalletCompletionInput = (...args) => runtime.verifyTransparentWalletCompletionInput(...args);
    const waitForCachedSelectionReady = (...args) => runtime.waitForCachedSelectionReady(...args);
    const waitForFastpayConvergedGroup = (...args) => runtime.waitForFastpayConvergedGroup(...args);
    const walletSnapshotDigest = (...args) => runtime.walletSnapshotDigest(...args);

    function navswapIdempotencyHashBody(body = {}) {
        const normalized = cloneJson(body || {});
        if (normalized && typeof normalized === 'object' && !Array.isArray(normalized)) {
            delete normalized.idempotency_key;
            delete normalized.idempotencyKey;
        }
        return crypto.createHash('sha256').update(navswapStableJson(normalized)).digest('hex');
    }

    function pruneNavswapIdempotencyRecords(nowMs = Date.now()) {
        for (const [key, record] of navswapIdempotencyRecords.entries()) {
            if (record.expires_at_ms <= nowMs) navswapIdempotencyRecords.delete(key);
        }
    }

    function annotateNavswapIdempotency(payload, key, replayed, principalId = 'internal') {
        if (!key || !payload || typeof payload !== 'object' || Array.isArray(payload)) return payload;
        return {
            ...cloneJson(payload),
            idempotency_key: key,
            idempotency: {
                key,
                replayed,
                principal_id: principalId,
            },
        };
    }

    async function executeNavswapIdempotentRequest({ method = 'POST', pathname = '', body = {}, req = null } = {}, fn) {
        const key = navswapIdempotencyKeyFromRequest(req, body);
        if (!key) return fn();

        const invalid = navswapValidateIdempotencyKey(key);
        if (invalid) return invalid;

        const nowMs = Date.now();
        pruneNavswapIdempotencyRecords(nowMs);
        const principalId = String(req?.walletProxyPrincipal || 'internal');
        const scope = `${String(method).toUpperCase()} ${pathname}`;
        const recordKey = JSON.stringify([principalId, scope, key]);
        const request_hash = navswapIdempotencyHashBody(body);
        const existing = navswapIdempotencyRecords.get(recordKey);
        if (existing) {
            if (existing.request_hash !== request_hash) {
                return {
                    ok: false,
                    schema: 'postfiat-navswap-idempotency-v1',
                    code: 'navswap_idempotency_key_reused',
                    message: 'NAVSwap idempotency_key was reused with a different request body.',
                    idempotency_key: key,
                    idempotency: {
                        key,
                        replayed: false,
                        conflict: true,
                        scope,
                        principal_id: principalId,
                    },
                };
            }
            const payload = existing.response || await existing.promise;
            return annotateNavswapIdempotency(payload, key, true, principalId);
        }

        const record = {
            scope,
            principal_id: principalId,
            idempotency_key: key,
            request_hash,
            created_at_ms: nowMs,
            expires_at_ms: nowMs + Math.max(1000, NAVSWAP_IDEMPOTENCY_TTL_MS),
            response: null,
            promise: null,
        };
        record.promise = Promise.resolve()
            .then(fn)
            .then((payload) => {
                record.response = payload;
                record.promise = null;
                persistNavswapIdempotencyRecord(recordKey, record);
                return payload;
            })
            .catch((error) => {
                navswapIdempotencyRecords.delete(recordKey);
                throw error;
            });
        navswapIdempotencyRecords.set(recordKey, record);
        const payload = await record.promise;
        return annotateNavswapIdempotency(payload, key, false, principalId);
    }

    function clearNavswapIdempotencyForTest() {
        navswapIdempotencyRecords.clear();
    }

    function navswapIdempotencyStoreSnapshot(recordKey, record) {
        return {
            schema: NAVSWAP_IDEMPOTENCY_STORE_SCHEMA,
            kind: 'record',
            record_key: recordKey,
            stored_at: new Date().toISOString(),
            record: {
                scope: record.scope,
                principal_id: record.principal_id,
                idempotency_key: record.idempotency_key || null,
                request_hash: record.request_hash,
                created_at_ms: record.created_at_ms,
                expires_at_ms: record.expires_at_ms,
                response: cloneJson(record.response),
            },
        };
    }

    function persistNavswapIdempotencyRecord(recordKey, record) {
        const storePath = navswapIdempotencyStorePath();
        if (!storePath || !recordKey || !record?.response) return { ok: true, enabled: false };
        try {
            fs.mkdirSync(path.dirname(storePath), { recursive: true });
            fs.appendFileSync(storePath, `${JSON.stringify(navswapIdempotencyStoreSnapshot(recordKey, record))}\n`, 'utf8');
            return { ok: true, enabled: true, path: storePath };
        } catch (error) {
            console.warn(`NAVSwap idempotency store write failed: ${error.message || error}`);
            return { ok: false, enabled: true, path: storePath, error: error.message || String(error) };
        }
    }

    function normalizeStoredNavswapIdempotencyRecord(snapshot, nowMs = Date.now()) {
        if (!snapshot || typeof snapshot !== 'object') return null;
        const source = [
            NAVSWAP_IDEMPOTENCY_STORE_SCHEMA,
            'postfiat-navswap-idempotency-store-v1',
        ].includes(snapshot.schema)
            ? (snapshot.record || {})
            : snapshot;
        if (typeof source !== 'object') return null;
        if (typeof source.scope !== 'string' || typeof source.request_hash !== 'string') return null;
        const principalId = typeof source.principal_id === 'string'
            ? source.principal_id
            : 'default';
        const idempotencyKey = typeof source.idempotency_key === 'string'
            ? source.idempotency_key
            : null;
        if (!/^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$/.test(principalId) || !idempotencyKey) return null;
        const recordKey = JSON.stringify([principalId, source.scope, idempotencyKey]);
        const createdAtMs = Number(source.created_at_ms);
        const expiresAtMs = Number(source.expires_at_ms);
        if (!Number.isFinite(createdAtMs) || !Number.isFinite(expiresAtMs)) return null;
        if (expiresAtMs <= nowMs) return { expired: true };
        if (!source.response || typeof source.response !== 'object') return null;
        return {
            recordKey,
            record: {
                scope: source.scope,
                principal_id: principalId,
                idempotency_key: idempotencyKey,
                request_hash: source.request_hash,
                created_at_ms: createdAtMs,
                expires_at_ms: expiresAtMs,
                response: cloneJson(source.response),
                promise: null,
            },
        };
    }

    function loadNavswapIdempotencyStore(options = {}) {
        const storePath = navswapIdempotencyStorePath();
        if (!storePath) {
            return { ok: true, enabled: false, path: null, loaded_count: 0, skipped_count: 0, expired_count: 0 };
        }
        if (!fs.existsSync(storePath)) {
            return { ok: true, enabled: true, path: storePath, loaded_count: 0, skipped_count: 0, expired_count: 0 };
        }
        const nowMs = Number.isFinite(options.now_ms) ? Number(options.now_ms) : Date.now();
        const latest = new Map();
        let skippedCount = 0;
        let expiredCount = 0;
        let raw = '';
        try {
            raw = fs.readFileSync(storePath, 'utf8');
        } catch (error) {
            console.warn(`NAVSwap idempotency store read failed: ${error.message || error}`);
            return {
                ok: false,
                enabled: true,
                path: storePath,
                loaded_count: 0,
                skipped_count: 0,
                expired_count: 0,
                error: error.message || String(error),
            };
        }
        for (const line of raw.split(/\r?\n/)) {
            if (!line.trim()) continue;
            try {
                const parsed = JSON.parse(line);
                const normalized = normalizeStoredNavswapIdempotencyRecord(parsed, nowMs);
                if (normalized?.expired) {
                    expiredCount += 1;
                    continue;
                }
                if (!normalized) {
                    skippedCount += 1;
                    continue;
                }
                latest.set(normalized.recordKey, normalized.record);
            } catch (_) {
                skippedCount += 1;
            }
        }
        for (const [recordKey, record] of latest.entries()) {
            navswapIdempotencyRecords.set(recordKey, record);
        }
        return {
            ok: true,
            enabled: true,
            path: storePath,
            loaded_count: latest.size,
            skipped_count: skippedCount,
            expired_count: expiredCount,
        };
    }

    function normalizeStoredNavswapRun(snapshot) {
        if (!snapshot || typeof snapshot !== 'object' || typeof snapshot.run_id !== 'string') return null;
        const events = Array.isArray(snapshot.events) ? snapshot.events : [];
        const receipts = Array.isArray(snapshot.receipts) ? snapshot.receipts : [];
        const createdAt = typeof snapshot.created_at === 'string' ? snapshot.created_at : new Date().toISOString();
        const updatedAt = typeof snapshot.updated_at === 'string' ? snapshot.updated_at : createdAt;
        let ok = null;
        if (snapshot.ok === true) ok = true;
        if (snapshot.ok === false) ok = false;
        return {
            ok,
            run_id: snapshot.run_id,
            route: typeof snapshot.route === 'string' ? snapshot.route : 'unknown',
            status: typeof snapshot.status === 'string' ? snapshot.status : 'unknown',
            code: snapshot.code || null,
            message: typeof snapshot.message === 'string' ? snapshot.message : 'Restored NAVSwap run.',
            created_at: createdAt,
            updated_at: updatedAt,
            request: snapshot.request && typeof snapshot.request === 'object' ? snapshot.request : {},
            quote: snapshot.quote && typeof snapshot.quote === 'object' ? snapshot.quote : null,
            result: snapshot.result === undefined ? null : snapshot.result,
            error: snapshot.error && typeof snapshot.error === 'object' ? snapshot.error : null,
            events,
            receipts,
        };
    }

    function navswapRunStoreSnapshot(run) {
        return {
            schema: NAVSWAP_RUN_STORE_SCHEMA,
            kind: 'snapshot',
            run_id: run.run_id,
            stored_at: new Date().toISOString(),
            run: cloneJson(run),
        };
    }

    function persistNavswapRun(run) {
        const storePath = navswapRunStorePath();
        if (!storePath || !run?.run_id) return { ok: true, enabled: false };
        try {
            fs.mkdirSync(path.dirname(storePath), { recursive: true });
            fs.appendFileSync(storePath, `${JSON.stringify(navswapRunStoreSnapshot(run))}\n`, 'utf8');
            return { ok: true, enabled: true, path: storePath };
        } catch (error) {
            console.warn(`NAVSwap run store write failed: ${error.message || error}`);
            return { ok: false, enabled: true, path: storePath, error: error.message || String(error) };
        }
    }

    function markStoredNavswapRunInterrupted(run) {
        if (!run || navswapRunIsTerminal(run)) return false;
        const now = new Date().toISOString();
        run.ok = false;
        run.status = 'interrupted';
        run.code = 'navswap_run_interrupted';
        run.message = 'NAVSwap run was interrupted by a wallet proxy restart before it reached a terminal receipt.';
        run.error = {
            code: run.code,
            message: run.message,
        };
        run.updated_at = now;
        const alreadyRecorded = run.events.some((event) => event?.type === 'run_interrupted');
        if (!alreadyRecorded) {
            run.events.push({
                sequence: run.events.length,
                at: now,
                type: 'run_interrupted',
                message: run.message,
                details: { restored_from_store: true },
            });
        }
        return true;
    }

    function loadNavswapRunStore(options = {}) {
        const storePath = navswapRunStorePath();
        if (!storePath) {
            return { ok: true, enabled: false, path: null, loaded_count: 0, skipped_count: 0 };
        }
        if (!fs.existsSync(storePath)) {
            return { ok: true, enabled: true, path: storePath, loaded_count: 0, skipped_count: 0 };
        }
        const markInterrupted = options.mark_interrupted !== false;
        const latest = new Map();
        let skippedCount = 0;
        let raw = '';
        try {
            raw = fs.readFileSync(storePath, 'utf8');
        } catch (error) {
            console.warn(`NAVSwap run store read failed: ${error.message || error}`);
            return {
                ok: false,
                enabled: true,
                path: storePath,
                loaded_count: 0,
                skipped_count: 0,
                interrupted_count: 0,
                error: error.message || String(error),
            };
        }
        for (const line of raw.split(/\r?\n/)) {
            if (!line.trim()) continue;
            try {
                const parsed = JSON.parse(line);
                const snapshot = parsed?.schema === NAVSWAP_RUN_STORE_SCHEMA ? parsed.run : parsed;
                const run = normalizeStoredNavswapRun(snapshot);
                if (!run) {
                    skippedCount += 1;
                    continue;
                }
                latest.set(run.run_id, run);
            } catch (_) {
                skippedCount += 1;
            }
        }
        let interruptedCount = 0;
        for (const run of latest.values()) {
            if (markInterrupted && markStoredNavswapRunInterrupted(run)) {
                interruptedCount += 1;
                persistNavswapRun(run);
            }
            navswapRuns.set(run.run_id, run);
        }
        return {
            ok: true,
            enabled: true,
            path: storePath,
            loaded_count: latest.size,
            skipped_count: skippedCount,
            interrupted_count: interruptedCount,
        };
    }

    function clearNavswapRunsForTest() {
        navswapRuns.clear();
        for (const subscribers of navswapRunStreams.values()) {
            for (const subscriber of subscribers) {
                if (subscriber.heartbeat) clearInterval(subscriber.heartbeat);
                try {
                    subscriber.res.end();
                } catch (_) {}
            }
        }
        navswapRunStreams.clear();
    }

    function sanitizeNavswapRunRequest(body = {}) {
        return {
            route: navswapRouteFromBody(body),
            from_asset: body.from_asset || body.from || null,
            to_asset: body.to_asset || body.to || null,
            amount: body.amount || null,
            wallet_address: body.wallet_address || body.owner || null,
        };
    }

    function navswapRunPublic(run) {
        if (typeof run === 'string') run = navswapRuns.get(run);
        if (!run) return null;
        return {
            ok: run.ok,
            schema: NAVSWAP_RUN_STATUS_SCHEMA,
            run_id: run.run_id,
            route: run.route,
            status: run.status,
            terminal: navswapRunIsTerminal(run),
            code: run.code,
            message: run.message,
            created_at: run.created_at,
            updated_at: run.updated_at,
            request: run.request,
            quote: run.quote,
            result: run.result,
            error: run.error,
            events_endpoint: `/api/navswap/runs/${run.run_id}/events`,
            stream_endpoint: `/api/navswap/runs/${run.run_id}/stream`,
            receipts_endpoint: `/api/navswap/runs/${run.run_id}/receipts`,
        };
    }

    function navswapRunList(params = {}) {
        let walletAddress;
        try {
            walletAddress = parseNavswapWalletAddress(params.wallet_address || params.owner || params.source);
        } catch (error) {
            return {
                ok: false,
                schema: NAVSWAP_RUN_LIST_SCHEMA,
                code: error.code || 'invalid_navswap_wallet_address',
                message: error.message || 'wallet_address must be a lowercase PostFiat account address',
            };
        }
        const route = typeof params.route === 'string' && params.route.trim()
            ? params.route.trim()
            : null;
        const includeTerminal = navswapTruthyParam(params.include_terminal || params.includeTerminal);
        const limit = navswapListLimit(params.limit);
        const runs = Array.from(navswapRuns.values())
            .filter((run) => run?.request?.wallet_address === walletAddress)
            .filter((run) => !route || run.route === route)
            .filter((run) => includeTerminal || !navswapRunIsTerminal(run))
            .sort(compareNavswapRunsNewestFirst)
            .slice(0, limit)
            .map(navswapRunPublic)
            .filter(Boolean);
        return {
            ok: true,
            schema: NAVSWAP_RUN_LIST_SCHEMA,
            wallet_address: walletAddress,
            route,
            include_terminal: includeTerminal,
            limit,
            count: runs.length,
            latest_run: runs[0] || null,
            runs,
        };
    }

    function navswapTruthyParam(value) {
        if (value === true) return true;
        if (typeof value !== 'string') return false;
        return ['1', 'true', 'yes', 'on'].includes(value.trim().toLowerCase());
    }

    function navswapListLimit(value) {
        const parsed = Number.parseInt(value, 10);
        if (!Number.isFinite(parsed)) return 10;
        return Math.max(1, Math.min(50, parsed));
    }

    function compareNavswapRunsNewestFirst(a, b) {
        return navswapRunSortTime(b) - navswapRunSortTime(a);
    }

    function navswapRunSortTime(run) {
        const parsed = Date.parse(run?.updated_at || run?.created_at || '');
        return Number.isFinite(parsed) ? parsed : 0;
    }

    function navswapRunEvents(runId) {
        const run = navswapRuns.get(runId);
        if (!run) return null;
        return {
            ok: true,
            schema: NAVSWAP_RUN_EVENTS_SCHEMA,
            run_id: runId,
            events: run.events,
        };
    }

    function navswapRunReceipts(runId) {
        const run = navswapRuns.get(runId);
        if (!run) return null;
        return {
            ok: true,
            schema: NAVSWAP_RUN_RECEIPTS_SCHEMA,
            run_id: runId,
            receipts: run.receipts,
        };
    }

    function navswapRunIsTerminal(run) {
        if (!run) return false;
        return run.ok === true
            || run.ok === false
            || [
                'operator_mint_submitted',
                'operator_redeem_settle_submitted',
                'destination_consume_submitted',
                'complete',
                'failed',
                'transparent_complete',
            ].includes(run.status);
    }

    function navswapRunStreamSnapshot(run, event = null) {
        if (typeof run === 'string') run = navswapRuns.get(run);
        if (!run) return null;
        return {
            ok: true,
            schema: NAVSWAP_RUN_STREAM_EVENT_SCHEMA,
            run_id: run.run_id,
            at: new Date().toISOString(),
            event,
            terminal: navswapRunIsTerminal(run),
            status: navswapRunPublic(run),
            events: run.events,
            receipts: run.receipts,
        };
    }

    function writeSseEvent(res, eventName, payload) {
        res.write(`event: ${eventName}\n`);
        res.write(`data: ${JSON.stringify(payload)}\n\n`);
    }

    function removeNavswapRunStreamSubscriber(runId, subscriber) {
        if (subscriber.heartbeat) {
            clearInterval(subscriber.heartbeat);
            subscriber.heartbeat = null;
        }
        const subscribers = navswapRunStreams.get(runId);
        if (!subscribers) return;
        subscribers.delete(subscriber);
        if (subscribers.size === 0) navswapRunStreams.delete(runId);
    }

    function publishNavswapRunUpdate(run, event) {
        const subscribers = navswapRunStreams.get(run.run_id);
        if (!subscribers || subscribers.size === 0) return;
        const payload = navswapRunStreamSnapshot(run, event);
        const terminal = navswapRunIsTerminal(run);
        for (const subscriber of Array.from(subscribers)) {
            const res = subscriber.res;
            if (res.destroyed || res.writableEnded) {
                removeNavswapRunStreamSubscriber(run.run_id, subscriber);
                continue;
            }
            try {
                writeSseEvent(res, 'navswap_run_update', payload);
                if (terminal) {
                    writeSseEvent(res, 'navswap_run_done', payload);
                    res.end();
                    removeNavswapRunStreamSubscriber(run.run_id, subscriber);
                }
            } catch (_) {
                removeNavswapRunStreamSubscriber(run.run_id, subscriber);
            }
        }
    }

    function recordNavswapRunEvent(run, type, message, details = null) {
        const event = {
            sequence: run.events.length,
            at: new Date().toISOString(),
            type,
            message,
            details,
        };
        run.events.push(event);
        run.updated_at = event.at;
        persistNavswapRun(run);
        publishNavswapRunUpdate(run, event);
        return event;
    }

    function createNavswapRun(route, body, quote) {
        const now = new Date().toISOString();
        const run = {
            ok: null,
            run_id: newNavswapRunId(),
            route,
            status: 'running',
            code: null,
            message: 'NAVSwap run started.',
            created_at: now,
            updated_at: now,
            request: sanitizeNavswapRunRequest(body),
            quote,
            result: null,
            error: null,
            events: [],
            receipts: [],
        };
        navswapRuns.set(run.run_id, run);
        recordNavswapRunEvent(run, 'run_started', 'NAVSwap run started.', { route });
        return run;
    }

    function navswapAsyncRunRequested(body = {}) {
        return body?.async === true || body?.async_run === true || body?.mode === 'async';
    }

    function finishNavswapRun(run, payload) {
        const ok = payload?.ok === true;
        run.ok = ok;
        run.status = payload?.status || (ok ? 'complete' : 'failed');
        run.code = ok ? null : (payload?.code || 'navswap_run_failed');
        run.message = payload?.message || (ok ? 'NAVSwap run completed.' : 'NAVSwap run failed.');
        run.result = payload?.result !== undefined ? payload.result : (payload || null);
        run.error = ok ? null : {
            code: run.code,
            message: run.message,
        };
        if (run.result) {
            run.receipts.push({
                type: payload?.receipt_type || (run.route === 'stakehub_transparent_roundtrip' ? 'stakehub_result' : 'navswap_result'),
                at: new Date().toISOString(),
                payload: run.result,
            });
        }
        recordNavswapRunEvent(run, ok ? 'run_completed' : 'run_failed', run.message, {
            status: run.status,
            code: run.code,
        });
        return navswapRunPublic(run);
    }

    function buildNavswapRunResponse(body = {}) {
        const route = navswapRouteFromBody(body);
        const quote = buildNavswapQuoteResponse(body);
        if (quote.ok !== true) {
            return {
                ok: false,
                schema: NAVSWAP_RUN_SCHEMA,
                route,
                code: quote.code || 'route_not_runnable',
                message: quote.message || 'NAVSwap route is not runnable.',
            };
        }
        if (route === 'stakehub_transparent_roundtrip') {
            const config = navswapStakehubTransparentConfig();
            if (!config.runs_enabled) {
                return {
                    ok: false,
                    schema: NAVSWAP_RUN_SCHEMA,
                    route,
                    code: 'stakehub_transparent_runs_disabled',
                    message: 'StakeHub transparent roundtrip execution is disabled; set NAVSWAP_ENABLE_STAKEHUB_TRANSPARENT_RUNS=true to allow live operator-backed runs.',
                    quote,
                    config,
                };
            }
            return {
                ok: true,
                schema: NAVSWAP_RUN_SCHEMA,
                route,
                status: 'ready_to_forward',
                message: 'StakeHub transparent roundtrip is ready to forward to the configured StakeHub action endpoint.',
                quote,
                config,
            };
        }
        if (route === 'uniswap_atomic_handoff') {
            const bridge = navswapBridgeConfig();
            const beta = navswapUniswapBetaRouteState(bridge);
            if (!beta.run_enabled) {
                return {
                    ok: false,
                    schema: NAVSWAP_RUN_SCHEMA,
                    route,
                    code: 'uniswap_handoff_beta_runs_disabled',
                    message: 'Controlled PFTL-Uniswap beta run packets require NAVSWAP_ENABLE_UNISWAP_BETA_ROUTE=true and NAVSWAP_ENABLE_UNISWAP_BETA_RUNS=true.',
                    quote,
                    blockers: beta.blockers,
                    config: bridge,
                };
            }
            return {
                ok: true,
                schema: NAVSWAP_RUN_SCHEMA,
                route,
                status: 'controlled_beta_packet_ready',
                message: 'Controlled beta PFTL-Uniswap run packet generated. Public routing is disabled.',
                quote,
                run_packet: {
                    schema: 'postfiat-pftl-uniswap-controlled-beta-run-packet-v1',
                    route,
                    route_id: bridge.route_id,
                    route_config_digest: bridge.route_config_digest,
                    route_trust_class: bridge.route_trust_class,
                    release_stage: 'explicit_beta',
                    public_routing_enabled: false,
                    route_supply_cap_atoms: bridge.route_supply_cap_atoms,
                    supply_cap_remaining_atoms: bridge.supply_cap_remaining_atoms,
                    packet_notional_cap_atoms: bridge.packet_notional_cap_atoms,
                    failure_behavior: bridge.failure_behavior,
                    mint_and_swap_uniswap: quote.mint_and_swap_uniswap,
                    quote_binding_hash: quote.quote_binding_hash,
                    terminal_states: [
                        'destination_consumed_and_swapped',
                        'destination_consumed_mint_only',
                        'source_refundable_after_timeout',
                    ],
                },
            };
        }
        return {
            ok: false,
            schema: NAVSWAP_RUN_SCHEMA,
            route,
            code: 'wallet_execution_not_implemented',
            message: 'This adapter does not execute custody-bearing NAVSwap runs yet. The wallet must sign the exact prepared action locally before submission.',
        };
    }

    async function forwardStakehubTransparentRun(run, body, quote, config) {
        const route = run.route;
        let endpoint;
        try {
            endpoint = new URL(config.action_path, config.base_url).toString();
        } catch (_) {
            return {
                ok: false,
                schema: NAVSWAP_RUN_SCHEMA,
                route,
                code: 'stakehub_transparent_invalid_url',
                message: 'NAVSWAP_STAKEHUB_BASE_URL is not a valid HTTP(S) URL.',
                run: finishNavswapRun(run, {
                    ok: false,
                    status: 'failed',
                    code: 'stakehub_transparent_invalid_url',
                    message: 'NAVSWAP_STAKEHUB_BASE_URL is not a valid HTTP(S) URL.',
                }),
                config,
            };
        }
        if (!/^https?:\/\//i.test(endpoint)) {
            return {
                ok: false,
                schema: NAVSWAP_RUN_SCHEMA,
                route,
                code: 'stakehub_transparent_invalid_url',
                message: 'StakeHub transparent roundtrip endpoint must use http or https.',
                run: finishNavswapRun(run, {
                    ok: false,
                    status: 'failed',
                    code: 'stakehub_transparent_invalid_url',
                    message: 'StakeHub transparent roundtrip endpoint must use http or https.',
                }),
                endpoint,
            };
        }

        const amount = parseStakehubTransparentAmount(body.amount);
        if (amount === null) {
            return finishNavswapRun(run, {
                ok: false,
                status: 'failed',
                code: 'stakehub_transparent_amount_invalid',
                message: 'StakeHub transparent roundtrip amount must be a positive a651 decimal.',
            });
        }
        const controller = new AbortController();
        const timer = setTimeout(() => controller.abort(), config.timeout_ms);
        try {
            recordNavswapRunEvent(run, 'stakehub_forward_started', 'Forwarding transparent roundtrip to StakeHub.', { endpoint, amount });
            const response = await fetch(endpoint, {
                method: 'POST',
                headers: {
                    'Accept': 'application/json',
                    'Content-Type': 'application/json',
                },
                body: JSON.stringify({
                    action: NAVSWAP_STAKEHUB_TRANSPARENT_ACTION,
                    amount,
                    wallet_address: body.wallet_address || body.owner || null,
                    source: 'wallet-proxy-navswap-adapter',
                }),
                signal: controller.signal,
            });
            let payload = null;
            try {
                payload = await response.json();
            } catch (_) {
                const failed = finishNavswapRun(run, {
                    ok: false,
                    status: 'failed',
                    code: 'stakehub_transparent_non_json_response',
                    message: `StakeHub returned non-JSON HTTP ${response.status}.`,
                });
                return {
                    ok: false,
                    schema: NAVSWAP_RUN_SCHEMA,
                    route,
                    code: failed.code,
                    message: failed.message,
                    run_id: run.run_id,
                    status_endpoint: `/api/navswap/runs/${run.run_id}`,
                    events_endpoint: failed.events_endpoint,
                    stream_endpoint: failed.stream_endpoint,
                    receipts_endpoint: failed.receipts_endpoint,
                    http_status: response.status,
                    endpoint,
                };
            }
            const runPayload = {
                ok: response.ok && payload?.ok === true,
                status: payload?.status || (response.ok ? 'submitted' : 'failed'),
                code: response.ok && payload?.ok === true ? undefined : (payload?.code || 'stakehub_transparent_run_failed'),
                message: payload?.message || payload?.error || (response.ok ? 'StakeHub transparent roundtrip response received.' : 'StakeHub transparent roundtrip failed.'),
                result: payload,
            };
            const finalRun = finishNavswapRun(run, runPayload);
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
                endpoint,
                amount,
                custody_boundary: config.custody_boundary,
                quote,
                result: payload,
            };
        } catch (error) {
            const failed = finishNavswapRun(run, {
                ok: false,
                status: 'failed',
                code: error?.name === 'AbortError' ? 'stakehub_transparent_timeout' : 'stakehub_transparent_request_failed',
                message: error?.name === 'AbortError'
                    ? `StakeHub transparent roundtrip timed out after ${config.timeout_ms} ms.`
                    : (error?.message || 'StakeHub transparent roundtrip request failed.'),
            });
            return {
                ok: false,
                schema: NAVSWAP_RUN_SCHEMA,
                route,
                code: failed.code,
                message: failed.message,
                run_id: run.run_id,
                status_endpoint: `/api/navswap/runs/${run.run_id}`,
                events_endpoint: failed.events_endpoint,
                stream_endpoint: failed.stream_endpoint,
                receipts_endpoint: failed.receipts_endpoint,
                endpoint,
            };
        } finally {
            clearTimeout(timer);
        }
    }

    async function executeNavswapRun(body = {}, rpcRequest = rpcTcpRequest) {
        const route = navswapRouteFromBody(body);
        if (route === 'transparent_navswap') {
            return executeTransparentNavswapRun(body, rpcRequest);
        }
        if (route === 'uniswap_atomic_handoff') {
            return executePftlUniswapHandoffRun(body, rpcRequest);
        }
        const quote = await executeNavswapQuote(body, rpcRequest);
        if (quote.ok !== true) {
            return {
                ok: false,
                schema: NAVSWAP_RUN_SCHEMA,
                route,
                code: quote.code || 'route_not_runnable',
                message: quote.message || 'NAVSwap route is not runnable.',
                quote,
            };
        }
        const preflight = buildNavswapRunResponse(body);
        if (preflight.ok !== true) return preflight;
        if (route !== 'stakehub_transparent_roundtrip') return preflight;

        const config = navswapStakehubTransparentConfig();
        const run = createNavswapRun(route, body, quote);
        if (quote.nav_proof) {
            recordNavswapRunEvent(run, 'nav_proof_checked', 'Fresh NAV proof accepted for StakeHub transparent roundtrip.', quote.nav_proof);
        }

        if (navswapAsyncRunRequested(body)) {
            recordNavswapRunEvent(run, 'async_run_accepted', 'NAVSwap run accepted for background execution.', {
                route,
                custody_boundary: config.custody_boundary,
            });
            void forwardStakehubTransparentRun(run, body, quote, config).catch((error) => {
                finishNavswapRun(run, {
                    ok: false,
                    status: 'failed',
                    code: 'navswap_background_run_failed',
                    message: error?.message || 'NAVSwap background run failed.',
                });
            });
            return {
                ok: true,
                schema: NAVSWAP_RUN_SCHEMA,
                route,
                status: 'running',
                message: 'NAVSwap run accepted. Poll the status and events endpoints for live progress.',
                run_id: run.run_id,
                status_endpoint: `/api/navswap/runs/${run.run_id}`,
                events_endpoint: `/api/navswap/runs/${run.run_id}/events`,
                stream_endpoint: `/api/navswap/runs/${run.run_id}/stream`,
                receipts_endpoint: `/api/navswap/runs/${run.run_id}/receipts`,
                custody_boundary: config.custody_boundary,
                quote,
            };
        }

        return forwardStakehubTransparentRun(run, body, quote, config);
    }

    function normalizeAtomicTemplateParams(body = {}) {
        const params = {
            left_owner: body.left_owner,
            left_recipient: body.left_recipient,
            left_asset_id: assetIdForNavswapSymbol(body.left_asset_id),
            left_amount: parseAtomicInteger(body.left_amount, 'left_amount'),
            right_owner: body.right_owner,
            right_recipient: body.right_recipient,
            right_asset_id: assetIdForNavswapSymbol(body.right_asset_id),
            right_amount: parseAtomicInteger(body.right_amount, 'right_amount'),
            condition: body.condition,
            finish_after: parseAtomicInteger(body.finish_after ?? 0, 'finish_after', { allowZero: true }),
            cancel_after: parseAtomicInteger(body.cancel_after, 'cancel_after'),
        };
        if (body.left_sequence !== undefined && body.left_sequence !== null && body.left_sequence !== '') {
            params.left_sequence = parseAtomicInteger(body.left_sequence, 'left_sequence');
        }
        if (body.right_sequence !== undefined && body.right_sequence !== null && body.right_sequence !== '') {
            params.right_sequence = parseAtomicInteger(body.right_sequence, 'right_sequence');
        }

        const required = [
            'left_owner',
            'left_recipient',
            'left_asset_id',
            'left_amount',
            'right_owner',
            'right_recipient',
            'right_asset_id',
            'right_amount',
            'condition',
            'cancel_after',
        ];
        const missing = required.filter((field) => params[field] === undefined || params[field] === null || params[field] === '');
        if (missing.length > 0) {
            const err = new Error(`missing atomic template fields: ${missing.join(', ')}`);
            err.code = 'missing_atomic_template_fields';
            throw err;
        }
        if (!isPftAsset(params.left_asset_id) && !isPftAsset(params.right_asset_id)) {
            const err = new Error('ESCROW-009 currently supports one PFT leg and one issued-asset leg; use an explicit PFT intermediary for issued-asset to issued-asset NAVSwap routing.');
            err.code = 'issued_to_issued_requires_pft_intermediary';
            throw err;
        }
        return params;
    }

    function swapAtomicTemplateParams(params) {
        const swapped = {
            left_owner: params.right_owner,
            left_recipient: params.right_recipient,
            left_asset_id: params.right_asset_id,
            left_amount: params.right_amount,
            right_owner: params.left_owner,
            right_recipient: params.left_recipient,
            right_asset_id: params.left_asset_id,
            right_amount: params.left_amount,
            condition: params.condition,
            finish_after: params.finish_after,
            cancel_after: params.cancel_after,
        };
        if (params.right_sequence !== undefined) swapped.left_sequence = params.right_sequence;
        if (params.left_sequence !== undefined) swapped.right_sequence = params.left_sequence;
        return swapped;
    }

    function verifyAtomicTemplateResult(result) {
        if (!result || typeof result !== 'object') {
            throw new Error('atomic_settlement_template returned a non-object result');
        }
        if (result.schema !== 'postfiat-atomic-settlement-template-v1') {
            throw new Error(`unexpected atomic template schema: ${result.schema || 'missing'}`);
        }
        const left = result.left || {};
        const right = result.right || {};
        const settlement = result.settlement || {};
        const conditionHash = result.condition_hash || settlement.condition_hash;
        const settlementId = result.settlement_id || settlement.settlement_id;
        if (!settlementId) {
            throw new Error('atomic template missing settlement_id');
        }
        if (!conditionHash) {
            throw new Error('atomic template missing condition_hash');
        }
        for (const [side, leg] of [['left', left], ['right', right]]) {
            for (const field of ['owner', 'recipient', 'asset_id', 'escrow_id']) {
                if (!leg[field]) {
                    throw new Error(`${side} leg missing ${field}`);
                }
            }
            const operationKind = leg.transaction_kind
                || leg.operation?.transaction_kind
                || leg.operation?.operation
                || leg.operation?.kind;
            if (operationKind !== 'escrow_create') {
                throw new Error(`${side} leg missing escrow_create operation`);
            }
        }
        if (left.owner !== right.recipient || right.owner !== left.recipient) {
            throw new Error('atomic template legs must be reciprocal');
        }
        const leftIsPft = isPftAsset(left.asset_id);
        const rightIsPft = isPftAsset(right.asset_id);
        if (leftIsPft === rightIsPft || (!leftIsPft && !isIssuedAsset(left.asset_id)) || (!rightIsPft && !isIssuedAsset(right.asset_id))) {
            throw new Error('atomic template must pair one PFT leg with one issued-asset leg');
        }
        if (left.condition_hash && left.condition_hash !== conditionHash) {
            throw new Error('left leg condition hash does not match template condition hash');
        }
        if (right.condition_hash && right.condition_hash !== conditionHash) {
            throw new Error('right leg condition hash does not match template condition hash');
        }
        if (left.escrow_id && right.escrow_id && left.escrow_id === right.escrow_id) {
            throw new Error('atomic template legs must have distinct escrow ids');
        }
        return {
            schema: result.schema,
            settlement_id: settlementId,
            condition_hash: conditionHash,
            left_owner: left.owner,
            right_owner: right.owner,
            left_asset_id: left.asset_id,
            right_asset_id: right.asset_id,
            left_escrow_id: left.escrow_id || null,
            right_escrow_id: right.escrow_id || null,
        };
    }

    function verifyAtomicTemplateSymmetry(firstResult, swappedResult) {
        const first = verifyAtomicTemplateResult(firstResult);
        const swapped = verifyAtomicTemplateResult(swappedResult);
        if (first.settlement_id !== swapped.settlement_id) {
            throw new Error('atomic template settlement_id is not symmetric under swapped legs');
        }
        if (first.condition_hash !== swapped.condition_hash) {
            throw new Error('atomic template condition_hash is not symmetric under swapped legs');
        }
        if (
            first.left_escrow_id !== swapped.right_escrow_id
            || first.right_escrow_id !== swapped.left_escrow_id
        ) {
            throw new Error('atomic template escrow ids did not swap cleanly under swapped legs');
        }
        return {
            schema: 'postfiat-navswap-atomic-template-symmetry-v1',
            stable: true,
            settlement_id: first.settlement_id,
            condition_hash: first.condition_hash,
            left_escrow_id: first.left_escrow_id,
            right_escrow_id: first.right_escrow_id,
        };
    }

    async function executeNavswapAtomicTemplate(body = {}, rpcRequest = rpcTcpRequest) {
        const params = normalizeAtomicTemplateParams(body);
        const requestId = `navswap-atomic-${Date.now()}`;
        const rpcResponse = await rpcRequest(RPC_HOST, RPC_PORT, {
            version: 'postfiat-local-rpc-v1',
            id: requestId,
            method: 'atomic_settlement_template',
            params,
        });
        if (rpcResponse.ok !== true) {
            return {
                ok: false,
                schema: 'postfiat-navswap-atomic-template-v1',
                code: rpcResponse.error?.code || 'atomic_template_rpc_failed',
                message: rpcResponse.error?.message || 'atomic_settlement_template RPC failed',
                rpc_error: rpcResponse.error || null,
            };
        }
        const verification = verifyAtomicTemplateResult(rpcResponse.result);
        const swappedParams = swapAtomicTemplateParams(params);
        const swappedResponse = await rpcRequest(RPC_HOST, RPC_PORT, {
            version: 'postfiat-local-rpc-v1',
            id: `${requestId}-swapped`,
            method: 'atomic_settlement_template',
            params: swappedParams,
        });
        if (swappedResponse.ok !== true) {
            return {
                ok: false,
                schema: 'postfiat-navswap-atomic-template-v1',
                code: swappedResponse.error?.code || 'atomic_template_symmetry_rpc_failed',
                message: swappedResponse.error?.message || 'swapped atomic_settlement_template RPC failed',
                verification,
                rpc_error: swappedResponse.error || null,
            };
        }
        const symmetry = verifyAtomicTemplateSymmetry(rpcResponse.result, swappedResponse.result);
        return {
            ok: true,
            schema: 'postfiat-navswap-atomic-template-v1',
            verification,
            symmetry,
            result: rpcResponse.result,
            events: rpcResponse.events || [],
        };
    }

    function jsonHeaders(req) {
        const headers = {
            'Content-Type': 'application/json',
            'Cache-Control': 'no-store',
        };
        const origin = req?.headers?.origin;
        if (origin && ALLOWED_ORIGINS.includes(origin)) {
            headers['Access-Control-Allow-Origin'] = origin;
            headers['Vary'] = 'Origin';
            headers['Access-Control-Allow-Headers'] = 'Authorization, Content-Type, Accept, Idempotency-Key';
            headers['Access-Control-Allow-Methods'] = 'GET, POST, OPTIONS';
        }
        return headers;
    }

    function sseHeaders(req) {
        const headers = jsonHeaders(req);
        headers['Content-Type'] = 'text/event-stream; charset=utf-8';
        headers['Cache-Control'] = 'no-store, no-transform';
        headers['Connection'] = 'keep-alive';
        headers['X-Accel-Buffering'] = 'no';
        return headers;
    }

    function sendJson(req, res, status, payload) {
        res.writeHead(status, jsonHeaders(req));
        res.end(JSON.stringify(payload));
    }

    function sendNavswapRunStream(req, res, runId) {
        const run = navswapRuns.get(runId);
        if (!run) {
            sendJson(req, res, 404, {
                ok: false,
                schema: NAVSWAP_RUN_STREAM_SCHEMA,
                code: 'navswap_run_not_found',
                message: `NAVSwap run not found: ${runId}`,
                run_id: runId,
            });
            return;
        }

        res.writeHead(200, sseHeaders(req));
        res.write(': navswap run stream connected\n\n');
        writeSseEvent(res, 'navswap_run_snapshot', {
            ok: true,
            schema: NAVSWAP_RUN_STREAM_SCHEMA,
            run_id: run.run_id,
            at: new Date().toISOString(),
            status: navswapRunPublic(run),
            events: run.events,
            receipts: run.receipts,
            terminal: navswapRunIsTerminal(run),
        });

        if (navswapRunIsTerminal(run)) {
            writeSseEvent(res, 'navswap_run_done', navswapRunStreamSnapshot(run));
            res.end();
            return;
        }

        const subscriber = {
            res,
            heartbeat: null,
        };
        const cleanup = () => removeNavswapRunStreamSubscriber(run.run_id, subscriber);
        subscriber.heartbeat = setInterval(() => {
            if (res.destroyed || res.writableEnded) {
                cleanup();
                return;
            }
            try {
                res.write(': heartbeat\n\n');
            } catch (_) {
                cleanup();
            }
        }, 15000);

        let subscribers = navswapRunStreams.get(run.run_id);
        if (!subscribers) {
            subscribers = new Set();
            navswapRunStreams.set(run.run_id, subscribers);
        }
        subscribers.add(subscriber);
        req.on('close', cleanup);
    }

    function originAllowed(req, requireOrigin = false) {
        const origin = req.headers.origin || '';
        if (!origin) return !requireOrigin;
        return ALLOWED_ORIGINS.includes(origin);
    }

    function readJsonBody(req, maxBytes = 256 * 1024) {
        return new Promise((resolve, reject) => {
            const limit = boundedHttpBodyLimit(maxBytes);
            const chunks = [];
            let totalBytes = 0;
            let settled = false;
            req.on('data', (chunk) => {
                if (settled) return;
                totalBytes += chunk.length;
                if (totalBytes > limit) {
                    settled = true;
                    const err = new Error('request body too large');
                    err.code = 'request_body_too_large';
                    reject(err);
                    req.resume();
                    return;
                }
                chunks.push(chunk);
            });
            req.on('end', () => {
                if (settled) return;
                settled = true;
                const raw = Buffer.concat(chunks, totalBytes).toString('utf8');
                if (!raw.trim()) {
                    resolve({});
                    return;
                }
                try {
                    resolve(JSON.parse(raw));
                } catch (_) {
                    const err = new Error('request body is not valid JSON');
                    err.code = 'invalid_json';
                    reject(err);
                }
            });
            req.on('error', (error) => {
                if (settled) return;
                settled = true;
                reject(error);
            });
        });
    }

    async function handleNavswapHttp(req, res, url) {
        const requiresAuth = httpRequestRequiresAuth(req.method, url.pathname);
        if (!originAllowed(req, requiresAuth)) {
            sendJson(req, res, 403, { ok: false, error: 'origin not allowed' });
            return true;
        }
        if (req.method === 'OPTIONS') {
            res.writeHead(204, jsonHeaders(req));
            res.end();
            return true;
        }
        const principalId = requiresAuth ? httpMutationPrincipal(req) : null;
        if (requiresAuth && !principalId) {
            sendJson(req, res, 401, {
                ok: false,
                code: 'proxy_auth_required',
                message: 'authenticated wallet proxy mutation required',
            });
            return true;
        }
        let admission = null;
        if (requiresAuth) {
            admission = acquireMutationAdmission(principalId);
            if (!admission.ok) {
                sendJson(req, res, 429, {
                    ok: false,
                    code: admission.code,
                    message: admission.code === 'proxy_mutation_rate_limited'
                        ? 'authenticated mutation rate limit exceeded'
                        : 'authenticated mutation concurrency limit exceeded',
                    retry_after_ms: admission.retry_after_ms,
                });
                return true;
            }
            req.walletProxyPrincipal = principalId;
        }

        try {
            if (req.method === 'GET' && url.pathname === '/api/navswap/capabilities') {
                sendJson(req, res, 200, await executeNavswapCapabilities());
                return true;
            }

            if (req.method === 'POST' && url.pathname === '/api/bridge/relay') {
                const body = await readJsonBody(req);
                const result = await executeNavswapIdempotentRequest({
                    method: req.method,
                    pathname: url.pathname,
                    body,
                    req,
                }, () => executeVaultBridgeRelay(body));
                sendJson(req, res, result.ok ? 200 : 409, result);
                return true;
            }

            if (req.method === 'GET' && url.pathname === '/api/shielded-nav-swap/status') {
                const result = await executeShieldedNavswapStatus();
                sendJson(req, res, result.ok ? 200 : 409, result);
                return true;
            }

            if (req.method === 'GET' && url.pathname === '/api/shielded-nav-swap/balances') {
                sendJson(req, res, 200, await executeShieldedNavswapBalances());
                return true;
            }

            if (req.method === 'GET' && url.pathname === '/api/shielded-nav-swap/note-capability') {
                sendJson(req, res, 200, await executeShieldedNavswapNoteCapability());
                return true;
            }

            if (req.method === 'GET' && url.pathname === '/api/shielded-nav-swap/prover-readiness') {
                sendJson(req, res, 200, await executeShieldedNavswapProverReadiness());
                return true;
            }

            if (req.method === 'POST' && url.pathname === '/api/shielded-nav-swap/quote') {
                const body = await readJsonBody(req);
                const result = await executeShieldedNavswapQuote(body);
                sendJson(req, res, result.ok ? 200 : 409, result);
                return true;
            }

            if (req.method === 'POST' && url.pathname === '/api/shielded-nav-swap/preflight') {
                const body = await readJsonBody(req);
                const result = await executeShieldedNavswapIngressPreflight(body);
                sendJson(req, res, result.ok ? 200 : 409, result);
                return true;
            }

            if (req.method === 'POST' && url.pathname === '/api/shielded-nav-swap/ingress') {
                const body = await readJsonBody(req, 1024 * 1024);
                const result = await executeNavswapIdempotentRequest({
                    method: req.method,
                    pathname: url.pathname,
                    body,
                    req,
                }, () => executeShieldedNavswapIngress(body));
                sendJson(req, res, result.ok ? 200 : 409, result);
                return true;
            }

            if (req.method === 'POST' && url.pathname === '/api/shielded-nav-swap/swap') {
                const body = await readJsonBody(req, 16 * 1024 * 1024);
                const result = await executeNavswapIdempotentRequest({
                    method: req.method,
                    pathname: url.pathname,
                    body,
                    req,
                }, () => executeShieldedNavswapSwap(body));
                sendJson(req, res, result.ok ? 200 : 409, result);
                return true;
            }

            if (req.method === 'POST' && url.pathname === '/api/shielded-nav-swap/egress') {
                const body = await readJsonBody(req, 16 * 1024 * 1024);
                const result = await executeNavswapIdempotentRequest({
                    method: req.method,
                    pathname: url.pathname,
                    body,
                    req,
                }, () => executeShieldedNavswapEgress(body));
                sendJson(req, res, result.ok ? 200 : 409, result);
                return true;
            }

            if (req.method === 'GET' && url.pathname === '/api/navswap/nav-proof') {
                const proof = await buildNavswapNavProofResponse(url.searchParams);
                sendJson(req, res, proof.ok ? 200 : 502, proof);
                return true;
            }

            if (req.method === 'POST' && url.pathname === '/api/navswap/planner-inputs') {
                const body = await readJsonBody(req);
                const plan = await planTransparentNavswapWalletActions(body);
                sendJson(req, res, plan.ok ? 200 : 409, plan);
                return true;
            }

            if (req.method === 'GET' && url.pathname === '/api/navswap/runs') {
                const payload = navswapRunList(Object.fromEntries(url.searchParams.entries()));
                sendJson(req, res, payload.ok ? 200 : 400, payload);
                return true;
            }

            const runPath = url.pathname.match(/^\/api\/navswap\/runs\/([^/]+)(?:\/(events|receipts|stream))?$/);
            if (req.method === 'GET' && runPath) {
                const runId = decodeURIComponent(runPath[1]);
                const kind = runPath[2] || 'status';
                let payload;
                if (kind === 'events') {
                    payload = navswapRunEvents(runId);
                } else if (kind === 'receipts') {
                    payload = navswapRunReceipts(runId);
                } else if (kind === 'stream') {
                    sendNavswapRunStream(req, res, runId);
                    return true;
                } else {
                    payload = navswapRunPublic(navswapRuns.get(runId));
                }
                if (!payload) {
                    sendJson(req, res, 404, {
                        ok: false,
                        schema: NAVSWAP_RUN_STATUS_SCHEMA,
                        code: 'navswap_run_not_found',
                        message: `NAVSwap run not found: ${runId}`,
                        run_id: runId,
                    });
                    return true;
                }
                sendJson(req, res, 200, payload);
                return true;
            }

            if (req.method === 'POST' && url.pathname === '/api/navswap/quotes') {
                const body = await readJsonBody(req);
                const quote = await executeNavswapQuote(body);
                sendJson(req, res, quote.ok ? 200 : 409, quote);
                return true;
            }

            if (req.method === 'POST' && url.pathname === '/api/navswap/readiness') {
                const body = await readJsonBody(req);
                const result = await executeTransparentNavswapReadiness(body);
                sendJson(req, res, result.ok ? 200 : 409, result);
                return true;
            }

            if (req.method === 'POST' && url.pathname === '/api/navswap/devnet-fund-pfusdc') {
                const body = await readJsonBody(req);
                const result = await executeNavswapIdempotentRequest({
                    method: req.method,
                    pathname: url.pathname,
                    body,
                    req,
                }, () => executeNavswapDevnetPfusdcFunding(body));
                sendJson(req, res, result.ok ? 200 : 409, result);
                return true;
            }

            if (req.method === 'POST' && url.pathname === '/api/navswap/runs') {
                const body = await readJsonBody(req);
                const run = await executeNavswapIdempotentRequest({
                    method: req.method,
                    pathname: url.pathname,
                    body,
                    req,
                }, () => executeNavswapRun(body));
                sendJson(req, res, run.ok ? 200 : 409, run);
                return true;
            }

            if (req.method === 'POST' && url.pathname === '/api/navswap/actions/prepare') {
                const body = await readJsonBody(req);
                const result = await prepareNavswapWalletAction(body);
                sendJson(req, res, result.ok ? 200 : 409, result);
                return true;
            }

            if (req.method === 'POST' && url.pathname === '/api/navswap/actions/prepare-batch') {
                const body = await readJsonBody(req);
                const result = await prepareNavswapWalletActionBatch(body);
                sendJson(req, res, result.ok ? 200 : 409, result);
                return true;
            }

            if (req.method === 'POST' && url.pathname === '/api/navswap/atomic-templates') {
                const body = await readJsonBody(req);
                const result = await executeNavswapAtomicTemplate(body);
                sendJson(req, res, result.ok ? 200 : 502, result);
                return true;
            }
        } catch (error) {
            sendJson(req, res, error.code === 'request_body_too_large' ? 413 : 400, {
                ok: false,
                code: error.code || 'navswap_adapter_error',
                message: error.message || 'NAVSwap adapter error',
            });
            return true;
        } finally {
            if (admission?.release) admission.release();
            delete req.walletProxyPrincipal;
        }

        return false;
    }


    return { annotateNavswapIdempotency,buildNavswapRunResponse,clearNavswapIdempotencyForTest,clearNavswapRunsForTest,compareNavswapRunsNewestFirst,createNavswapRun,executeNavswapAtomicTemplate,executeNavswapIdempotentRequest,executeNavswapRun,finishNavswapRun,forwardStakehubTransparentRun,handleNavswapHttp,jsonHeaders,loadNavswapIdempotencyStore,loadNavswapRunStore,markStoredNavswapRunInterrupted,navswapAsyncRunRequested,navswapIdempotencyHashBody,navswapIdempotencyStoreSnapshot,navswapListLimit,navswapRunEvents,navswapRunIsTerminal,navswapRunList,navswapRunPublic,navswapRunReceipts,navswapRunSortTime,navswapRunStoreSnapshot,navswapRunStreamSnapshot,navswapTruthyParam,normalizeAtomicTemplateParams,normalizeStoredNavswapIdempotencyRecord,normalizeStoredNavswapRun,originAllowed,persistNavswapIdempotencyRecord,persistNavswapRun,pruneNavswapIdempotencyRecords,publishNavswapRunUpdate,readJsonBody,recordNavswapRunEvent,removeNavswapRunStreamSubscriber,sanitizeNavswapRunRequest,sendJson,sendNavswapRunStream,sseHeaders,swapAtomicTemplateParams,verifyAtomicTemplateResult,verifyAtomicTemplateSymmetry,writeSseEvent };
}

module.exports = { create };
