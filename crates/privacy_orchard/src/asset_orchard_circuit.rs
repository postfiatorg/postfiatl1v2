use ff::{Field, PrimeField};
use group::prime::PrimeCurveAffine;
use halo2_gadgets::{
    ecc::{CircuitVersion, FixedPoint, NonIdentityPoint, ScalarFixed, ScalarVar},
    poseidon::{PaddedWord, Pow5Chip, Pow5Config, Sponge},
    sinsemilla::{
        merkle::MERKLE_CRH_PERSONALIZATION,
        merkle::{
            chip::{MerkleChip, MerkleConfig},
            MerklePath,
        },
        CommitDomains, HashDomains,
    },
    utilities::lookup_range_check::{LookupRangeCheck, PallasLookupRangeCheckConfig},
};
use halo2_poseidon::{Absorbing, Domain, P128Pow5T3, Spec};
#[cfg(test)]
use halo2_proofs::plonk::keygen_vk;
use halo2_proofs::{
    circuit::{AssignedCell, Layouter, SimpleFloorPlanner, Value},
    plonk::{
        create_proof, keygen_pk, keygen_vk_from_pinned_assembly, keygen_vk_pinned_assembly,
        verify_proof, Advice, Circuit, Column, ConstraintSystem, Error, Expression, Fixed,
        Instance, Selector, SingleVerifier, VerifyingKeyPinnedAssembly,
        VerifyingKeyPinnedAssemblyLimits,
    },
    poly::commitment::Params,
    poly::Rotation,
    transcript::{Blake2bRead, Blake2bWrite, Challenge255},
};
use incrementalmerkletree::{Hashable, Level};
use orchard::{
    note::ExtractedNoteCommitment,
    primitives::redpallas::{SigningKey, SpendAuth, VerificationKey},
    tree::MerkleHashOrchard,
};
use pasta_curves::{
    arithmetic::{Coordinates, CurveAffine, CurveExt},
    group::{Curve, GroupEncoding},
    pallas, vesta,
};
use postfiat_crypto_provider::{bytes_to_hex, hex_to_bytes};
use rand::rngs::OsRng;
use rand::{CryptoRng, RngCore};
use sha3::{Digest, Sha3_384};
#[cfg(feature = "asset-orchard-vk-dev-env")]
use std::env;
use std::{
    collections::BTreeMap,
    fs,
    io::Cursor,
    path::{Path, PathBuf},
    sync::OnceLock,
};

use crate::timing::{
    asset_orchard_timing_elapsed_ms, record_asset_orchard_private_egress_action_build_timing,
    record_asset_orchard_private_egress_proof_verify_timing,
    record_asset_orchard_private_egress_vk_build_timing,
    record_asset_orchard_private_egress_vk_cached_timing,
    record_asset_orchard_swap_vk_build_timing, record_asset_orchard_swap_vk_cached_timing,
    AssetOrchardPrivateEgressActionBuildTimingReport,
    AssetOrchardPrivateEgressProofVerifyTimingReport, AssetOrchardPrivateEgressVkBuildTimingReport,
    AssetOrchardPrivateEgressVkCachedTimingReport, AssetOrchardSwapVkBuildTimingReport,
    AssetOrchardSwapVkCachedTimingReport,
};

use crate::asset_orchard::{
    asset_derive_nullifier, asset_derive_nullifier_poseidon_inputs, asset_note_message_segments,
    asset_orchard_private_egress_exit_binding_hash, asset_orchard_swap_accounting_records,
    asset_output_rho, asset_output_rho_poseidon_inputs, build_asset_orchard_wallet_note_with_rho,
    encrypted_output_hash, h_action_poseidon_inputs_from_public_instance,
    private_egress_h_action_poseidon_inputs_from_public_instance, random_pallas_scalar_nonzero,
    swap_binding_hash, AssetNoteMessageSource, AssetNoteOpening, AssetOrchardActionPublicFields,
    AssetOrchardBoundedBytes, AssetOrchardError, AssetOrchardFieldElement, AssetOrchardPoint,
    AssetOrchardPrivateEgressAction, AssetOrchardPrivateEgressExitBindingPreimage,
    AssetOrchardPrivateEgressPublicFields, AssetOrchardProofBytes, AssetOrchardSpendAuthSignature,
    AssetOrchardSwapAction, AssetOrchardSwapBindingHash, AssetOrchardWalletNote, AssetTag,
    RandomizedVerificationKeyFields, ASSET_ORCHARD_ACTION_SCHEMA_V1,
    ASSET_ORCHARD_ACTION_VERSION_V1, ASSET_ORCHARD_CIRCUIT_ID_V1,
    ASSET_ORCHARD_H_ACTION_POSEIDON_INPUT_COUNT, ASSET_ORCHARD_LEG_COUNT,
    ASSET_ORCHARD_NOTE_MESSAGE_PIECE_BITS, ASSET_ORCHARD_NOTE_MESSAGE_PIECE_COUNT,
    ASSET_ORCHARD_POOL_ID_V1, ASSET_ORCHARD_POSEIDON_RATE, ASSET_ORCHARD_POSEIDON_WIDTH,
    ASSET_ORCHARD_PRIVATE_EGRESS_ACTION_SCHEMA_V1, ASSET_ORCHARD_PRIVATE_EGRESS_CIRCUIT_ID_V1,
    ASSET_ORCHARD_PRIVATE_EGRESS_H_ACTION_POSEIDON_INPUT_COUNT,
    ASSET_ORCHARD_PRIVATE_EGRESS_PUBLIC_INSTANCE_LEN, ASSET_ORCHARD_PROOF_SYSTEM_ID_V1,
    ASSET_ORCHARD_PUBLIC_INSTANCE_LEN,
};
use crate::asset_orchard_sinsemilla::asset_spend_auth_g;
use crate::asset_orchard_sinsemilla::{
    synthesize_asset_note_commitment_from_assigned_subpieces,
    synthesize_sinsemilla_commitment_from_assigned_subpieces, AssetOrchardAssignedSubpiece,
    AssetOrchardCommitDomain, AssetOrchardEccChip, AssetOrchardEccConfig, AssetOrchardFixedBases,
    AssetOrchardFullScalarBase, AssetOrchardMessagePieceConstraintConfig,
    AssetOrchardSinsemillaChip, AssetOrchardSinsemillaConfig,
};

pub const ASSET_ORCHARD_CONSERVATION_CORE_K: u32 = 12;
pub const ASSET_ORCHARD_SWAP_V1_K: u32 = 15;
pub const ASSET_ORCHARD_PRIVATE_EGRESS_V1_K: u32 = ASSET_ORCHARD_SWAP_V1_K;
// Active NAV policy uses NAV_USD_E8_UNIT=1e8. Bounding both rounding
// witnesses to 32 bits covers that denominator while avoiding two generic
// 64-bit decomposition gadgets in the release circuit.
const ASSET_ORCHARD_PRICING_ROUNDING_BITS: usize = 32;
pub const ASSET_ORCHARD_SWAP_V1_PUBLIC_INSTANCE_LAYOUT_HASH: &str =
    "4a97e6254fe6ce1416723ebc0908f6a2a617a8d223f902905a91d7f006a8d1e8cee4cb2ccd81decd29c85b4a2c0f7ed1";
pub const ASSET_ORCHARD_SWAP_V1_PARAMS_HASH: &str =
    "9be0057af858459fe2b4545dec144e83f4951be0bef2bc90e30e5f26e75f88ba69f1be10ac376a6af5ce973c6b7ad0d8";
pub const ASSET_ORCHARD_SWAP_V1_VK_HASH: &str =
    "640e83ff0edc500eae1499eebf8c658097088de0b0e44ed6dd10906f158bbf55b781772a43fff839095ed7431823b828";
pub const ASSET_ORCHARD_SWAP_V1_RUNTIME_PINNED_VK_FINGERPRINT: &str =
    "1b38a9d9906cbfce9addf9a500a1b4bec720a33118507946f427a628772fac48f1786bffa390a5254db215fadf7f3460";
pub const ASSET_ORCHARD_SWAP_V3_REPLAY_VK_HASH: &str =
    "f8d58f14e008bef29905530f11ebdee800dacc44f19d1a52de902be643f0fbaa13074c91c12799a2fb91fa6b30fef06c";
pub const ASSET_ORCHARD_SWAP_V3_REPLAY_RUNTIME_PINNED_VK_FINGERPRINT: &str =
    "0cd57d13af7cd85965c4be283b40243d0fb582b7d5c99d77237b7f72dd18529879eb776b4ac0cd04bbe2198393c4eb97";
pub const ASSET_ORCHARD_SWAP_V1_POSEIDON_PARAMETER_HASH: &str =
    "7249e21c01fa7cd5020c40cd2aacf08b3e22990aae202a1cf37ce6fc73ae448536a77c6f668fa23749981a69fd6fcdf3";
pub const ASSET_ORCHARD_SWAP_V1_NOTE_MESSAGE_LAYOUT_HASH: &str =
    "e3d9b8681cce4331e82ffa689805bf097f575bf11b7582e87a8ed3cba98d55686bf26a7c573f9b5919c95d8e998e923c";
pub const ASSET_ORCHARD_SWAP_V1_MERKLE_PARAMETER_HASH: &str =
    "9dbd4596db4256918bfc85b017f0a3b9e70c881827ca9de5c238b6abe3f532fb6becb94006568aff3fd9b78c40a789e6";
pub const ASSET_ORCHARD_SWAP_V1_VK_ATTESTATION: &str = concat!(
    "postfiat.asset_orchard.swap_vk_attestation.v1\n",
    "halo2_proofs=0.3.2\n",
    "curve=vesta\n",
    "proof_system=halo2-ipa\n",
    "circuit_id=asset_orchard.swap.pricing_bound.v4\n",
    "k=15\n",
    "public_instance_len=28\n",
    "public_instance_layout_hash=4a97e6254fe6ce1416723ebc0908f6a2a617a8d223f902905a91d7f006a8d1e8cee4cb2ccd81decd29c85b4a2c0f7ed1\n",
    "params_hash=9be0057af858459fe2b4545dec144e83f4951be0bef2bc90e30e5f26e75f88ba69f1be10ac376a6af5ce973c6b7ad0d8\n",
    "poseidon_parameter_hash=7249e21c01fa7cd5020c40cd2aacf08b3e22990aae202a1cf37ce6fc73ae448536a77c6f668fa23749981a69fd6fcdf3\n",
    "note_message_layout_hash=e3d9b8681cce4331e82ffa689805bf097f575bf11b7582e87a8ed3cba98d55686bf26a7c573f9b5919c95d8e998e923c\n",
    "merkle_tree_depth=32\n",
    "merkle_parameter_hash=9dbd4596db4256918bfc85b017f0a3b9e70c881827ca9de5c238b6abe3f532fb6becb94006568aff3fd9b78c40a789e6\n",
    "runtime_pinned_vk_fingerprint_domain=sha3_384(asset_orchard_swap_vk || len_le || halo2_pinned_debug)\n",
    "runtime_pinned_vk_fingerprint=1b38a9d9906cbfce9addf9a500a1b4bec720a33118507946f427a628772fac48f1786bffa390a5254db215fadf7f3460\n",
);
pub const ASSET_ORCHARD_SWAP_V3_REPLAY_VK_ATTESTATION: &str = concat!(
    "postfiat.asset_orchard.swap_vk_attestation.v1\n",
    "halo2_proofs=0.3.2\n",
    "curve=vesta\n",
    "proof_system=halo2-ipa\n",
    "circuit_id=asset_orchard.swap.pricing_bound.v3\n",
    "k=15\n",
    "public_instance_len=28\n",
    "public_instance_layout_hash=4a97e6254fe6ce1416723ebc0908f6a2a617a8d223f902905a91d7f006a8d1e8cee4cb2ccd81decd29c85b4a2c0f7ed1\n",
    "params_hash=9be0057af858459fe2b4545dec144e83f4951be0bef2bc90e30e5f26e75f88ba69f1be10ac376a6af5ce973c6b7ad0d8\n",
    "poseidon_parameter_hash=7249e21c01fa7cd5020c40cd2aacf08b3e22990aae202a1cf37ce6fc73ae448536a77c6f668fa23749981a69fd6fcdf3\n",
    "note_message_layout_hash=e3d9b8681cce4331e82ffa689805bf097f575bf11b7582e87a8ed3cba98d55686bf26a7c573f9b5919c95d8e998e923c\n",
    "merkle_tree_depth=32\n",
    "merkle_parameter_hash=9dbd4596db4256918bfc85b017f0a3b9e70c881827ca9de5c238b6abe3f532fb6becb94006568aff3fd9b78c40a789e6\n",
    "runtime_pinned_vk_fingerprint_domain=sha3_384(asset_orchard_swap_vk || len_le || halo2_pinned_debug)\n",
    "runtime_pinned_vk_fingerprint=0cd57d13af7cd85965c4be283b40243d0fb582b7d5c99d77237b7f72dd18529879eb776b4ac0cd04bbe2198393c4eb97\n",
);
const ASSET_ORCHARD_SWAP_VK_ARTIFACT_SCHEMA_V1: &str =
    "postfiat.asset_orchard.swap_vk_pinned_assembly.v1";
#[cfg(feature = "asset-orchard-vk-dev-env")]
const ASSET_ORCHARD_SWAP_VK_ARTIFACT_LOAD_ENV: &str = "POSTFIAT_ASSET_ORCHARD_SWAP_VK_ARTIFACT";
#[cfg(feature = "asset-orchard-vk-dev-env")]
const ASSET_ORCHARD_SWAP_VK_ARTIFACT_WRITE_ENV: &str =
    "POSTFIAT_ASSET_ORCHARD_SWAP_VK_WRITE_ARTIFACT";
#[cfg(feature = "asset-orchard-vk-dev-env")]
const ASSET_ORCHARD_SWAP_VK_REBUILD_ENV: &str = "POSTFIAT_ASSET_ORCHARD_SWAP_VK_REBUILD";
const ASSET_ORCHARD_SWAP_VK_ARTIFACT_MAX_BYTES: u64 = 64 * 1024 * 1024;
const ASSET_ORCHARD_SWAP_VK_EMBEDDED_ARTIFACT: &[u8] =
    include_bytes!("../artifacts/asset_orchard_swap_vk_pinned_assembly.v1.bin");
const ASSET_ORCHARD_SWAP_V3_REPLAY_VK_EMBEDDED_ARTIFACT: &[u8] = include_bytes!(
    "../artifacts/replay/asset_orchard_swap_vk_pinned_assembly.custom_poseidon_v3.pre_3218ec53.bin"
);
const ASSET_ORCHARD_K15_PARAMS_ARTIFACT_BYTES: usize = 2_097_220;
const ASSET_ORCHARD_K15_PARAMS_ARTIFACT_HASH: &str =
    "e77674f1a7de07fd6a896350e1af30b2c7b77a1398891959d33e7605078ad239b6757a937035eea9e2400f04ac9a438e";
const ASSET_ORCHARD_K15_PARAMS_EMBEDDED_ARTIFACT: &[u8] =
    include_bytes!("../artifacts/asset_orchard_k15_params.v1.bin");
pub const ASSET_ORCHARD_PRIVATE_EGRESS_V1_PUBLIC_INSTANCE_LAYOUT_HASH: &str =
    "21e4ba88556d23e3d1c53d3f309ee90bc6321a9f6f8e7b1662083e2d712c9d576020248dd1727b2ca154e8203d09dc44";
pub const ASSET_ORCHARD_PRIVATE_EGRESS_V1_PARAMS_HASH: &str =
    "bcd57a07fc6729861fa7524a16825722d7c96e1703990f673d95e7c28c77db2da7844a4a9f981dd54b4499edddd3d555";
pub const ASSET_ORCHARD_PRIVATE_EGRESS_V1_VK_HASH: &str =
    "a8e020b877a45f9691a266990e3466b52b1518518d3a5264f9725be201b9884db80d4c9cb457c29020e65cfefa85d6be";
pub const ASSET_ORCHARD_PRIVATE_EGRESS_V1_RUNTIME_PINNED_VK_FINGERPRINT: &str =
    "d29cab6577eb9d968ec67d9134844c605f88ddd0241ce1147fc364e882f259a35318b869a95927be3fdb1e2e521ed4cc";
pub const ASSET_ORCHARD_PRIVATE_EGRESS_V1_REPLAY_VK_HASH: &str =
    "3e850f6d1d0b2df310fd5b48c1917bbce7d0397fdd0bb6eaff75e02556edf382145df6c8c3ca5d6612ec5d8e94eb8520";
pub const ASSET_ORCHARD_PRIVATE_EGRESS_V1_REPLAY_RUNTIME_PINNED_VK_FINGERPRINT: &str =
    "a5118831487dc46577a66806ce11d3a10b977ce0fd4d12552d9728b6b64e63283266b5fe6adc2d5d75ab3d83e3a38114";
pub const ASSET_ORCHARD_PRIVATE_EGRESS_V1_POSEIDON_PARAMETER_HASH: &str =
    ASSET_ORCHARD_SWAP_V1_POSEIDON_PARAMETER_HASH;
pub const ASSET_ORCHARD_PRIVATE_EGRESS_V1_NOTE_MESSAGE_LAYOUT_HASH: &str =
    ASSET_ORCHARD_SWAP_V1_NOTE_MESSAGE_LAYOUT_HASH;
pub const ASSET_ORCHARD_PRIVATE_EGRESS_V1_MERKLE_PARAMETER_HASH: &str =
    ASSET_ORCHARD_SWAP_V1_MERKLE_PARAMETER_HASH;
pub const ASSET_ORCHARD_PRIVATE_EGRESS_V1_VK_ATTESTATION: &str = concat!(
    "postfiat.asset_orchard.private_egress_vk_attestation.v1\n",
    "halo2_proofs=0.3.2\n",
    "curve=vesta\n",
    "proof_system=halo2-ipa\n",
    "circuit_id=asset_orchard.private_egress.v2\n",
    "k=15\n",
    "public_instance_len=13\n",
    "public_instance_layout_hash=21e4ba88556d23e3d1c53d3f309ee90bc6321a9f6f8e7b1662083e2d712c9d576020248dd1727b2ca154e8203d09dc44\n",
    "params_hash=bcd57a07fc6729861fa7524a16825722d7c96e1703990f673d95e7c28c77db2da7844a4a9f981dd54b4499edddd3d555\n",
    "poseidon_parameter_hash=7249e21c01fa7cd5020c40cd2aacf08b3e22990aae202a1cf37ce6fc73ae448536a77c6f668fa23749981a69fd6fcdf3\n",
    "note_message_layout_hash=e3d9b8681cce4331e82ffa689805bf097f575bf11b7582e87a8ed3cba98d55686bf26a7c573f9b5919c95d8e998e923c\n",
    "merkle_tree_depth=32\n",
    "merkle_parameter_hash=9dbd4596db4256918bfc85b017f0a3b9e70c881827ca9de5c238b6abe3f532fb6becb94006568aff3fd9b78c40a789e6\n",
    "runtime_pinned_vk_fingerprint_domain=sha3_384(asset_orchard_private_egress_vk || len_le || halo2_pinned_debug)\n",
    "runtime_pinned_vk_fingerprint=d29cab6577eb9d968ec67d9134844c605f88ddd0241ce1147fc364e882f259a35318b869a95927be3fdb1e2e521ed4cc\n",
);
pub const ASSET_ORCHARD_PRIVATE_EGRESS_V1_REPLAY_VK_ATTESTATION: &str = concat!(
    "postfiat.asset_orchard.private_egress_vk_attestation.v1\n",
    "halo2_proofs=0.3.2\n",
    "curve=vesta\n",
    "proof_system=halo2-ipa\n",
    "circuit_id=asset_orchard.private_egress.v1\n",
    "k=15\n",
    "public_instance_len=13\n",
    "public_instance_layout_hash=21e4ba88556d23e3d1c53d3f309ee90bc6321a9f6f8e7b1662083e2d712c9d576020248dd1727b2ca154e8203d09dc44\n",
    "params_hash=bcd57a07fc6729861fa7524a16825722d7c96e1703990f673d95e7c28c77db2da7844a4a9f981dd54b4499edddd3d555\n",
    "poseidon_parameter_hash=7249e21c01fa7cd5020c40cd2aacf08b3e22990aae202a1cf37ce6fc73ae448536a77c6f668fa23749981a69fd6fcdf3\n",
    "note_message_layout_hash=e3d9b8681cce4331e82ffa689805bf097f575bf11b7582e87a8ed3cba98d55686bf26a7c573f9b5919c95d8e998e923c\n",
    "merkle_tree_depth=32\n",
    "merkle_parameter_hash=9dbd4596db4256918bfc85b017f0a3b9e70c881827ca9de5c238b6abe3f532fb6becb94006568aff3fd9b78c40a789e6\n",
    "runtime_pinned_vk_fingerprint_domain=sha3_384(asset_orchard_private_egress_vk || len_le || halo2_pinned_debug)\n",
    "runtime_pinned_vk_fingerprint=a5118831487dc46577a66806ce11d3a10b977ce0fd4d12552d9728b6b64e63283266b5fe6adc2d5d75ab3d83e3a38114\n",
);
const ASSET_ORCHARD_PRIVATE_EGRESS_VK_ARTIFACT_SCHEMA_V1: &str =
    "postfiat.asset_orchard.private_egress_vk_pinned_assembly.v1";
