'use strict';

function create(runtime) {
    const { A651_ASSET_ID,A652_ASSET_ID,ALLOWED_ORIGINS,ASSET_ORCHARD_ACTION_CLEAR_KEYS,ASSET_ORCHARD_INGRESS_FILE_SCHEMA,ASSET_ORCHARD_POOL_ID,ASSET_ORCHARD_PRIVATE_EGRESS_ACTION_SCHEMA,ASSET_ORCHARD_PRIVATE_EGRESS_FILE_SCHEMA,ASSET_ORCHARD_SWAP_ACTION_SCHEMA,DEFAULT_ASSET_ORCHARD_LOCAL_SERVICE_READINESS_TIMEOUT_MS,DEFAULT_ASSET_ORCHARD_LOCAL_SERVICE_URL,DEFAULT_RPC_FLEET,ENABLE_FINALITY_RESPONDER_READ_CACHE,ENABLE_FIRST_READY_SEQUENCED_READ,ENABLE_PROPOSER_ROUTING,ENABLE_RPC_PARENT_WAIT_FINALITY_ROUTE,ENABLE_RPC_PARENT_WAIT_SEQUENCED_READ_ROUTE,ENABLE_UPSTREAM_KEEPALIVE,ETHEREUM_USDC_TOKEN,FASTPAY_BROADCAST_METHODS,FASTPAY_CERTIFICATE_FINALITY_ENABLED,FASTPAY_CERTIFICATE_RETRY_MS,FASTPAY_FLEET_STATUS_CACHE_MS,FASTPAY_OWNED_OBJECT_LOOKUP_LIMIT,FASTPAY_REQUIRE_PRIMARY_SUCCESS,FASTPAY_ROUTE_RETRY_MS,FASTPAY_ROUTE_TIMEOUT_MS,FINALITY_METHODS,FIRST_READY_SEQUENCED_READ_PROPOSERS,INJECT_RPC_CAPS,LEGACY_A651_ETH_TOKEN,LEGACY_A651_UNISWAP_POOL_ID,LISTEN_PORT,MAX_TCP_PER_WS,MAX_WS_MESSAGE_BYTES,NAVSWAP_CAPABILITIES_SCHEMA,NAVSWAP_DEVNET_FUNDING_SCHEMA,NAVSWAP_IDEMPOTENCY_STORE_DEFAULT_PATH,NAVSWAP_IDEMPOTENCY_STORE_SCHEMA,NAVSWAP_IDEMPOTENCY_TTL_MS,NAVSWAP_MAX_LIVE_USD,NAVSWAP_NAV_PROOF_SCHEMA,NAVSWAP_PRIMARY_MINT_ROUTE_FAMILY,NAVSWAP_QUOTE_FRESHNESS_TTL_MS,NAVSWAP_QUOTE_SCHEMA,NAVSWAP_READINESS_SCHEMA,NAVSWAP_ROUTE_TRUST_CLASSES,NAVSWAP_RUN_EVENTS_SCHEMA,NAVSWAP_RUN_LIST_SCHEMA,NAVSWAP_RUN_RECEIPTS_SCHEMA,NAVSWAP_RUN_SCHEMA,NAVSWAP_RUN_STATUS_SCHEMA,NAVSWAP_RUN_STORE_DEFAULT_PATH,NAVSWAP_RUN_STORE_SCHEMA,NAVSWAP_RUN_STREAM_EVENT_SCHEMA,NAVSWAP_RUN_STREAM_SCHEMA,NAVSWAP_SETTLEMENT_RECEIPT_MAX_SNAPSHOT_AGE_BLOCKS,NAVSWAP_SETTLEMENT_RECEIPT_SAFETY_BLOCKS,NAVSWAP_STAKEHUB_TRANSPARENT_ACTION,NAVSWAP_TRANSPARENT_PLANNER_INPUTS_SCHEMA,NAVSWAP_WALLET_ACTION_BATCH_PREPARE_SCHEMA,NAVSWAP_WALLET_ACTION_PREPARE_SCHEMA,NAVSWAP_WALLET_ACTION_SCHEMA,OPTIMISTIC_CACHED_FINALITY_ROUTE,PFUSDC_ASSET_ID,PREFERRED_SEQUENCED_READ_VALIDATORS,PROPOSER_READY_RETRY_MS,PROPOSER_ROUTE_CACHE_MS,PROPOSER_ROUTE_RETRY_MS,PROPOSER_ROUTE_TIMEOUT_MS,RPC_CAPS,RPC_FLEET,RPC_HOST,RPC_PORT,SEQUENCED_ACCOUNT_METHODS,SHIELDED_NAVSWAP_EGRESS_POLICY_ID,SHIELDED_NAVSWAP_EGRESS_SCHEMA,SHIELDED_NAVSWAP_INGRESS_SCHEMA,SHIELDED_NAVSWAP_LIQUIDITY_MODES,SHIELDED_NAVSWAP_PREFLIGHT_SCHEMA,SHIELDED_NAVSWAP_QUOTE_SCHEMA,SHIELDED_NAVSWAP_STATUS_SCHEMA,SHIELDED_NAVSWAP_SWAP_SCHEMA,SHIELDED_PRIVATE_KEY_PATTERNS,SHIELDED_ROUND_TIMEOUT_DEFAULT_MS,TCP_TIMEOUT_MS,VAULT_BRIDGE_ALLOCATION_PURPOSE_NAV_SUBSCRIPTION,VAULT_BRIDGE_ALLOCATION_PURPOSE_SUPPLY,VAULT_BRIDGE_BUCKET_STATUS_ACTIVE,VAULT_BRIDGE_RECEIPT_STATUS_COUNTED,VAULT_BRIDGE_RECIPIENT_SPONSOR_AMOUNT,VAULT_BRIDGE_RECIPIENT_SPONSOR_MIN_AMOUNT_ATOMS,VAULT_BRIDGE_RELAY_DEFAULT_ACCOUNT,VAULT_BRIDGE_RELAY_EXPIRES_AT_HEIGHT,VAULT_BRIDGE_RELAY_POLICY_HASH,VAULT_BRIDGE_RELAY_SCHEMA,VAULT_BRIDGE_RELAY_SOURCE_CHAIN_ID,VAULT_BRIDGE_RELAY_SOURCE_RPC_URL,VAULT_BRIDGE_RELAY_TOKEN_ADDRESS,VAULT_BRIDGE_RELAY_VAULT_ADDRESS,WALLET_SUBSCRIPTION_INTERVAL_MS,WALLET_SUBSCRIPTION_MIN_INTERVAL_MS,WALLET_SUBSCRIPTION_READ_TIMEOUT_MS,crypto,execFileAsync,fastpayCertificateOutbox,fs,http,navswapDevnetFundingUsage,navswapIdempotencyRecords,navswapRunStreams,navswapRuns,net,os,path,server,wss } = runtime;
    let { fastpayFleetStatusCache,fastpayFleetStatusInFlight,latestFinalizedReadCache,preferredSequencedReadIndex,proposerRouteCache,shieldedCertifierLoopState } = runtime;
    const annotateNavswapIdempotency = (...args) => runtime.annotateNavswapIdempotency(...args);
    const assertNoShieldedPrivateMaterial = (...args) => runtime.assertNoShieldedPrivateMaterial(...args);
    const assertVaultBridgeEvidenceMatches = (...args) => runtime.assertVaultBridgeEvidenceMatches(...args);
    const assetIdForNavswapSymbol = (...args) => runtime.assetIdForNavswapSymbol(...args);
    const assetOrchardLocalServiceConfig = (...args) => runtime.assetOrchardLocalServiceConfig(...args);
    const buildNavswapNavProofResponse = (...args) => runtime.buildNavswapNavProofResponse(...args);
    const buildNavswapQuoteResponse = (...args) => runtime.buildNavswapQuoteResponse(...args);
    const buildNavswapRunResponse = (...args) => runtime.buildNavswapRunResponse(...args);
    const buildPftlUniswapReceiptVerification = (...args) => runtime.buildPftlUniswapReceiptVerification(...args);
    const buildShieldedCertifiedRoundArgs = (...args) => runtime.buildShieldedCertifiedRoundArgs(...args);
    const buildStakehubTransparentPreflight = (...args) => runtime.buildStakehubTransparentPreflight(...args);
    const buildTransparentNavswapReceiptVerification = (...args) => runtime.buildTransparentNavswapReceiptVerification(...args);
    const buildTransparentNavswapRedeemReceiptVerification = (...args) => runtime.buildTransparentNavswapRedeemReceiptVerification(...args);
    const buildUniswapHandoffQuoteBinding = (...args) => runtime.buildUniswapHandoffQuoteBinding(...args);
    const buildUrl = (...args) => runtime.buildUrl(...args);
    const buildVaultBridgeRelayBundle = (...args) => runtime.buildVaultBridgeRelayBundle(...args);
    const certifiedRoundFailure = (...args) => runtime.certifiedRoundFailure(...args);
    const certifiedRoundHasQuorumCertificate = (...args) => runtime.certifiedRoundHasQuorumCertificate(...args);
    const certifiedRoundHeight = (...args) => runtime.certifiedRoundHeight(...args);
    const certifiedRoundReceipts = (...args) => runtime.certifiedRoundReceipts(...args);
    const certifyShieldedBatchViaWarmLoop = (...args) => runtime.certifyShieldedBatchViaWarmLoop(...args);
    const chooseShieldedCatchUpSource = (...args) => runtime.chooseShieldedCatchUpSource(...args);
    const clearNavswapDevnetFundingUsageForTest = (...args) => runtime.clearNavswapDevnetFundingUsageForTest(...args);
    const clearNavswapIdempotencyForTest = (...args) => runtime.clearNavswapIdempotencyForTest(...args);
    const clearNavswapRunsForTest = (...args) => runtime.clearNavswapRunsForTest(...args);
    const cloneJson = (...args) => runtime.cloneJson(...args);
    const collectShieldedTopologyStatuses = (...args) => runtime.collectShieldedTopologyStatuses(...args);
    const compareNavswapRunsNewestFirst = (...args) => runtime.compareNavswapRunsNewestFirst(...args);
    const completePftlUniswapHandoffRun = (...args) => runtime.completePftlUniswapHandoffRun(...args);
    const completeTransparentNavswapRun = (...args) => runtime.completeTransparentNavswapRun(...args);
    const createNavswapRun = (...args) => runtime.createNavswapRun(...args);
    const createShieldedSwapBatchViaLocalService = (...args) => runtime.createShieldedSwapBatchViaLocalService(...args);
    const currentA652AssetId = (...args) => runtime.currentA652AssetId(...args);
    const ensureVaultBridgeRecipientAccount = (...args) => runtime.ensureVaultBridgeRecipientAccount(...args);
    const executeNavswapAtomicTemplate = (...args) => runtime.executeNavswapAtomicTemplate(...args);
    const executeNavswapCapabilities = (...args) => runtime.executeNavswapCapabilities(...args);
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
    const executeVaultBridgeRelay = (...args) => runtime.executeVaultBridgeRelay(...args);
    const fetchJsonWithTimeout = (...args) => runtime.fetchJsonWithTimeout(...args);
    const fileMtimeUnixMs = (...args) => runtime.fileMtimeUnixMs(...args);
    const findAssetOrchardActionCleartext = (...args) => runtime.findAssetOrchardActionCleartext(...args);
    const findShieldedPrivateMaterialPaths = (...args) => runtime.findShieldedPrivateMaterialPaths(...args);
    const finishNavswapRun = (...args) => runtime.finishNavswapRun(...args);
    const forwardStakehubTransparentRun = (...args) => runtime.forwardStakehubTransparentRun(...args);
    const handleNavswapHttp = (...args) => runtime.handleNavswapHttp(...args);
    const isBadSequenceSubmitResponse = (...args) => runtime.isBadSequenceSubmitResponse(...args);
    const isIssuedAsset = (...args) => runtime.isIssuedAsset(...args);
    const isPftAsset = (...args) => runtime.isPftAsset(...args);
    const isReplayableVaultBridgeRelayDuplicate = (...args) => runtime.isReplayableVaultBridgeRelayDuplicate(...args);
    const jsonHeaders = (...args) => runtime.jsonHeaders(...args);
    const loadNavswapIdempotencyStore = (...args) => runtime.loadNavswapIdempotencyStore(...args);
    const loadNavswapRunStore = (...args) => runtime.loadNavswapRunStore(...args);
    const loadPftlUniswapWalletActionContext = (...args) => runtime.loadPftlUniswapWalletActionContext(...args);
    const loadShieldedTopologyPeers = (...args) => runtime.loadShieldedTopologyPeers(...args);
    const lower = (...args) => runtime.lower(...args);
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
    const navswapIdempotencyKeyFromRequest = (...args) => runtime.navswapIdempotencyKeyFromRequest(...args);
    const navswapIdempotencyStorePath = (...args) => runtime.navswapIdempotencyStorePath(...args);
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
    const navswapRunStorePath = (...args) => runtime.navswapRunStorePath(...args);
    const navswapRunStoreSnapshot = (...args) => runtime.navswapRunStoreSnapshot(...args);
    const navswapRunStreamSnapshot = (...args) => runtime.navswapRunStreamSnapshot(...args);
    const navswapSafeU64Number = (...args) => runtime.navswapSafeU64Number(...args);
    const navswapSettlementReceiptFreshnessConfig = (...args) => runtime.navswapSettlementReceiptFreshnessConfig(...args);
    const navswapSettlementReceiptHash = (...args) => runtime.navswapSettlementReceiptHash(...args);
    const navswapStableJson = (...args) => runtime.navswapStableJson(...args);
    const navswapStakehubTransparentConfig = (...args) => runtime.navswapStakehubTransparentConfig(...args);
    const navswapSubscriptionId = (...args) => runtime.navswapSubscriptionId(...args);
    const navswapTransparentOperatorConfig = (...args) => runtime.navswapTransparentOperatorConfig(...args);
    const navswapTrustlessFinalityAgreement = (...args) => runtime.navswapTrustlessFinalityAgreement(...args);
    const navswapTruthyParam = (...args) => runtime.navswapTruthyParam(...args);
    const navswapUniswapBetaRouteState = (...args) => runtime.navswapUniswapBetaRouteState(...args);
    const navswapValidateIdempotencyKey = (...args) => runtime.navswapValidateIdempotencyKey(...args);
    const navswapValuationUnitScale = (...args) => runtime.navswapValuationUnitScale(...args);
    const navswapWalletActionBatchItems = (...args) => runtime.navswapWalletActionBatchItems(...args);
    const navswapWalletActionId = (...args) => runtime.navswapWalletActionId(...args);
    const newNavswapRunId = (...args) => runtime.newNavswapRunId(...args);
    const normalizeAtomicTemplateParams = (...args) => runtime.normalizeAtomicTemplateParams(...args);
    const normalizePftlUniswapPacketStatus = (...args) => runtime.normalizePftlUniswapPacketStatus(...args);
    const normalizeShieldedKey = (...args) => runtime.normalizeShieldedKey(...args);
    const normalizeShieldedLiquidityMode = (...args) => runtime.normalizeShieldedLiquidityMode(...args);
    const normalizeStoredNavswapIdempotencyRecord = (...args) => runtime.normalizeStoredNavswapIdempotencyRecord(...args);
    const normalizeStoredNavswapRun = (...args) => runtime.normalizeStoredNavswapRun(...args);
    const normalizeVaultBridgeAddress = (...args) => runtime.normalizeVaultBridgeAddress(...args);
    const normalizeVaultBridgeBytes32 = (...args) => runtime.normalizeVaultBridgeBytes32(...args);
    const normalizeVaultBridgeTxHash = (...args) => runtime.normalizeVaultBridgeTxHash(...args);
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
    const parseUniswapHandoffBytes32 = (...args) => runtime.parseUniswapHandoffBytes32(...args);
    const parseUniswapHandoffPositiveInteger = (...args) => runtime.parseUniswapHandoffPositiveInteger(...args);
    const persistNavswapIdempotencyRecord = (...args) => runtime.persistNavswapIdempotencyRecord(...args);
    const persistNavswapRun = (...args) => runtime.persistNavswapRun(...args);
    const pftlUniswapCompletionError = (...args) => runtime.pftlUniswapCompletionError(...args);
    const pftlUniswapCompletionQuote = (...args) => runtime.pftlUniswapCompletionQuote(...args);
    const pftlUniswapPreparedAction = (...args) => runtime.pftlUniswapPreparedAction(...args);
    const planTransparentNavswapWalletActions = (...args) => runtime.planTransparentNavswapWalletActions(...args);
    const preflightNavswapPreparedActionFees = (...args) => runtime.preflightNavswapPreparedActionFees(...args);
    const prepareNavswapWalletAction = (...args) => runtime.prepareNavswapWalletAction(...args);
    const prepareNavswapWalletActionBatch = (...args) => runtime.prepareNavswapWalletActionBatch(...args);
    const prepareNavswapWalletNavRedeemAtNavAction = (...args) => runtime.prepareNavswapWalletNavRedeemAtNavAction(...args);
    const prepareNavswapWalletNavSubscriptionAllocateAction = (...args) => runtime.prepareNavswapWalletNavSubscriptionAllocateAction(...args);
    const preparePftlUniswapWalletActionBatch = (...args) => runtime.preparePftlUniswapWalletActionBatch(...args);
    const presentEnv = (...args) => runtime.presentEnv(...args);
    const presentPositiveSafeIntegerEnv = (...args) => runtime.presentPositiveSafeIntegerEnv(...args);
    const pruneNavswapIdempotencyRecords = (...args) => runtime.pruneNavswapIdempotencyRecords(...args);
    const publishNavswapRunUpdate = (...args) => runtime.publishNavswapRunUpdate(...args);
    const readJsonBody = (...args) => runtime.readJsonBody(...args);
    const readNavswapKeyFileAddress = (...args) => runtime.readNavswapKeyFileAddress(...args);
    const recordNavswapRunEvent = (...args) => runtime.recordNavswapRunEvent(...args);
    const releaseNavswapDevnetFundingUsage = (...args) => runtime.releaseNavswapDevnetFundingUsage(...args);
    const removeNavswapRunStreamSubscriber = (...args) => runtime.removeNavswapRunStreamSubscriber(...args);
    const reserveNavswapDevnetFundingUsage = (...args) => runtime.reserveNavswapDevnetFundingUsage(...args);
    const routedRpcRead = (...args) => runtime.routedRpcRead(...args);
    const runShieldedLaggardCatchUp = (...args) => runtime.runShieldedLaggardCatchUp(...args);
    const runShieldedRpcCatchUp = (...args) => runtime.runShieldedRpcCatchUp(...args);
    const sanitizeNavswapRunRequest = (...args) => runtime.sanitizeNavswapRunRequest(...args);
    const selectNavswapIssuedSettlementSource = (...args) => runtime.selectNavswapIssuedSettlementSource(...args);
    const selectTransparentRedeemSettlementAllocation = (...args) => runtime.selectTransparentRedeemSettlementAllocation(...args);
    const sendJson = (...args) => runtime.sendJson(...args);
    const sendNavswapRunStream = (...args) => runtime.sendNavswapRunStream(...args);
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
    const signAndSubmitNavswapOperatorAssetTransaction = (...args) => runtime.signAndSubmitNavswapOperatorAssetTransaction(...args);
    const signAndSubmitVaultBridgeRecipientSponsor = (...args) => runtime.signAndSubmitVaultBridgeRecipientSponsor(...args);
    const signAndSubmitVaultBridgeRelayOperation = (...args) => runtime.signAndSubmitVaultBridgeRelayOperation(...args);
    const sseHeaders = (...args) => runtime.sseHeaders(...args);
    const stakehubTransparentAmountError = (...args) => runtime.stakehubTransparentAmountError(...args);
    const startShieldedCertifierLoop = (...args) => runtime.startShieldedCertifierLoop(...args);
    const swapAtomicTemplateParams = (...args) => runtime.swapAtomicTemplateParams(...args);
    const transparentCompletionError = (...args) => runtime.transparentCompletionError(...args);
    const transparentCompletionPreparedAction = (...args) => runtime.transparentCompletionPreparedAction(...args);
    const transparentCompletionQuote = (...args) => runtime.transparentCompletionQuote(...args);
    const transparentCompletionStage = (...args) => runtime.transparentCompletionStage(...args);
    const transparentCompletionSubmission = (...args) => runtime.transparentCompletionSubmission(...args);
    const transparentCompletionWalletResult = (...args) => runtime.transparentCompletionWalletResult(...args);
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
    const verifyAtomicTemplateResult = (...args) => runtime.verifyAtomicTemplateResult(...args);
    const verifyAtomicTemplateSymmetry = (...args) => runtime.verifyAtomicTemplateSymmetry(...args);
    const verifyPftlUniswapExportPacket = (...args) => runtime.verifyPftlUniswapExportPacket(...args);
    const verifyPftlUniswapWalletCompletionInput = (...args) => runtime.verifyPftlUniswapWalletCompletionInput(...args);
    const verifyTransparentNavRedeemSettlement = (...args) => runtime.verifyTransparentNavRedeemSettlement(...args);
    const verifyTransparentNavSubscriptionAllocation = (...args) => runtime.verifyTransparentNavSubscriptionAllocation(...args);
    const verifyTransparentWalletCompletionInput = (...args) => runtime.verifyTransparentWalletCompletionInput(...args);
    const writeSseEvent = (...args) => runtime.writeSseEvent(...args);

    function isFinalityMethod(method) {
        return FINALITY_METHODS.has(method);
    }

    function isSequencedAccountMethod(method) {
        return SEQUENCED_ACCOUNT_METHODS.has(method);
    }

    function isFastpayBroadcastMethod(method) {
        return FASTPAY_BROADCAST_METHODS.has(method);
    }

    function shouldUseFirstReadySequencedRead() {
        if (ENABLE_FIRST_READY_SEQUENCED_READ) return true;
        const proposer = proposerRouteCache?.selection?.route?.proposer;
        return typeof proposer === 'string' && FIRST_READY_SEQUENCED_READ_PROPOSERS.has(proposer);
    }

    function deterministicProposer(validators, height, view = 0) {
        if (!Array.isArray(validators) || validators.length === 0) {
            throw new Error('validator set must be nonempty');
        }
        const sorted = [...validators].sort();
        const count = BigInt(sorted.length);
        const index = Number((BigInt(height) % count + BigInt(view) % count) % count);
        return sorted[index];
    }

    function bftQuorumThreshold(validatorCount) {
        return Math.floor((validatorCount * 2) / 3) + 1;
    }

    function sleep(ms) {
        return new Promise((resolve) => setTimeout(resolve, ms));
    }

    function rpcTcpRequestOneShotLine(host, port, request, timeoutMs = TCP_TIMEOUT_MS) {
        return new Promise((resolve, reject) => {
            const tcp = net.connect(port, host);
            let buffer = '';
            let settled = false;
            let tcpClosed = false;
            const timer = setTimeout(() => {
                if (!settled && !tcpClosed) {
                    settled = true;
                    tcp.destroy();
                    reject(new Error(`RPC timeout from ${host}:${port}`));
                }
            }, timeoutMs);

            tcp.on('connect', () => {
                tcp.write(JSON.stringify(request) + '\n');
            });
            tcp.on('data', (chunk) => {
                buffer += chunk.toString('utf8');
                const idx = buffer.indexOf('\n');
                if (idx < 0) return;
                const line = buffer.slice(0, idx).trim();
                if (!line || settled) return;
                settled = true;
                clearTimeout(timer);
                tcp.end();
                resolve(line);
            });
            tcp.on('error', (e) => {
                if (!settled) {
                    settled = true;
                    clearTimeout(timer);
                    reject(e);
                }
            });
            tcp.on('close', () => {
                tcpClosed = true;
                if (!settled) {
                    settled = true;
                    clearTimeout(timer);
                    reject(new Error(`RPC connection closed by ${host}:${port}`));
                }
            });
        });
    }

    function upstreamEndpointKey(host, port, channel = 'default') {
        return `${channel}|${host}:${port}`;
    }

    class UpstreamRpcConnection {
        constructor(host, port) {
            this.host = host;
            this.port = port;
            this.socket = null;
            this.buffer = '';
            this.queue = [];
            this.current = null;
            this.connecting = false;
        }

        requestLine(request, timeoutMs = TCP_TIMEOUT_MS) {
            return new Promise((resolve, reject) => {
                this.queue.push({ request, timeoutMs, resolve, reject, timer: null });
                this._processQueue();
            });
        }

        close() {
            this._rejectCurrentAndQueue(new Error(`RPC connection closed for ${this.host}:${this.port}`));
            if (this.socket) {
                this.socket.destroy();
            }
            this.socket = null;
            this.buffer = '';
            this.connecting = false;
        }

        _socketReady() {
            return this.socket && !this.socket.destroyed && this.socket.readyState === 'open';
        }

        _processQueue() {
            if (this.current || this.queue.length === 0) return;
            if (!this._socketReady()) {
                this._connect();
                return;
            }
            const item = this.queue.shift();
            this.current = item;
            item.timer = setTimeout(() => {
                if (this.current === item) {
                    item.reject(new Error(`RPC timeout from ${this.host}:${this.port}`));
                    this.current = null;
                    this.close();
                }
            }, item.timeoutMs);
            try {
                this.socket.write(JSON.stringify(item.request) + '\n');
            } catch (error) {
                clearTimeout(item.timer);
                this.current = null;
                item.reject(error);
                this.close();
            }
        }

        _connect() {
            if (this.connecting) return;
            this.connecting = true;
            this.buffer = '';
            const socket = net.connect(this.port, this.host);
            this.socket = socket;
            socket.setKeepAlive(true);
            socket.on('connect', () => {
                this.connecting = false;
                this._processQueue();
            });
            socket.on('data', (chunk) => {
                this.buffer += chunk.toString('utf8');
                let idx;
                while ((idx = this.buffer.indexOf('\n')) >= 0) {
                    const line = this.buffer.slice(0, idx).trim();
                    this.buffer = this.buffer.slice(idx + 1);
                    if (!line) continue;
                    const item = this.current;
                    if (!item) {
                        this.close();
                        return;
                    }
                    clearTimeout(item.timer);
                    this.current = null;
                    item.resolve(line);
                    this._processQueue();
                }
            });
            socket.on('error', (error) => {
                this._handleDisconnect(error);
            });
            socket.on('close', () => {
                this._handleDisconnect(new Error(`RPC connection closed by ${this.host}:${this.port}`));
            });
        }

        _handleDisconnect(error) {
            this.connecting = false;
            this.socket = null;
            this.buffer = '';
            this._rejectCurrentAndQueue(error);
        }

        _rejectCurrentAndQueue(error) {
            if (this.current) {
                clearTimeout(this.current.timer);
                this.current.reject(error);
                this.current = null;
            }
            const queued = this.queue.splice(0);
            for (const item of queued) {
                if (item.timer) clearTimeout(item.timer);
                item.reject(error);
            }
        }
    }

    const upstreamRpcConnections = new Map();

    function upstreamRpcConnection(host, port, channel = 'default') {
        const key = upstreamEndpointKey(host, port, channel);
        let connection = upstreamRpcConnections.get(key);
        if (!connection) {
            connection = new UpstreamRpcConnection(host, port);
            upstreamRpcConnections.set(key, connection);
        }
        return connection;
    }

    function closeUpstreamRpcConnections() {
        for (const connection of upstreamRpcConnections.values()) {
            connection.close();
        }
        upstreamRpcConnections.clear();
    }

    function rpcTcpRequestLine(host, port, request, timeoutMs = TCP_TIMEOUT_MS, channel = 'default') {
        if (!ENABLE_UPSTREAM_KEEPALIVE) {
            return rpcTcpRequestOneShotLine(host, port, request, timeoutMs);
        }
        return upstreamRpcConnection(host, port, channel).requestLine(request, timeoutMs);
    }

    async function rpcTcpRequest(host, port, request, timeoutMs = TCP_TIMEOUT_MS, channel = 'default') {
        const line = await rpcTcpRequestLine(host, port, request, timeoutMs, channel);
        try {
            return JSON.parse(line);
        } catch (e) {
            throw new Error(`invalid JSON from ${host}:${port}`);
        }
    }

    async function collectFleetStatuses(fleet, options = {}) {
        const timeoutMs = Number.isInteger(options.timeoutMs) && options.timeoutMs > 0
            ? options.timeoutMs
            : 8000;
        const requests = fleet.map(async (endpoint) => {
            const response = await rpcTcpRequest(endpoint.host, endpoint.port, {
                version: 'postfiat-local-rpc-v1',
                id: `proxy-status-${endpoint.validatorId}`,
                method: 'status',
                params: {},
            }, timeoutMs, options.channel || 'status');
            if (!response.ok || !response.result) {
                throw new Error(response.error?.message || `status failed for ${endpoint.validatorId}`);
            }
            return { endpoint, status: response.result };
        });
        const settled = await Promise.allSettled(requests);
        return settled.map((result, index) => (
            result.status === 'fulfilled'
                ? { ok: true, ...result.value }
                : { ok: false, endpoint: fleet[index], error: result.reason?.message || String(result.reason) }
        ));
    }

    async function collectFastpayFleetStatuses(fleet, options = {}) {
        const now = Date.now();
        if (
            options.forceRefresh !== true
            && fastpayFleetStatusCache
            && Number.isFinite(FASTPAY_FLEET_STATUS_CACHE_MS)
            && FASTPAY_FLEET_STATUS_CACHE_MS > 0
            && now - fastpayFleetStatusCache.cached_at_ms <= FASTPAY_FLEET_STATUS_CACHE_MS
        ) {
            return fastpayFleetStatusCache.statuses;
        }
        if (fastpayFleetStatusInFlight) {
            return fastpayFleetStatusInFlight;
        }

        fastpayFleetStatusInFlight = collectFleetStatuses(fleet, {
            channel: options.channel || 'status',
        })
            .then((statuses) => {
                fastpayFleetStatusCache = {
                    cached_at_ms: Date.now(),
                    statuses,
                };
                return statuses;
            })
            .finally(() => {
                fastpayFleetStatusInFlight = null;
            });
        return fastpayFleetStatusInFlight;
    }

    function clearFastpayFleetStatusCache() {
        fastpayFleetStatusCache = null;
        fastpayFleetStatusInFlight = null;
    }

    function chooseProposerEndpointFromStatuses(fleetStatuses) {
        const okStatuses = fleetStatuses.filter((entry) => entry.ok);
        if (okStatuses.length === 0) {
            throw new Error('no reachable validators for proposer routing');
        }
        const groups = new Map();
        for (const entry of okStatuses) {
            const key = [
                entry.status.block_height,
                entry.status.block_tip_hash,
                entry.status.state_root,
            ].join('|');
            if (!groups.has(key)) groups.set(key, []);
            groups.get(key).push(entry);
        }
        const groupsBySize = [...groups.values()].sort((a, b) => b.length - a.length);
        const majority = groupsBySize[0];
        const quorum = bftQuorumThreshold(fleetStatuses.length);
        if (majority.length < quorum) {
            throw new Error(`fleet is not converged enough for proposer routing: got ${majority.length}, need ${quorum}`);
        }
        const currentHeight = majority[0].status.block_height;
        const nextHeight = Number(currentHeight) + 1;
        const validators = fleetStatuses.map((entry) => entry.endpoint.validatorId);
        const proposer = deterministicProposer(validators, nextHeight, 0);
        const endpoint = fleetStatuses.find((entry) => entry.endpoint.validatorId === proposer)?.endpoint;
        const proposerInMajority = majority.some((entry) => entry.endpoint.validatorId === proposer);
        if (!endpoint || !proposerInMajority) {
            throw new Error(`deterministic proposer ${proposer} is not in the converged majority`);
        }
        return {
            endpoint,
            route: {
                routed: true,
                proposer,
                height: nextHeight,
                view: 0,
                quorum,
                converged_count: majority.length,
                required_current_height: Number(currentHeight),
                required_state_root: majority[0].status.state_root || null,
                required_parent_hash: majority[0].status.block_tip_hash || null,
            },
        };
    }

    async function chooseProposerEndpointWithRetry(fleet, options = {}) {
        const timeoutMs = options.timeoutMs ?? PROPOSER_ROUTE_TIMEOUT_MS;
        const retryMs = options.retryMs ?? PROPOSER_ROUTE_RETRY_MS;
        const collectStatuses = options.collectStatuses || collectFleetStatuses;
        const started = Date.now();
        let attempts = 0;
        let lastError = null;

        while (true) {
            attempts += 1;
            const fleetStatuses = await collectStatuses(fleet, attempts);
            try {
                const selected = chooseProposerEndpointFromStatuses(fleetStatuses);
                selected.route.route_attempts = attempts;
                selected.route.route_wait_ms = Date.now() - started;
                return selected;
            } catch (error) {
                lastError = error;
                if (Date.now() - started >= timeoutMs) {
                    throw new Error(`${lastError.message || lastError} after ${attempts} route attempt(s)`);
                }
                await sleep(Math.max(0, retryMs));
            }
        }
    }

    function invalidateProposerRouteCache() {
        proposerRouteCache = null;
    }

    function rememberFinalizedReadEndpoint(line, selection) {
        if (!selection?.endpoint) return null;
        try {
            const response = JSON.parse(line);
            if (response.ok !== true) return null;
            const header = response.result?.finality?.block?.header || {};
            const height = Number(header.height);
            const stateRoot = header.state_root || null;
            if (!Number.isFinite(height) || !stateRoot) return null;
            latestFinalizedReadCache = {
                cached_at_ms: Date.now(),
                endpoint: selection.endpoint,
                height,
                state_root: stateRoot,
            };
            return latestFinalizedReadCache;
        } catch (e) {
            return null;
        }
    }

    function proposerEndpointForHeight(height, view = 0) {
        const validators = RPC_FLEET.map((entry) => entry.validatorId);
        const proposer = deterministicProposer(validators, height, view);
        const endpoint = RPC_FLEET.find((entry) => entry.validatorId === proposer);
        if (!endpoint) {
            throw new Error(`deterministic proposer ${proposer} is not in RPC_FLEET`);
        }
        return { proposer, endpoint };
    }

    function finalityFailureCanAdvanceView(line) {
        try {
            const response = JSON.parse(line);
            if (response?.ok === true) return false;
            if (response?.error?.code !== 'rpc_finality_submit_failed') return false;
            return /(?:vote request|vote workers|quorum|certificate|transport|timed out|connection)/i
                .test(String(response.error.message || ''));
        } catch (_) {
            return false;
        }
    }

    function exactParentStatus(entry, route) {
        if (!entry?.ok || !entry.status) return false;
        if (Number(entry.status.block_height) !== Number(route.required_current_height)) return false;
        if (route.required_state_root && entry.status.state_root !== route.required_state_root) return false;
        if (route.required_parent_hash && entry.status.block_tip_hash !== route.required_parent_hash) return false;
        return true;
    }

    async function collectFinalityTimeoutVotes(request, route, view, options = {}) {
        const fleet = options.fleet || RPC_FLEET;
        const statusCollector = options.collectStatuses || collectFleetStatuses;
        const requester = options.requester || rpcTcpRequest;
        const quorum = bftQuorumThreshold(fleet.length);
        const statuses = await statusCollector(fleet, { channel: 'finality-recovery-status' });
        const exactParent = statuses.filter((entry) => exactParentStatus(entry, route));
        if (exactParent.length < quorum) {
            throw new Error(
                `finality recovery parent is not held by quorum: got ${exactParent.length}, need ${quorum}`,
            );
        }
        const blockHeight = Number(route.height);
        const timeoutRequests = exactParent.map(async (entry) => {
            const response = await requester(entry.endpoint.host, entry.endpoint.port, {
                version: 'postfiat-local-rpc-v1',
                id: `${request.id}-timeout-v${view}-${entry.endpoint.validatorId}`,
                method: 'consensus_v2_timeout_vote',
                params: {
                    block_height: blockHeight,
                    view,
                    proxy_required_current_height: Number(route.required_current_height),
                    proxy_required_state_root: route.required_state_root,
                    proxy_required_parent_hash: route.required_parent_hash,
                    proxy_readiness_timeout_ms: PROPOSER_ROUTE_TIMEOUT_MS,
                },
            }, options.timeoutMs || TCP_TIMEOUT_MS, 'finality-timeout');
            if (response?.ok !== true || !response.result) {
                throw new Error(response?.error?.message || 'timeout vote request failed');
            }
            const vote = response.result;
            const validator = vote?.vote?.validator;
            if (vote.schema !== 'postfiat.block_timeout_vote.v1'
                || Number(vote.block_height) !== blockHeight
                || Number(vote.view) !== view
                || validator !== entry.endpoint.validatorId
                || vote?.consensus_v2_vote?.validator !== validator) {
                throw new Error(`invalid timeout vote from ${entry.endpoint.validatorId}`);
            }
            return { validator, vote };
        });
        const settled = await Promise.allSettled(timeoutRequests);
        const byValidator = new Map();
        for (const result of settled) {
            if (result.status === 'fulfilled') {
                byValidator.set(result.value.validator, result.value.vote);
            }
        }
        if (byValidator.size < quorum) {
            throw new Error(
                `finality recovery collected ${byValidator.size} distinct timeout votes, need ${quorum}`,
            );
        }
        return [...byValidator.entries()]
            .sort(([left], [right]) => left.localeCompare(right))
            .slice(0, quorum)
            .map(([, vote]) => vote);
    }

    async function recoverFinalityAcrossViews(request, route, options = {}) {
        if (!route || !Number.isFinite(Number(route.height))) {
            throw new Error('finality recovery requires an exact proposer route');
        }
        const fleet = options.fleet || RPC_FLEET;
        const requestLine = options.requestLine || rpcTcpRequestLine;
        const maxRecoveryViews = Math.min(
            fleet.length - 1,
            Number.isInteger(options.maxRecoveryViews) ? options.maxRecoveryViews : fleet.length - 1,
        );
        let lastLine = options.initialLine || null;
        let lastError = options.initialError || null;
        for (let timedOutView = 0; timedOutView < maxRecoveryViews; timedOutView += 1) {
            const votes = await collectFinalityTimeoutVotes(request, route, timedOutView, options);
            const recoveryView = timedOutView + 1;
            const { proposer, endpoint } = proposerEndpointForHeight(route.height, recoveryView);
            const recoveryRoute = {
                ...route,
                proposer,
                view: recoveryView,
                recovery_from_view: timedOutView,
                route_source: 'signed_timeout_recovery',
            };
            const compressedVotes = require('zlib')
                .gzipSync(Buffer.from(JSON.stringify(votes), 'utf8'), { level: 1 })
                .toString('base64');
            const voteChunks = compressedVotes.match(/.{1,4000}/g) || [];
            const timeoutVoteParams = {
                proxy_timeout_votes_encoding: 'gzip-base64-chunks-v1',
                proxy_timeout_votes_chunk_count: voteChunks.length,
            };
            voteChunks.forEach((chunk, index) => {
                timeoutVoteParams[`proxy_timeout_votes_chunk_${String(index).padStart(4, '0')}`] = chunk;
            });
            const params = {
                ...(request.params || {}),
                proxy_consensus_view: recoveryView,
                ...timeoutVoteParams,
            };
            const outbound = requestWithProxyReadiness({ ...request, params }, recoveryRoute);
            try {
                lastLine = await requestLine(
                    endpoint.host,
                    endpoint.port,
                    outbound,
                    options.timeoutMs || TCP_TIMEOUT_MS,
                    'finality-recovery',
                );
                if (!finalityFailureCanAdvanceView(lastLine)) {
                    return { line: lastLine, route: recoveryRoute, endpoint };
                }
                lastError = null;
            } catch (error) {
                lastError = error;
                lastLine = null;
            }
        }
        if (lastLine) return { line: lastLine, route };
        throw lastError || new Error('finality recovery exhausted its bounded view window');
    }

    function primeNextProposerRouteCache(route, options = {}) {
        if (!route || route.route_kind || !Number.isFinite(Number(route.height))) {
            return null;
        }
        const nextHeight = Number(route.height) + 1;
        const { proposer, endpoint } = proposerEndpointForHeight(nextHeight);
        const routeSource = options.routeSource || 'post_finality_cache';
        proposerRouteCache = {
            cached_at_ms: Date.now(),
            selection: {
                endpoint,
                route: {
                    routed: true,
                    proposer,
                    height: nextHeight,
                    view: 0,
                    quorum: bftQuorumThreshold(RPC_FLEET.length),
                    converged_count: RPC_FLEET.length,
                    route_attempts: 0,
                    route_wait_ms: 0,
                    route_source: routeSource,
                    required_current_height: options.requiredCurrentHeight ?? null,
                    required_state_root: options.requiredStateRoot || null,
                },
            },
        };
        if (options.warmReadiness === true) {
            startCachedSelectionReadinessProbe(proposerRouteCache.selection, options);
        }
        return proposerRouteCache.selection;
    }

    function primeNextProposerRouteCacheFromResponse(line, route, options = {}) {
        if (!route) return null;
        try {
            const response = JSON.parse(line);
            if (response.ok === true) {
                const header = response.result?.finality?.block?.header || {};
                const finalizedHeight = Number(header.height || route.height);
                return primeNextProposerRouteCache(route, {
                    ...options,
                    routeSource: response.result?.certified_sends_deferred === true
                        ? 'post_finality_deferred_cache'
                        : 'post_finality_cache',
                    requiredCurrentHeight: Number.isFinite(finalizedHeight) ? finalizedHeight : null,
                    requiredStateRoot: header.state_root || null,
                });
            } else {
                invalidateProposerRouteCache();
            }
        } catch (e) {
            invalidateProposerRouteCache();
        }
        return null;
    }

    function cachedSelection(selection, routeKind = null, cacheHit = false, cacheAgeMs = null) {
        const route = { ...selection.route };
        if (routeKind) route.route_kind = routeKind;
        route.route_cache_hit = cacheHit;
        if (cacheAgeMs !== null) route.route_cache_age_ms = cacheAgeMs;
        return {
            endpoint: selection.endpoint,
            route,
        };
    }

    function endpointStatusMeetsRoute(status, route) {
        const requiredHeight = Number(route.required_current_height);
        if (!Number.isFinite(requiredHeight)) return true;
        const observedHeight = Number(status?.block_height);
        if (!Number.isFinite(observedHeight) || observedHeight !== requiredHeight) {
            return false;
        }
        if (
            route.required_state_root
            && status?.state_root !== route.required_state_root
        ) {
            return false;
        }
        return true;
    }

    function endpointStatusMeetsSequencedReadRoute(status, route) {
        const requiredHeight = Number(route.required_current_height);
        if (!Number.isFinite(requiredHeight)) return true;
        const observedHeight = Number(status?.block_height);
        if (!Number.isFinite(observedHeight) || observedHeight < requiredHeight) {
            return false;
        }
        if (
            observedHeight === requiredHeight
            && route.required_state_root
            && status?.state_root !== route.required_state_root
        ) {
            return false;
        }
        return true;
    }

    async function waitForCachedSelectionReady(selection, options = {}) {
        const route = selection.route || {};
        if (!Number.isFinite(Number(route.required_current_height))) {
            return { waitMs: 0, attempts: 0, observedStatus: null };
        }
        const requirementKey = [
            route.required_current_height,
            route.required_state_root || '',
        ].join('|');
        if (selection.ready_requirement_key === requirementKey) {
            return {
                waitMs: 0,
                attempts: 0,
                observedStatus: selection.ready_observed_status || null,
            };
        }
        const timeoutMs = options.timeoutMs ?? PROPOSER_ROUTE_TIMEOUT_MS;
        const retryMs = options.readyRetryMs ?? PROPOSER_READY_RETRY_MS;
        const statusRequester = options.statusRequester || (async (endpoint) => {
            const response = await rpcTcpRequest(endpoint.host, endpoint.port, {
                version: 'postfiat-local-rpc-v1',
                id: `proxy-ready-${endpoint.validatorId}`,
                method: 'status',
                params: {},
            }, 8000);
            if (!response.ok || !response.result) {
                throw new Error(response.error?.message || `status failed for ${endpoint.validatorId}`);
            }
            return response.result;
        });
        const started = Date.now();
        let attempts = 0;
        let lastError = null;
        let observedStatus = null;

        while (true) {
            attempts += 1;
            try {
                observedStatus = await statusRequester(selection.endpoint, attempts);
                if (endpointStatusMeetsRoute(observedStatus, route)) {
                    selection.ready_requirement_key = requirementKey;
                    selection.ready_observed_status = observedStatus;
                    return {
                        waitMs: Date.now() - started,
                        attempts,
                        observedStatus,
                    };
                }
                lastError = new Error(
                    `${selection.endpoint.validatorId} at height ${observedStatus?.block_height}, `
                    + `need exact parent height ${route.required_current_height}`,
                );
            } catch (error) {
                lastError = error;
            }
            if (Date.now() - started >= timeoutMs) {
                throw new Error(
                    `cached proposer ${selection.endpoint.validatorId} not ready: `
                    + `${lastError?.message || lastError} after ${attempts} attempt(s)`,
                );
            }
            await sleep(Math.max(0, retryMs));
        }
    }

    function startCachedSelectionReadinessProbe(selection, options = {}) {
        if (!selection || selection.ready_in_flight) return;
        if (selection.ready_requirement_key) return;
        selection.ready_in_flight = waitForCachedSelectionReady(selection, options)
            .catch((error) => {
                selection.ready_error = error?.message || String(error);
            })
            .finally(() => {
                selection.ready_in_flight = null;
            });
    }

    async function firstReadyEndpointForRoute(fleet, route, options = {}) {
        const statusMeetsRoute = options.statusMeetsRoute || endpointStatusMeetsRoute;
        const statusRequester = options.statusRequester || (async (endpoint) => {
            const response = await rpcTcpRequest(endpoint.host, endpoint.port, {
                version: 'postfiat-local-rpc-v1',
                id: `proxy-read-ready-${endpoint.validatorId}`,
                method: 'status',
                params: {},
            }, 8000);
            if (!response.ok || !response.result) {
                throw new Error(response.error?.message || `status failed for ${endpoint.validatorId}`);
            }
            return response.result;
        });

        return new Promise((resolve) => {
            let pending = fleet.length;
            let resolved = false;
            for (const endpoint of fleet) {
                statusRequester(endpoint)
                    .then((status) => {
                        if (!resolved && statusMeetsRoute(status, route)) {
                            resolved = true;
                            resolve({ endpoint, status });
                        }
                    })
                    .catch(() => {})
                    .finally(() => {
                        pending -= 1;
                        if (!resolved && pending === 0) {
                            resolve(null);
                        }
                    });
            }
        });
    }

    function preferredSequencedReadEndpoint(fleet) {
        const preferred = PREFERRED_SEQUENCED_READ_VALIDATORS
            .map((validatorId) => fleet.find((endpoint) => endpoint.validatorId === validatorId))
            .filter(Boolean);
        const candidates = preferred.length > 0 ? preferred : fleet;
        if (candidates.length === 0) {
            throw new Error('no sequenced read RPC endpoints configured');
        }
        const selected = candidates[preferredSequencedReadIndex % candidates.length];
        preferredSequencedReadIndex += 1;
        return selected;
    }

    async function chooseSequencedAccountReadEndpoint(fleet, options = {}) {
        const now = Date.now();
        if (
            proposerRouteCache
            && Number.isFinite(PROPOSER_ROUTE_CACHE_MS)
            && PROPOSER_ROUTE_CACHE_MS > 0
            && now - proposerRouteCache.cached_at_ms <= PROPOSER_ROUTE_CACHE_MS
            && Number.isFinite(Number(proposerRouteCache.selection.route?.required_current_height))
        ) {
            const route = proposerRouteCache.selection.route;
            if (ENABLE_RPC_PARENT_WAIT_SEQUENCED_READ_ROUTE) {
                const started = Date.now();
                const ready = await firstReadyEndpointForRoute(fleet, route, {
                    ...options,
                    statusMeetsRoute: endpointStatusMeetsSequencedReadRoute,
                });
                if (ready) {
                    return {
                        endpoint: ready.endpoint,
                        route: {
                            ...route,
                            route_kind: 'sequenced_account_read',
                            read_validator: ready.endpoint.validatorId,
                            route_cache_hit: true,
                            route_cache_age_ms: now - proposerRouteCache.cached_at_ms,
                            route_wait_ms: Date.now() - started,
                            route_attempts: 1,
                            ready_observed_height: ready.status?.block_height ?? null,
                            ready_observed_state_root: ready.status?.state_root || null,
                            readiness_check: 'proxy_min_height_sequenced_read_route',
                            read_source: 'first_ready_min_height',
                        },
                    };
                }
            }
            if (
                ENABLE_FINALITY_RESPONDER_READ_CACHE
                &&
                latestFinalizedReadCache
                && now - latestFinalizedReadCache.cached_at_ms <= PROPOSER_ROUTE_CACHE_MS
                && Number(latestFinalizedReadCache.height) === Number(route.required_current_height)
                && latestFinalizedReadCache.state_root === route.required_state_root
            ) {
                return {
                    endpoint: latestFinalizedReadCache.endpoint,
                    route: {
                        ...route,
                        route_kind: 'sequenced_account_read',
                        read_validator: latestFinalizedReadCache.endpoint.validatorId,
                        route_cache_hit: true,
                        route_cache_age_ms: now - latestFinalizedReadCache.cached_at_ms,
                        route_wait_ms: 0,
                        route_attempts: 0,
                        ready_observed_height: latestFinalizedReadCache.height,
                        ready_observed_state_root: latestFinalizedReadCache.state_root,
                        read_source: 'finality_response_endpoint',
                    },
                };
            }
            const started = Date.now();
            const timeoutMs = options.timeoutMs ?? PROPOSER_ROUTE_TIMEOUT_MS;
            const retryMs = options.readyRetryMs ?? PROPOSER_READY_RETRY_MS;
            let attempts = 0;
            while (true) {
                attempts += 1;
                const ready = await firstReadyEndpointForRoute(fleet, proposerRouteCache.selection.route, options);
                if (ready) {
                    if (!options.skipBackgroundReadinessProbe) {
                        startCachedSelectionReadinessProbe(proposerRouteCache.selection, options);
                    }
                    const route = {
                        ...proposerRouteCache.selection.route,
                        route_kind: 'sequenced_account_read',
                        read_validator: ready.endpoint.validatorId,
                        route_cache_hit: true,
                        route_cache_age_ms: now - proposerRouteCache.cached_at_ms,
                        route_wait_ms: Date.now() - started,
                        route_attempts: attempts,
                        ready_observed_height: ready.status?.block_height ?? null,
                        ready_observed_state_root: ready.status?.state_root || null,
                        read_source: 'first_ready_parent',
                    };
                    return { endpoint: ready.endpoint, route };
                }
                if (Date.now() - started >= timeoutMs) {
                    invalidateProposerRouteCache();
                    break;
                }
                await sleep(Math.max(0, retryMs));
            }
        }

        if (ENABLE_RPC_PARENT_WAIT_SEQUENCED_READ_ROUTE) {
            const endpoint = preferredSequencedReadEndpoint(fleet);
            return {
                endpoint,
                route: {
                    routed: true,
                    route_kind: 'sequenced_account_read',
                    read_validator: endpoint.validatorId,
                    route_source: 'preferred_current_read',
                    route_cache_hit: false,
                    route_wait_ms: 0,
                    route_attempts: 0,
                    read_source: 'preferred_current_read',
                },
            };
        }

        return chooseProposerEndpointCached(fleet, {
            ...options,
            routeKind: 'sequenced_account_read',
        });
    }

    async function chooseProposerEndpointCached(fleet, options = {}) {
        const routeKind = options.routeKind || null;
        const now = Date.now();
        if (
            proposerRouteCache
            && Number.isFinite(PROPOSER_ROUTE_CACHE_MS)
            && PROPOSER_ROUTE_CACHE_MS > 0
            && now - proposerRouteCache.cached_at_ms <= PROPOSER_ROUTE_CACHE_MS
        ) {
            try {
                if (
                    (ENABLE_RPC_PARENT_WAIT_FINALITY_ROUTE || OPTIMISTIC_CACHED_FINALITY_ROUTE)
                    && routeKind === null
                    && Number.isFinite(Number(proposerRouteCache.selection.route?.required_current_height))
                ) {
                    startCachedSelectionReadinessProbe(proposerRouteCache.selection, options);
                    const selected = cachedSelection(
                        proposerRouteCache.selection,
                        null,
                        true,
                        now - proposerRouteCache.cached_at_ms,
                    );
                    selected.route.route_wait_ms = 0;
                    selected.route.route_attempts = 0;
                    selected.route.readiness_check = ENABLE_RPC_PARENT_WAIT_FINALITY_ROUTE
                        ? 'rpc_parent_wait_finality_route'
                        : 'optimistic_cached_finality_route';
                    return selected;
                }
                const ready = await waitForCachedSelectionReady(proposerRouteCache.selection, options);
                const selected = cachedSelection(
                    proposerRouteCache.selection,
                    routeKind,
                    true,
                    now - proposerRouteCache.cached_at_ms,
                );
                selected.route.route_wait_ms = ready.waitMs;
                selected.route.route_attempts = ready.attempts;
                selected.route.ready_observed_height = ready.observedStatus?.block_height ?? null;
                selected.route.ready_observed_state_root = ready.observedStatus?.state_root || null;
                return selected;
            } catch (error) {
                invalidateProposerRouteCache();
            }
        }

        const selected = await chooseProposerEndpointWithRetry(fleet, options);
        proposerRouteCache = {
            cached_at_ms: Date.now(),
            selection: {
                endpoint: selected.endpoint,
                route: { ...selected.route },
            },
        };
        return cachedSelection(selected, routeKind, false, 0);
    }

    function convergedFleetGroup(fleetStatuses) {
        const okStatuses = fleetStatuses.filter((entry) => entry.ok);
        if (okStatuses.length === 0) {
            throw new Error('no reachable validators');
        }
        const groups = new Map();
        for (const entry of okStatuses) {
            const key = [
                entry.status.block_height,
                entry.status.block_tip_hash,
                entry.status.state_root,
            ].join('|');
            if (!groups.has(key)) groups.set(key, []);
            groups.get(key).push(entry);
        }
        return [...groups.values()].sort((a, b) => b.length - a.length)[0];
    }

    async function waitForFastpayConvergedGroup(fleet, options = {}) {
        const timeoutMs = options.timeoutMs ?? FASTPAY_ROUTE_TIMEOUT_MS;
        const retryMs = options.retryMs ?? FASTPAY_ROUTE_RETRY_MS;
        const collectStatuses = options.collectStatuses || collectFastpayFleetStatuses;
        const quorum = bftQuorumThreshold(fleet.length);
        const requiredCount = options.requiredCount ?? quorum;
        const started = Date.now();
        let attempts = 0;
        let lastError = null;

        while (true) {
            attempts += 1;
            try {
                const statuses = await collectStatuses(fleet, { forceRefresh: attempts > 1 });
                const majority = convergedFleetGroup(statuses);
                if (majority.length >= requiredCount) {
                    return {
                        majority,
                        quorum,
                        required_count: requiredCount,
                        attempts,
                        wait_ms: Date.now() - started,
                    };
                }
                lastError = new Error(
                    `fleet is not converged enough for FastPay routing: got ${majority.length}, need ${requiredCount}`,
                );
            } catch (error) {
                lastError = error;
            }

            if (Date.now() - started >= timeoutMs) {
                throw new Error(`${lastError?.message || lastError} after ${attempts} route attempt(s)`);
            }
            await sleep(Math.max(0, retryMs));
        }
    }

    async function chooseOwnedVoteEndpoint(request, method = 'owned_sign') {
        const validatorId = request?.params?.validator_id;
        if (!validatorId || typeof validatorId !== 'string') {
            throw new Error(`${method} requires validator_id`);
        }
        const { majority, quorum, attempts, wait_ms } = await waitForFastpayConvergedGroup(RPC_FLEET);
        const entry = majority.find((candidate) => candidate.endpoint.validatorId === validatorId);
        if (!entry) {
            throw new Error(`FastPay vote validator ${validatorId} is not in the converged majority`);
        }
        return {
            endpoint: entry.endpoint,
            route: {
                routed: true,
                route_kind: 'fastpay_vote',
                method,
                validator: validatorId,
                height: Number(entry.status.block_height),
                quorum,
                converged_count: majority.length,
                convergence_attempts: attempts,
                convergence_wait_ms: wait_ms,
            },
        };
    }

    async function resolveRpcTarget(method) {
        if (['owned_sign', 'owned_unwrap_sign', 'owned_sign_v3', 'owned_unwrap_sign_v3'].includes(method)) {
            throw new Error(`${method} requires request-aware routing`);
        }
        if (!ENABLE_PROPOSER_ROUTING || !isFinalityMethod(method)) {
            if (ENABLE_PROPOSER_ROUTING && isSequencedAccountMethod(method)) {
                if (shouldUseFirstReadySequencedRead()) {
                    return chooseSequencedAccountReadEndpoint(RPC_FLEET);
                }
                return chooseProposerEndpointCached(RPC_FLEET, {
                    routeKind: 'sequenced_account_read',
                });
            }
            return {
                endpoint: { validatorId: 'primary', host: RPC_HOST, port: RPC_PORT },
                route: null,
            };
        }
        return chooseProposerEndpointCached(RPC_FLEET);
    }

    function addProxyRouteEvent(line, route) {
        if (!route) return line;
        try {
            const response = JSON.parse(line);
            if (!Array.isArray(response.events)) response.events = [];
            const subject = route.read_validator || route.proposer || route.validator || route.route_kind || 'proxy';
            let message;
            if (route.route_kind === 'fastpay_vote') {
                message = `FastPay vote request routed to ${route.validator} at height ${route.height}`;
            } else if (route.route_kind === 'sequenced_account_read') {
                const readValidator = route.read_validator || route.proposer;
                message = `sequenced account read routed to ${readValidator} for parent height ${route.required_current_height || route.height}`;
            } else {
                message = `finality request routed to ${route.proposer} for height ${route.height} view ${route.view}`;
            }
            response.events.unshift({
                event_type: route.route_kind === 'fastpay_vote'
                    ? 'proxy_fastpay_vote_route'
                    : route.route_kind === 'sequenced_account_read'
                        ? 'proxy_sequence_read_route'
                        : 'proxy_proposer_route',
                subject,
                message,
            });
            response.proxy_route = route;
            return JSON.stringify(response);
        } catch (e) {
            return line;
        }
    }

    function responseEnvelope(id, ok, result, error = null, events = []) {
        return {
            version: 'postfiat-local-rpc-v1',
            id,
            ok,
            result: ok ? result : null,
            error,
            events,
        };
    }

    function canonicalReadResult(method, result) {
        if (!result || typeof result !== 'object' || Array.isArray(result)) return result;
        if (method === 'owned_objects') {
            const copy = { ...result };
            if (Array.isArray(copy.objects)) {
                copy.objects = [...copy.objects].sort((a, b) => {
                    const aKey = `${a?.id || ''}:${a?.version ?? ''}`;
                    const bKey = `${b?.id || ''}:${b?.version ?? ''}`;
                    return aKey.localeCompare(bKey);
                });
            }
            return copy;
        }
        return result;
    }

    function readGroupKey(method, result) {
        return JSON.stringify(canonicalReadResult(method, result));
    }

    function conciseRpcError(error, fallback = 'RPC request failed') {
        if (!error) return fallback;
        if (error.message) return error.message;
        if (error.code) return error.code;
        return String(error);
    }

    async function readFleetRpcMajority(method, params, timeoutMs = WALLET_SUBSCRIPTION_READ_TIMEOUT_MS) {
        const reads = RPC_FLEET.map(async (endpoint) => {
            const request = {
                version: 'postfiat-local-rpc-v1',
                id: `proxy-live-${method}-${endpoint.validatorId}-${Date.now()}`,
                method,
                params,
            };
            try {
                const response = await rpcTcpRequest(endpoint.host, endpoint.port, request, timeoutMs);
                return {
                    validator_id: endpoint.validatorId,
                    ok: response.ok === true,
                    result: response.result || null,
                    error: response.error || null,
                };
            } catch (error) {
                return {
                    validator_id: endpoint.validatorId,
                    ok: false,
                    result: null,
                    error: {
                        code: 'proxy_wallet_feed_read_error',
                        message: error?.message || String(error),
                    },
                };
            }
        });

        const results = await Promise.all(reads);
        const successes = results.filter((entry) => entry.ok);
        if (successes.length === 0) {
            const firstError = results.find((entry) => entry.error)?.error;
            return {
                ok: false,
                result: null,
                source: {
                    method,
                    consensus_count: 0,
                    fleet_count: RPC_FLEET.length,
                    validators: results,
                },
                error: {
                    code: 'proxy_wallet_feed_read_failed',
                    message: conciseRpcError(firstError, `${method} unavailable from validator fleet`),
                },
            };
        }

        const groups = new Map();
        for (const entry of successes) {
            const key = readGroupKey(method, entry.result);
            if (!groups.has(key)) groups.set(key, []);
            groups.get(key).push(entry);
        }
        const [key, group] = [...groups.entries()].sort((a, b) => b[1].length - a[1].length)[0];
        const result = JSON.parse(key);
        return {
            ok: true,
            result,
            source: {
                method,
                consensus_count: group.length,
                fleet_count: RPC_FLEET.length,
                validators: group.map((entry) => entry.validator_id),
            },
            error: null,
        };
    }

    async function fetchWalletSnapshot(params) {
        const address = typeof params.address === 'string' ? params.address.trim() : '';
        const ownerPublicKeyHex = typeof params.owner_public_key_hex === 'string'
            ? params.owner_public_key_hex.trim()
            : '';
        const asset = typeof params.asset === 'string' && params.asset.trim()
            ? params.asset.trim()
            : 'PFT';
        const ownedLimit = Number.isInteger(params.owned_limit)
            ? Math.max(1, Math.min(params.owned_limit, FASTPAY_OWNED_OBJECT_LOOKUP_LIMIT))
            : FASTPAY_OWNED_OBJECT_LOOKUP_LIMIT;

        const includeAssets = params.include_assets === true;

        const [accountRead, ownedRead, assetsRead] = await Promise.all([
            address
                ? readFleetRpcMajority('account', { address })
                : Promise.resolve({ ok: false, result: null, error: null, source: null }),
            ownerPublicKeyHex
                ? readFleetRpcMajority('owned_objects', {
                    owner_public_key_hex: ownerPublicKeyHex,
                    asset,
                    limit: ownedLimit,
                })
                : Promise.resolve({ ok: false, result: null, error: null, source: null }),
            includeAssets && address
                ? readFleetRpcMajority('account_assets', { account: address })
                : Promise.resolve({ ok: false, result: null, error: null, source: null }),
        ]);

        return {
            schema: 'postfiat-wallet-snapshot-v1',
            address: address || null,
            owner_public_key_hex: ownerPublicKeyHex || null,
            asset,
            include_assets: includeAssets,
            observed_at_ms: Date.now(),
            account: accountRead.ok ? accountRead.result : null,
            account_error: accountRead.ok ? null : accountRead.error,
            owned: ownedRead.ok ? ownedRead.result : null,
            owned_error: ownedRead.ok ? null : ownedRead.error,
            assets: assetsRead.ok ? assetsRead.result : null,
            assets_error: includeAssets && !assetsRead.ok ? assetsRead.error : null,
            sources: {
                account: accountRead.source,
                owned: ownedRead.source,
                assets: assetsRead.source,
            },
        };
    }

    function walletSnapshotDigest(snapshot) {
        return crypto.createHash('sha256').update(JSON.stringify({
            account: snapshot.account,
            account_error: snapshot.account_error,
            owned: snapshot.owned,
            owned_error: snapshot.owned_error,
            assets: snapshot.assets,
            assets_error: snapshot.assets_error,
        })).digest('hex');
    }

    function normalizeWalletSubscriptionParams(params = {}) {
        const address = typeof params.address === 'string' ? params.address.trim() : '';
        const ownerPublicKeyHex = typeof params.owner_public_key_hex === 'string'
            ? params.owner_public_key_hex.trim()
            : '';
        if (!address && !ownerPublicKeyHex) {
            throw new Error('wallet_subscribe requires address or owner_public_key_hex');
        }
        const requestedInterval = Number.parseInt(params.interval_ms || WALLET_SUBSCRIPTION_INTERVAL_MS, 10);
        return {
            address,
            owner_public_key_hex: ownerPublicKeyHex,
            asset: typeof params.asset === 'string' && params.asset.trim() ? params.asset.trim() : 'PFT',
            include_assets: params.include_assets === true,
            owned_limit: Number.isInteger(params.owned_limit)
                ? Math.max(1, Math.min(params.owned_limit, FASTPAY_OWNED_OBJECT_LOOKUP_LIMIT))
                : FASTPAY_OWNED_OBJECT_LOOKUP_LIMIT,
            interval_ms: Math.max(
                WALLET_SUBSCRIPTION_MIN_INTERVAL_MS,
                Number.isFinite(requestedInterval) ? requestedInterval : WALLET_SUBSCRIPTION_INTERVAL_MS,
            ),
        };
    }

    function stopWalletSubscription(subscription) {
        subscription.stopped = true;
        if (subscription.timer) {
            clearInterval(subscription.timer);
            subscription.timer = null;
        }
    }

    function sendWalletNotification(ws, subscription, snapshot, reason) {
        if (ws.readyState !== 1 || subscription.stopped) return;
        ws.send(JSON.stringify({
            version: 'postfiat-local-rpc-v1',
            method: 'wallet_update',
            params: {
                subscription_id: subscription.id,
                reason,
                interval_ms: subscription.params.interval_ms,
                snapshot,
            },
            events: [{
                event_type: 'proxy_wallet_feed',
                subject: subscription.id,
                message: 'wallet snapshot changed',
            }],
        }));
    }

    function startWalletSubscription(ws, request, subscriptions) {
        const params = normalizeWalletSubscriptionParams(request.params || {});
        const id = `wallet-sub-${Date.now()}-${crypto.randomBytes(4).toString('hex')}`;
        const subscription = {
            id,
            params,
            timer: null,
            stopped: false,
            polling: false,
            lastDigest: null,
        };
        subscriptions.set(id, subscription);

        const poll = async (reason) => {
            if (subscription.stopped || subscription.polling) return;
            subscription.polling = true;
            try {
                const snapshot = await fetchWalletSnapshot(subscription.params);
                const digest = walletSnapshotDigest(snapshot);
                if (reason === 'initial' || digest !== subscription.lastDigest) {
                    subscription.lastDigest = digest;
                    sendWalletNotification(ws, subscription, snapshot, reason);
                }
            } catch (error) {
                const snapshot = {
                    schema: 'postfiat-wallet-snapshot-v1',
                    address: subscription.params.address || null,
                    owner_public_key_hex: subscription.params.owner_public_key_hex || null,
                    asset: subscription.params.asset,
                    observed_at_ms: Date.now(),
                    account: null,
                    account_error: {
                        code: 'proxy_wallet_feed_error',
                        message: error?.message || String(error),
                    },
                    owned: null,
                    owned_error: {
                        code: 'proxy_wallet_feed_error',
                        message: error?.message || String(error),
                    },
                    assets: null,
                    assets_error: subscription.params.include_assets ? {
                        code: 'proxy_wallet_feed_error',
                        message: error?.message || String(error),
                    } : null,
                    sources: { account: null, owned: null, assets: null },
                };
                sendWalletNotification(ws, subscription, snapshot, 'error');
            } finally {
                subscription.polling = false;
            }
        };

        subscription.timer = setInterval(() => poll('interval'), params.interval_ms);
        setImmediate(() => poll('initial'));
        return subscription;
    }

    function requestWithProxyReadiness(request, route) {
        if (
            !route
            || !isFinalityMethod(request.method)
            || !Number.isFinite(Number(route.required_current_height))
        ) {
            return request;
        }
        const params = (
            request.params
            && typeof request.params === 'object'
            && !Array.isArray(request.params)
        ) ? { ...request.params } : {};
        params.proxy_required_current_height = Number(route.required_current_height);
        if (route.required_state_root) {
            params.proxy_required_state_root = route.required_state_root;
        }
        if (route.required_parent_hash) {
            params.proxy_required_parent_hash = route.required_parent_hash;
        }
        params.proxy_readiness_timeout_ms = PROPOSER_ROUTE_TIMEOUT_MS;
        return { ...request, params };
    }

    function normalizeFastpayBroadcastRequest(request) {
        const normalized = JSON.parse(JSON.stringify(request));
        if (!normalized.params || typeof normalized.params !== 'object') {
            normalized.params = {};
        }
        return normalized;
    }

    function firstStructuredFastpayResult(results) {
        const hit = results.find((entry) => (
            entry.ok
            && entry.result
            && typeof entry.result === 'object'
            && !Array.isArray(entry.result)
        ));
        return hit ? hit.result : null;
    }

    const fastpayCertificateReplicationInFlight = new Map();

    function isFastpayCertificateApplyMethod(method) {
        return method === 'owned_apply'
            || method === 'owned_unwrap_apply'
            || method === 'owned_apply_v3'
            || method === 'owned_unwrap_apply_v3';
    }

    function fastpayCertificateRecord(request) {
        const certJson = request?.params?.cert_json;
        if (typeof certJson !== 'string' || certJson.length === 0) {
            throw new Error(`${request?.method || 'FastPay apply'} requires cert_json`);
        }
        let certificate;
        try {
            certificate = JSON.parse(certJson);
        } catch (_) {
            throw new Error('FastPay certificate is not valid JSON');
        }
        const validatorIds = new Set(RPC_FLEET.map(entry => entry.validatorId));
        const distinctVotes = new Set(
            (Array.isArray(certificate?.votes) ? certificate.votes : [])
                .map(vote => vote?.validator_id)
                .filter(validatorId => validatorIds.has(validatorId)),
        );
        const quorum = bftQuorumThreshold(RPC_FLEET.length);
        if (distinctVotes.size < quorum) {
            throw new Error(`FastPay certificate has ${distinctVotes.size} distinct fleet votes, need ${quorum}`);
        }
        const certificateId = crypto.createHash('sha256')
            .update('postfiat.fastpay.proxy-certificate.v1\0')
            .update(request.method)
            .update('\0')
            .update(certJson)
            .digest('hex');
        return {
            certificate_id: certificateId,
            method: request.method,
            request: normalizeFastpayBroadcastRequest(request),
            created_at_ms: Date.now(),
        };
    }

    function fastpayApplyAck(response) {
        if (response?.ok === true) return { accepted: true, idempotent: false };
        const code = String(response?.error?.code || '');
        const message = String(response?.error?.message || '');
        // Validators verify owner auth and the quorum certificate before they
        // inspect live inputs. UnknownInput therefore means this valid,
        // quorum-locked certificate was already applied during a prior relay
        // attempt; a conflicting quorum certificate cannot exist without an
        // honest validator equivocating.
        if ((code === 'owned_apply_failed'
            || code === 'owned_unwrap_apply_failed'
            || code === 'owned_apply_v3_failed'
            || code === 'owned_unwrap_apply_v3_failed')
            && /UnknownInput/.test(message)) {
            return { accepted: true, idempotent: true };
        }
        return { accepted: false, idempotent: false };
    }

    function launchFastpayCertificateReplication(record, majority, quorum) {
        const existing = fastpayCertificateReplicationInFlight.get(record.certificate_id);
        if (existing) return existing;

        const requiresSignedQuorum = record.method === 'owned_apply_v3'
            || record.method === 'owned_unwrap_apply_v3';
        let resolveFirst;
        let rejectFirst;
        let firstSettled = false;
        const firstAck = new Promise((resolve, reject) => {
            resolveFirst = resolve;
            rejectFirst = reject;
        });
        const current = fastpayCertificateOutbox.pending()
            .find(item => item.certificate_id === record.certificate_id) || record;
        const alreadyApplied = new Set(current.applied_validators || []);
        const signedAcknowledgements = new Map(
            (current.apply_acknowledgements || [])
                .map(acknowledgement => [acknowledgement.validator_id, acknowledgement]),
        );
        let resolveQuorum;
        let rejectQuorum;
        let quorumSettled = false;
        const quorumAck = new Promise((resolve, reject) => {
            resolveQuorum = resolve;
            rejectQuorum = reject;
        });
        const maybeResolveQuorum = (updated) => {
            if (quorumSettled || signedAcknowledgements.size < quorum) return;
            quorumSettled = true;
            resolveQuorum({
                acknowledgements: [...signedAcknowledgements.values()]
                    .sort((left, right) => left.validator_id.localeCompare(right.validator_id)),
                record: updated,
            });
        };
        if (alreadyApplied.size > 0) {
            firstSettled = true;
            resolveFirst({
                validator_id: [...alreadyApplied][0],
                response: null,
                idempotent: true,
                record: current,
            });
        }
        maybeResolveQuorum(current);

        const targets = majority.filter(entry => !alreadyApplied.has(entry.endpoint.validatorId));
        const tasks = targets.map(async (entry) => {
            const started = Date.now();
            try {
                const outbound = (record.method === 'owned_apply_v3'
                    || record.method === 'owned_unwrap_apply_v3')
                    ? {
                        ...record.request,
                        params: {
                            ...record.request.params,
                            validator_id: entry.endpoint.validatorId,
                        },
                    }
                    : record.request;
                const response = await rpcTcpRequest(
                    entry.endpoint.host,
                    entry.endpoint.port,
                    outbound,
                    TCP_TIMEOUT_MS,
                    'fastpay-apply',
                );
                const ack = fastpayApplyAck(response);
                if (!ack.accepted) {
                    return {
                        validator_id: entry.endpoint.validatorId,
                        ok: false,
                        duration_ms: Date.now() - started,
                        error: response?.error || { code: 'fastpay_apply_rejected' },
                    };
                }
                const structured = response?.result && typeof response.result === 'object'
                    && !Array.isArray(response.result) ? response.result : null;
                const signedAcknowledgement = structured?.schema === 'postfiat-fastpay-apply-ack-v1'
                    && structured.validator_id === entry.endpoint.validatorId
                    ? structured : null;
                if (requiresSignedQuorum && response?.ok === true && !signedAcknowledgement) {
                    return {
                        validator_id: entry.endpoint.validatorId,
                        ok: false,
                        duration_ms: Date.now() - started,
                        error: { code: 'proxy_fastpay_signed_ack_missing' },
                    };
                }
                const updated = fastpayCertificateOutbox.markApplied(
                    record.certificate_id,
                    entry.endpoint.validatorId,
                    signedAcknowledgement,
                );
                if (signedAcknowledgement) {
                    signedAcknowledgements.set(
                        entry.endpoint.validatorId,
                        structuredClone(signedAcknowledgement),
                    );
                    maybeResolveQuorum(updated);
                }
                const result = {
                    validator_id: entry.endpoint.validatorId,
                    ok: true,
                    idempotent: ack.idempotent,
                    duration_ms: Date.now() - started,
                    result: response?.result || null,
                };
                if (!firstSettled) {
                    firstSettled = true;
                    resolveFirst({
                        validator_id: entry.endpoint.validatorId,
                        response,
                        idempotent: ack.idempotent,
                        record: updated,
                    });
                }
                return result;
            } catch (error) {
                return {
                    validator_id: entry.endpoint.validatorId,
                    ok: false,
                    duration_ms: Date.now() - started,
                    error: { code: 'proxy_fastpay_endpoint_error', message: error?.message || String(error) },
                };
            }
        });

        const done = Promise.all(tasks).then((results) => {
            const updated = fastpayCertificateOutbox.pending()
                .find(item => item.certificate_id === record.certificate_id);
            if (updated && updated.applied_validators.length === RPC_FLEET.length) {
                fastpayCertificateOutbox.complete(record.certificate_id);
            }
            if (!firstSettled) {
                firstSettled = true;
                rejectFirst(new Error('FastPay certificate was not accepted by any validator'));
            }
            if (requiresSignedQuorum && !quorumSettled) {
                quorumSettled = true;
                rejectQuorum(new Error(
                    `FastPay certificate received ${signedAcknowledgements.size}/${quorum} signed durable apply acknowledgements`,
                ));
            }
            return results;
        }).finally(() => {
            fastpayCertificateReplicationInFlight.delete(record.certificate_id);
        });
        const launched = { firstAck, quorumAck, done };
        fastpayCertificateReplicationInFlight.set(record.certificate_id, launched);
        return launched;
    }

    async function broadcastFastpayCertificate(request) {
        const record = fastpayCertificateRecord(request);
        const replayedTerminal = fastpayCertificateOutbox.terminal(record);
        if (replayedTerminal) {
            return responseEnvelope(request.id, true, replayedTerminal, null, [{
                event_type: 'proxy_fastpay_certificate_finality',
                subject: request.method,
                message: `FastPay certificate finality replayed from its durable completed record`,
            }]);
        }
        const durable = fastpayCertificateOutbox.enqueue(record);
        const { majority, quorum, attempts, wait_ms } = await waitForFastpayConvergedGroup(RPC_FLEET);
        const launched = launchFastpayCertificateReplication(durable, majority, quorum);
        const requiresSignedQuorum = request.method === 'owned_apply_v3'
            || request.method === 'owned_unwrap_apply_v3';
        const finality = requiresSignedQuorum
            ? await launched.quorumAck
            : await launched.firstAck.then(acknowledgement => ({
                acknowledgements: [],
                record: acknowledgement.record,
                legacy_ack: acknowledgement,
            }));
        // Do not await launched.done: exact-six replication continues from the
        // durable outbox and is replayed after proxy restart.
        launched.done.catch(error => console.error(`FastPay certificate replication failed: ${error.message || error}`));
        const applied = finality.record?.applied_validators || [];
        const acknowledgements = finality.acknowledgements || [];
        const structured = acknowledgements[0]
            || (finality.legacy_ack?.response?.result
                && typeof finality.legacy_ack.response.result === 'object'
                ? finality.legacy_ack.response.result : null);
        const terminalResult = {
            schema: 'postfiat-fastpay-certificate-finality-v1',
            method: request.method,
            certificate_id: record.certificate_id,
            certificate_final: true,
            certificate_quorum: quorum,
            certificate_vote_count: new Set(JSON.parse(request.params.cert_json).votes.map(vote => vote.validator_id)).size,
            apply_ack_validator: finality.legacy_ack?.validator_id || null,
            apply_ack_idempotent: finality.legacy_ack?.idempotent || false,
            apply_acknowledgements: acknowledgements,
            applied_count: applied.length,
            fleet_count: RPC_FLEET.length,
            replication_pending_count: Math.max(0, RPC_FLEET.length - applied.length),
            convergence_attempts: attempts,
            convergence_wait_ms: wait_ms,
            created_objects: Array.isArray(structured?.created_objects) ? structured.created_objects : [],
            summary: structured?.summary || 'quorum certificate finalized; fleet replication continues asynchronously',
        };
        const terminalRecord = fastpayCertificateOutbox.markTerminal(
            record.certificate_id,
            terminalResult,
        );
        if (terminalRecord?.applied_validators?.length === RPC_FLEET.length) {
            fastpayCertificateOutbox.complete(record.certificate_id);
        }
        return responseEnvelope(request.id, true, terminalResult, null, [{
            event_type: 'proxy_fastpay_certificate_finality',
            subject: request.method,
            message: `FastPay certificate finalized with ${acknowledgements.length}/${quorum} signed durable apply acknowledgements; exact-six replication continues`,
        }]);
    }

    let fastpayRouteWarmupTimer = null;
    let fastpayRouteWarmupInitial = null;

    function startFastpayRouteWarmup() {
        if (runtime.FASTPAY_ROUTE_WARMUP_ENABLED !== true || fastpayRouteWarmupTimer) {
            return {
                timer: fastpayRouteWarmupTimer,
                initial: fastpayRouteWarmupInitial,
            };
        }

        const refresh = async (channel) => {
            const statuses = await collectFastpayFleetStatuses(RPC_FLEET, {
                forceRefresh: true,
                channel,
            });
            const majority = convergedFleetGroup(statuses);
            const quorum = bftQuorumThreshold(RPC_FLEET.length);
            if (majority.length < quorum) {
                throw new Error(
                    `FastPay route warmup found only ${majority.length}/${RPC_FLEET.length} converged validators`,
                );
            }
            return { converged_count: majority.length, quorum };
        };

        // The first status wave intentionally uses the dedicated vote channel.
        // That establishes every validator TCP session before a payment arrives.
        // Certificate application has its own channel because mutation handlers
        // may close their upstream connection; an apply must never evict the hot
        // owned_sign lane needed by the next payment.
        // Periodic health refreshes use an isolated channel so they can never
        // queue in front of an owned_sign or owned_apply request.
        fastpayRouteWarmupInitial = refresh('fastpay-vote').catch((error) => {
            console.error(`FastPay route warmup deferred: ${error.message || error}`);
            return null;
        });
        const refreshMs = Number.isInteger(runtime.FASTPAY_ROUTE_REFRESH_MS)
            && runtime.FASTPAY_ROUTE_REFRESH_MS >= 250
            ? runtime.FASTPAY_ROUTE_REFRESH_MS
            : Math.max(250, Math.floor(FASTPAY_FLEET_STATUS_CACHE_MS / 2));
        fastpayRouteWarmupTimer = setInterval(
            () => refresh('status').catch((error) => {
                console.error(`FastPay route refresh deferred: ${error.message || error}`);
            }),
            refreshMs,
        );
        fastpayRouteWarmupTimer.unref?.();
        return { timer: fastpayRouteWarmupTimer, initial: fastpayRouteWarmupInitial };
    }

    let fastpayCertificateRecoveryTimer = null;
    function startFastpayCertificateRecovery() {
        if (!FASTPAY_CERTIFICATE_FINALITY_ENABLED || fastpayCertificateRecoveryTimer) return null;
        const recover = async () => {
            for (const record of fastpayCertificateOutbox.pending()) {
                if (fastpayCertificateReplicationInFlight.has(record.certificate_id)) continue;
                try {
                    const { majority, quorum } = await waitForFastpayConvergedGroup(RPC_FLEET);
                    launchFastpayCertificateReplication(record, majority, quorum).done.catch(() => {});
                } catch (error) {
                    console.error(`FastPay certificate recovery deferred: ${error.message || error}`);
                }
            }
        };
        recover().catch(() => {});
        fastpayCertificateRecoveryTimer = setInterval(
            () => recover().catch(() => {}),
            Number.isInteger(FASTPAY_CERTIFICATE_RETRY_MS) && FASTPAY_CERTIFICATE_RETRY_MS >= 250
                ? FASTPAY_CERTIFICATE_RETRY_MS
                : 2000,
        );
        fastpayCertificateRecoveryTimer.unref?.();
        return fastpayCertificateRecoveryTimer;
    }

    async function broadcastFastpayMutation(request) {
        if (FASTPAY_CERTIFICATE_FINALITY_ENABLED
            && fastpayCertificateOutbox
            && isFastpayCertificateApplyMethod(request.method)) {
            return broadcastFastpayCertificate(request);
        }
        // BFT FastPay apply semantics: an owned-apply (wrap/unwrap/owned_apply) is
        // final once a quorum of validators apply it — the L1 execution engine
        // (apply_owned_certificate) accepts a certificate at quorum
        // (bft_quorum_threshold(n) = floor(2n/3)+1 = 5 of 6). Demanding ALL
        // validators (the previous `RPC_FLEET.length`) made every FastPay
        // transfer fail if any single validator was slow or down, even though the
        // transfer had already reached final quorum. We still broadcast to every
        // converged validator for replication, but resolve success at quorum.
        //
        // FASTPAY_BROADCAST_REQUIRED_COUNT overrides the threshold (e.g. set to
        // the fleet size to restore the old all-validator requirement for testing).
        const broadcastRequiredCount = parseInt(
            process.env.FASTPAY_BROADCAST_REQUIRED_COUNT || '0',
            10,
        ) || bftQuorumThreshold(RPC_FLEET.length);
        const {
            majority,
            quorum,
            required_count,
            attempts,
            wait_ms,
        } = await waitForFastpayConvergedGroup(RPC_FLEET, {
            requiredCount: broadcastRequiredCount,
        });
        const normalized = normalizeFastpayBroadcastRequest(request);
        const primary = RPC_FLEET.find((entry) => entry.host === RPC_HOST && entry.port === RPC_PORT)
            || RPC_FLEET[0];

        return new Promise((resolve) => {
            const results = [];
            let settled = 0;
            let resolved = false;

            const successes = () => results.filter((entry) => entry.ok);
            const primarySucceeded = () => successes()
                .some((entry) => entry.validator_id === primary.validatorId);

            const successResponse = () => {
                const applied = successes();
                const structured = firstStructuredFastpayResult(applied);
                return responseEnvelope(
                    request.id,
                    true,
                    {
                        schema: 'postfiat-fastpay-broadcast-result-v1',
                        method: request.method,
                        applied_count: applied.length,
                        converged_count: majority.length,
                        fleet_count: RPC_FLEET.length,
                        quorum,
                        required_count,
                        quorum_fast: true,
                        convergence_attempts: attempts,
                        convergence_wait_ms: wait_ms,
                        primary_validator: primary.validatorId,
                        primary_required: FASTPAY_REQUIRE_PRIMARY_SUCCESS,
                        primary_succeeded: primarySucceeded(),
                        pending_count: majority.length - settled,
                        object_id: normalized.params.object_id || null,
                        created_objects: Array.isArray(structured?.created_objects)
                            ? structured.created_objects
                            : [],
                        summary: structured?.summary || null,
                        validators: results,
                    },
                    null,
                    [{
                        event_type: 'proxy_fastpay_broadcast',
                        subject: request.method,
                        message: `FastPay ${request.method} applied on ${applied.length}/${RPC_FLEET.length} validators`,
                    }],
                );
            };

            const failureResponse = () => {
                const failures = results.filter((entry) => !entry.ok);
                return responseEnvelope(
                    request.id,
                    false,
                    null,
                    {
                        code: 'proxy_fastpay_broadcast_failed',
                        message: `FastPay ${request.method} failed before ${required_count}/${RPC_FLEET.length} validator completion`,
                        validators: failures,
                    },
                    [{
                        event_type: 'proxy_fastpay_broadcast',
                        subject: request.method,
                        message: `FastPay ${request.method} broadcast failed before required validator completion`,
                    }],
                );
            };

            const maybeResolve = () => {
                if (resolved) return;
                if (
                    successes().length >= required_count
                    && (!FASTPAY_REQUIRE_PRIMARY_SUCCESS || primarySucceeded())
                ) {
                    resolved = true;
                    resolve(successResponse());
                    return;
                }
                if (settled === majority.length) {
                    resolved = true;
                    const ok = successes().length >= required_count
                        && (!FASTPAY_REQUIRE_PRIMARY_SUCCESS || primarySucceeded());
                    // DIAGNOSTIC: log the per-validator broadcast outcome so apply
                    // failures on individual validators are visible.
                    const tally = results.map((r) => ({
                        v: r.validator_id,
                        ok: r.ok,
                        err: r.error ? (r.error.message || r.error.code || JSON.stringify(r.error)).slice(0, 200) : null,
                    }));
                    console.log(`[fastpay-broadcast] method=${request.method} ok=${ok} applied=${successes().length}/${majority.length} required=${required_count} tally=${JSON.stringify(tally)}`);
                    resolve(ok ? successResponse() : failureResponse());
                }
            };

            for (const entry of majority) {
                const started = Date.now();
                const outbound = (request.method === 'owned_apply_v3'
                    || request.method === 'owned_unwrap_apply_v3')
                    ? {
                        ...normalized,
                        params: {
                            ...normalized.params,
                            validator_id: entry.endpoint.validatorId,
                        },
                    }
                    : normalized;
                rpcTcpRequest(
                    entry.endpoint.host,
                    entry.endpoint.port,
                    outbound,
                    TCP_TIMEOUT_MS,
                    'fastpay-apply',
                )
                    .then((response) => {
                        results.push({
                            validator_id: entry.endpoint.validatorId,
                            ok: response.ok === true,
                            duration_ms: Date.now() - started,
                            result: response.result || null,
                            error: response.error || null,
                        });
                    })
                    .catch((error) => {
                        results.push({
                            validator_id: entry.endpoint.validatorId,
                            ok: false,
                            duration_ms: Date.now() - started,
                            result: null,
                            error: {
                                code: 'proxy_fastpay_endpoint_error',
                                message: error?.message || String(error),
                            },
                        });
                    })
                    .finally(() => {
                        settled += 1;
                        maybeResolve();
                    });
            }
        });
    }


    return { UpstreamRpcConnection,addProxyRouteEvent,bftQuorumThreshold,broadcastFastpayMutation,cachedSelection,canonicalReadResult,chooseOwnedVoteEndpoint,chooseProposerEndpointCached,chooseProposerEndpointFromStatuses,chooseProposerEndpointWithRetry,chooseSequencedAccountReadEndpoint,clearFastpayFleetStatusCache,closeUpstreamRpcConnections,collectFastpayFleetStatuses,collectFinalityTimeoutVotes,collectFleetStatuses,conciseRpcError,convergedFleetGroup,deterministicProposer,endpointStatusMeetsRoute,endpointStatusMeetsSequencedReadRoute,exactParentStatus,fetchWalletSnapshot,finalityFailureCanAdvanceView,firstReadyEndpointForRoute,firstStructuredFastpayResult,invalidateProposerRouteCache,isFastpayBroadcastMethod,isFastpayCertificateApplyMethod,isFinalityMethod,isSequencedAccountMethod,normalizeFastpayBroadcastRequest,normalizeWalletSubscriptionParams,preferredSequencedReadEndpoint,primeNextProposerRouteCache,primeNextProposerRouteCacheFromResponse,proposerEndpointForHeight,readFleetRpcMajority,readGroupKey,recoverFinalityAcrossViews,rememberFinalizedReadEndpoint,requestWithProxyReadiness,resolveRpcTarget,responseEnvelope,rpcTcpRequest,rpcTcpRequestLine,rpcTcpRequestOneShotLine,sendWalletNotification,shouldUseFirstReadySequencedRead,sleep,startCachedSelectionReadinessProbe,startFastpayCertificateRecovery,startFastpayRouteWarmup,startWalletSubscription,stopWalletSubscription,upstreamEndpointKey,upstreamRpcConnection,upstreamRpcConnections,waitForCachedSelectionReady,waitForFastpayConvergedGroup,walletSnapshotDigest };
}

module.exports = { create };