#[cfg(feature = "asset-orchard-vk-dev-env")]
const ASSET_ORCHARD_PRIVATE_EGRESS_VK_ARTIFACT_LOAD_ENV: &str =
    "POSTFIAT_ASSET_ORCHARD_PRIVATE_EGRESS_VK_ARTIFACT";
#[cfg(feature = "asset-orchard-vk-dev-env")]
const ASSET_ORCHARD_PRIVATE_EGRESS_VK_ARTIFACT_WRITE_ENV: &str =
    "POSTFIAT_ASSET_ORCHARD_PRIVATE_EGRESS_VK_WRITE_ARTIFACT";
#[cfg(feature = "asset-orchard-vk-dev-env")]
const ASSET_ORCHARD_PRIVATE_EGRESS_VK_REBUILD_ENV: &str =
    "POSTFIAT_ASSET_ORCHARD_PRIVATE_EGRESS_VK_REBUILD";
const ASSET_ORCHARD_PRIVATE_EGRESS_VK_ARTIFACT_MAX_BYTES: u64 = 64 * 1024 * 1024;
const ASSET_ORCHARD_PRIVATE_EGRESS_VK_EMBEDDED_ARTIFACT: &[u8] =
    include_bytes!("../artifacts/asset_orchard_private_egress_vk_pinned_assembly.v1.bin");
const ASSET_ORCHARD_PRIVATE_EGRESS_V1_REPLAY_VK_EMBEDDED_ARTIFACT: &[u8] = include_bytes!(
    "../artifacts/replay/asset_orchard_private_egress_vk_pinned_assembly.custom_poseidon_v1.pre_3218ec53.bin"
);
pub const ASSET_ORCHARD_MERKLE_DEPTH: usize = orchard::NOTE_COMMITMENT_TREE_DEPTH;
const ASSET_ORCHARD_ASSET_TAG_BITS: usize = 128;
const ASSET_ORCHARD_VALUE_BITS: usize = 64;

type AssetOrchardMerkleConfig = MerkleConfig<
    AssetOrchardMerkleHashDomain,
    AssetOrchardMerkleCommitDomain,
    AssetOrchardFixedBases,
>;
type AssetOrchardMerkleChip = MerkleChip<
    AssetOrchardMerkleHashDomain,
    AssetOrchardMerkleCommitDomain,
    AssetOrchardFixedBases,
>;
type AssetOrchardMerklePath = MerklePath<
    pallas::Affine,
    AssetOrchardMerkleChip,
    { ASSET_ORCHARD_MERKLE_DEPTH },
    { ::sinsemilla::K },
    { ::sinsemilla::C },
    2,
>;

#[derive(Debug, Clone, Eq, PartialEq)]
struct AssetOrchardMerkleHashDomain;

#[derive(Debug, Clone, Eq, PartialEq)]
struct AssetOrchardMerkleCommitDomain;

static ASSET_ORCHARD_MERKLE_Q: OnceLock<pallas::Affine> = OnceLock::new();
static ASSET_ORCHARD_K15_PARAMS: OnceLock<Result<Params<vesta::Affine>, AssetOrchardError>> =
    OnceLock::new();
static ASSET_ORCHARD_SWAP_PROVING_KEY: OnceLock<
    Result<AssetOrchardSwapProvingKey, AssetOrchardError>,
> = OnceLock::new();
static ASSET_ORCHARD_SWAP_VERIFYING_KEY: OnceLock<
    Result<AssetOrchardSwapVerifyingKey, AssetOrchardError>,
> = OnceLock::new();
static ASSET_ORCHARD_SWAP_V3_REPLAY_VERIFYING_KEY: OnceLock<
    Result<AssetOrchardSwapVerifyingKey, AssetOrchardError>,
> = OnceLock::new();
static ASSET_ORCHARD_PRIVATE_EGRESS_PROVING_KEY: OnceLock<
    Result<AssetOrchardPrivateEgressProvingKey, AssetOrchardError>,
> = OnceLock::new();
static ASSET_ORCHARD_PRIVATE_EGRESS_VERIFYING_KEY: OnceLock<
    Result<AssetOrchardPrivateEgressVerifyingKey, AssetOrchardError>,
> = OnceLock::new();
static ASSET_ORCHARD_PRIVATE_EGRESS_V1_REPLAY_VERIFYING_KEY: OnceLock<
    Result<AssetOrchardPrivateEgressVerifyingKey, AssetOrchardError>,
> = OnceLock::new();

fn decode_asset_orchard_k15_params(
    bytes: &[u8],
) -> Result<Params<vesta::Affine>, AssetOrchardError> {
    if bytes.len() != ASSET_ORCHARD_K15_PARAMS_ARTIFACT_BYTES {
        return Err(AssetOrchardError::new(
            "asset_orchard_k15_params_artifact_length_mismatch",
            format!(
                "parameter artifact is {} bytes; expected {}",
                bytes.len(),
                ASSET_ORCHARD_K15_PARAMS_ARTIFACT_BYTES
            ),
        ));
    }
    let encoded_k = u32::from_le_bytes(bytes[..4].try_into().map_err(|_| {
        AssetOrchardError::new(
            "asset_orchard_k15_params_artifact_malformed",
            "parameter artifact lacks k header",
        )
    })?);
    if encoded_k != ASSET_ORCHARD_SWAP_V1_K {
        return Err(AssetOrchardError::new(
            "asset_orchard_k15_params_artifact_k_mismatch",
            format!(
                "parameter artifact k is {encoded_k}; expected {}",
                ASSET_ORCHARD_SWAP_V1_K
            ),
        ));
    }
    let actual_hash = hash_bytes("asset_orchard_k15_params_artifact", bytes);
    if actual_hash != ASSET_ORCHARD_K15_PARAMS_ARTIFACT_HASH {
        return Err(AssetOrchardError::new(
            "asset_orchard_k15_params_artifact_hash_mismatch",
            format!(
                "parameter artifact hash {actual_hash} does not match pinned {}",
                ASSET_ORCHARD_K15_PARAMS_ARTIFACT_HASH
            ),
        ));
    }
    let mut cursor = Cursor::new(bytes);
    let params = Params::<vesta::Affine>::read(&mut cursor).map_err(|error| {
        AssetOrchardError::new(
            "asset_orchard_k15_params_artifact_malformed",
            error.to_string(),
        )
    })?;
    if cursor.position() != bytes.len() as u64 || params.k() != ASSET_ORCHARD_SWAP_V1_K {
        return Err(AssetOrchardError::new(
            "asset_orchard_k15_params_artifact_malformed",
            "parameter artifact did not decode exactly",
        ));
    }
    Ok(params)
}

fn asset_orchard_k15_params() -> Result<&'static Params<vesta::Affine>, AssetOrchardError> {
    match ASSET_ORCHARD_K15_PARAMS
        .get_or_init(|| decode_asset_orchard_k15_params(ASSET_ORCHARD_K15_PARAMS_EMBEDDED_ARTIFACT))
    {
        Ok(params) => Ok(params),
        Err(error) => Err(error.clone()),
    }
}

fn asset_orchard_merkle_q() -> pallas::Affine {
    *ASSET_ORCHARD_MERKLE_Q.get_or_init(|| {
        pallas::Point::hash_to_curve(::sinsemilla::Q_PERSONALIZATION)(
            MERKLE_CRH_PERSONALIZATION.as_bytes(),
        )
        .to_affine()
    })
}

impl HashDomains<pallas::Affine> for AssetOrchardMerkleHashDomain {
    fn Q(&self) -> pallas::Affine {
        asset_orchard_merkle_q()
    }
}

impl CommitDomains<pallas::Affine, AssetOrchardFixedBases, AssetOrchardMerkleHashDomain>
    for AssetOrchardMerkleCommitDomain
{
    fn r(&self) -> AssetOrchardFullScalarBase {
        AssetOrchardFullScalarBase::NoteCommitR
    }

    fn hash_domain(&self) -> AssetOrchardMerkleHashDomain {
        AssetOrchardMerkleHashDomain
    }
}

#[derive(Debug, Copy, Clone)]
pub struct AssetOrchardSwapPrivateLeg {
    pub asset_tag: AssetTag,
    pub value: u64,
}

impl AssetOrchardSwapPrivateLeg {
    pub fn validate(&self) -> Result<(), AssetOrchardError> {
        self.asset_tag.validate()?;
        if self.value == 0 {
            return Err(AssetOrchardError::new(
                "zero_swap_value",
                "asset-orchard swap leg value must be nonzero",
            ));
        }
        Ok(())
    }

    fn fields(&self) -> [pallas::Base; 3] {
        [
            pallas::Base::from_u128(self.asset_tag.lo),
            pallas::Base::from_u128(self.asset_tag.hi),
            pallas::Base::from(self.value),
        ]
    }
}

#[derive(Clone, Debug)]
pub struct AssetOrchardSwapNoteWitness {
    pub note: AssetNoteOpening,
    pub nk: pallas::Base,
    pub cmx: pallas::Base,
    pub merkle_witness: Option<AssetOrchardMerkleWitness>,
    pub spend_authority: Option<AssetOrchardSpendAuthorityWitness>,
}

#[derive(Clone, Debug)]
pub struct AssetOrchardMerkleWitness {
    pub position: u32,
    pub auth_path: [pallas::Base; ASSET_ORCHARD_MERKLE_DEPTH],
}

#[derive(Clone, Debug)]
pub struct AssetOrchardSpendAuthorityWitness {
    pub ak: pallas::Affine,
    pub alpha: pallas::Scalar,
    pub rivk: pallas::Scalar,
}

impl AssetOrchardSwapNoteWitness {
    pub fn from_note(
        pool_domain: pallas::Base,
        note: AssetNoteOpening,
    ) -> Result<Self, AssetOrchardError> {
        Self::from_note_with_nk(pool_domain, note, pallas::Base::ONE)
    }

    pub fn from_note_with_nk(
        pool_domain: pallas::Base,
        note: AssetNoteOpening,
        nk: pallas::Base,
    ) -> Result<Self, AssetOrchardError> {
        let cmx = note.cmx(pool_domain)?;
        Ok(Self {
            note,
            nk,
            cmx,
            merkle_witness: None,
            spend_authority: None,
        })
    }

    pub fn with_merkle_witness(mut self, merkle_witness: AssetOrchardMerkleWitness) -> Self {
        self.merkle_witness = Some(merkle_witness);
        self
    }

    pub fn with_spend_authority(
        mut self,
        spend_authority: AssetOrchardSpendAuthorityWitness,
    ) -> Self {
        self.spend_authority = Some(spend_authority);
        self
    }

    pub fn leg(&self) -> AssetOrchardSwapPrivateLeg {
        AssetOrchardSwapPrivateLeg {
            asset_tag: self.note.asset_tag,
            value: self.note.value,
        }
    }

    fn validate_for_pool(&self, pool_domain: pallas::Base) -> Result<(), AssetOrchardError> {
        self.note.validate()?;
        if self.note.cmx(pool_domain)? != self.cmx {
            return Err(AssetOrchardError::new(
                "asset_orchard_note_cmx_mismatch",
                "asset-orchard note witness cmx does not match note opening",
            ));
        }
        Ok(())
    }

    fn dummy_for_shape() -> Self {
        let g_d = pallas::Point::hash_to_curve("postfiat.asset_orchard.swap_shape.gd")(b"dummy-gd")
            .to_affine();
        let pk_d =
            pallas::Point::hash_to_curve("postfiat.asset_orchard.swap_shape.pkd")(b"dummy-pkd")
                .to_affine();
        Self {
            note: AssetNoteOpening {
                diversifier: [0u8; crate::asset_orchard::ASSET_ORCHARD_DIVERSIFIER_BYTES],
                g_d,
                pk_d,
                asset_tag: AssetTag { lo: 1, hi: 0 },
                value: 1,
                rho: pallas::Base::ONE,
                psi: pallas::Base::ONE,
                rcm: pallas::Scalar::ONE,
            },
            nk: pallas::Base::ONE,
            cmx: pallas::Base::ONE,
            merkle_witness: Some(AssetOrchardMerkleWitness {
                position: 0,
                auth_path: [pallas::Base::ZERO; ASSET_ORCHARD_MERKLE_DEPTH],
            }),
            spend_authority: Some(AssetOrchardSpendAuthorityWitness {
                ak: g_d,
                alpha: pallas::Scalar::ONE,
                rivk: pallas::Scalar::ONE,
            }),
        }
    }
}

/// Swap conservation circuit wrapper.
///
/// The zero-note-witness constructor is test-only. Public callers must use
/// `new_with_note_witnesses`, which binds note commitments, Merkle witnesses,
/// and spend authority before a real proof can be created.
///
/// ```compile_fail
/// use postfiat_privacy_orchard::{
///     AssetOrchardActionPublicFields, AssetOrchardSwapConservationCircuit,
///     AssetOrchardSwapPrivateLeg, ASSET_ORCHARD_LEG_COUNT,
/// };
///
/// fn zero_witness_constructor_is_not_public(
///     inputs: [AssetOrchardSwapPrivateLeg; ASSET_ORCHARD_LEG_COUNT],
///     outputs: [AssetOrchardSwapPrivateLeg; ASSET_ORCHARD_LEG_COUNT],
///     public_fields: &AssetOrchardActionPublicFields,
/// ) {
///     let _ = AssetOrchardSwapConservationCircuit::new(
///         inputs,
///         outputs,
///         true,
///         public_fields,
///     );
/// }
/// ```
#[derive(Clone, Debug)]
pub struct AssetOrchardSwapConservationCircuit {
    inputs: [Option<AssetOrchardSwapPrivateLeg>; ASSET_ORCHARD_LEG_COUNT],
    outputs: [Option<AssetOrchardSwapPrivateLeg>; ASSET_ORCHARD_LEG_COUNT],
    input_notes: [Option<AssetOrchardSwapNoteWitness>; ASSET_ORCHARD_LEG_COUNT],
    output_notes: [Option<AssetOrchardSwapNoteWitness>; ASSET_ORCHARD_LEG_COUNT],
    permutation_swap: Option<bool>,
    #[cfg(test)]
    permutation_swap_rows: Option<[bool; 3]>,
    public_instance: Option<[pallas::Base; ASSET_ORCHARD_PUBLIC_INSTANCE_LEN]>,
}

impl AssetOrchardSwapConservationCircuit {
    #[cfg(test)]
    pub fn new(
        inputs: [AssetOrchardSwapPrivateLeg; ASSET_ORCHARD_LEG_COUNT],
        outputs: [AssetOrchardSwapPrivateLeg; ASSET_ORCHARD_LEG_COUNT],
        permutation_swap: bool,
        public_fields: &AssetOrchardActionPublicFields,
    ) -> Result<Self, AssetOrchardError> {
        for leg in inputs.iter().chain(outputs.iter()) {
            leg.validate()?;
        }
        Ok(Self {
            inputs: inputs.map(Some),
            outputs: outputs.map(Some),
            input_notes: [None, None],
            output_notes: [None, None],
            permutation_swap: Some(permutation_swap),
            #[cfg(test)]
            permutation_swap_rows: None,
            public_instance: Some(public_fields.public_instance()?),
        })
    }

    pub fn new_with_note_witnesses(
        inputs: [AssetOrchardSwapNoteWitness; ASSET_ORCHARD_LEG_COUNT],
        outputs: [AssetOrchardSwapNoteWitness; ASSET_ORCHARD_LEG_COUNT],
        permutation_swap: bool,
        public_fields: &AssetOrchardActionPublicFields,
    ) -> Result<Self, AssetOrchardError> {
        let public_instance = public_fields.public_instance()?;
        for note in inputs.iter().chain(outputs.iter()) {
            note.validate_for_pool(public_fields.pool_domain)?;
        }
        for (index, output) in outputs.iter().enumerate() {
            if output.cmx != public_fields.output_commitments[index] {
                return Err(AssetOrchardError::new(
                    "asset_orchard_output_cmx_mismatch",
                    "asset-orchard output note witness cmx does not match public output commitment",
                ));
            }
        }
        Ok(Self {
            inputs: inputs
                .each_ref()
                .map(AssetOrchardSwapNoteWitness::leg)
                .map(Some),
            outputs: outputs
                .each_ref()
                .map(AssetOrchardSwapNoteWitness::leg)
                .map(Some),
            input_notes: inputs.map(Some),
            output_notes: outputs.map(Some),
            permutation_swap: Some(permutation_swap),
            #[cfg(test)]
            permutation_swap_rows: None,
            public_instance: Some(public_instance),
        })
    }

    pub fn public_instance(&self) -> Option<[pallas::Base; ASSET_ORCHARD_PUBLIC_INSTANCE_LEN]> {
        self.public_instance
    }

    pub fn full_shape() -> Self {
        Self {
            inputs: [None, None],
            outputs: [None, None],
            input_notes: [(); ASSET_ORCHARD_LEG_COUNT]
                .map(|_| Some(AssetOrchardSwapNoteWitness::dummy_for_shape())),
            output_notes: [(); ASSET_ORCHARD_LEG_COUNT]
                .map(|_| Some(AssetOrchardSwapNoteWitness::dummy_for_shape())),
            permutation_swap: None,
            #[cfg(test)]
            permutation_swap_rows: None,
            public_instance: None,
        }
    }

    fn leg_values(leg: Option<AssetOrchardSwapPrivateLeg>) -> [Value<pallas::Base>; 3] {
        match leg {
            Some(leg) => leg.fields().map(Value::known),
            None => [Value::unknown(), Value::unknown(), Value::unknown()],
        }
    }

    fn permutation_selector_for_row(&self, _row: usize) -> Value<pallas::Base> {
        #[cfg(test)]
        if let Some(rows) = self.permutation_swap_rows {
            return Value::known(if rows[_row] {
                pallas::Base::ONE
            } else {
                pallas::Base::ZERO
            });
        }

        match self.permutation_swap {
            Some(true) => Value::known(pallas::Base::ONE),
            Some(false) => Value::known(pallas::Base::ZERO),
            None => Value::unknown(),
        }
    }

    fn note_witness_count(&self) -> usize {
        self.input_notes
            .iter()
            .filter(|note| note.is_some())
            .count()
            + self
                .output_notes
                .iter()
                .filter(|note| note.is_some())
                .count()
    }

    fn has_full_note_witnesses(&self) -> bool {
        self.note_witness_count() == ASSET_ORCHARD_LEG_COUNT * 2
    }

    fn require_full_note_witnesses(&self) -> Result<(), AssetOrchardError> {
        if self.has_full_note_witnesses() {
            Ok(())
        } else {
            Err(AssetOrchardError::new(
                "asset_orchard_swap_missing_note_witness",
                "full AssetOrchard swap proofs require all input and output note witnesses",
            ))
        }
    }
}

#[derive(Clone, Debug)]
pub struct AssetOrchardPrivateEgressCircuit {
    pub note: Option<AssetOrchardSwapNoteWitness>,
    pub public_instance: Option<[pallas::Base; ASSET_ORCHARD_PRIVATE_EGRESS_PUBLIC_INSTANCE_LEN]>,
}

impl AssetOrchardPrivateEgressCircuit {
    pub fn new_with_note_witness(
        note: AssetOrchardSwapNoteWitness,
        public_fields: &AssetOrchardPrivateEgressPublicFields,
    ) -> Result<Self, AssetOrchardError> {
        note.validate_for_pool(public_fields.pool_domain)?;
        if note.note.asset_tag != public_fields.asset_tag {
            return Err(AssetOrchardError::new(
                "asset_orchard_private_egress_asset_tag_mismatch",
                "private egress note asset tag does not match public exit asset tag",
            ));
        }
        if note.note.value != public_fields.amount {
            return Err(AssetOrchardError::new(
                "asset_orchard_private_egress_amount_mismatch",
                "private egress note value does not match public exit amount",
            ));
        }
        Ok(Self {
            note: Some(note),
            public_instance: Some(public_fields.public_instance()?),
        })
    }

    pub fn public_instance(
        &self,
    ) -> Option<[pallas::Base; ASSET_ORCHARD_PRIVATE_EGRESS_PUBLIC_INSTANCE_LEN]> {
        self.public_instance
    }

    pub fn full_shape() -> Self {
        Self {
            note: Some(AssetOrchardSwapNoteWitness::dummy_for_shape()),
            public_instance: None,
        }
    }

    fn require_full_note_witness(&self) -> Result<(), AssetOrchardError> {
        let note = self.note.as_ref().ok_or_else(|| {
            AssetOrchardError::new(
                "asset_orchard_private_egress_missing_note_witness",
                "full AssetOrchard private egress proofs require an input note witness",
            )
        })?;
        if note.merkle_witness.is_none() || note.spend_authority.is_none() {
            return Err(AssetOrchardError::new(
                "asset_orchard_private_egress_incomplete_note_witness",
                "private egress note witness must include Merkle and spend-authority witnesses",
            ));
        }
        Ok(())
    }
}

impl Circuit<pallas::Base> for AssetOrchardPrivateEgressCircuit {
    type Config = AssetOrchardSwapConservationConfig;
    type FloorPlanner = SimpleFloorPlanner;

    fn without_witnesses(&self) -> Self {
        Self {
            note: self
                .note
                .as_ref()
                .map(|_| AssetOrchardSwapNoteWitness::dummy_for_shape()),
            public_instance: self.public_instance,
        }
    }

    fn configure(meta: &mut ConstraintSystem<pallas::Base>) -> Self::Config {
        AssetOrchardSwapConservationCircuit::configure(meta)
    }

    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<pallas::Base>,
    ) -> Result<(), Error> {
        AssetOrchardSinsemillaChip::load(config.sinsemilla.clone(), &mut layouter)?;
        let public_values = self
            .public_instance
            .unwrap_or([pallas::Base::ZERO; ASSET_ORCHARD_PRIVATE_EGRESS_PUBLIC_INSTANCE_LEN]);
        let public_cells = layouter.assign_region(
            || "asset-orchard private egress public instance shape",
            |mut region| {
                let mut cells = Vec::with_capacity(public_values.len());
                for (row, value) in public_values.iter().copied().enumerate() {
                    cells.push(region.assign_advice(
                        || "private egress public instance",
                        config.advice[0],
                        row,
                        || Value::known(value),
                    )?);
                }
                Ok(cells)
            },
        )?;
        for (row, cell) in public_cells.iter().enumerate() {
            layouter.constrain_instance(cell.cell(), config.instance, row)?;
        }
        for row in [5usize, 6usize] {
            synthesize_range_check(
                &mut layouter,
                &config,
                public_cells[row].clone(),
                Value::known(public_values[row]),
                ASSET_ORCHARD_ASSET_TAG_BITS,
            )?;
        }
        for row in [7usize, 8usize] {
            synthesize_range_check(
                &mut layouter,
                &config,
                public_cells[row].clone(),
                Value::known(public_values[row]),
                ASSET_ORCHARD_VALUE_BITS,
            )?;
        }
        let h_action_cells =
            synthesize_private_egress_h_action_binding(&mut layouter, &config, public_values)?;
        for (cell, instance_row) in h_action_cells.public_inputs {
            layouter.constrain_instance(cell.cell(), config.instance, instance_row)?;
        }
        layouter.constrain_instance(
            h_action_cells.action_context[0].cell(),
            config.instance,
            11,
        )?;
        layouter.constrain_instance(
            h_action_cells.action_context[1].cell(),
            config.instance,
            12,
        )?;

        if let Some(note) = &self.note {
            let ecc_chip =
                AssetOrchardEccChip::construct(config.ecc.clone(), CircuitVersion::AnchoredBase);
            let sinsemilla_chip = AssetOrchardSinsemillaChip::construct(config.sinsemilla.clone());
            let empty_swap_public_instance =
                [pallas::Base::ZERO; ASSET_ORCHARD_PUBLIC_INSTANCE_LEN];
            let leg_cells = vec![
                public_cells[5].clone(),
                public_cells[6].clone(),
                public_cells[7].clone(),
            ];
            synthesize_note_commitment(
                &mut layouter,
                &config,
                sinsemilla_chip,
                ecc_chip,
                "private egress input note".to_string(),
                note,
                public_cells[0].clone(),
                public_values[0],
                public_cells[1].clone(),
                &[],
                empty_swap_public_instance,
                &leg_cells,
                Some(2),
                None,
                Some((3, 4)),
            )?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct AssetOrchardSwapConservationConfig {
    advice: [Column<Advice>; 9],
    fixed: [Column<Fixed>; 5],
    instance: Column<Instance>,
    ecc: AssetOrchardEccConfig,
    sinsemilla: AssetOrchardSinsemillaConfig,
    message_piece: AssetOrchardMessagePieceConstraintConfig,
    merkle_1: AssetOrchardMerkleConfig,
    merkle_2: AssetOrchardMerkleConfig,
    poseidon: Pow5Config<pallas::Base, ASSET_ORCHARD_POSEIDON_WIDTH, ASSET_ORCHARD_POSEIDON_RATE>,
    q_conservation: Selector,
    q_value_nonzero: Selector,
    q_asset_tag_nonzero: Selector,
    q_range: Selector,
    q_public_distinct: Selector,
    q_pricing_binding: Selector,
}

impl Circuit<pallas::Base> for AssetOrchardSwapConservationCircuit {
    type Config = AssetOrchardSwapConservationConfig;
    type FloorPlanner = SimpleFloorPlanner;

    fn without_witnesses(&self) -> Self {
        Self {
            inputs: [None, None],
            outputs: [None, None],
            input_notes: self.input_notes.each_ref().map(|note| {
                note.as_ref()
                    .map(|_| AssetOrchardSwapNoteWitness::dummy_for_shape())
            }),
            output_notes: self.output_notes.each_ref().map(|note| {
                note.as_ref()
                    .map(|_| AssetOrchardSwapNoteWitness::dummy_for_shape())
            }),
            permutation_swap: None,
            #[cfg(test)]
            permutation_swap_rows: None,
            public_instance: self.public_instance,
        }
    }

    fn configure(meta: &mut ConstraintSystem<pallas::Base>) -> Self::Config {
        let advice = [
            meta.advice_column(),
            meta.advice_column(),
            meta.advice_column(),
            meta.advice_column(),
            meta.advice_column(),
            meta.advice_column(),
            meta.advice_column(),
            meta.advice_column(),
            meta.advice_column(),
        ];
        let fixed = [
            meta.fixed_column(),
            meta.fixed_column(),
            meta.fixed_column(),
            meta.fixed_column(),
            meta.fixed_column(),
        ];
        let instance = meta.instance_column();
        let _constants = meta.fixed_column();
        meta.enable_constant(_constants);
        let sinsemilla_advices: [Column<Advice>; 10] =
            std::array::from_fn(|_| meta.advice_column());
        let table_idx = meta.lookup_table_column();
        let lagrange_coeffs = std::array::from_fn(|_| meta.fixed_column());
        let lookup = (
            table_idx,
            meta.lookup_table_column(),
            meta.lookup_table_column(),
        );
        let message_piece_weight = meta.fixed_column();
        let sinsemilla_range_check =
            PallasLookupRangeCheckConfig::configure(meta, sinsemilla_advices[9], table_idx);
        let ecc = AssetOrchardEccChip::configure(
            meta,
            sinsemilla_advices,
            lagrange_coeffs,
            sinsemilla_range_check,
        );
        let sinsemilla = AssetOrchardSinsemillaChip::configure(
            meta,
            sinsemilla_advices[..5]
                .try_into()
                .expect("five advice columns"),
            sinsemilla_advices[2],
            lagrange_coeffs[0],
            lookup,
            sinsemilla_range_check,
            false,
        );
        let merkle_sinsemilla_1 = halo2_gadgets::sinsemilla::chip::SinsemillaChip::<
            AssetOrchardMerkleHashDomain,
            AssetOrchardMerkleCommitDomain,
            AssetOrchardFixedBases,
        >::configure(
            meta,
            sinsemilla_advices[..5]
                .try_into()
                .expect("five advice columns"),
            sinsemilla_advices[2],
            lagrange_coeffs[1],
            lookup,
            sinsemilla_range_check,
            false,
        );
        let merkle_1 = AssetOrchardMerkleChip::configure(meta, merkle_sinsemilla_1.clone());
        let merkle_sinsemilla_2 = halo2_gadgets::sinsemilla::chip::SinsemillaChip::<
            AssetOrchardMerkleHashDomain,
            AssetOrchardMerkleCommitDomain,
            AssetOrchardFixedBases,
        >::configure(
            meta,
            sinsemilla_advices[5..]
                .try_into()
                .expect("five advice columns"),
            sinsemilla_advices[7],
            lagrange_coeffs[2],
            lookup,
            sinsemilla_range_check,
            false,
        );
        let merkle_2 = AssetOrchardMerkleChip::configure(meta, merkle_sinsemilla_2.clone());
        let message_piece = AssetOrchardMessagePieceConstraintConfig::configure(
            meta,
            sinsemilla_advices[5],
            sinsemilla_advices[6],
            sinsemilla_advices[7],
            sinsemilla_advices[8],
            message_piece_weight,
        );
        let poseidon_state = std::array::from_fn(|_| meta.advice_column());
        let poseidon_partial_sbox = meta.advice_column();
        let poseidon_rc_a = std::array::from_fn(|_| meta.fixed_column());
        let poseidon_rc_b = std::array::from_fn(|_| meta.fixed_column());
        let poseidon = Pow5Chip::configure::<P128Pow5T3>(
            meta,
            poseidon_state,
            poseidon_partial_sbox,
            poseidon_rc_a,
            poseidon_rc_b,
        );
        for column in &advice {
            meta.enable_equality(*column);
        }
        meta.enable_equality(instance);

        let q_conservation = meta.selector();
        let q_value_nonzero = meta.selector();
        let q_asset_tag_nonzero = meta.selector();
        let q_range = meta.selector();
        let q_public_distinct = meta.selector();
        let q_pricing_binding = meta.selector();

        meta.create_gate("asset-orchard private pair conservation", |meta| {
            let q = meta.query_selector(q_conservation);
            let s = meta.query_advice(advice[0], Rotation::cur());
            let in0 = meta.query_advice(advice[1], Rotation::cur());
            let in1 = meta.query_advice(advice[2], Rotation::cur());
            let out0 = meta.query_advice(advice[3], Rotation::cur());
            let out1 = meta.query_advice(advice[4], Rotation::cur());
            let one = Expression::Constant(pallas::Base::ONE);

            vec![
                q.clone() * s.clone() * (s.clone() - one),
                q.clone() * (out0 - (in0.clone() + s.clone() * (in1.clone() - in0.clone()))),
                q * (out1 - (in1.clone() + s * (in0 - in1))),
            ]
        });

        meta.create_gate("asset-orchard nonzero private values", |meta| {
            let q = meta.query_selector(q_value_nonzero);
            let in0 = meta.query_advice(advice[1], Rotation::cur());
            let in1 = meta.query_advice(advice[2], Rotation::cur());
            let out0 = meta.query_advice(advice[3], Rotation::cur());
            let out1 = meta.query_advice(advice[4], Rotation::cur());
            let inv_in0 = meta.query_advice(advice[5], Rotation::cur());
            let inv_in1 = meta.query_advice(advice[6], Rotation::cur());
            let inv_out0 = meta.query_advice(advice[7], Rotation::cur());
            let inv_out1 = meta.query_advice(advice[8], Rotation::cur());
            let one = Expression::Constant(pallas::Base::ONE);

            vec![
                q.clone() * (in0 * inv_in0 - one.clone()),
                q.clone() * (in1 * inv_in1 - one.clone()),
                q.clone() * (out0 * inv_out0 - one.clone()),
                q * (out1 * inv_out1 - one),
            ]
        });

        meta.create_gate("asset-orchard nonzero private asset tags", |meta| {
            let q = meta.query_selector(q_asset_tag_nonzero);
            let in0_lo = meta.query_advice(advice[1], Rotation::cur());
            let in0_hi = meta.query_advice(advice[1], Rotation::next());
            let in1_lo = meta.query_advice(advice[2], Rotation::cur());
            let in1_hi = meta.query_advice(advice[2], Rotation::next());
            let out0_lo = meta.query_advice(advice[3], Rotation::cur());
            let out0_hi = meta.query_advice(advice[3], Rotation::next());
            let out1_lo = meta.query_advice(advice[4], Rotation::cur());
            let out1_hi = meta.query_advice(advice[4], Rotation::next());
            let inv_in0_lo = meta.query_advice(advice[5], Rotation::cur());
            let inv_in0_hi = meta.query_advice(advice[5], Rotation::next());
            let inv_in1_lo = meta.query_advice(advice[6], Rotation::cur());
            let inv_in1_hi = meta.query_advice(advice[6], Rotation::next());
            let inv_out0_lo = meta.query_advice(advice[7], Rotation::cur());
            let inv_out0_hi = meta.query_advice(advice[7], Rotation::next());
            let inv_out1_lo = meta.query_advice(advice[8], Rotation::cur());
            let inv_out1_hi = meta.query_advice(advice[8], Rotation::next());
            let one = Expression::Constant(pallas::Base::ONE);

            let mut constraints = Vec::new();
            for (lo, hi, inv_lo, inv_hi) in [
                (in0_lo, in0_hi, inv_in0_lo, inv_in0_hi),
                (in1_lo, in1_hi, inv_in1_lo, inv_in1_hi),
                (out0_lo, out0_hi, inv_out0_lo, inv_out0_hi),
                (out1_lo, out1_hi, inv_out1_lo, inv_out1_hi),
            ] {
                let lo_is_nonzero = lo * inv_lo;
                let hi_is_nonzero = hi * inv_hi;
                constraints.push(
                    q.clone() * lo_is_nonzero.clone() * (lo_is_nonzero.clone() - one.clone()),
                );
                constraints.push(
                    q.clone() * hi_is_nonzero.clone() * (hi_is_nonzero.clone() - one.clone()),
                );
                constraints.push(
                    q.clone() * (one.clone() - lo_is_nonzero) * (one.clone() - hi_is_nonzero),
                );
            }
            constraints
        });

        meta.create_gate("asset-orchard bit range accumulator", |meta| {
            let q = meta.query_selector(q_range);
            let bit = meta.query_advice(advice[0], Rotation::cur());
            let acc = meta.query_advice(advice[1], Rotation::cur());
            let next_acc = meta.query_advice(advice[2], Rotation::cur());
            let weight = meta.query_fixed(fixed[0]);
            let one = Expression::Constant(pallas::Base::ONE);

            vec![
                q.clone() * bit.clone() * (bit.clone() - one),
                q * (next_acc - acc - bit * weight),
            ]
        });

        meta.create_gate("asset-orchard distinct public state fields", |meta| {
            let q = meta.query_selector(q_public_distinct);
            let left = meta.query_advice(advice[0], Rotation::cur());
            let right = meta.query_advice(advice[0], Rotation::next());
            let inverse = meta.query_advice(advice[1], Rotation::cur());
            let one = Expression::Constant(pallas::Base::ONE);

            vec![q * ((left - right) * inverse - one)]
        });

        meta.create_gate("asset-orchard private pricing claim binding", |meta| {
            let q = meta.query_selector(q_pricing_binding);
            let base_tag_lo = meta.query_advice(advice[0], Rotation::cur());
            let base_tag_hi = meta.query_advice(advice[1], Rotation::cur());
            let quote_tag_lo = meta.query_advice(advice[2], Rotation::cur());
            let quote_tag_hi = meta.query_advice(advice[3], Rotation::cur());
            let base_value = meta.query_advice(advice[4], Rotation::cur());
            let quote_value = meta.query_advice(advice[5], Rotation::cur());
            let numerator = meta.query_advice(advice[6], Rotation::cur());
            let denominator = meta.query_advice(advice[7], Rotation::cur());
            let input_base_lo = meta.query_advice(advice[0], Rotation::next());
            let input_base_hi = meta.query_advice(advice[1], Rotation::next());
            let input_quote_lo = meta.query_advice(advice[2], Rotation::next());
            let input_quote_hi = meta.query_advice(advice[3], Rotation::next());
            let rounding_remainder = meta.query_advice(advice[4], Rotation::next());
            let rounding_slack = meta.query_advice(advice[5], Rotation::next());
            let one = Expression::Constant(pallas::Base::ONE);
            vec![
                q.clone() * (base_tag_lo - input_base_lo),
                q.clone() * (base_tag_hi - input_base_hi),
                q.clone() * (quote_tag_lo - input_quote_lo),
                q.clone() * (quote_tag_hi - input_quote_hi),
                q.clone()
                    * (base_value * numerator
                        - quote_value * denominator.clone()
                        - rounding_remainder.clone()),
                q * (rounding_remainder + rounding_slack + one - denominator),
            ]
        });

        AssetOrchardSwapConservationConfig {
            advice,
            fixed,
            instance,
            ecc,
            sinsemilla,
            message_piece,
            merkle_1,
            merkle_2,
            poseidon,
            q_conservation,
            q_value_nonzero,
            q_asset_tag_nonzero,
            q_range,
            q_public_distinct,
            q_pricing_binding,
        }
    }

    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<pallas::Base>,
    ) -> Result<(), Error> {
        AssetOrchardSinsemillaChip::load(config.sinsemilla.clone(), &mut layouter)?;
        let input0 = Self::leg_values(self.inputs[0]);
        let input1 = Self::leg_values(self.inputs[1]);
        let output0 = Self::leg_values(self.outputs[0]);
        let output1 = Self::leg_values(self.outputs[1]);

        let private_cells = layouter.assign_region(
            || "asset-orchard private conservation",
            |mut region| {
                let mut range_cells = Vec::new();
                let mut input_cells = [Vec::new(), Vec::new()];
                let mut output_cells = [Vec::new(), Vec::new()];
                let mut selector_cells = Vec::with_capacity(3);
                for row in 0..3 {
                    config.q_conservation.enable(&mut region, row)?;
                    let selector_cell = region.assign_advice(
                        || "swap selector",
                        config.advice[0],
                        row,
                        || self.permutation_selector_for_row(row),
                    )?;
                    selector_cells.push(selector_cell);
                    let bit_len = if row == 2 {
                        ASSET_ORCHARD_VALUE_BITS
                    } else {
                        ASSET_ORCHARD_ASSET_TAG_BITS
                    };
                    let input0_cell = region.assign_advice(
                        || "input 0",
                        config.advice[1],
                        row,
                        || input0[row],
                    )?;
                    let input1_cell = region.assign_advice(
                        || "input 1",
                        config.advice[2],
                        row,
                        || input1[row],
                    )?;
                    let output0_cell = region.assign_advice(
                        || "output 0",
                        config.advice[3],
                        row,
                        || output0[row],
                    )?;
                    let output1_cell = region.assign_advice(
                        || "output 1",
                        config.advice[4],
                        row,
                        || output1[row],
                    )?;
                    input_cells[0].push(input0_cell.clone());
                    input_cells[1].push(input1_cell.clone());
                    output_cells[0].push(output0_cell.clone());
                    output_cells[1].push(output1_cell.clone());
                    range_cells.push((input0_cell, input0[row], bit_len));
                    range_cells.push((input1_cell, input1[row], bit_len));
                    range_cells.push((output0_cell, output0[row], bit_len));
                    range_cells.push((output1_cell, output1[row], bit_len));
                }
                let first_selector_cell = selector_cells[0].cell();
                for selector_cell in selector_cells.iter().skip(1) {
                    region.constrain_equal(first_selector_cell, selector_cell.cell())?;
                }

                config.q_asset_tag_nonzero.enable(&mut region, 0)?;
                region.assign_advice(
                    || "input 0 asset tag lo inverse",
                    config.advice[5],
                    0,
                    || input0[0].map(invert_or_zero),
                )?;
                region.assign_advice(
                    || "input 0 asset tag hi inverse",
                    config.advice[5],
                    1,
                    || input0[1].map(invert_or_zero),
                )?;
                region.assign_advice(
                    || "input 1 asset tag lo inverse",
                    config.advice[6],
                    0,
                    || input1[0].map(invert_or_zero),
                )?;
                region.assign_advice(
                    || "input 1 asset tag hi inverse",
                    config.advice[6],
                    1,
                    || input1[1].map(invert_or_zero),
                )?;
                region.assign_advice(
                    || "output 0 asset tag lo inverse",
                    config.advice[7],
                    0,
                    || output0[0].map(invert_or_zero),
                )?;
                region.assign_advice(
                    || "output 0 asset tag hi inverse",
                    config.advice[7],
                    1,
                    || output0[1].map(invert_or_zero),
                )?;
                region.assign_advice(
                    || "output 1 asset tag lo inverse",
                    config.advice[8],
                    0,
                    || output1[0].map(invert_or_zero),
                )?;
                region.assign_advice(
                    || "output 1 asset tag hi inverse",
                    config.advice[8],
                    1,
                    || output1[1].map(invert_or_zero),
                )?;

                let value_row = 2;
                config.q_value_nonzero.enable(&mut region, value_row)?;
                region.assign_advice(
                    || "input 0 value inverse",
                    config.advice[5],
                    value_row,
                    || input0[value_row].map(invert_or_zero),
                )?;
                region.assign_advice(
                    || "input 1 value inverse",
                    config.advice[6],
                    value_row,
                    || input1[value_row].map(invert_or_zero),
                )?;
                region.assign_advice(
                    || "output 0 value inverse",
                    config.advice[7],
                    value_row,
                    || output0[value_row].map(invert_or_zero),
                )?;
                region.assign_advice(
                    || "output 1 value inverse",
                    config.advice[8],
                    value_row,
                    || output1[value_row].map(invert_or_zero),
                )?;
                Ok(PrivateConservationCells {
                    range_cells,
                    input_cells,
                    output_cells,
                })
            },
        )?;
        for (cell, value, bit_len) in private_cells.range_cells.iter() {
            synthesize_range_check(&mut layouter, &config, cell.clone(), *value, *bit_len)?;
        }

        let public_values = self
            .public_instance
            .unwrap_or([pallas::Base::ZERO; ASSET_ORCHARD_PUBLIC_INSTANCE_LEN]);
        let public_cells = layouter.assign_region(
            || "asset-orchard public instance shape",
            |mut region| {
                let mut cells = Vec::with_capacity(public_values.len());
                for (row, value) in public_values.iter().copied().enumerate() {
                    let assigned_value = if row == 16 { pallas::Base::ZERO } else { value };
                    cells.push(region.assign_advice(
                        || "public instance",
                        config.advice[0],
                        row,
                        || Value::known(assigned_value),
                    )?);
                }
                for row in [2usize, 8usize] {
                    config.q_public_distinct.enable(&mut region, row)?;
                    let left = public_values[row];
                    let right = public_values[row + 1];
                    region.assign_advice(
                        || "public distinct inverse",
                        config.advice[1],
                        row,
                        || Value::known(invert_or_zero(left - right)),
                    )?;
                }
                Ok(cells)
            },
        )?;
        for (row, cell) in public_cells.iter().enumerate() {
            layouter.constrain_instance(cell.cell(), config.instance, row)?;
        }
        for row in 10..=15 {
            synthesize_range_check(
                &mut layouter,
                &config,
                public_cells[row].clone(),
                Value::known(public_values[row]),
                ASSET_ORCHARD_ASSET_TAG_BITS,
            )?;
        }
        // Bind the public pricing tuple to both the private asset ordering and
        // private values. A claim copied from another action, or a ratio that
        // is merely JSON-consistent, cannot satisfy this gate.
        let (rounding_remainder_cell, rounding_slack_cell, rounding_remainder, rounding_slack) =
            layouter.assign_region(
                || "asset-orchard pricing claim binding",
                |mut region| {
                    config.q_pricing_binding.enable(&mut region, 0)?;
                    for (column, public_row) in [
                        (0usize, 17usize),
                        (1, 18),
                        (2, 19),
                        (3, 20),
                        (6, 21),
                        (7, 22),
                    ] {
                        let copied = region.assign_advice(
                            || "pricing public value",
                            config.advice[column],
                            0,
                            || Value::known(public_values[public_row]),
                        )?;
                        region.constrain_equal(copied.cell(), public_cells[public_row].cell())?;
                    }
                    for (column, private_cell) in [
                        (4usize, &private_cells.input_cells[0][2]),
                        (5usize, &private_cells.input_cells[1][2]),
                    ] {
                        let copied = region.assign_advice(
                            || "pricing private value",
                            config.advice[column],
                            0,
                            || private_cell.value().copied(),
                        )?;
                        region.constrain_equal(copied.cell(), private_cell.cell())?;
                    }
                    for (column, private_cell) in [
                        (0usize, &private_cells.input_cells[0][0]),
                        (1usize, &private_cells.input_cells[0][1]),
                        (2usize, &private_cells.input_cells[1][0]),
                        (3usize, &private_cells.input_cells[1][1]),
                    ] {
                        let copied = region.assign_advice(
                            || "pricing private asset tag",
                            config.advice[column],
                            1,
                            || private_cell.value().copied(),
                        )?;
                        region.constrain_equal(copied.cell(), private_cell.cell())?;
                    }
                    let (remainder, slack) = pricing_rounding_witness(
                        self.inputs[0],
                        self.inputs[1],
                        self.public_instance,
                    );
                    let remainder_cell = region.assign_advice(
                        || "pricing rounding remainder",
                        config.advice[4],
                        1,
                        || remainder,
                    )?;
                    let slack_cell = region.assign_advice(
                        || "pricing rounding slack",
                        config.advice[5],
                        1,
                        || slack,
                    )?;
                    Ok((remainder_cell, slack_cell, remainder, slack))
                },
            )?;
        synthesize_range_check(
            &mut layouter,
            &config,
            rounding_remainder_cell,
            rounding_remainder,
            ASSET_ORCHARD_PRICING_ROUNDING_BITS,
        )?;
        synthesize_range_check(
            &mut layouter,
            &config,
            rounding_slack_cell,
            rounding_slack,
            ASSET_ORCHARD_PRICING_ROUNDING_BITS,
        )?;
        let h_action_cells = synthesize_h_action_binding(&mut layouter, &config, public_values)?;
        for (cell, instance_row) in h_action_cells.public_inputs {
            layouter.constrain_instance(cell.cell(), config.instance, instance_row)?;
        }
        layouter.constrain_instance(
            h_action_cells.action_context[0].cell(),
            config.instance,
            26,
        )?;
        layouter.constrain_instance(
            h_action_cells.action_context[1].cell(),
            config.instance,
            27,
        )?;
        let ecc_chip =
            AssetOrchardEccChip::construct(config.ecc.clone(), CircuitVersion::AnchoredBase);
        let sinsemilla_chip = AssetOrchardSinsemillaChip::construct(config.sinsemilla.clone());
        let note_witness_count = self.note_witness_count();
        if note_witness_count != 0 && note_witness_count != ASSET_ORCHARD_LEG_COUNT * 2 {
            return Err(Error::Synthesis);
        }
        if note_witness_count == ASSET_ORCHARD_LEG_COUNT * 2 {
            for (index, note) in self.input_notes.iter().enumerate() {
                let note = note.as_ref().ok_or(Error::Synthesis)?;
                synthesize_note_commitment(
                    &mut layouter,
                    &config,
                    sinsemilla_chip.clone(),
                    ecc_chip.clone(),
                    format!("input note {index}"),
                    note,
                    public_cells[0].clone(),
                    public_values[0],
                    public_cells[1].clone(),
                    &public_cells,
                    public_values,
                    &private_cells.input_cells[index],
                    Some(2 + index),
                    None,
                    None,
                )?;
            }
            for (index, note) in self.output_notes.iter().enumerate() {
                let note = note.as_ref().ok_or(Error::Synthesis)?;
                synthesize_note_commitment(
                    &mut layouter,
                    &config,
                    sinsemilla_chip.clone(),
                    ecc_chip.clone(),
                    format!("output note {index}"),
                    note,
                    public_cells[0].clone(),
                    public_values[0],
                    public_cells[1].clone(),
                    &public_cells,
                    public_values,
                    &private_cells.output_cells[index],
                    None,
                    Some(8 + index),
                    None,
                )?;
            }
        }
        Ok(())
    }
}

struct HActionBindingCells {
    action_context: [AssignedCell<pallas::Base, pallas::Base>; 2],
    public_inputs: Vec<(AssignedCell<pallas::Base, pallas::Base>, usize)>,
}

struct PrivateConservationCells {
    range_cells: Vec<(
        AssignedCell<pallas::Base, pallas::Base>,
        Value<pallas::Base>,
        usize,
    )>,
    input_cells: [Vec<AssignedCell<pallas::Base, pallas::Base>>; ASSET_ORCHARD_LEG_COUNT],
    output_cells: [Vec<AssignedCell<pallas::Base, pallas::Base>>; ASSET_ORCHARD_LEG_COUNT],
}

struct NoteSourceBitCells {
    pool_domain: Vec<AssignedCell<pallas::Base, pallas::Base>>,
    asset_tag_lo: Vec<AssignedCell<pallas::Base, pallas::Base>>,
    asset_tag_hi: Vec<AssignedCell<pallas::Base, pallas::Base>>,
    gd_x: Vec<AssignedCell<pallas::Base, pallas::Base>>,
    gd_y_sign: Vec<AssignedCell<pallas::Base, pallas::Base>>,
    pkd_x: Vec<AssignedCell<pallas::Base, pallas::Base>>,
    pkd_y_sign: Vec<AssignedCell<pallas::Base, pallas::Base>>,
    value: Vec<AssignedCell<pallas::Base, pallas::Base>>,
    rho: Vec<AssignedCell<pallas::Base, pallas::Base>>,
    psi: Vec<AssignedCell<pallas::Base, pallas::Base>>,
    padding: Vec<AssignedCell<pallas::Base, pallas::Base>>,
    gd_x_cell: AssignedCell<pallas::Base, pallas::Base>,
    gd_y_sign_cell: AssignedCell<pallas::Base, pallas::Base>,
    pkd_x_cell: AssignedCell<pallas::Base, pallas::Base>,
    pkd_y_sign_cell: AssignedCell<pallas::Base, pallas::Base>,
    nk_cell: AssignedCell<pallas::Base, pallas::Base>,
    rho_cell: AssignedCell<pallas::Base, pallas::Base>,
    psi_cell: AssignedCell<pallas::Base, pallas::Base>,
}

impl NoteSourceBitCells {
    fn source(
        &self,
        source: AssetNoteMessageSource,
    ) -> &[AssignedCell<pallas::Base, pallas::Base>] {
        match source {
            AssetNoteMessageSource::PoolDomain => &self.pool_domain,
            AssetNoteMessageSource::AssetTagLo => &self.asset_tag_lo,
            AssetNoteMessageSource::AssetTagHi => &self.asset_tag_hi,
            AssetNoteMessageSource::GdX => &self.gd_x,
            AssetNoteMessageSource::GdYSign => &self.gd_y_sign,
            AssetNoteMessageSource::PkdX => &self.pkd_x,
            AssetNoteMessageSource::PkdYSign => &self.pkd_y_sign,
            AssetNoteMessageSource::Value => &self.value,
            AssetNoteMessageSource::Rho => &self.rho,
            AssetNoteMessageSource::Psi => &self.psi,
            AssetNoteMessageSource::Padding => &self.padding,
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn synthesize_note_commitment(
    layouter: &mut impl Layouter<pallas::Base>,
    config: &AssetOrchardSwapConservationConfig,
    sinsemilla_chip: AssetOrchardSinsemillaChip,
    ecc_chip: AssetOrchardEccChip,
    label: String,
    note: &AssetOrchardSwapNoteWitness,
    pool_domain_cell: AssignedCell<pallas::Base, pallas::Base>,
    pool_domain: pallas::Base,
    anchor_cell: AssignedCell<pallas::Base, pallas::Base>,
    public_cells: &[AssignedCell<pallas::Base, pallas::Base>],
    public_values: [pallas::Base; ASSET_ORCHARD_PUBLIC_INSTANCE_LEN],
    leg_cells: &[AssignedCell<pallas::Base, pallas::Base>],
    public_nullifier_row: Option<usize>,
    public_output_row: Option<usize>,
    public_rk_rows: Option<(usize, usize)>,
) -> Result<(), Error> {
    if leg_cells.len() != 3 {
        return Err(Error::Synthesis);
    }
    let source_bits = synthesize_note_source_bits(
        layouter,
        config,
        &label,
        note,
        pool_domain_cell.clone(),
        pool_domain,
        leg_cells,
    )?;
    let mut piece_bits = vec![
        Vec::<AssignedCell<pallas::Base, pallas::Base>>::new();
        ASSET_ORCHARD_NOTE_MESSAGE_PIECE_COUNT
    ];
    for segment in asset_note_message_segments() {
        let source = source_bits.source(segment.source);
        piece_bits[segment.piece_index].extend_from_slice(
            &source[segment.source_bit_offset..segment.source_bit_offset + segment.bit_len],
        );
    }
    let mut piece_subpieces = Vec::with_capacity(piece_bits.len());
    for (piece_index, bits) in piece_bits.iter().enumerate() {
        let piece = synthesize_packed_bit_subpiece(
            layouter,
            config,
            &format!("{label} message piece {piece_index}"),
            bits,
        )?;
        piece_subpieces.push(vec![piece]);
    }
    let cmx = synthesize_asset_note_commitment_from_assigned_subpieces(
        layouter,
        &config.message_piece,
        sinsemilla_chip.clone(),
        ecc_chip.clone(),
        &piece_subpieces,
        Value::known(note.note.rcm),
    )?;
    if let Some(row) = public_output_row {
        layouter.constrain_instance(cmx.inner().cell(), config.instance, row)?;
        let output_index = row.checked_sub(8).ok_or(Error::Synthesis)?;
        if output_index >= ASSET_ORCHARD_LEG_COUNT {
            return Err(Error::Synthesis);
        }
        synthesize_asset_output_rho(
            layouter,
            config,
            &format!("{label} output rho"),
            public_cells,
            public_values,
            output_index,
            source_bits.rho_cell.clone(),
        )?;
    } else {
        let cmx_cell = cmx.inner().cell();
        layouter.assign_region(
            || format!("{label} private input cmx"),
            |mut region| {
                let expected_cmx = region.assign_advice(
                    || "private input cmx",
                    config.advice[0],
                    0,
                    || Value::known(note.cmx),
                )?;
                region.constrain_equal(cmx_cell, expected_cmx.cell())?;
                Ok(())
            },
        )?;
    }
    if let Some(row) = public_nullifier_row {
        let input_index = row.checked_sub(2).ok_or(Error::Synthesis)?;
        let nullifier = synthesize_asset_nullifier(
            layouter,
            config,
            &format!("{label} nullifier"),
            pool_domain,
            note,
            pool_domain_cell,
            source_bits.nk_cell.clone(),
            source_bits.rho_cell.clone(),
            source_bits.psi_cell.clone(),
            cmx.inner().clone(),
        )?;
        layouter.constrain_instance(nullifier.cell(), config.instance, row)?;
        let (public_rk_x_row, public_rk_y_row) =
            public_rk_rows.unwrap_or((4 + input_index * 2, 5 + input_index * 2));
        synthesize_input_spend_authority(
            layouter,
            config,
            ecc_chip,
            sinsemilla_chip,
            &format!("{label} spend authority"),
            note,
            &source_bits,
            public_rk_x_row,
            public_rk_y_row,
        )?;
        synthesize_input_merkle_anchor(
            layouter,
            config,
            &format!("{label} merkle anchor"),
            note,
            cmx.inner().clone(),
            anchor_cell,
        )?;
    }
    Ok(())
}

fn synthesize_asset_output_rho(
    layouter: &mut impl Layouter<pallas::Base>,
    config: &AssetOrchardSwapConservationConfig,
    label: &str,
    public_cells: &[AssignedCell<pallas::Base, pallas::Base>],
    public_values: [pallas::Base; ASSET_ORCHARD_PUBLIC_INSTANCE_LEN],
    output_index: usize,
    rho_cell: AssignedCell<pallas::Base, pallas::Base>,
) -> Result<(), Error> {
    if public_cells.len() != ASSET_ORCHARD_PUBLIC_INSTANCE_LEN {
        return Err(Error::Synthesis);
    }
    let output_index_u8 = u8::try_from(output_index).map_err(|_| Error::Synthesis)?;
    let inputs = asset_output_rho_poseidon_inputs(
        public_values[0],
        public_values[1],
        [public_values[2], public_values[3]],
        [
            crate::asset_orchard::RandomizedVerificationKeyFields {
                x: public_values[4],
                y: public_values[5],
            },
            crate::asset_orchard::RandomizedVerificationKeyFields {
                x: public_values[6],
                y: public_values[7],
            },
        ],
        output_index_u8,
    )
    .map_err(|_| Error::Synthesis)?;
    let sources = [
        PoseidonAssignedSource::Constant(inputs[0]),
        PoseidonAssignedSource::Constant(inputs[1]),
        PoseidonAssignedSource::Cell {
            value: inputs[2],
            cell: public_cells[0].clone(),
        },
        PoseidonAssignedSource::Cell {
            value: inputs[3],
            cell: public_cells[1].clone(),
        },
        PoseidonAssignedSource::Cell {
            value: inputs[4],
            cell: public_cells[2].clone(),
        },
        PoseidonAssignedSource::Cell {
            value: inputs[5],
            cell: public_cells[3].clone(),
        },
        PoseidonAssignedSource::Cell {
            value: inputs[6],
            cell: public_cells[4].clone(),
        },
        PoseidonAssignedSource::Cell {
            value: inputs[7],
            cell: public_cells[5].clone(),
        },
        PoseidonAssignedSource::Cell {
            value: inputs[8],
            cell: public_cells[6].clone(),
        },
        PoseidonAssignedSource::Cell {
            value: inputs[9],
            cell: public_cells[7].clone(),
        },
        PoseidonAssignedSource::Constant(inputs[10]),
    ];
    let computed = synthesize_poseidon_hash1_from_sources(layouter, config, label, &sources)?;
    layouter.assign_region(
        || format!("{label} binding"),
        |mut region| {
            let computed =
                computed.copy_advice(|| "computed output rho", &mut region, config.advice[0], 0)?;
            let rho =
                rho_cell.copy_advice(|| "note output rho", &mut region, config.advice[1], 0)?;
            region.constrain_equal(computed.cell(), rho.cell())?;
            Ok(())
        },
    )
}

fn synthesize_input_spend_authority(
    layouter: &mut impl Layouter<pallas::Base>,
    config: &AssetOrchardSwapConservationConfig,
    ecc_chip: AssetOrchardEccChip,
    sinsemilla_chip: AssetOrchardSinsemillaChip,
    label: &str,
    note: &AssetOrchardSwapNoteWitness,
    source_bits: &NoteSourceBitCells,
    public_rk_x_row: usize,
    public_rk_y_row: usize,
) -> Result<(), Error> {
    let authority = note.spend_authority.as_ref().ok_or(Error::Synthesis)?;
    let ak = NonIdentityPoint::new(
        ecc_chip.clone(),
        layouter.namespace(|| format!("{label} ak")),
        Value::known(authority.ak),
    )?;
    let alpha = ScalarFixed::new(
        ecc_chip.clone(),
        layouter.namespace(|| format!("{label} alpha")),
        Value::known(authority.alpha),
    )?;
    let spend_auth_g =
        FixedPoint::from_inner(ecc_chip.clone(), AssetOrchardFullScalarBase::SpendAuthG);
    let (alpha_commitment, _) =
        spend_auth_g.mul(layouter.namespace(|| format!("{label} [alpha]G")), alpha)?;
    let rk = alpha_commitment.add(layouter.namespace(|| format!("{label} rk")), &ak)?;
    layouter.constrain_instance(rk.inner().x().cell(), config.instance, public_rk_x_row)?;
    layouter.constrain_instance(rk.inner().y().cell(), config.instance, public_rk_y_row)?;

    let g_d = synthesize_encoded_note_point(
        layouter,
        config,
        ecc_chip.clone(),
        &format!("{label} g_d"),
        note.note.g_d,
        source_bits.gd_x_cell.clone(),
        source_bits.gd_y_sign_cell.clone(),
    )?;
    let ivk_ecc_chip = ecc_chip.clone();
    let pk_d = synthesize_encoded_note_point(
        layouter,
        config,
        ecc_chip,
        &format!("{label} pk_d"),
        note.note.pk_d,
        source_bits.pkd_x_cell.clone(),
        source_bits.pkd_y_sign_cell.clone(),
    )?;
    let ivk = synthesize_orchard_commit_ivk(
        layouter,
        config,
        sinsemilla_chip,
        ak.inner().x().clone(),
        source_bits.nk_cell.clone(),
        Value::known(authority.rivk),
        &format!("{label} commit ivk"),
    )?;
    let ivk = ScalarVar::from_base(
        ivk_ecc_chip,
        layouter.namespace(|| format!("{label} ivk scalar")),
        ivk.inner(),
    )?;
    let (derived_pk_d, _) = g_d.mul(layouter.namespace(|| format!("{label} [ivk]g_d")), ivk)?;
    derived_pk_d.constrain_equal(
        layouter.namespace(|| format!("{label} pk_d equality")),
        &pk_d,
    )?;
    Ok(())
}

fn synthesize_encoded_note_point(
    layouter: &mut impl Layouter<pallas::Base>,
    config: &AssetOrchardSwapConservationConfig,
    ecc_chip: AssetOrchardEccChip,
    label: &str,
    point: pallas::Affine,
    x_cell: AssignedCell<pallas::Base, pallas::Base>,
    y_sign_cell: AssignedCell<pallas::Base, pallas::Base>,
) -> Result<NonIdentityPoint<pallas::Affine, AssetOrchardEccChip>, Error> {
    let point = NonIdentityPoint::new(
        ecc_chip,
        layouter.namespace(|| format!("{label} point")),
        Value::known(point),
    )?;
    layouter.assign_region(
        || format!("{label} x binding"),
        |mut region| {
            let point_x =
                point
                    .inner()
                    .x()
                    .copy_advice(|| "point x", &mut region, config.advice[0], 0)?;
            let encoded_x = x_cell.copy_advice(|| "encoded x", &mut region, config.advice[1], 0)?;
            region.constrain_equal(point_x.cell(), encoded_x.cell())?;
            Ok(())
        },
    )?;
    let y_bits = synthesize_range_decomposition(
        layouter,
        config,
        format!("{label} y parity bits"),
        point.inner().y().clone(),
        point.inner().y().value().copied(),
        255,
    )?;
    layouter.assign_region(
        || format!("{label} y sign binding"),
        |mut region| {
            let y_lsb = y_bits[0].copy_advice(|| "y lsb", &mut region, config.advice[0], 0)?;
            let encoded_sign =
                y_sign_cell.copy_advice(|| "encoded y sign", &mut region, config.advice[1], 0)?;
            region.constrain_equal(y_lsb.cell(), encoded_sign.cell())?;
            Ok(())
        },
    )?;
    Ok(point)
}

fn synthesize_orchard_commit_ivk(
    layouter: &mut impl Layouter<pallas::Base>,
    config: &AssetOrchardSwapConservationConfig,
    sinsemilla_chip: AssetOrchardSinsemillaChip,
    ak_x_cell: AssignedCell<pallas::Base, pallas::Base>,
    nk_cell: AssignedCell<pallas::Base, pallas::Base>,
    rivk: Value<pallas::Scalar>,
    label: &str,
) -> Result<halo2_gadgets::ecc::X<pallas::Affine, AssetOrchardEccChip>, Error> {
    let mut bits = synthesize_range_decomposition(
        layouter,
        config,
        format!("{label} ak bits"),
        ak_x_cell.clone(),
        ak_x_cell.value().copied(),
        255,
    )?;
    bits.extend(synthesize_range_decomposition(
        layouter,
        config,
        format!("{label} nk bits"),
        nk_cell.clone(),
        nk_cell.value().copied(),
        255,
    )?);
    let mut piece_subpieces = Vec::with_capacity(3);
    for (piece_index, chunk) in bits
        .chunks(ASSET_ORCHARD_NOTE_MESSAGE_PIECE_BITS)
        .enumerate()
    {
        let piece = synthesize_packed_bit_subpiece(
            layouter,
            config,
            &format!("{label} message piece {piece_index}"),
            chunk,
        )?;
        piece_subpieces.push(vec![piece]);
    }
    let ecc_chip = AssetOrchardEccChip::construct(config.ecc.clone(), CircuitVersion::AnchoredBase);
    synthesize_sinsemilla_commitment_from_assigned_subpieces(
        layouter,
        &config.message_piece,
        sinsemilla_chip,
        ecc_chip,
        AssetOrchardCommitDomain::CommitIvk,
        label,
        &piece_subpieces,
        rivk,
    )
}

fn synthesize_input_merkle_anchor(
    layouter: &mut impl Layouter<pallas::Base>,
    config: &AssetOrchardSwapConservationConfig,
    label: &str,
    note: &AssetOrchardSwapNoteWitness,
    cmx_cell: AssignedCell<pallas::Base, pallas::Base>,
    anchor_cell: AssignedCell<pallas::Base, pallas::Base>,
) -> Result<(), Error> {
    let witness = note.merkle_witness.as_ref().ok_or(Error::Synthesis)?;
    let merkle_chip_1 = AssetOrchardMerkleChip::construct(config.merkle_1.clone());
    let merkle_chip_2 = AssetOrchardMerkleChip::construct(config.merkle_2.clone());
    let path = AssetOrchardMerklePath::construct(
        [merkle_chip_1, merkle_chip_2],
        AssetOrchardMerkleHashDomain,
        Value::known(witness.position),
        Value::known(witness.auth_path),
    );
    let root = path.calculate_root(layouter.namespace(|| label.to_string()), cmx_cell)?;
    layouter.assign_region(
        || format!("{label} public anchor binding"),
        |mut region| {
            let root =
                root.copy_advice(|| "computed merkle root", &mut region, config.advice[0], 0)?;
            let anchor =
                anchor_cell.copy_advice(|| "public anchor", &mut region, config.advice[1], 0)?;
            region.constrain_equal(root.cell(), anchor.cell())?;
            Ok(())
        },
    )
}

#[allow(clippy::too_many_arguments)]
fn synthesize_note_source_bits(
    layouter: &mut impl Layouter<pallas::Base>,
    config: &AssetOrchardSwapConservationConfig,
    label: &str,
    note: &AssetOrchardSwapNoteWitness,
    pool_domain_cell: AssignedCell<pallas::Base, pallas::Base>,
    pool_domain: pallas::Base,
    leg_cells: &[AssignedCell<pallas::Base, pallas::Base>],
) -> Result<NoteSourceBitCells, Error> {
    let (gd_x, gd_y_sign) = point_encoding_fields(note.note.g_d)?;
    let (pkd_x, pkd_y_sign) = point_encoding_fields(note.note.pk_d)?;
    let extra = layouter.assign_region(
        || format!("{label} note private fields"),
        |mut region| {
            let gd_x_cell =
                region.assign_advice(|| "g_d x", config.advice[0], 0, || Value::known(gd_x))?;
            let gd_y_sign_cell = region.assign_advice(
                || "g_d y sign",
                config.advice[1],
                0,
                || Value::known(gd_y_sign),
            )?;
            let pkd_x_cell =
                region.assign_advice(|| "pk_d x", config.advice[2], 0, || Value::known(pkd_x))?;
            let pkd_y_sign_cell = region.assign_advice(
                || "pk_d y sign",
                config.advice[3],
                0,
                || Value::known(pkd_y_sign),
            )?;
            let rho_cell = region.assign_advice(
                || "rho",
                config.advice[4],
                0,
                || Value::known(note.note.rho),
            )?;
            let psi_cell = region.assign_advice(
                || "psi",
                config.advice[5],
                0,
                || Value::known(note.note.psi),
            )?;
            let nk_cell =
                region.assign_advice(|| "nk", config.advice[7], 0, || Value::known(note.nk))?;
            let padding_cell = region.assign_advice(
                || "zero padding",
                config.advice[6],
                0,
                || Value::known(pallas::Base::ZERO),
            )?;
            Ok((
                gd_x_cell,
                gd_y_sign_cell,
                pkd_x_cell,
                pkd_y_sign_cell,
                rho_cell,
                psi_cell,
                nk_cell,
                padding_cell,
            ))
        },
    )?;

    Ok(NoteSourceBitCells {
        pool_domain: synthesize_range_decomposition(
            layouter,
            config,
            format!("{label} pool_domain bits"),
            pool_domain_cell,
            Value::known(pool_domain),
            255,
        )?,
        asset_tag_lo: synthesize_range_decomposition(
            layouter,
            config,
            format!("{label} asset_tag_lo bits"),
            leg_cells[0].clone(),
            Value::known(pallas::Base::from_u128(note.note.asset_tag.lo)),
            ASSET_ORCHARD_ASSET_TAG_BITS,
        )?,
        asset_tag_hi: synthesize_range_decomposition(
            layouter,
            config,
            format!("{label} asset_tag_hi bits"),
            leg_cells[1].clone(),
            Value::known(pallas::Base::from_u128(note.note.asset_tag.hi)),
            ASSET_ORCHARD_ASSET_TAG_BITS,
        )?,
        gd_x: synthesize_range_decomposition(
            layouter,
            config,
            format!("{label} g_d x bits"),
            extra.0.clone(),
            Value::known(gd_x),
            255,
        )?,
        gd_y_sign: synthesize_range_decomposition(
            layouter,
            config,
            format!("{label} g_d y sign bits"),
            extra.1.clone(),
            Value::known(gd_y_sign),
            1,
        )?,
        pkd_x: synthesize_range_decomposition(
            layouter,
            config,
            format!("{label} pk_d x bits"),
            extra.2.clone(),
            Value::known(pkd_x),
            255,
        )?,
        pkd_y_sign: synthesize_range_decomposition(
            layouter,
            config,
            format!("{label} pk_d y sign bits"),
            extra.3.clone(),
            Value::known(pkd_y_sign),
            1,
        )?,
        value: synthesize_range_decomposition(
            layouter,
            config,
            format!("{label} value bits"),
            leg_cells[2].clone(),
            Value::known(pallas::Base::from(note.note.value)),
            ASSET_ORCHARD_VALUE_BITS,
        )?,
        rho: synthesize_range_decomposition(
            layouter,
            config,
            format!("{label} rho bits"),
            extra.4.clone(),
            Value::known(note.note.rho),
            255,
        )?,
        psi: synthesize_range_decomposition(
            layouter,
            config,
            format!("{label} psi bits"),
            extra.5.clone(),
            Value::known(note.note.psi),
            255,
        )?,
        padding: synthesize_range_decomposition(
            layouter,
            config,
            format!("{label} padding bits"),
            extra.7,
            Value::known(pallas::Base::ZERO),
            3,
        )?,
        gd_x_cell: extra.0,
        gd_y_sign_cell: extra.1,
        pkd_x_cell: extra.2,
        pkd_y_sign_cell: extra.3,
        nk_cell: extra.6,
        rho_cell: extra.4,
        psi_cell: extra.5,
    })
}

fn synthesize_h_action_binding(
    layouter: &mut impl Layouter<pallas::Base>,
    config: &AssetOrchardSwapConservationConfig,
    public_values: [pallas::Base; ASSET_ORCHARD_PUBLIC_INSTANCE_LEN],
) -> Result<HActionBindingCells, Error> {
    let inputs = h_action_poseidon_inputs_from_public_instance(&public_values)
        .map_err(|_| Error::Synthesis)?;
    let sources = h_action_input_sources(inputs);
    synthesize_poseidon_sponge(
        layouter,
        config,
        "asset-orchard h_action poseidon binding",
        &sources,
    )
}

fn synthesize_private_egress_h_action_binding(
    layouter: &mut impl Layouter<pallas::Base>,
    config: &AssetOrchardSwapConservationConfig,
    public_values: [pallas::Base; ASSET_ORCHARD_PRIVATE_EGRESS_PUBLIC_INSTANCE_LEN],
) -> Result<HActionBindingCells, Error> {
    let inputs = private_egress_h_action_poseidon_inputs_from_public_instance(&public_values)
        .map_err(|_| Error::Synthesis)?;
    let sources = private_egress_h_action_input_sources(inputs);
    synthesize_poseidon_sponge(
        layouter,
        config,
        "asset-orchard private egress h_action poseidon binding",
        &sources,
    )
}

#[allow(clippy::too_many_arguments)]
fn synthesize_asset_nullifier(
    layouter: &mut impl Layouter<pallas::Base>,
    config: &AssetOrchardSwapConservationConfig,
    label: &str,
    pool_domain: pallas::Base,
    note: &AssetOrchardSwapNoteWitness,
    pool_domain_cell: AssignedCell<pallas::Base, pallas::Base>,
    nk_cell: AssignedCell<pallas::Base, pallas::Base>,
    rho_cell: AssignedCell<pallas::Base, pallas::Base>,
    psi_cell: AssignedCell<pallas::Base, pallas::Base>,
    cmx_cell: AssignedCell<pallas::Base, pallas::Base>,
) -> Result<AssignedCell<pallas::Base, pallas::Base>, Error> {
    let inputs = asset_derive_nullifier_poseidon_inputs(
        pool_domain,
        note.nk,
        note.note.rho,
        note.note.psi,
        note.cmx,
    )
    .map_err(|_| Error::Synthesis)?;
    let sources = [
        PoseidonAssignedSource::Constant(inputs[0]),
        PoseidonAssignedSource::Constant(inputs[1]),
        PoseidonAssignedSource::Constant(inputs[2]),
        PoseidonAssignedSource::Cell {
            value: inputs[3],
            cell: pool_domain_cell,
        },
        PoseidonAssignedSource::Cell {
            value: inputs[4],
            cell: nk_cell,
        },
        PoseidonAssignedSource::Cell {
            value: inputs[5],
            cell: rho_cell,
        },
        PoseidonAssignedSource::Cell {
            value: inputs[6],
            cell: psi_cell,
        },
        PoseidonAssignedSource::Cell {
            value: inputs[7],
            cell: cmx_cell,
        },
    ];
    synthesize_poseidon_hash1_from_sources(layouter, config, label, &sources)
}

#[derive(Clone)]
enum PoseidonAssignedSource {
    Constant(pallas::Base),
    Cell {
        value: pallas::Base,
        cell: AssignedCell<pallas::Base, pallas::Base>,
    },
}

fn synthesize_poseidon_hash1_from_sources(
    layouter: &mut impl Layouter<pallas::Base>,
    config: &AssetOrchardSwapConservationConfig,
    label: &str,
    sources: &[PoseidonAssignedSource],
) -> Result<AssignedCell<pallas::Base, pallas::Base>, Error> {
    let assigned = assign_poseidon_sources(layouter, config, label, sources)?;
    Ok(pow5_sponge_outputs(layouter, config, label, assigned)?[0].clone())
}

fn assign_poseidon_sources(
    layouter: &mut impl Layouter<pallas::Base>,
    config: &AssetOrchardSwapConservationConfig,
    label: &str,
    sources: &[PoseidonAssignedSource],
) -> Result<Vec<AssignedCell<pallas::Base, pallas::Base>>, Error> {
    layouter.assign_region(
        || format!("{label} inputs"),
        |mut region| {
            sources
                .iter()
                .enumerate()
                .map(|(row, source)| match source {
                    PoseidonAssignedSource::Constant(value) => region.assign_advice_from_constant(
                        || "poseidon constant input",
                        config.advice[0],
                        row,
                        *value,
                    ),
                    PoseidonAssignedSource::Cell { value, cell } => {
                        let assigned = region.assign_advice(
                            || "poseidon constrained input",
                            config.advice[0],
                            row,
                            || Value::known(*value),
                        )?;
                        region.constrain_equal(assigned.cell(), cell.cell())?;
                        Ok(assigned)
                    }
                })
                .collect()
        },
    )
}

#[derive(Clone, Copy, Debug)]
struct AssetOrchardZeroCapacityDomain;

impl Domain<pallas::Base, ASSET_ORCHARD_POSEIDON_RATE> for AssetOrchardZeroCapacityDomain {
    type Padding = std::iter::Empty<pallas::Base>;

    fn name() -> String {
        "AssetOrchardZeroCapacityDomain".to_string()
    }

    fn initial_capacity_element() -> pallas::Base {
        pallas::Base::ZERO
    }

    fn padding(_input_len: usize) -> Self::Padding {
        std::iter::empty()
    }
}

fn pow5_sponge_outputs(
    layouter: &mut impl Layouter<pallas::Base>,
    config: &AssetOrchardSwapConservationConfig,
    label: &str,
    inputs: Vec<AssignedCell<pallas::Base, pallas::Base>>,
) -> Result<[AssignedCell<pallas::Base, pallas::Base>; 2], Error> {
    if inputs.is_empty() {
        return Err(Error::Synthesis);
    }
    let chip = Pow5Chip::construct(config.poseidon.clone());
    let mut sponge = Sponge::<
        pallas::Base,
        Pow5Chip<pallas::Base, ASSET_ORCHARD_POSEIDON_WIDTH, ASSET_ORCHARD_POSEIDON_RATE>,
        P128Pow5T3,
        Absorbing<PaddedWord<pallas::Base>, ASSET_ORCHARD_POSEIDON_RATE>,
        AssetOrchardZeroCapacityDomain,
        ASSET_ORCHARD_POSEIDON_WIDTH,
        ASSET_ORCHARD_POSEIDON_RATE,
    >::new(chip, layouter.namespace(|| format!("{label} initialize")))?;
    let odd = inputs.len() % ASSET_ORCHARD_POSEIDON_RATE != 0;
    for (index, input) in inputs.into_iter().enumerate() {
        sponge.absorb(
            layouter.namespace(|| format!("{label} absorb {index}")),
            PaddedWord::Message(input),
        )?;
    }
    if odd {
        sponge.absorb(
            layouter.namespace(|| format!("{label} zero padding")),
            PaddedWord::Padding(pallas::Base::ZERO),
        )?;
    }
    let mut squeezing =
        sponge.finish_absorbing(layouter.namespace(|| format!("{label} finish")))?;
    let output0 = squeezing.squeeze(layouter.namespace(|| format!("{label} output 0")))?;
    let output1 = squeezing.squeeze(layouter.namespace(|| format!("{label} output 1")))?;
    Ok([output0, output1])
}

fn pricing_rounding_witness(
    base: Option<AssetOrchardSwapPrivateLeg>,
    quote: Option<AssetOrchardSwapPrivateLeg>,
    public_instance: Option<[pallas::Base; ASSET_ORCHARD_PUBLIC_INSTANCE_LEN]>,
) -> (Value<pallas::Base>, Value<pallas::Base>) {
    let Some((base, quote, public_instance)) = base
        .zip(quote)
        .zip(public_instance)
        .map(|((base, quote), public_instance)| (base, quote, public_instance))
    else {
        return (Value::unknown(), Value::unknown());
    };
    let Some(numerator) = field_to_u64(public_instance[21]) else {
        return (Value::unknown(), Value::unknown());
    };
    let Some(denominator) = field_to_u64(public_instance[22]) else {
        return (Value::unknown(), Value::unknown());
    };
    let product = u128::from(base.value) * u128::from(numerator);
    let consumed = u128::from(quote.value) * u128::from(denominator);
    let remainder = product.saturating_sub(consumed);
    let slack = u128::from(denominator).saturating_sub(remainder.saturating_add(1));
    (
        Value::known(pallas::Base::from_u128(remainder)),
        Value::known(pallas::Base::from_u128(slack)),
    )
}

fn field_to_u64(value: pallas::Base) -> Option<u64> {
    let repr = value.to_repr();
    let bytes = repr.as_ref();
    if bytes[8..].iter().any(|byte| *byte != 0) {
        return None;
    }
    Some(u64::from_le_bytes(bytes[..8].try_into().ok()?))
}

fn synthesize_range_check(
    layouter: &mut impl Layouter<pallas::Base>,
    config: &AssetOrchardSwapConservationConfig,
    target_cell: AssignedCell<pallas::Base, pallas::Base>,
    target_value: Value<pallas::Base>,
    bit_len: usize,
) -> Result<(), Error> {
    layouter.assign_region(
        || "asset-orchard range check",
        |mut region| {
            let mut acc_value = Value::known(pallas::Base::ZERO);
            let mut final_acc = None;
            for bit_index in 0..bit_len {
                config.q_range.enable(&mut region, bit_index)?;
                let bit_value = bit_from_field_value(target_value, bit_index);
                let weight = two_pow(bit_index);
                let next_acc_value = acc_value
                    .zip(bit_value)
                    .map(|(acc, bit)| acc + bit * weight);
                region.assign_fixed(
                    || "range weight",
                    config.fixed[0],
                    bit_index,
                    || Value::known(weight),
                )?;
                region.assign_advice(|| "range bit", config.advice[0], bit_index, || bit_value)?;
                region.assign_advice(
                    || "range accumulator",
                    config.advice[1],
                    bit_index,
                    || acc_value,
                )?;
                let next_cell = region.assign_advice(
                    || "range next accumulator",
                    config.advice[2],
                    bit_index,
                    || next_acc_value,
                )?;
                acc_value = next_acc_value;
                final_acc = Some(next_cell);
            }
            let final_acc = final_acc.ok_or(Error::Synthesis)?;
            let target_copy = target_cell.copy_advice(
                || "range target copy",
                &mut region,
                config.advice[3],
                bit_len,
            )?;
            region.constrain_equal(final_acc.cell(), target_copy.cell())?;
            Ok(())
        },
    )
}

fn synthesize_range_decomposition(
    layouter: &mut impl Layouter<pallas::Base>,
    config: &AssetOrchardSwapConservationConfig,
    label: String,
    target_cell: AssignedCell<pallas::Base, pallas::Base>,
    target_value: Value<pallas::Base>,
    bit_len: usize,
) -> Result<Vec<AssignedCell<pallas::Base, pallas::Base>>, Error> {
    layouter.assign_region(
        || label.clone(),
        |mut region| {
            let mut acc_value = Value::known(pallas::Base::ZERO);
            let mut final_acc = None;
            let mut bits = Vec::with_capacity(bit_len);
            for bit_index in 0..bit_len {
                config.q_range.enable(&mut region, bit_index)?;
                let bit_value = bit_from_field_value(target_value, bit_index);
                let weight = two_pow(bit_index);
                let next_acc_value = acc_value
                    .zip(bit_value)
                    .map(|(acc, bit)| acc + bit * weight);
                region.assign_fixed(
                    || "range weight",
                    config.fixed[0],
                    bit_index,
                    || Value::known(weight),
                )?;
                let bit_cell = region.assign_advice(
                    || "range bit",
                    config.advice[0],
                    bit_index,
                    || bit_value,
                )?;
                bits.push(bit_cell);
                region.assign_advice(
                    || "range accumulator",
                    config.advice[1],
                    bit_index,
                    || acc_value,
                )?;
                let next_cell = region.assign_advice(
                    || "range next accumulator",
                    config.advice[2],
                    bit_index,
                    || next_acc_value,
                )?;
                acc_value = next_acc_value;
                final_acc = Some(next_cell);
            }
            let final_acc = final_acc.ok_or(Error::Synthesis)?;
            let target_copy = target_cell.copy_advice(
                || "range target copy",
                &mut region,
                config.advice[3],
                bit_len,
            )?;
            region.constrain_equal(final_acc.cell(), target_copy.cell())?;
            Ok(bits)
        },
    )
}

fn synthesize_packed_bit_subpiece(
    layouter: &mut impl Layouter<pallas::Base>,
    config: &AssetOrchardSwapConservationConfig,
    label: &str,
    bits: &[AssignedCell<pallas::Base, pallas::Base>],
) -> Result<AssetOrchardAssignedSubpiece, Error> {
    if bits.is_empty() {
        return Err(Error::Synthesis);
    }
    layouter.assign_region(
        || format!("{label} packed bit subpiece"),
        |mut region| {
            let mut acc_value = Value::known(pallas::Base::ZERO);
            let mut final_acc = None;
            for (bit_index, bit) in bits.iter().enumerate() {
                config.q_range.enable(&mut region, bit_index)?;
                let weight = two_pow(bit_index);
                let bit_cell =
                    bit.copy_advice(|| "subpiece bit", &mut region, config.advice[0], bit_index)?;
                let next_acc_value = acc_value
                    .zip(bit_cell.value().copied())
                    .map(|(acc, bit)| acc + bit * weight);
                region.assign_fixed(
                    || "subpiece bit weight",
                    config.fixed[0],
                    bit_index,
                    || Value::known(weight),
                )?;
                region.assign_advice(
                    || "subpiece accumulator",
                    config.advice[1],
                    bit_index,
                    || acc_value,
                )?;
                let next_cell = region.assign_advice(
                    || "subpiece next accumulator",
                    config.advice[2],
                    bit_index,
                    || next_acc_value,
                )?;
                acc_value = next_acc_value;
                final_acc = Some(next_cell);
            }
            let final_acc = final_acc.ok_or(Error::Synthesis)?;
            let packed = region.assign_advice(
                || "packed subpiece",
                config.advice[3],
                bits.len(),
                || acc_value,
            )?;
            region.constrain_equal(final_acc.cell(), packed.cell())?;
            Ok(AssetOrchardAssignedSubpiece::unsound_unchecked(
                packed,
                bits.len(),
            ))
        },
    )
}

fn point_encoding_fields(point: pallas::Affine) -> Result<(pallas::Base, pallas::Base), Error> {
    let coordinates: Coordinates<pallas::Affine> =
        Option::from(point.coordinates()).ok_or(Error::Synthesis)?;
    let mut bytes = coordinates.x().to_repr();
    let encoded = point.to_bytes();
    let sign = (encoded[31] >> 7) & 1;
    bytes[31] &= 0x7f;
    let x = Option::<pallas::Base>::from(pallas::Base::from_repr(bytes)).ok_or(Error::Synthesis)?;
    Ok((x, pallas::Base::from(sign as u64)))
}

fn bit_from_field_value(value: Value<pallas::Base>, bit_index: usize) -> Value<pallas::Base> {
    value.map(|field| {
        let bytes = field.to_repr();
        let bit = (bytes[bit_index / 8] >> (bit_index % 8)) & 1;
        pallas::Base::from(bit as u64)
    })
}

fn two_pow(bit_index: usize) -> pallas::Base {
    let mut value = pallas::Base::ONE;
    for _ in 0..bit_index {
        value = value.double();
    }
    value
}

#[derive(Debug, Copy, Clone)]
enum HActionInputSource {
    Constant(pallas::Base),
    Public { value: pallas::Base, row: usize },
}

fn h_action_input_sources(
    inputs: [pallas::Base; ASSET_ORCHARD_H_ACTION_POSEIDON_INPUT_COUNT],
) -> [HActionInputSource; ASSET_ORCHARD_H_ACTION_POSEIDON_INPUT_COUNT] {
    [
        HActionInputSource::Constant(inputs[0]),
        HActionInputSource::Constant(inputs[1]),
        HActionInputSource::Constant(inputs[2]),
        HActionInputSource::Constant(inputs[3]),
        HActionInputSource::Constant(inputs[4]),
        HActionInputSource::Constant(inputs[5]),
        HActionInputSource::Constant(inputs[6]),
        HActionInputSource::Public {
            value: inputs[7],
            row: 0,
        },
        HActionInputSource::Public {
            value: inputs[8],
            row: 1,
        },
        HActionInputSource::Public {
            value: inputs[9],
            row: 2,
        },
        HActionInputSource::Public {
            value: inputs[10],
            row: 3,
        },
        HActionInputSource::Public {
            value: inputs[11],
            row: 4,
        },
        HActionInputSource::Public {
            value: inputs[12],
            row: 5,
        },
        HActionInputSource::Public {
            value: inputs[13],
            row: 6,
        },
        HActionInputSource::Public {
            value: inputs[14],
            row: 7,
        },
        HActionInputSource::Public {
            value: inputs[15],
            row: 8,
        },
        HActionInputSource::Public {
            value: inputs[16],
            row: 9,
        },
        HActionInputSource::Public {
            value: inputs[17],
            row: 10,
        },
        HActionInputSource::Public {
            value: inputs[18],
            row: 11,
        },
        HActionInputSource::Public {
            value: inputs[19],
            row: 12,
        },
        HActionInputSource::Public {
            value: inputs[20],
            row: 13,
        },
        HActionInputSource::Public {
            value: inputs[21],
            row: 14,
        },
        HActionInputSource::Public {
            value: inputs[22],
            row: 15,
        },
        HActionInputSource::Public {
            value: inputs[23],
            row: 16,
        },
        HActionInputSource::Public {
            value: inputs[24],
            row: 17,
        },
        HActionInputSource::Public {
            value: inputs[25],
            row: 18,
        },
        HActionInputSource::Public {
            value: inputs[26],
            row: 19,
        },
        HActionInputSource::Public {
            value: inputs[27],
            row: 20,
        },
        HActionInputSource::Public {
            value: inputs[28],
            row: 21,
        },
        HActionInputSource::Public {
            value: inputs[29],
            row: 22,
        },
        HActionInputSource::Public {
            value: inputs[30],
            row: 23,
        },
        HActionInputSource::Public {
            value: inputs[31],
            row: 24,
        },
        HActionInputSource::Public {
            value: inputs[32],
            row: 25,
        },
        HActionInputSource::Constant(inputs[33]),
    ]
}

fn private_egress_h_action_input_sources(
    inputs: [pallas::Base; ASSET_ORCHARD_PRIVATE_EGRESS_H_ACTION_POSEIDON_INPUT_COUNT],
) -> [HActionInputSource; ASSET_ORCHARD_PRIVATE_EGRESS_H_ACTION_POSEIDON_INPUT_COUNT] {
    [
        HActionInputSource::Constant(inputs[0]),
        HActionInputSource::Constant(inputs[1]),
        HActionInputSource::Constant(inputs[2]),
        HActionInputSource::Constant(inputs[3]),
        HActionInputSource::Constant(inputs[4]),
        HActionInputSource::Constant(inputs[5]),
        HActionInputSource::Constant(inputs[6]),
        HActionInputSource::Public {
            value: inputs[7],
            row: 0,
        },
        HActionInputSource::Public {
            value: inputs[8],
            row: 1,
        },
        HActionInputSource::Public {
            value: inputs[9],
            row: 2,
        },
        HActionInputSource::Public {
            value: inputs[10],
            row: 3,
        },
        HActionInputSource::Public {
            value: inputs[11],
            row: 4,
        },
        HActionInputSource::Public {
            value: inputs[12],
            row: 5,
        },
        HActionInputSource::Public {
            value: inputs[13],
            row: 6,
        },
        HActionInputSource::Public {
            value: inputs[14],
            row: 7,
        },
        HActionInputSource::Public {
            value: inputs[15],
            row: 8,
        },
        HActionInputSource::Public {
            value: inputs[16],
            row: 9,
        },
        HActionInputSource::Public {
            value: inputs[17],
            row: 10,
        },
    ]
}

fn synthesize_poseidon_sponge(
    layouter: &mut impl Layouter<pallas::Base>,
    config: &AssetOrchardSwapConservationConfig,
    label: &str,
    inputs: &[HActionInputSource],
) -> Result<HActionBindingCells, Error> {
    let (assigned, public_inputs) = layouter.assign_region(
        || format!("{label} inputs"),
        |mut region| {
            let mut assigned = Vec::with_capacity(inputs.len());
            let mut public_inputs = Vec::new();
            for (row, source) in inputs.iter().copied().enumerate() {
                let cell = match source {
                    HActionInputSource::Constant(value) => region.assign_advice_from_constant(
                        || "h_action constant input",
                        config.advice[0],
                        row,
                        value,
                    )?,
                    HActionInputSource::Public {
                        value,
                        row: instance_row,
                    } => {
                        let cell = region.assign_advice(
                            || "h_action public input",
                            config.advice[0],
                            row,
                            || Value::known(value),
                        )?;
                        public_inputs.push((cell.clone(), instance_row));
                        cell
                    }
                };
                assigned.push(cell);
            }
            Ok((assigned, public_inputs))
        },
    )?;
    let final_cells = pow5_sponge_outputs(layouter, config, label, assigned)?;
    Ok(HActionBindingCells {
        action_context: [final_cells[0].clone(), final_cells[1].clone()],
        public_inputs,
    })
}

fn invert_or_zero(value: pallas::Base) -> pallas::Base {
    Option::<pallas::Base>::from(value.invert()).unwrap_or(pallas::Base::ZERO)
}

include!("asset_orchard_legacy_circuits.rs");
include!("asset_orchard_action_builders.rs");
fn fixed_hex_array<const N: usize>(label: &str, value: &str) -> Result<[u8; N], AssetOrchardError> {
    let bytes = hex_to_bytes(value).map_err(|error| {
        AssetOrchardError::new(
            "invalid_asset_orchard_hex",
            format!("{label} is not valid hex: {error}"),
        )
    })?;
    if bytes.len() != N {
        return Err(AssetOrchardError::new(
            "invalid_asset_orchard_hex_len",
            format!("{label} must be {N} bytes, got {}", bytes.len()),
        ));
    }
    Ok(bytes.try_into().expect("checked fixed hex length"))
}

fn scalar_from_hex(label: &str, value: &str) -> Result<pallas::Scalar, AssetOrchardError> {
    let bytes = fixed_hex_array::<32>(label, value)?;
    Option::<pallas::Scalar>::from(pallas::Scalar::from_repr(bytes)).ok_or_else(|| {
        AssetOrchardError::new(
            "invalid_asset_orchard_scalar",
            format!("{label} is not a canonical Pallas scalar"),
        )
    })
}

impl AssetOrchardSwapPinnedMetadata {
    pub fn from_vk(
        vk: &halo2_proofs::plonk::VerifyingKey<vesta::Affine>,
        k: u32,
    ) -> Result<Self, AssetOrchardError> {
        Self::from_vk_for_circuit(vk, k, crate::asset_orchard::ASSET_ORCHARD_CIRCUIT_ID_V1)
    }

    fn from_vk_for_circuit(
        vk: &halo2_proofs::plonk::VerifyingKey<vesta::Affine>,
        k: u32,
        circuit_id: &'static str,
    ) -> Result<Self, AssetOrchardError> {
        crate::asset_orchard::supported_asset_orchard_swap_circuit_id(circuit_id)?;
        Ok(Self {
            circuit_id,
            k,
            proof_system_id: crate::asset_orchard::ASSET_ORCHARD_PROOF_SYSTEM_ID_V1,
            public_instance_len: ASSET_ORCHARD_PUBLIC_INSTANCE_LEN,
            public_instance_layout_hash: hash_public_instance_layout(),
            params_hash: hash_text(
                "asset_orchard_swap_params",
                &format!("halo2_ipa_params:k={k}"),
            ),
            vk_hash: asset_orchard_swap_vk_attestation_hash_for_circuit(circuit_id)?,
            poseidon_parameter_hash: hash_poseidon_parameters(),
            note_message_layout_hash: hash_note_message_layout(),
            merkle_tree_depth: ASSET_ORCHARD_MERKLE_DEPTH,
            merkle_parameter_hash: hash_text(
                "asset_orchard_merkle_parameters",
                &format!(
                    "depth={};hash_domain=orchard_merkle_crh;commit_r=asset_note_commit_r",
                    ASSET_ORCHARD_MERKLE_DEPTH
                ),
            ),
            runtime_pinned_vk_fingerprint: asset_orchard_swap_runtime_pinned_vk_fingerprint(vk),
        })
    }

    pub fn validate_release_pin(&self) -> Result<(), AssetOrchardError> {
        let (expected_vk_hash, expected_fingerprint) = match self.circuit_id {
            crate::asset_orchard::ASSET_ORCHARD_CIRCUIT_ID_V3_REPLAY => (
                ASSET_ORCHARD_SWAP_V3_REPLAY_VK_HASH,
                ASSET_ORCHARD_SWAP_V3_REPLAY_RUNTIME_PINNED_VK_FINGERPRINT,
            ),
            crate::asset_orchard::ASSET_ORCHARD_CIRCUIT_ID_V4 => (
                ASSET_ORCHARD_SWAP_V1_VK_HASH,
                ASSET_ORCHARD_SWAP_V1_RUNTIME_PINNED_VK_FINGERPRINT,
            ),
            _ => {
                return Err(AssetOrchardError::new(
                    "asset_orchard_swap_pinned_circuit_id_mismatch",
                    format!("unsupported pinned swap circuit id `{}`", self.circuit_id),
                ));
            }
        };
        let checks = [
            (
                "asset_orchard_swap_public_instance_layout_hash",
                self.public_instance_layout_hash.as_str(),
                ASSET_ORCHARD_SWAP_V1_PUBLIC_INSTANCE_LAYOUT_HASH,
            ),
            (
                "asset_orchard_swap_params_hash",
                self.params_hash.as_str(),
                ASSET_ORCHARD_SWAP_V1_PARAMS_HASH,
            ),
            (
                "asset_orchard_swap_vk_hash",
                self.vk_hash.as_str(),
                expected_vk_hash,
            ),
            (
                "asset_orchard_swap_poseidon_parameter_hash",
                self.poseidon_parameter_hash.as_str(),
                ASSET_ORCHARD_SWAP_V1_POSEIDON_PARAMETER_HASH,
            ),
            (
                "asset_orchard_swap_note_message_layout_hash",
                self.note_message_layout_hash.as_str(),
                ASSET_ORCHARD_SWAP_V1_NOTE_MESSAGE_LAYOUT_HASH,
            ),
            (
                "asset_orchard_swap_merkle_parameter_hash",
                self.merkle_parameter_hash.as_str(),
                ASSET_ORCHARD_SWAP_V1_MERKLE_PARAMETER_HASH,
            ),
            (
                "asset_orchard_swap_runtime_pinned_vk_fingerprint",
                self.runtime_pinned_vk_fingerprint.as_str(),
                expected_fingerprint,
            ),
        ];
        if self.k != ASSET_ORCHARD_SWAP_V1_K {
            return Err(AssetOrchardError::new(
                "asset_orchard_swap_pinned_k_mismatch",
                format!(
                    "AssetOrchard swap K {} does not match pinned {}",
                    self.k, ASSET_ORCHARD_SWAP_V1_K
                ),
            ));
        }
        if self.public_instance_len != ASSET_ORCHARD_PUBLIC_INSTANCE_LEN {
            return Err(AssetOrchardError::new(
                "asset_orchard_swap_pinned_public_instance_len_mismatch",
                format!(
                    "AssetOrchard swap public instance length {} does not match pinned {}",
                    self.public_instance_len, ASSET_ORCHARD_PUBLIC_INSTANCE_LEN
                ),
            ));
        }
        if self.merkle_tree_depth != ASSET_ORCHARD_MERKLE_DEPTH {
            return Err(AssetOrchardError::new(
                "asset_orchard_swap_pinned_merkle_depth_mismatch",
                format!(
                    "AssetOrchard swap Merkle depth {} does not match pinned {}",
                    self.merkle_tree_depth, ASSET_ORCHARD_MERKLE_DEPTH
                ),
            ));
        }
        for (label, actual, expected) in checks {
            if actual != expected {
                return Err(AssetOrchardError::new(
                    "asset_orchard_swap_pinned_metadata_mismatch",
                    format!("{label} {actual} does not match pinned {expected}"),
                ));
            }
        }
        Ok(())
    }
}

pub fn asset_orchard_swap_vk_attestation_bytes() -> &'static [u8] {
    ASSET_ORCHARD_SWAP_V1_VK_ATTESTATION.as_bytes()
}

#[cfg(test)]
fn asset_orchard_swap_vk_attestation_hash() -> String {
    asset_orchard_swap_vk_attestation_hash_for_circuit(
        crate::asset_orchard::ASSET_ORCHARD_CIRCUIT_ID_V1,
    )
    .expect("active swap circuit id is supported")
}

fn asset_orchard_swap_vk_attestation_hash_for_circuit(
    circuit_id: &str,
) -> Result<String, AssetOrchardError> {
    let attestation = match circuit_id {
        crate::asset_orchard::ASSET_ORCHARD_CIRCUIT_ID_V3_REPLAY => {
            ASSET_ORCHARD_SWAP_V3_REPLAY_VK_ATTESTATION.as_bytes()
        }
        crate::asset_orchard::ASSET_ORCHARD_CIRCUIT_ID_V4 => {
            ASSET_ORCHARD_SWAP_V1_VK_ATTESTATION.as_bytes()
        }
        _ => {
            return Err(AssetOrchardError::new(
                "unsupported_asset_orchard_circuit",
                format!("unsupported asset-orchard circuit `{circuit_id}`"),
            ));
        }
    };
    Ok(hash_bytes("asset_orchard_swap_vk_attestation", attestation))
}

fn asset_orchard_swap_runtime_pinned_vk_fingerprint(
    vk: &halo2_proofs::plonk::VerifyingKey<vesta::Affine>,
) -> String {
    let pinned = format!("{:?}", vk.pinned());
    hash_bytes("asset_orchard_swap_vk", pinned.as_bytes())
}

fn record_swap_vk_build_timing_result(
    mut timing: AssetOrchardSwapVkBuildTimingReport,
    total_start: std::time::Instant,
    result: &str,
) {
    timing.total_ms = asset_orchard_timing_elapsed_ms(total_start);
    timing.result = result.to_string();
    record_asset_orchard_swap_vk_build_timing(timing);
}

fn swap_vk_artifact_load_path() -> Option<PathBuf> {
    #[cfg(not(feature = "asset-orchard-vk-dev-env"))]
    {
        None
    }
    #[cfg(feature = "asset-orchard-vk-dev-env")]
    {
        env::var(ASSET_ORCHARD_SWAP_VK_ARTIFACT_LOAD_ENV)
            .ok()
            .map(|path| path.trim().to_string())
            .filter(|path| !path.is_empty())
            .map(PathBuf::from)
    }
}

fn swap_vk_artifact_write_path() -> Option<PathBuf> {
    #[cfg(not(feature = "asset-orchard-vk-dev-env"))]
    {
        None
    }
    #[cfg(feature = "asset-orchard-vk-dev-env")]
    {
        env::var(ASSET_ORCHARD_SWAP_VK_ARTIFACT_WRITE_ENV)
            .ok()
            .map(|path| path.trim().to_string())
            .filter(|path| !path.is_empty())
            .map(PathBuf::from)
    }
}

fn swap_vk_rebuild_requested() -> bool {
    #[cfg(not(feature = "asset-orchard-vk-dev-env"))]
    {
        false
    }
    #[cfg(feature = "asset-orchard-vk-dev-env")]
    {
        env::var(ASSET_ORCHARD_SWAP_VK_REBUILD_ENV)
            .ok()
            .map(|value| {
                let value = value.trim();
                value == "1" || value.eq_ignore_ascii_case("true")
            })
            .unwrap_or(false)
    }
}

fn read_swap_vk_artifact_bytes(path: &Path) -> Result<Vec<u8>, AssetOrchardError> {
    let metadata = fs::metadata(path).map_err(|error| {
        AssetOrchardError::new(
            "asset_orchard_swap_vk_artifact_read_failed",
            format!("{}: {error}", path.display()),
        )
    })?;
    if metadata.len() > ASSET_ORCHARD_SWAP_VK_ARTIFACT_MAX_BYTES {
        return Err(AssetOrchardError::new(
            "asset_orchard_swap_vk_artifact_too_large",
            format!(
                "{} is {} bytes; max is {}",
                path.display(),
                metadata.len(),
                ASSET_ORCHARD_SWAP_VK_ARTIFACT_MAX_BYTES
            ),
        ));
    }
    fs::read(path).map_err(|error| {
        AssetOrchardError::new(
            "asset_orchard_swap_vk_artifact_read_failed",
            format!("{}: {error}", path.display()),
        )
    })
}

fn decode_swap_vk_artifact(
    bytes: &[u8],
) -> Result<VerifyingKeyPinnedAssembly<vesta::Affine>, AssetOrchardError> {
    decode_swap_vk_artifact_for_circuit(bytes, crate::asset_orchard::ASSET_ORCHARD_CIRCUIT_ID_V1)
}

fn decode_swap_vk_artifact_for_circuit(
    bytes: &[u8],
    circuit_id: &'static str,
) -> Result<VerifyingKeyPinnedAssembly<vesta::Affine>, AssetOrchardError> {
    if bytes.len() as u64 > ASSET_ORCHARD_SWAP_VK_ARTIFACT_MAX_BYTES {
        return Err(AssetOrchardError::new(
            "asset_orchard_swap_vk_artifact_too_large",
            format!(
                "artifact is {} bytes; max is {}",
                bytes.len(),
                ASSET_ORCHARD_SWAP_VK_ARTIFACT_MAX_BYTES
            ),
        ));
    }
    let header_end = bytes
        .windows(2)
        .position(|window| window == b"\n\n")
        .ok_or_else(|| {
            AssetOrchardError::new(
                "asset_orchard_swap_vk_artifact_malformed",
                "artifact header terminator not found",
            )
        })?;
    let header = std::str::from_utf8(&bytes[..header_end]).map_err(|error| {
        AssetOrchardError::new(
            "asset_orchard_swap_vk_artifact_malformed",
            format!("artifact header is not utf8: {error}"),
        )
    })?;
    let payload = &bytes[header_end + 2..];
    validate_swap_vk_artifact_header(header, payload, circuit_id)?;

    let mut cursor = Cursor::new(payload);
    let assembly = VerifyingKeyPinnedAssembly::read_with_limits(
        &mut cursor,
        VerifyingKeyPinnedAssemblyLimits {
            max_fixed_commitments: 8192,
            max_permutation_commitments: 8192,
            max_selectors: 8192,
            max_selector_rows: 1usize << ASSET_ORCHARD_SWAP_V1_K,
        },
    )
    .map_err(|error| {
        AssetOrchardError::new(
            "asset_orchard_swap_vk_artifact_malformed",
            error.to_string(),
        )
    })?;
    if cursor.position() != payload.len() as u64 {
        return Err(AssetOrchardError::new(
            "asset_orchard_swap_vk_artifact_malformed",
            "artifact payload has trailing bytes",
        ));
    }
    Ok(assembly)
}

fn validate_swap_vk_artifact_header(
    header: &str,
    payload: &[u8],
    circuit_id: &'static str,
) -> Result<(), AssetOrchardError> {
    let (expected_vk_hash, expected_fingerprint) = match circuit_id {
        crate::asset_orchard::ASSET_ORCHARD_CIRCUIT_ID_V3_REPLAY => (
            ASSET_ORCHARD_SWAP_V3_REPLAY_VK_HASH,
            ASSET_ORCHARD_SWAP_V3_REPLAY_RUNTIME_PINNED_VK_FINGERPRINT,
        ),
        crate::asset_orchard::ASSET_ORCHARD_CIRCUIT_ID_V4 => (
            ASSET_ORCHARD_SWAP_V1_VK_HASH,
            ASSET_ORCHARD_SWAP_V1_RUNTIME_PINNED_VK_FINGERPRINT,
        ),
        _ => {
            return Err(AssetOrchardError::new(
                "unsupported_asset_orchard_circuit",
                format!("unsupported asset-orchard circuit `{circuit_id}`"),
            ));
        }
    };
    let mut lines = header.lines();
    let schema = lines.next().ok_or_else(|| {
        AssetOrchardError::new(
            "asset_orchard_swap_vk_artifact_malformed",
            "artifact header is empty",
        )
    })?;
    if schema != ASSET_ORCHARD_SWAP_VK_ARTIFACT_SCHEMA_V1 {
        return Err(AssetOrchardError::new(
            "asset_orchard_swap_vk_artifact_schema_mismatch",
            format!(
                "artifact schema {schema} does not match {}",
                ASSET_ORCHARD_SWAP_VK_ARTIFACT_SCHEMA_V1
            ),
        ));
    }

    let mut fields = BTreeMap::new();
    for line in lines {
        let (key, value) = line.split_once('=').ok_or_else(|| {
            AssetOrchardError::new(
                "asset_orchard_swap_vk_artifact_malformed",
                format!("artifact header line lacks key=value: {line}"),
            )
        })?;
        if fields.insert(key.to_string(), value.to_string()).is_some() {
            return Err(AssetOrchardError::new(
                "asset_orchard_swap_vk_artifact_malformed",
                format!("duplicate artifact header key: {key}"),
            ));
        }
    }

    let expected_fields = [
        ("halo2_proofs", "0.3.2"),
        ("curve", "vesta"),
        (
            "proof_system",
            crate::asset_orchard::ASSET_ORCHARD_PROOF_SYSTEM_ID_V1,
        ),
        ("circuit_id", circuit_id),
        ("k", "15"),
        ("public_instance_len", "28"),
        (
            "public_instance_layout_hash",
            ASSET_ORCHARD_SWAP_V1_PUBLIC_INSTANCE_LAYOUT_HASH,
        ),
        ("params_hash", ASSET_ORCHARD_SWAP_V1_PARAMS_HASH),
        ("vk_hash", expected_vk_hash),
        (
            "poseidon_parameter_hash",
            ASSET_ORCHARD_SWAP_V1_POSEIDON_PARAMETER_HASH,
        ),
        (
            "note_message_layout_hash",
            ASSET_ORCHARD_SWAP_V1_NOTE_MESSAGE_LAYOUT_HASH,
        ),
        ("merkle_tree_depth", "32"),
        (
            "merkle_parameter_hash",
            ASSET_ORCHARD_SWAP_V1_MERKLE_PARAMETER_HASH,
        ),
        ("runtime_pinned_vk_fingerprint", expected_fingerprint),
    ];
    for (key, expected) in expected_fields {
        let actual = fields.get(key).ok_or_else(|| {
            AssetOrchardError::new(
                "asset_orchard_swap_vk_artifact_malformed",
                format!("missing artifact header key: {key}"),
            )
        })?;
        if actual != expected {
            return Err(AssetOrchardError::new(
                "asset_orchard_swap_vk_artifact_metadata_mismatch",
                format!("{key} {actual} does not match pinned {expected}"),
            ));
        }
    }

    let payload_hash = hash_bytes("asset_orchard_swap_vk_pinned_assembly_payload", payload);
    let payload_checks = [
        ("payload_len", payload.len().to_string()),
        ("payload_sha3_384", payload_hash),
    ];
    for (key, expected) in payload_checks {
        let actual = fields.get(key).ok_or_else(|| {
            AssetOrchardError::new(
                "asset_orchard_swap_vk_artifact_malformed",
                format!("missing artifact header key: {key}"),
            )
        })?;
        if actual != &expected {
            return Err(AssetOrchardError::new(
                "asset_orchard_swap_vk_artifact_payload_mismatch",
                format!("{key} {actual} does not match expected {expected}"),
            ));
        }
    }
    Ok(())
}

fn write_swap_vk_artifact(
    path: &Path,
    assembly: &VerifyingKeyPinnedAssembly<vesta::Affine>,
    metadata: &AssetOrchardSwapPinnedMetadata,
) -> Result<(), AssetOrchardError> {
    metadata.validate_release_pin()?;
    let mut payload = Vec::new();
    assembly.write(&mut payload).map_err(|error| {
        AssetOrchardError::new(
            "asset_orchard_swap_vk_artifact_write_failed",
            error.to_string(),
        )
    })?;
    let payload_hash = hash_bytes("asset_orchard_swap_vk_pinned_assembly_payload", &payload);
    let header = format!(
        "{schema}\n\
halo2_proofs=0.3.2\n\
curve=vesta\n\
proof_system={proof_system}\n\
circuit_id={circuit_id}\n\
k={k}\n\
public_instance_len={public_instance_len}\n\
public_instance_layout_hash={public_instance_layout_hash}\n\
params_hash={params_hash}\n\
vk_hash={vk_hash}\n\
poseidon_parameter_hash={poseidon_parameter_hash}\n\
note_message_layout_hash={note_message_layout_hash}\n\
merkle_tree_depth={merkle_tree_depth}\n\
merkle_parameter_hash={merkle_parameter_hash}\n\
runtime_pinned_vk_fingerprint={runtime_pinned_vk_fingerprint}\n\
payload_len={payload_len}\n\
payload_sha3_384={payload_hash}\n\n",
        schema = ASSET_ORCHARD_SWAP_VK_ARTIFACT_SCHEMA_V1,
        proof_system = metadata.proof_system_id,
        circuit_id = metadata.circuit_id,
        k = metadata.k,
        public_instance_len = metadata.public_instance_len,
        public_instance_layout_hash = metadata.public_instance_layout_hash,
        params_hash = metadata.params_hash,
        vk_hash = metadata.vk_hash,
        poseidon_parameter_hash = metadata.poseidon_parameter_hash,
        note_message_layout_hash = metadata.note_message_layout_hash,
        merkle_tree_depth = metadata.merkle_tree_depth,
        merkle_parameter_hash = metadata.merkle_parameter_hash,
        runtime_pinned_vk_fingerprint = metadata.runtime_pinned_vk_fingerprint,
        payload_len = payload.len(),
        payload_hash = payload_hash,
    );

    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).map_err(|error| {
                AssetOrchardError::new(
                    "asset_orchard_swap_vk_artifact_write_failed",
                    format!("{}: {error}", parent.display()),
                )
            })?;
        }
    }
    let mut bytes = header.into_bytes();
    bytes.extend_from_slice(&payload);
    let tmp_path = path.with_extension("tmp");
    fs::write(&tmp_path, bytes).map_err(|error| {
        AssetOrchardError::new(
            "asset_orchard_swap_vk_artifact_write_failed",
            format!("{}: {error}", tmp_path.display()),
        )
    })?;
    fs::rename(&tmp_path, path).map_err(|error| {
        AssetOrchardError::new(
            "asset_orchard_swap_vk_artifact_write_failed",
            format!("{} -> {}: {error}", tmp_path.display(), path.display()),
        )
    })
}

impl AssetOrchardPrivateEgressPinnedMetadata {
    pub fn from_vk(
        vk: &halo2_proofs::plonk::VerifyingKey<vesta::Affine>,
        k: u32,
    ) -> Result<Self, AssetOrchardError> {
        Self::from_vk_for_circuit(
            vk,
            k,
            crate::asset_orchard::ASSET_ORCHARD_PRIVATE_EGRESS_CIRCUIT_ID_V1,
        )
    }

    fn from_vk_for_circuit(
        vk: &halo2_proofs::plonk::VerifyingKey<vesta::Affine>,
        k: u32,
        circuit_id: &'static str,
    ) -> Result<Self, AssetOrchardError> {
        crate::asset_orchard::supported_asset_orchard_private_egress_circuit_id(circuit_id)?;
        Ok(Self {
            circuit_id,
            k,
            proof_system_id: crate::asset_orchard::ASSET_ORCHARD_PROOF_SYSTEM_ID_V1,
            public_instance_len: ASSET_ORCHARD_PRIVATE_EGRESS_PUBLIC_INSTANCE_LEN,
            public_instance_layout_hash: hash_private_egress_public_instance_layout(),
            params_hash: hash_text(
                "asset_orchard_private_egress_params",
                &format!("halo2_ipa_params:k={k}"),
            ),
            vk_hash: asset_orchard_private_egress_vk_attestation_hash_for_circuit(circuit_id)?,
            poseidon_parameter_hash: hash_poseidon_parameters(),
            note_message_layout_hash: hash_note_message_layout(),
            merkle_tree_depth: ASSET_ORCHARD_MERKLE_DEPTH,
            merkle_parameter_hash: hash_text(
                "asset_orchard_merkle_parameters",
                &format!(
                    "depth={};hash_domain=orchard_merkle_crh;commit_r=asset_note_commit_r",
                    ASSET_ORCHARD_MERKLE_DEPTH
                ),
            ),
            runtime_pinned_vk_fingerprint:
                asset_orchard_private_egress_runtime_pinned_vk_fingerprint(vk),
        })
    }

    pub fn validate_release_pin(&self) -> Result<(), AssetOrchardError> {
        let (expected_vk_hash, expected_fingerprint) = match self.circuit_id {
            crate::asset_orchard::ASSET_ORCHARD_PRIVATE_EGRESS_CIRCUIT_ID_V1_REPLAY => (
                ASSET_ORCHARD_PRIVATE_EGRESS_V1_REPLAY_VK_HASH,
                ASSET_ORCHARD_PRIVATE_EGRESS_V1_REPLAY_RUNTIME_PINNED_VK_FINGERPRINT,
            ),
            crate::asset_orchard::ASSET_ORCHARD_PRIVATE_EGRESS_CIRCUIT_ID_V2 => (
                ASSET_ORCHARD_PRIVATE_EGRESS_V1_VK_HASH,
                ASSET_ORCHARD_PRIVATE_EGRESS_V1_RUNTIME_PINNED_VK_FINGERPRINT,
            ),
            _ => {
                return Err(AssetOrchardError::new(
                    "asset_orchard_private_egress_pinned_circuit_id_mismatch",
                    format!(
                        "unsupported pinned private-egress circuit id `{}`",
                        self.circuit_id
                    ),
                ));
            }
        };
        let checks = [
            (
                "asset_orchard_private_egress_public_instance_layout_hash",
                self.public_instance_layout_hash.as_str(),
                ASSET_ORCHARD_PRIVATE_EGRESS_V1_PUBLIC_INSTANCE_LAYOUT_HASH,
            ),
            (
                "asset_orchard_private_egress_params_hash",
                self.params_hash.as_str(),
                ASSET_ORCHARD_PRIVATE_EGRESS_V1_PARAMS_HASH,
            ),
            (
                "asset_orchard_private_egress_vk_hash",
                self.vk_hash.as_str(),
                expected_vk_hash,
            ),
            (
                "asset_orchard_private_egress_poseidon_parameter_hash",
                self.poseidon_parameter_hash.as_str(),
                ASSET_ORCHARD_PRIVATE_EGRESS_V1_POSEIDON_PARAMETER_HASH,
            ),
            (
                "asset_orchard_private_egress_note_message_layout_hash",
                self.note_message_layout_hash.as_str(),
                ASSET_ORCHARD_PRIVATE_EGRESS_V1_NOTE_MESSAGE_LAYOUT_HASH,
            ),
            (
                "asset_orchard_private_egress_merkle_parameter_hash",
                self.merkle_parameter_hash.as_str(),
                ASSET_ORCHARD_PRIVATE_EGRESS_V1_MERKLE_PARAMETER_HASH,
            ),
            (
                "asset_orchard_private_egress_runtime_pinned_vk_fingerprint",
                self.runtime_pinned_vk_fingerprint.as_str(),
                expected_fingerprint,
            ),
        ];
        if self.k != ASSET_ORCHARD_PRIVATE_EGRESS_V1_K {
            return Err(AssetOrchardError::new(
                "asset_orchard_private_egress_pinned_k_mismatch",
                format!(
                    "AssetOrchard private egress K {} does not match pinned {}",
                    self.k, ASSET_ORCHARD_PRIVATE_EGRESS_V1_K
                ),
            ));
        }
        if self.public_instance_len != ASSET_ORCHARD_PRIVATE_EGRESS_PUBLIC_INSTANCE_LEN {
            return Err(AssetOrchardError::new(
                "asset_orchard_private_egress_pinned_public_instance_len_mismatch",
                format!(
                    "AssetOrchard private egress public instance length {} does not match pinned {}",
                    self.public_instance_len, ASSET_ORCHARD_PRIVATE_EGRESS_PUBLIC_INSTANCE_LEN
                ),
            ));
        }
        if self.merkle_tree_depth != ASSET_ORCHARD_MERKLE_DEPTH {
            return Err(AssetOrchardError::new(
                "asset_orchard_private_egress_pinned_merkle_depth_mismatch",
                format!(
                    "AssetOrchard private egress Merkle depth {} does not match pinned {}",
                    self.merkle_tree_depth, ASSET_ORCHARD_MERKLE_DEPTH
                ),
            ));
        }
        for (label, actual, expected) in checks {
            if actual != expected {
                return Err(AssetOrchardError::new(
                    "asset_orchard_private_egress_pinned_metadata_mismatch",
                    format!("{label} {actual} does not match pinned {expected}"),
                ));
            }
        }
        Ok(())
    }
}

pub fn asset_orchard_private_egress_vk_attestation_bytes() -> &'static [u8] {
    ASSET_ORCHARD_PRIVATE_EGRESS_V1_VK_ATTESTATION.as_bytes()
}

#[cfg(test)]
fn asset_orchard_private_egress_vk_attestation_hash() -> String {
    asset_orchard_private_egress_vk_attestation_hash_for_circuit(
        crate::asset_orchard::ASSET_ORCHARD_PRIVATE_EGRESS_CIRCUIT_ID_V1,
    )
    .expect("active private-egress circuit id is supported")
}

fn asset_orchard_private_egress_vk_attestation_hash_for_circuit(
    circuit_id: &str,
) -> Result<String, AssetOrchardError> {
    let attestation = match circuit_id {
        crate::asset_orchard::ASSET_ORCHARD_PRIVATE_EGRESS_CIRCUIT_ID_V1_REPLAY => {
            ASSET_ORCHARD_PRIVATE_EGRESS_V1_REPLAY_VK_ATTESTATION.as_bytes()
        }
        crate::asset_orchard::ASSET_ORCHARD_PRIVATE_EGRESS_CIRCUIT_ID_V2 => {
            ASSET_ORCHARD_PRIVATE_EGRESS_V1_VK_ATTESTATION.as_bytes()
        }
        _ => {
            return Err(AssetOrchardError::new(
                "unsupported_asset_orchard_private_egress_circuit",
                format!("unsupported asset-orchard private egress circuit `{circuit_id}`"),
            ));
        }
    };
    Ok(hash_bytes(
        "asset_orchard_private_egress_vk_attestation",
        attestation,
    ))
}

fn record_private_egress_vk_build_timing_result(
    mut timing: AssetOrchardPrivateEgressVkBuildTimingReport,
    total_start: std::time::Instant,
    result: &str,
) {
    timing.total_ms = asset_orchard_timing_elapsed_ms(total_start);
    timing.result = result.to_string();
    record_asset_orchard_private_egress_vk_build_timing(timing);
}

fn private_egress_vk_artifact_load_path() -> Option<PathBuf> {
    #[cfg(not(feature = "asset-orchard-vk-dev-env"))]
    {
        None
    }
    #[cfg(feature = "asset-orchard-vk-dev-env")]
    {
        env::var(ASSET_ORCHARD_PRIVATE_EGRESS_VK_ARTIFACT_LOAD_ENV)
            .ok()
            .map(|path| path.trim().to_string())
            .filter(|path| !path.is_empty())
            .map(PathBuf::from)
    }
}

fn private_egress_vk_artifact_write_path() -> Option<PathBuf> {
    #[cfg(not(feature = "asset-orchard-vk-dev-env"))]
    {
        None
    }
    #[cfg(feature = "asset-orchard-vk-dev-env")]
    {
        env::var(ASSET_ORCHARD_PRIVATE_EGRESS_VK_ARTIFACT_WRITE_ENV)
            .ok()
            .map(|path| path.trim().to_string())
            .filter(|path| !path.is_empty())
            .map(PathBuf::from)
    }
}

fn private_egress_vk_rebuild_requested() -> bool {
    #[cfg(not(feature = "asset-orchard-vk-dev-env"))]
    {
        false
    }
    #[cfg(feature = "asset-orchard-vk-dev-env")]
    {
        env::var(ASSET_ORCHARD_PRIVATE_EGRESS_VK_REBUILD_ENV)
            .ok()
            .map(|value| {
                let value = value.trim();
                value == "1" || value.eq_ignore_ascii_case("true")
            })
            .unwrap_or(false)
    }
}

fn read_private_egress_vk_artifact_bytes(path: &Path) -> Result<Vec<u8>, AssetOrchardError> {
    let metadata = fs::metadata(path).map_err(|error| {
        AssetOrchardError::new(
            "asset_orchard_private_egress_vk_artifact_read_failed",
            format!("{}: {error}", path.display()),
        )
    })?;
    if metadata.len() > ASSET_ORCHARD_PRIVATE_EGRESS_VK_ARTIFACT_MAX_BYTES {
        return Err(AssetOrchardError::new(
            "asset_orchard_private_egress_vk_artifact_too_large",
            format!(
                "{} is {} bytes; max is {}",
                path.display(),
                metadata.len(),
                ASSET_ORCHARD_PRIVATE_EGRESS_VK_ARTIFACT_MAX_BYTES
            ),
        ));
    }
    fs::read(path).map_err(|error| {
        AssetOrchardError::new(
            "asset_orchard_private_egress_vk_artifact_read_failed",
            format!("{}: {error}", path.display()),
        )
    })
}

fn decode_private_egress_vk_artifact(
    bytes: &[u8],
) -> Result<VerifyingKeyPinnedAssembly<vesta::Affine>, AssetOrchardError> {
    decode_private_egress_vk_artifact_for_circuit(
        bytes,
        crate::asset_orchard::ASSET_ORCHARD_PRIVATE_EGRESS_CIRCUIT_ID_V1,
    )
}

fn decode_private_egress_vk_artifact_for_circuit(
    bytes: &[u8],
    circuit_id: &'static str,
) -> Result<VerifyingKeyPinnedAssembly<vesta::Affine>, AssetOrchardError> {
    if bytes.len() as u64 > ASSET_ORCHARD_PRIVATE_EGRESS_VK_ARTIFACT_MAX_BYTES {
        return Err(AssetOrchardError::new(
            "asset_orchard_private_egress_vk_artifact_too_large",
            format!(
                "artifact is {} bytes; max is {}",
                bytes.len(),
                ASSET_ORCHARD_PRIVATE_EGRESS_VK_ARTIFACT_MAX_BYTES
            ),
        ));
    }
    let header_end = bytes
        .windows(2)
        .position(|window| window == b"\n\n")
        .ok_or_else(|| {
            AssetOrchardError::new(
                "asset_orchard_private_egress_vk_artifact_malformed",
                "artifact header terminator not found",
            )
        })?;
    let header = std::str::from_utf8(&bytes[..header_end]).map_err(|error| {
        AssetOrchardError::new(
            "asset_orchard_private_egress_vk_artifact_malformed",
            format!("artifact header is not utf8: {error}"),
        )
    })?;
    let payload = &bytes[header_end + 2..];
    validate_private_egress_vk_artifact_header(header, payload, circuit_id)?;

    let mut cursor = Cursor::new(payload);
    let assembly = VerifyingKeyPinnedAssembly::read_with_limits(
        &mut cursor,
        VerifyingKeyPinnedAssemblyLimits {
            max_fixed_commitments: 8192,
            max_permutation_commitments: 8192,
            max_selectors: 8192,
            max_selector_rows: 1usize << ASSET_ORCHARD_PRIVATE_EGRESS_V1_K,
        },
    )
    .map_err(|error| {
        AssetOrchardError::new(
            "asset_orchard_private_egress_vk_artifact_malformed",
            error.to_string(),
        )
    })?;
    if cursor.position() != payload.len() as u64 {
        return Err(AssetOrchardError::new(
            "asset_orchard_private_egress_vk_artifact_malformed",
            "artifact payload has trailing bytes",
        ));
    }
    Ok(assembly)
}

fn validate_private_egress_vk_artifact_header(
    header: &str,
    payload: &[u8],
    circuit_id: &'static str,
) -> Result<(), AssetOrchardError> {
    let (expected_vk_hash, expected_fingerprint) = match circuit_id {
        crate::asset_orchard::ASSET_ORCHARD_PRIVATE_EGRESS_CIRCUIT_ID_V1_REPLAY => (
            ASSET_ORCHARD_PRIVATE_EGRESS_V1_REPLAY_VK_HASH,
            ASSET_ORCHARD_PRIVATE_EGRESS_V1_REPLAY_RUNTIME_PINNED_VK_FINGERPRINT,
        ),
        crate::asset_orchard::ASSET_ORCHARD_PRIVATE_EGRESS_CIRCUIT_ID_V2 => (
            ASSET_ORCHARD_PRIVATE_EGRESS_V1_VK_HASH,
            ASSET_ORCHARD_PRIVATE_EGRESS_V1_RUNTIME_PINNED_VK_FINGERPRINT,
        ),
        _ => {
            return Err(AssetOrchardError::new(
                "unsupported_asset_orchard_private_egress_circuit",
                format!("unsupported asset-orchard private egress circuit `{circuit_id}`"),
            ));
        }
    };
    let mut lines = header.lines();
    let schema = lines.next().ok_or_else(|| {
        AssetOrchardError::new(
            "asset_orchard_private_egress_vk_artifact_malformed",
            "artifact header is empty",
        )
    })?;
    if schema != ASSET_ORCHARD_PRIVATE_EGRESS_VK_ARTIFACT_SCHEMA_V1 {
        return Err(AssetOrchardError::new(
            "asset_orchard_private_egress_vk_artifact_schema_mismatch",
            format!(
                "artifact schema {schema} does not match {}",
                ASSET_ORCHARD_PRIVATE_EGRESS_VK_ARTIFACT_SCHEMA_V1
            ),
        ));
    }

    let mut fields = BTreeMap::new();
    for line in lines {
        let (key, value) = line.split_once('=').ok_or_else(|| {
            AssetOrchardError::new(
                "asset_orchard_private_egress_vk_artifact_malformed",
                format!("artifact header line lacks key=value: {line}"),
            )
        })?;
        if fields.insert(key.to_string(), value.to_string()).is_some() {
            return Err(AssetOrchardError::new(
                "asset_orchard_private_egress_vk_artifact_malformed",
                format!("duplicate artifact header key: {key}"),
            ));
        }
    }

    let expected_fields = [
        ("halo2_proofs", "0.3.2"),
        ("curve", "vesta"),
        (
            "proof_system",
            crate::asset_orchard::ASSET_ORCHARD_PROOF_SYSTEM_ID_V1,
        ),
        ("circuit_id", circuit_id),
        ("k", "15"),
        ("public_instance_len", "13"),
        (
            "public_instance_layout_hash",
            ASSET_ORCHARD_PRIVATE_EGRESS_V1_PUBLIC_INSTANCE_LAYOUT_HASH,
        ),
        ("params_hash", ASSET_ORCHARD_PRIVATE_EGRESS_V1_PARAMS_HASH),
        ("vk_hash", expected_vk_hash),
        (
            "poseidon_parameter_hash",
            ASSET_ORCHARD_PRIVATE_EGRESS_V1_POSEIDON_PARAMETER_HASH,
        ),
        (
            "note_message_layout_hash",
            ASSET_ORCHARD_PRIVATE_EGRESS_V1_NOTE_MESSAGE_LAYOUT_HASH,
        ),
        ("merkle_tree_depth", "32"),
        (
            "merkle_parameter_hash",
            ASSET_ORCHARD_PRIVATE_EGRESS_V1_MERKLE_PARAMETER_HASH,
        ),
        ("runtime_pinned_vk_fingerprint", expected_fingerprint),
    ];
    for (key, expected) in expected_fields {
        let actual = fields.get(key).ok_or_else(|| {
            AssetOrchardError::new(
                "asset_orchard_private_egress_vk_artifact_malformed",
                format!("missing artifact header key: {key}"),
            )
        })?;
        if actual != expected {
            return Err(AssetOrchardError::new(
                "asset_orchard_private_egress_vk_artifact_metadata_mismatch",
                format!("{key} {actual} does not match pinned {expected}"),
            ));
        }
    }

    let payload_hash = hash_bytes(
        "asset_orchard_private_egress_vk_pinned_assembly_payload",
        payload,
    );
    let payload_checks = [
        ("payload_len", payload.len().to_string()),
        ("payload_sha3_384", payload_hash),
    ];
    for (key, expected) in payload_checks {
        let actual = fields.get(key).ok_or_else(|| {
            AssetOrchardError::new(
                "asset_orchard_private_egress_vk_artifact_malformed",
                format!("missing artifact header key: {key}"),
            )
        })?;
        if actual != &expected {
            return Err(AssetOrchardError::new(
                "asset_orchard_private_egress_vk_artifact_payload_mismatch",
                format!("{key} {actual} does not match expected {expected}"),
            ));
        }
    }

    Ok(())
}

fn write_private_egress_vk_artifact(
    path: &Path,
    assembly: &VerifyingKeyPinnedAssembly<vesta::Affine>,
    metadata: &AssetOrchardPrivateEgressPinnedMetadata,
) -> Result<(), AssetOrchardError> {
    metadata.validate_release_pin()?;

    let mut payload = Vec::new();
    assembly.write(&mut payload).map_err(|error| {
        AssetOrchardError::new(
            "asset_orchard_private_egress_vk_artifact_write_failed",
            error.to_string(),
        )
    })?;
    let payload_hash = hash_bytes(
        "asset_orchard_private_egress_vk_pinned_assembly_payload",
        &payload,
    );
    let header = format!(
        "{schema}\n\
halo2_proofs=0.3.2\n\
curve=vesta\n\
proof_system={proof_system}\n\
circuit_id={circuit_id}\n\
k={k}\n\
public_instance_len={public_instance_len}\n\
public_instance_layout_hash={public_instance_layout_hash}\n\
params_hash={params_hash}\n\
vk_hash={vk_hash}\n\
poseidon_parameter_hash={poseidon_parameter_hash}\n\
note_message_layout_hash={note_message_layout_hash}\n\
merkle_tree_depth={merkle_tree_depth}\n\
merkle_parameter_hash={merkle_parameter_hash}\n\
runtime_pinned_vk_fingerprint={runtime_pinned_vk_fingerprint}\n\
payload_len={payload_len}\n\
payload_sha3_384={payload_hash}\n\n",
        schema = ASSET_ORCHARD_PRIVATE_EGRESS_VK_ARTIFACT_SCHEMA_V1,
        proof_system = metadata.proof_system_id,
        circuit_id = metadata.circuit_id,
        k = metadata.k,
        public_instance_len = metadata.public_instance_len,
        public_instance_layout_hash = metadata.public_instance_layout_hash,
        params_hash = metadata.params_hash,
        vk_hash = metadata.vk_hash,
        poseidon_parameter_hash = metadata.poseidon_parameter_hash,
        note_message_layout_hash = metadata.note_message_layout_hash,
        merkle_tree_depth = metadata.merkle_tree_depth,
        merkle_parameter_hash = metadata.merkle_parameter_hash,
        runtime_pinned_vk_fingerprint = metadata.runtime_pinned_vk_fingerprint,
        payload_len = payload.len(),
        payload_hash = payload_hash,
    );

    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).map_err(|error| {
                AssetOrchardError::new(
                    "asset_orchard_private_egress_vk_artifact_write_failed",
                    format!("{}: {error}", parent.display()),
                )
            })?;
        }
    }

    let mut bytes = header.into_bytes();
    bytes.extend_from_slice(&payload);
    let tmp_path = path.with_extension("tmp");
    fs::write(&tmp_path, bytes).map_err(|error| {
        AssetOrchardError::new(
            "asset_orchard_private_egress_vk_artifact_write_failed",
            format!("{}: {error}", tmp_path.display()),
        )
    })?;
    fs::rename(&tmp_path, path).map_err(|error| {
        AssetOrchardError::new(
            "asset_orchard_private_egress_vk_artifact_write_failed",
            format!("{} -> {}: {error}", tmp_path.display(), path.display()),
        )
    })
}

fn asset_orchard_private_egress_runtime_pinned_vk_fingerprint(
    vk: &halo2_proofs::plonk::VerifyingKey<vesta::Affine>,
) -> String {
    let pinned = format!("{:?}", vk.pinned());
    hash_bytes("asset_orchard_private_egress_vk", pinned.as_bytes())
}

fn hash_public_instance_layout() -> String {
    hash_text(
        "asset_orchard_public_instance_layout",
        "0:pool_domain\n1:anchor\n2:nf_old[0]\n3:nf_old[1]\n4:rk[0].x\n5:rk[0].y\n6:rk[1].x\n7:rk[1].y\n8:cmx_new[0]\n9:cmx_new[1]\n10:eo_hash[0][0]\n11:eo_hash[0][1]\n12:eo_hash[0][2]\n13:eo_hash[1][0]\n14:eo_hash[1][1]\n15:eo_hash[1][2]\n16:fee_field\n17:pricing_base_asset_tag_lo\n18:pricing_base_asset_tag_hi\n19:pricing_quote_asset_tag_lo\n20:pricing_quote_asset_tag_hi\n21:pricing_ratio_numerator\n22:pricing_ratio_denominator\n23:pricing_claim_commitment_0\n24:pricing_claim_commitment_1\n25:pricing_claim_commitment_2\n26:action_ctx_0\n27:action_ctx_1\n",
    )
}

fn hash_private_egress_public_instance_layout() -> String {
    hash_text(
        "asset_orchard_private_egress_public_instance_layout",
        "0:pool_domain\n1:anchor\n2:nf_old\n3:rk.x\n4:rk.y\n5:asset_tag_lo\n6:asset_tag_hi\n7:amount\n8:fee_field\n9:exit_binding_hash_0\n10:exit_binding_hash_1\n11:action_ctx_0\n12:action_ctx_1\n",
    )
}

fn hash_note_message_layout() -> String {
    let mut payload = String::new();
    for segment in asset_note_message_segments() {
        payload.push_str(&format!(
            "{:?}:piece={}:piece_bit_offset={}:source_bit_offset={}:bit_len={}\n",
            segment.source,
            segment.piece_index,
            segment.piece_bit_offset,
            segment.source_bit_offset,
            segment.bit_len
        ));
    }
    hash_text("asset_orchard_note_message_layout", &payload)
}

fn hash_poseidon_parameters() -> String {
    let (round_constants, mds, _) = <P128Pow5T3 as Spec<
        pallas::Base,
        ASSET_ORCHARD_POSEIDON_WIDTH,
        ASSET_ORCHARD_POSEIDON_RATE,
    >>::constants();
    let mut payload = Vec::new();
    payload.extend_from_slice(b"P128Pow5T3;pallas_base;width=3;rate=2");
    payload.extend_from_slice(&(round_constants.len() as u64).to_le_bytes());
    for round in round_constants {
        for constant in round {
            payload.extend_from_slice(&constant.to_repr());
        }
    }
    for row in 0..3 {
        for col in 0..3 {
            payload.extend_from_slice(&mds[row][col].to_repr());
        }
    }
    hash_bytes("asset_orchard_poseidon_parameters", &payload)
}

fn hash_text(label: &str, text: &str) -> String {
    hash_bytes(label, text.as_bytes())
}

fn hash_bytes(label: &str, bytes: &[u8]) -> String {
    let mut hasher = Sha3_384::new();
    hasher.update(label.as_bytes());
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
    bytes_to_hex(&hasher.finalize())
}

#[cfg(test)]
#[derive(Debug)]
pub struct AssetOrchardConservationVerifyingKey {
    params: Params<vesta::Affine>,
    vk: halo2_proofs::plonk::VerifyingKey<vesta::Affine>,
}

#[cfg(test)]
impl AssetOrchardConservationVerifyingKey {
    pub fn build() -> Result<Self, AssetOrchardError> {
        let params = Params::new(ASSET_ORCHARD_CONSERVATION_CORE_K);
        let empty = AssetOrchardSwapConservationCircuit {
            inputs: [None, None],
            outputs: [None, None],
            input_notes: [None, None],
            output_notes: [None, None],
            permutation_swap: None,
            #[cfg(test)]
            permutation_swap_rows: None,
            public_instance: None,
        };
        let vk = keygen_vk(&params, &empty).map_err(|error| {
            AssetOrchardError::new(
                "asset_orchard_conservation_vk_build_failed",
                error.to_string(),
            )
        })?;
        Ok(Self { params, vk })
    }

    pub fn verify_proof(
        &self,
        proof: &[u8],
        public_instance: &[pallas::Base; ASSET_ORCHARD_PUBLIC_INSTANCE_LEN],
    ) -> Result<(), AssetOrchardError> {
        let instance_column = [&public_instance[..]];
        let instances = [&instance_column[..]];
        let strategy = SingleVerifier::new(&self.params);
        let mut transcript = Blake2bRead::<_, vesta::Affine, Challenge255<_>>::init(proof);
        verify_proof(
            &self.params,
            &self.vk,
            strategy,
            &instances,
            &mut transcript,
        )
        .map_err(|error| {
            AssetOrchardError::new(
                "asset_orchard_conservation_proof_verification_failed",
                error.to_string(),
            )
        })
    }
}

#[cfg(test)]
#[derive(Debug)]
pub struct AssetOrchardConservationProvingKey {
    params: Params<vesta::Affine>,
    pk: halo2_proofs::plonk::ProvingKey<vesta::Affine>,
}

#[cfg(test)]
impl AssetOrchardConservationProvingKey {
    pub fn build() -> Result<Self, AssetOrchardError> {
        let params = Params::new(ASSET_ORCHARD_CONSERVATION_CORE_K);
        let empty = AssetOrchardSwapConservationCircuit {
            inputs: [None, None],
            outputs: [None, None],
            input_notes: [None, None],
            output_notes: [None, None],
            permutation_swap: None,
            #[cfg(test)]
            permutation_swap_rows: None,
            public_instance: None,
        };
        let vk = keygen_vk(&params, &empty).map_err(|error| {
            AssetOrchardError::new(
                "asset_orchard_conservation_vk_build_failed",
                error.to_string(),
            )
        })?;
        let pk = keygen_pk(&params, vk, &empty).map_err(|error| {
            AssetOrchardError::new(
                "asset_orchard_conservation_pk_build_failed",
                error.to_string(),
            )
        })?;
        Ok(Self { params, pk })
    }

    pub fn create_proof(
        &self,
        circuit: &AssetOrchardSwapConservationCircuit,
        mut rng: impl RngCore + CryptoRng,
    ) -> Result<Vec<u8>, AssetOrchardError> {
        let public_instance = circuit.public_instance.ok_or_else(|| {
            AssetOrchardError::new(
                "missing_public_instance",
                "asset-orchard conservation proof requires a public instance",
            )
        })?;
        let instance_column = [&public_instance[..]];
        let instances = [&instance_column[..]];
        let mut transcript = Blake2bWrite::<_, vesta::Affine, Challenge255<_>>::init(vec![]);
        create_proof(
            &self.params,
            &self.pk,
            std::slice::from_ref(circuit),
            &instances,
            &mut rng,
            &mut transcript,
        )
        .map_err(|error| {
            AssetOrchardError::new(
                "asset_orchard_conservation_proof_create_failed",
                error.to_string(),
            )
        })?;
        Ok(transcript.finalize())
    }
}

#[cfg(test)]
#[path = "asset_orchard_circuit_tests.rs"]
mod tests;
