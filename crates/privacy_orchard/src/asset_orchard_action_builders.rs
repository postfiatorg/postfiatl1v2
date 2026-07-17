#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetOrchardSwapPinnedMetadata {
    pub circuit_id: &'static str,
    pub k: u32,
    pub proof_system_id: &'static str,
    pub public_instance_len: usize,
    pub public_instance_layout_hash: String,
    pub params_hash: String,
    pub vk_hash: String,
    pub poseidon_parameter_hash: String,
    pub note_message_layout_hash: String,
    pub merkle_tree_depth: usize,
    pub merkle_parameter_hash: String,
    pub runtime_pinned_vk_fingerprint: String,
}

#[derive(Debug)]
pub struct AssetOrchardSwapVerifyingKey {
    params: Params<vesta::Affine>,
    vk: halo2_proofs::plonk::VerifyingKey<vesta::Affine>,
    metadata: AssetOrchardSwapPinnedMetadata,
}

impl AssetOrchardSwapVerifyingKey {
    pub fn build() -> Result<Self, AssetOrchardError> {
        let total_start = std::time::Instant::now();
        let mut timing = AssetOrchardSwapVkBuildTimingReport {
            schema: "postfiat.asset_orchard_swap.vk_build_timing.v1".to_string(),
            artifact_mode: "rebuild".to_string(),
            total_ms: 0.0,
            params_new_ms: 0.0,
            full_shape_ms: 0.0,
            artifact_read_ms: 0.0,
            artifact_decode_ms: 0.0,
            artifact_vk_reconstruct_ms: 0.0,
            keygen_vk_ms: 0.0,
            metadata_ms: 0.0,
            release_pin_validation_ms: 0.0,
            artifact_write_ms: 0.0,
            result: "unknown".to_string(),
        };

        let stage_start = std::time::Instant::now();
        let params = asset_orchard_k15_params()?.clone();
        timing.params_new_ms = asset_orchard_timing_elapsed_ms(stage_start);

        if !swap_vk_rebuild_requested() {
            let artifact_bytes = if let Some(path) = swap_vk_artifact_load_path() {
                timing.artifact_mode = "load".to_string();
                let stage_start = std::time::Instant::now();
                let bytes = match read_swap_vk_artifact_bytes(&path) {
                    Ok(bytes) => bytes,
                    Err(error) => {
                        timing.artifact_read_ms = asset_orchard_timing_elapsed_ms(stage_start);
                        record_swap_vk_build_timing_result(
                            timing,
                            total_start,
                            &format!("error:{}", error.code()),
                        );
                        return Err(error);
                    }
                };
                timing.artifact_read_ms = asset_orchard_timing_elapsed_ms(stage_start);
                bytes
            } else {
                timing.artifact_mode = "embedded".to_string();
                ASSET_ORCHARD_SWAP_VK_EMBEDDED_ARTIFACT.to_vec()
            };

            let stage_start = std::time::Instant::now();
            let pinned_assembly = match decode_swap_vk_artifact(&artifact_bytes) {
                Ok(assembly) => assembly,
                Err(error) => {
                    timing.artifact_decode_ms = asset_orchard_timing_elapsed_ms(stage_start);
                    record_swap_vk_build_timing_result(
                        timing,
                        total_start,
                        &format!("error:{}", error.code()),
                    );
                    return Err(error);
                }
            };
            timing.artifact_decode_ms = asset_orchard_timing_elapsed_ms(stage_start);

            let stage_start = std::time::Instant::now();
            let vk = match keygen_vk_from_pinned_assembly::<
                vesta::Affine,
                AssetOrchardSwapConservationCircuit,
            >(&params, pinned_assembly)
            {
                Ok(vk) => vk,
                Err(error) => {
                    timing.artifact_vk_reconstruct_ms =
                        asset_orchard_timing_elapsed_ms(stage_start);
                    let error = AssetOrchardError::new(
                        "asset_orchard_swap_vk_artifact_reconstruct_failed",
                        error.to_string(),
                    );
                    record_swap_vk_build_timing_result(
                        timing,
                        total_start,
                        &format!("error:{}", error.code()),
                    );
                    return Err(error);
                }
            };
            timing.artifact_vk_reconstruct_ms = asset_orchard_timing_elapsed_ms(stage_start);

            let stage_start = std::time::Instant::now();
            let metadata =
                match AssetOrchardSwapPinnedMetadata::from_vk(&vk, ASSET_ORCHARD_SWAP_V1_K) {
                    Ok(metadata) => metadata,
                    Err(error) => {
                        timing.metadata_ms = asset_orchard_timing_elapsed_ms(stage_start);
                        record_swap_vk_build_timing_result(
                            timing,
                            total_start,
                            &format!("error:{}", error.code()),
                        );
                        return Err(error);
                    }
                };
            timing.metadata_ms = asset_orchard_timing_elapsed_ms(stage_start);

            let stage_start = std::time::Instant::now();
            if let Err(error) = metadata.validate_release_pin() {
                timing.release_pin_validation_ms = asset_orchard_timing_elapsed_ms(stage_start);
                record_swap_vk_build_timing_result(
                    timing,
                    total_start,
                    &format!("error:{}", error.code()),
                );
                return Err(error);
            }
            timing.release_pin_validation_ms = asset_orchard_timing_elapsed_ms(stage_start);
            record_swap_vk_build_timing_result(timing, total_start, "ok");
            return Ok(Self {
                params,
                vk,
                metadata,
            });
        }

        let stage_start = std::time::Instant::now();
        let full_shape = AssetOrchardSwapConservationCircuit::full_shape();
        timing.full_shape_ms = asset_orchard_timing_elapsed_ms(stage_start);

        let stage_start = std::time::Instant::now();
        let (vk, pinned_assembly) = match keygen_vk_pinned_assembly(&params, &full_shape) {
            Ok(result) => result,
            Err(error) => {
                timing.keygen_vk_ms = asset_orchard_timing_elapsed_ms(stage_start);
                let error =
                    AssetOrchardError::new("asset_orchard_swap_vk_build_failed", error.to_string());
                record_swap_vk_build_timing_result(
                    timing,
                    total_start,
                    &format!("error:{}", error.code()),
                );
                return Err(error);
            }
        };
        timing.keygen_vk_ms = asset_orchard_timing_elapsed_ms(stage_start);

        let stage_start = std::time::Instant::now();
        let metadata = AssetOrchardSwapPinnedMetadata::from_vk(&vk, ASSET_ORCHARD_SWAP_V1_K)?;
        timing.metadata_ms = asset_orchard_timing_elapsed_ms(stage_start);

        let stage_start = std::time::Instant::now();
        metadata.validate_release_pin()?;
        timing.release_pin_validation_ms = asset_orchard_timing_elapsed_ms(stage_start);

        if let Some(path) = swap_vk_artifact_write_path() {
            timing.artifact_mode = "rebuild_and_write".to_string();
            let stage_start = std::time::Instant::now();
            write_swap_vk_artifact(&path, &pinned_assembly, &metadata)?;
            timing.artifact_write_ms = asset_orchard_timing_elapsed_ms(stage_start);
        }
        record_swap_vk_build_timing_result(timing, total_start, "ok");
        Ok(Self {
            params,
            vk,
            metadata,
        })
    }

    fn build_v3_replay() -> Result<Self, AssetOrchardError> {
        let params = asset_orchard_k15_params()?.clone();
        let pinned_assembly = decode_swap_vk_artifact_for_circuit(
            ASSET_ORCHARD_SWAP_V3_REPLAY_VK_EMBEDDED_ARTIFACT,
            crate::asset_orchard::ASSET_ORCHARD_CIRCUIT_ID_V3_REPLAY,
        )?;
        let vk = keygen_vk_from_pinned_assembly::<vesta::Affine, LegacyAssetOrchardSwapV3Circuit>(
            &params,
            pinned_assembly,
        )
        .map_err(|error| {
            AssetOrchardError::new(
                "asset_orchard_swap_v3_replay_vk_artifact_reconstruct_failed",
                error.to_string(),
            )
        })?;
        let metadata = AssetOrchardSwapPinnedMetadata::from_vk_for_circuit(
            &vk,
            ASSET_ORCHARD_SWAP_V1_K,
            crate::asset_orchard::ASSET_ORCHARD_CIRCUIT_ID_V3_REPLAY,
        )?;
        metadata.validate_release_pin()?;
        Ok(Self {
            params,
            vk,
            metadata,
        })
    }

    pub fn cached() -> Result<&'static Self, AssetOrchardError> {
        let total_start = std::time::Instant::now();
        let cache_was_populated = ASSET_ORCHARD_SWAP_VERIFYING_KEY.get().is_some();
        let mut build_triggered = false;
        let result = ASSET_ORCHARD_SWAP_VERIFYING_KEY.get_or_init(|| {
            build_triggered = true;
            Self::build()
        });
        record_asset_orchard_swap_vk_cached_timing(AssetOrchardSwapVkCachedTimingReport {
            schema: "postfiat.asset_orchard_swap.vk_cached_timing.v1".to_string(),
            total_ms: asset_orchard_timing_elapsed_ms(total_start),
            cache_was_populated,
            build_triggered,
            result: match result {
                Ok(_) => "ok".to_string(),
                Err(error) => format!("error:{}", error.code()),
            },
        });
        match result {
            Ok(key) => Ok(key),
            Err(error) => Err(error.clone()),
        }
    }

    pub(crate) fn cached_for_archive_replay(
        circuit_id: &str,
    ) -> Result<&'static Self, AssetOrchardError> {
        match circuit_id {
            crate::asset_orchard::ASSET_ORCHARD_CIRCUIT_ID_V4 => Self::cached(),
            crate::asset_orchard::ASSET_ORCHARD_CIRCUIT_ID_V3_REPLAY => {
                match ASSET_ORCHARD_SWAP_V3_REPLAY_VERIFYING_KEY
                    .get_or_init(Self::build_v3_replay)
                {
                    Ok(key) => Ok(key),
                    Err(error) => Err(error.clone()),
                }
            }
            _ => Err(AssetOrchardError::new(
                "unsupported_asset_orchard_circuit",
                format!("unsupported asset-orchard circuit `{circuit_id}`"),
            )),
        }
    }

    pub fn metadata(&self) -> &AssetOrchardSwapPinnedMetadata {
        &self.metadata
    }

    pub fn verify_proof(
        &self,
        proof: &[u8],
        public_instance: &[pallas::Base; ASSET_ORCHARD_PUBLIC_INSTANCE_LEN],
    ) -> Result<(), AssetOrchardError> {
        // The cached key is pinned and release-validated during build. Recomputing
        // pinned metadata here debug-walks the full Halo2 VK for every live proof
        // verification, which belongs on the key construction path, not consensus
        // proposal hot path.
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
                "asset_orchard_swap_proof_verification_failed",
                error.to_string(),
            )
        })
    }
}

#[derive(Debug)]
pub struct AssetOrchardSwapProvingKey {
    params: Params<vesta::Affine>,
    pk: halo2_proofs::plonk::ProvingKey<vesta::Affine>,
    metadata: AssetOrchardSwapPinnedMetadata,
}

impl AssetOrchardSwapProvingKey {
    pub fn build() -> Result<Self, AssetOrchardError> {
        let params = asset_orchard_k15_params()?.clone();
        let full_shape = AssetOrchardSwapConservationCircuit::full_shape();
        let verifying_key = AssetOrchardSwapVerifyingKey::cached()?;
        let vk = verifying_key.vk.clone();
        let metadata = verifying_key.metadata.clone();
        let pk = keygen_pk(&params, vk, &full_shape).map_err(|error| {
            AssetOrchardError::new("asset_orchard_swap_pk_build_failed", error.to_string())
        })?;
        Ok(Self {
            params,
            pk,
            metadata,
        })
    }

    pub fn cached() -> Result<&'static Self, AssetOrchardError> {
        match ASSET_ORCHARD_SWAP_PROVING_KEY.get_or_init(Self::build) {
            Ok(key) => Ok(key),
            Err(error) => Err(error.clone()),
        }
    }

    pub fn metadata(&self) -> &AssetOrchardSwapPinnedMetadata {
        &self.metadata
    }

    pub fn create_proof(
        &self,
        circuit: &AssetOrchardSwapConservationCircuit,
        mut rng: impl RngCore + CryptoRng,
    ) -> Result<Vec<u8>, AssetOrchardError> {
        circuit.require_full_note_witnesses()?;
        let public_instance = circuit.public_instance.ok_or_else(|| {
            AssetOrchardError::new(
                "missing_public_instance",
                "asset-orchard swap proof requires a public instance",
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
            AssetOrchardError::new("asset_orchard_swap_proof_create_failed", error.to_string())
        })?;
        Ok(transcript.finalize())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetOrchardPrivateEgressPinnedMetadata {
    pub circuit_id: &'static str,
    pub k: u32,
    pub proof_system_id: &'static str,
    pub public_instance_len: usize,
    pub public_instance_layout_hash: String,
    pub params_hash: String,
    pub vk_hash: String,
    pub poseidon_parameter_hash: String,
    pub note_message_layout_hash: String,
    pub merkle_tree_depth: usize,
    pub merkle_parameter_hash: String,
    pub runtime_pinned_vk_fingerprint: String,
}

#[derive(Debug)]
pub struct AssetOrchardPrivateEgressVerifyingKey {
    params: Params<vesta::Affine>,
    vk: halo2_proofs::plonk::VerifyingKey<vesta::Affine>,
    metadata: AssetOrchardPrivateEgressPinnedMetadata,
}

impl AssetOrchardPrivateEgressVerifyingKey {
    pub fn build() -> Result<Self, AssetOrchardError> {
        let total_start = std::time::Instant::now();
        let mut timing = AssetOrchardPrivateEgressVkBuildTimingReport {
            schema: "postfiat.asset_orchard_private_egress.vk_build_timing.v1".to_string(),
            artifact_mode: "rebuild".to_string(),
            total_ms: 0.0,
            params_new_ms: 0.0,
            full_shape_ms: 0.0,
            artifact_read_ms: 0.0,
            artifact_decode_ms: 0.0,
            artifact_vk_reconstruct_ms: 0.0,
            keygen_vk_ms: 0.0,
            metadata_ms: 0.0,
            release_pin_validation_ms: 0.0,
            artifact_write_ms: 0.0,
            result: "unknown".to_string(),
        };

        let stage_start = std::time::Instant::now();
        let params = asset_orchard_k15_params()?.clone();
        timing.params_new_ms = asset_orchard_timing_elapsed_ms(stage_start);

        if !private_egress_vk_rebuild_requested() {
            let artifact_bytes = if let Some(path) = private_egress_vk_artifact_load_path() {
                timing.artifact_mode = "load".to_string();

                let stage_start = std::time::Instant::now();
                let artifact_bytes = match read_private_egress_vk_artifact_bytes(&path) {
                    Ok(bytes) => bytes,
                    Err(error) => {
                        timing.artifact_read_ms = asset_orchard_timing_elapsed_ms(stage_start);
                        record_private_egress_vk_build_timing_result(
                            timing,
                            total_start,
                            &format!("error:{}", error.code()),
                        );
                        return Err(error);
                    }
                };
                timing.artifact_read_ms = asset_orchard_timing_elapsed_ms(stage_start);
                artifact_bytes
            } else {
                timing.artifact_mode = "embedded".to_string();
                ASSET_ORCHARD_PRIVATE_EGRESS_VK_EMBEDDED_ARTIFACT.to_vec()
            };

            let stage_start = std::time::Instant::now();
            let pinned_assembly = match decode_private_egress_vk_artifact(&artifact_bytes) {
                Ok(assembly) => assembly,
                Err(error) => {
                    timing.artifact_decode_ms = asset_orchard_timing_elapsed_ms(stage_start);
                    record_private_egress_vk_build_timing_result(
                        timing,
                        total_start,
                        &format!("error:{}", error.code()),
                    );
                    return Err(error);
                }
            };
            timing.artifact_decode_ms = asset_orchard_timing_elapsed_ms(stage_start);

            let stage_start = std::time::Instant::now();
            let vk = match keygen_vk_from_pinned_assembly::<
                vesta::Affine,
                AssetOrchardPrivateEgressCircuit,
            >(&params, pinned_assembly)
            {
                Ok(vk) => vk,
                Err(error) => {
                    timing.artifact_vk_reconstruct_ms =
                        asset_orchard_timing_elapsed_ms(stage_start);
                    let error = AssetOrchardError::new(
                        "asset_orchard_private_egress_vk_artifact_reconstruct_failed",
                        error.to_string(),
                    );
                    record_private_egress_vk_build_timing_result(
                        timing,
                        total_start,
                        &format!("error:{}", error.code()),
                    );
                    return Err(error);
                }
            };
            timing.artifact_vk_reconstruct_ms = asset_orchard_timing_elapsed_ms(stage_start);

            let stage_start = std::time::Instant::now();
            let metadata = match AssetOrchardPrivateEgressPinnedMetadata::from_vk(
                &vk,
                ASSET_ORCHARD_PRIVATE_EGRESS_V1_K,
            ) {
                Ok(metadata) => metadata,
                Err(error) => {
                    timing.metadata_ms = asset_orchard_timing_elapsed_ms(stage_start);
                    record_private_egress_vk_build_timing_result(
                        timing,
                        total_start,
                        &format!("error:{}", error.code()),
                    );
                    return Err(error);
                }
            };
            timing.metadata_ms = asset_orchard_timing_elapsed_ms(stage_start);

            let stage_start = std::time::Instant::now();
            if let Err(error) = metadata.validate_release_pin() {
                timing.release_pin_validation_ms = asset_orchard_timing_elapsed_ms(stage_start);
                record_private_egress_vk_build_timing_result(
                    timing,
                    total_start,
                    &format!("error:{}", error.code()),
                );
                return Err(error);
            }
            timing.release_pin_validation_ms = asset_orchard_timing_elapsed_ms(stage_start);
            record_private_egress_vk_build_timing_result(timing, total_start, "ok");

            return Ok(Self {
                params,
                vk,
                metadata,
            });
        }

        let stage_start = std::time::Instant::now();
        let full_shape = AssetOrchardPrivateEgressCircuit::full_shape();
        timing.full_shape_ms = asset_orchard_timing_elapsed_ms(stage_start);

        let stage_start = std::time::Instant::now();
        let (vk, pinned_assembly) = match keygen_vk_pinned_assembly(&params, &full_shape) {
            Ok(result) => result,
            Err(error) => {
                timing.keygen_vk_ms = asset_orchard_timing_elapsed_ms(stage_start);
                let error = AssetOrchardError::new(
                    "asset_orchard_private_egress_vk_build_failed",
                    error.to_string(),
                );
                record_private_egress_vk_build_timing_result(
                    timing,
                    total_start,
                    &format!("error:{}", error.code()),
                );
                return Err(error);
            }
        };
        timing.keygen_vk_ms = asset_orchard_timing_elapsed_ms(stage_start);

        let stage_start = std::time::Instant::now();
        let metadata = match AssetOrchardPrivateEgressPinnedMetadata::from_vk(
            &vk,
            ASSET_ORCHARD_PRIVATE_EGRESS_V1_K,
        ) {
            Ok(metadata) => metadata,
            Err(error) => {
                timing.metadata_ms = asset_orchard_timing_elapsed_ms(stage_start);
                record_private_egress_vk_build_timing_result(
                    timing,
                    total_start,
                    &format!("error:{}", error.code()),
                );
                return Err(error);
            }
        };
        timing.metadata_ms = asset_orchard_timing_elapsed_ms(stage_start);

        let stage_start = std::time::Instant::now();
        if let Err(error) = metadata.validate_release_pin() {
            timing.release_pin_validation_ms = asset_orchard_timing_elapsed_ms(stage_start);
            record_private_egress_vk_build_timing_result(
                timing,
                total_start,
                &format!("error:{}", error.code()),
            );
            return Err(error);
        }
        timing.release_pin_validation_ms = asset_orchard_timing_elapsed_ms(stage_start);

        if let Some(path) = private_egress_vk_artifact_write_path() {
            timing.artifact_mode = "rebuild_and_write".to_string();
            let stage_start = std::time::Instant::now();
            if let Err(error) = write_private_egress_vk_artifact(&path, &pinned_assembly, &metadata)
            {
                timing.artifact_write_ms = asset_orchard_timing_elapsed_ms(stage_start);
                record_private_egress_vk_build_timing_result(
                    timing,
                    total_start,
                    &format!("error:{}", error.code()),
                );
                return Err(error);
            }
            timing.artifact_write_ms = asset_orchard_timing_elapsed_ms(stage_start);
        }

        record_private_egress_vk_build_timing_result(timing, total_start, "ok");

        Ok(Self {
            params,
            vk,
            metadata,
        })
    }

    fn build_v1_replay() -> Result<Self, AssetOrchardError> {
        let params = asset_orchard_k15_params()?.clone();
        let pinned_assembly = decode_private_egress_vk_artifact_for_circuit(
            ASSET_ORCHARD_PRIVATE_EGRESS_V1_REPLAY_VK_EMBEDDED_ARTIFACT,
            crate::asset_orchard::ASSET_ORCHARD_PRIVATE_EGRESS_CIRCUIT_ID_V1_REPLAY,
        )?;
        let vk = keygen_vk_from_pinned_assembly::<
            vesta::Affine,
            LegacyAssetOrchardPrivateEgressV1Circuit,
        >(&params, pinned_assembly)
        .map_err(|error| {
            AssetOrchardError::new(
                "asset_orchard_private_egress_v1_replay_vk_artifact_reconstruct_failed",
                error.to_string(),
            )
        })?;
        let metadata = AssetOrchardPrivateEgressPinnedMetadata::from_vk_for_circuit(
            &vk,
            ASSET_ORCHARD_PRIVATE_EGRESS_V1_K,
            crate::asset_orchard::ASSET_ORCHARD_PRIVATE_EGRESS_CIRCUIT_ID_V1_REPLAY,
        )?;
        metadata.validate_release_pin()?;
        Ok(Self {
            params,
            vk,
            metadata,
        })
    }

    pub fn cached() -> Result<&'static Self, AssetOrchardError> {
        let total_start = std::time::Instant::now();
        let cache_was_populated = ASSET_ORCHARD_PRIVATE_EGRESS_VERIFYING_KEY.get().is_some();
        let mut build_triggered = false;
        let result = ASSET_ORCHARD_PRIVATE_EGRESS_VERIFYING_KEY.get_or_init(|| {
            build_triggered = true;
            Self::build()
        });
        let timing = AssetOrchardPrivateEgressVkCachedTimingReport {
            schema: "postfiat.asset_orchard_private_egress.vk_cached_timing.v1".to_string(),
            total_ms: asset_orchard_timing_elapsed_ms(total_start),
            cache_was_populated,
            build_triggered,
            result: match result {
                Ok(_) => "ok".to_string(),
                Err(error) => format!("error:{}", error.code()),
            },
        };
        record_asset_orchard_private_egress_vk_cached_timing(timing);
        match result {
            Ok(key) => Ok(key),
            Err(error) => Err(error.clone()),
        }
    }

    pub(crate) fn cached_for_archive_replay(
        circuit_id: &str,
    ) -> Result<&'static Self, AssetOrchardError> {
        match circuit_id {
            crate::asset_orchard::ASSET_ORCHARD_PRIVATE_EGRESS_CIRCUIT_ID_V2 => Self::cached(),
            crate::asset_orchard::ASSET_ORCHARD_PRIVATE_EGRESS_CIRCUIT_ID_V1_REPLAY => {
                match ASSET_ORCHARD_PRIVATE_EGRESS_V1_REPLAY_VERIFYING_KEY
                    .get_or_init(Self::build_v1_replay)
                {
                    Ok(key) => Ok(key),
                    Err(error) => Err(error.clone()),
                }
            }
            _ => Err(AssetOrchardError::new(
                "unsupported_asset_orchard_private_egress_circuit",
                format!("unsupported asset-orchard private egress circuit `{circuit_id}`"),
            )),
        }
    }

    pub fn metadata(&self) -> &AssetOrchardPrivateEgressPinnedMetadata {
        &self.metadata
    }

    pub fn verify_proof(
        &self,
        proof: &[u8],
        public_instance: &[pallas::Base; ASSET_ORCHARD_PRIVATE_EGRESS_PUBLIC_INSTANCE_LEN],
    ) -> Result<(), AssetOrchardError> {
        let total_start = std::time::Instant::now();
        let mut timing = AssetOrchardPrivateEgressProofVerifyTimingReport {
            schema: "postfiat.asset_orchard_private_egress.proof_verify_timing.v1".to_string(),
            total_ms: 0.0,
            vk_metadata_recompute_ms: 0.0,
            instance_setup_ms: 0.0,
            verifier_strategy_ms: 0.0,
            transcript_init_ms: 0.0,
            halo2_verify_proof_ms: 0.0,
            result: "unknown".to_string(),
        };

        let stage_start = std::time::Instant::now();
        let expected = match AssetOrchardPrivateEgressPinnedMetadata::from_vk_for_circuit(
            &self.vk,
            ASSET_ORCHARD_PRIVATE_EGRESS_V1_K,
            self.metadata.circuit_id,
        ) {
            Ok(expected) => expected,
            Err(error) => {
                timing.vk_metadata_recompute_ms = asset_orchard_timing_elapsed_ms(stage_start);
                timing.total_ms = asset_orchard_timing_elapsed_ms(total_start);
                timing.result = format!("error:{}", error.code());
                record_asset_orchard_private_egress_proof_verify_timing(timing);
                return Err(error);
            }
        };
        timing.vk_metadata_recompute_ms = asset_orchard_timing_elapsed_ms(stage_start);
        if expected != self.metadata {
            let error = AssetOrchardError::new(
                "asset_orchard_private_egress_vk_metadata_mismatch",
                "AssetOrchard private egress verifying key metadata changed after build",
            );
            timing.total_ms = asset_orchard_timing_elapsed_ms(total_start);
            timing.result = format!("error:{}", error.code());
            record_asset_orchard_private_egress_proof_verify_timing(timing);
            return Err(error);
        }

        let stage_start = std::time::Instant::now();
        let instance_column = [&public_instance[..]];
        let instances = [&instance_column[..]];
        timing.instance_setup_ms = asset_orchard_timing_elapsed_ms(stage_start);

        let stage_start = std::time::Instant::now();
        let strategy = SingleVerifier::new(&self.params);
        timing.verifier_strategy_ms = asset_orchard_timing_elapsed_ms(stage_start);

        let stage_start = std::time::Instant::now();
        let mut transcript = Blake2bRead::<_, vesta::Affine, Challenge255<_>>::init(proof);
        timing.transcript_init_ms = asset_orchard_timing_elapsed_ms(stage_start);

        let stage_start = std::time::Instant::now();
        let result = verify_proof(
            &self.params,
            &self.vk,
            strategy,
            &instances,
            &mut transcript,
        );
        timing.halo2_verify_proof_ms = asset_orchard_timing_elapsed_ms(stage_start);
        timing.total_ms = asset_orchard_timing_elapsed_ms(total_start);
        match result {
            Ok(()) => {
                timing.result = "ok".to_string();
                record_asset_orchard_private_egress_proof_verify_timing(timing);
                Ok(())
            }
            Err(error) => {
                let error = AssetOrchardError::new(
                    "asset_orchard_private_egress_proof_verification_failed",
                    error.to_string(),
                );
                timing.result = format!("error:{}", error.code());
                record_asset_orchard_private_egress_proof_verify_timing(timing);
                Err(error)
            }
        }
    }
}

#[derive(Debug)]
pub struct AssetOrchardPrivateEgressProvingKey {
    params: Params<vesta::Affine>,
    pk: halo2_proofs::plonk::ProvingKey<vesta::Affine>,
    metadata: AssetOrchardPrivateEgressPinnedMetadata,
}

impl AssetOrchardPrivateEgressProvingKey {
    pub fn build() -> Result<Self, AssetOrchardError> {
        let params = asset_orchard_k15_params()?.clone();
        let full_shape = AssetOrchardPrivateEgressCircuit::full_shape();
        let verifying_key = AssetOrchardPrivateEgressVerifyingKey::cached()?;
        let vk = verifying_key.vk.clone();
        let metadata = verifying_key.metadata.clone();
        let pk = keygen_pk(&params, vk, &full_shape).map_err(|error| {
            AssetOrchardError::new(
                "asset_orchard_private_egress_pk_build_failed",
                error.to_string(),
            )
        })?;
        Ok(Self {
            params,
            pk,
            metadata,
        })
    }

    pub fn cached() -> Result<&'static Self, AssetOrchardError> {
        match ASSET_ORCHARD_PRIVATE_EGRESS_PROVING_KEY.get_or_init(Self::build) {
            Ok(key) => Ok(key),
            Err(error) => Err(error.clone()),
        }
    }

    pub fn metadata(&self) -> &AssetOrchardPrivateEgressPinnedMetadata {
        &self.metadata
    }

    pub fn create_proof(
        &self,
        circuit: &AssetOrchardPrivateEgressCircuit,
        mut rng: impl RngCore + CryptoRng,
    ) -> Result<Vec<u8>, AssetOrchardError> {
        circuit.require_full_note_witness()?;
        let public_instance = circuit.public_instance.ok_or_else(|| {
            AssetOrchardError::new(
                "missing_public_instance",
                "asset-orchard private egress proof requires a public instance",
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
                "asset_orchard_private_egress_proof_create_failed",
                error.to_string(),
            )
        })?;
        Ok(transcript.finalize())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetOrchardSwapBuildResult {
    pub action: AssetOrchardSwapAction,
    pub output_notes: [AssetOrchardWalletNote; ASSET_ORCHARD_LEG_COUNT],
    pub anchor: AssetOrchardFieldElement,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetOrchardPrivateEgressBuildResult {
    pub action: AssetOrchardPrivateEgressAction,
    pub anchor: AssetOrchardFieldElement,
}

#[allow(clippy::too_many_arguments)]
pub fn build_asset_orchard_private_egress_action(
    chain_id: &str,
    genesis_hash: [u8; 32],
    protocol_version: u32,
    input_note: AssetOrchardWalletNote,
    to: &str,
    asset_id: &str,
    amount: u64,
    fee: u64,
    policy_id: &str,
    disclosure_hash: &str,
    pool_output_commitments: &[String],
) -> Result<AssetOrchardPrivateEgressBuildResult, AssetOrchardError> {
    let total_start = std::time::Instant::now();
    let mut timing = AssetOrchardPrivateEgressActionBuildTimingReport::default();

    let stage_start = std::time::Instant::now();
    let pool_domain =
        AssetOrchardSwapAction::expected_pool_domain(chain_id, genesis_hash, protocol_version)?;
    validate_wallet_note_for_swap(&input_note, pool_domain)?;
    if input_note.asset_id != asset_id {
        return Err(AssetOrchardError::new(
            "asset_orchard_private_egress_asset_id_mismatch",
            "private egress asset_id must match the shielded input note asset_id",
        ));
    }
    if input_note.value != amount {
        return Err(AssetOrchardError::new(
            "asset_orchard_private_egress_amount_mismatch",
            "private egress v1 requires amount to equal the full shielded note value",
        ));
    }
    if fee != 0 {
        return Err(AssetOrchardError::new(
            "unsupported_asset_orchard_private_egress_fee",
            "asset-orchard private egress v1 requires fee 0",
        ));
    }
    let expected_tag = AssetTag::derive(asset_id)?;
    if input_note.note.asset_tag_lo != expected_tag.lo
        || input_note.note.asset_tag_hi != expected_tag.hi
    {
        return Err(AssetOrchardError::new(
            "asset_orchard_private_egress_asset_tag_mismatch",
            "private egress wallet note asset tag does not match asset_id",
        ));
    }
    timing.validation_domain_ms = asset_orchard_timing_elapsed_ms(stage_start);

    let stage_start = std::time::Instant::now();
    let (anchor, merkle_witness) = asset_orchard_merkle_witness_from_commitments(
        pool_output_commitments,
        input_note.output_commitment.as_hex(),
    )?;
    timing.merkle_witness_ms = asset_orchard_timing_elapsed_ms(stage_start);

    let stage_start = std::time::Instant::now();
    let signing_key = asset_orchard_spend_signing_key(&input_note)?;
    let alpha = private_egress_spend_randomizer();
    let input_witness = wallet_note_to_swap_witness(
        pool_domain,
        &input_note,
        Some(merkle_witness),
        Some(spend_authority_from_wallet_note_with_alpha(
            &input_note,
            &signing_key,
            alpha,
        )?),
    )?;
    timing.signing_witness_prep_ms = asset_orchard_timing_elapsed_ms(stage_start);

    let stage_start = std::time::Instant::now();
    let nullifier = asset_derive_nullifier(
        pool_domain,
        input_witness.nk,
        input_witness.note.rho,
        input_witness.note.psi,
        input_witness.cmx,
    )?;
    let rk_point = randomized_verification_key_point(&input_witness)?;
    let rk = RandomizedVerificationKeyFields::from_affine(rk_point)?;
    timing.nullifier_rvk_ms = asset_orchard_timing_elapsed_ms(stage_start);

    let stage_start = std::time::Instant::now();
    let exit_binding_hash = asset_orchard_private_egress_exit_binding_hash(
        &AssetOrchardPrivateEgressExitBindingPreimage {
            chain_id,
            genesis_hash,
            protocol_version,
            pool_id: ASSET_ORCHARD_POOL_ID_V1,
            circuit_id: ASSET_ORCHARD_PRIVATE_EGRESS_CIRCUIT_ID_V1,
            pool_domain,
            to,
            asset_id,
            amount,
            fee,
            policy_id,
            disclosure_hash,
        },
    )?;
    let public_fields = AssetOrchardPrivateEgressPublicFields {
        pool_domain,
        anchor: anchor.to_field()?,
        nullifier,
        randomized_verification_key: rk,
        asset_tag: expected_tag,
        amount,
        fee,
        exit_binding_hash,
    };
    let circuit =
        AssetOrchardPrivateEgressCircuit::new_with_note_witness(input_witness, &public_fields)?;
    timing.exit_binding_public_fields_circuit_ms = asset_orchard_timing_elapsed_ms(stage_start);
    timing.pre_key_ms = timing.validation_domain_ms
        + timing.merkle_witness_ms
        + timing.signing_witness_prep_ms
        + timing.nullifier_rvk_ms
        + timing.exit_binding_public_fields_circuit_ms;

    let stage_start = std::time::Instant::now();
    let proof_key = AssetOrchardPrivateEgressProvingKey::cached()?;
    timing.proving_key_cached_ms = asset_orchard_timing_elapsed_ms(stage_start);
    timing.key_build_ms = timing.proving_key_cached_ms;

    let stage_start = std::time::Instant::now();
    let proof = proof_key.create_proof(&circuit, OsRng)?;
    timing.proof_generation_ms = asset_orchard_timing_elapsed_ms(stage_start);
    timing.proof_gen_ms = timing.proof_generation_ms;

    let stage_start = std::time::Instant::now();
    let placeholder =
        AssetOrchardSpendAuthSignature::from_orchard(&signing_key.sign(OsRng, b"placeholder"));
    let mut action = AssetOrchardPrivateEgressAction {
        version: ASSET_ORCHARD_ACTION_VERSION_V1,
        schema: ASSET_ORCHARD_PRIVATE_EGRESS_ACTION_SCHEMA_V1.to_string(),
        pool_id: ASSET_ORCHARD_POOL_ID_V1.to_string(),
        proof_system_id: ASSET_ORCHARD_PROOF_SYSTEM_ID_V1.to_string(),
        circuit_id: ASSET_ORCHARD_PRIVATE_EGRESS_CIRCUIT_ID_V1.to_string(),
        pool_domain: AssetOrchardFieldElement::from_field(pool_domain),
        anchor: anchor.clone(),
        nullifier: AssetOrchardFieldElement::from_field(nullifier),
        randomized_verification_key: AssetOrchardPoint::from_affine(rk_point)?,
        asset_tag_lo: expected_tag.lo,
        asset_tag_hi: expected_tag.hi,
        amount,
        fee,
        exit_binding_hash: AssetOrchardSwapBindingHash::from_bytes(&exit_binding_hash),
        proof: AssetOrchardProofBytes::from_bytes(&proof)?,
        spend_authorization_signature: placeholder,
    };
    let sighash = action.sighash(
        chain_id,
        genesis_hash,
        protocol_version,
        to,
        asset_id,
        policy_id,
        disclosure_hash,
    )?;
    action.spend_authorization_signature = AssetOrchardSpendAuthSignature::from_orchard(
        &signing_key.randomize(&alpha).sign(OsRng, &sighash),
    );
    action.validate()?;
    timing.action_assembly_sighash_signature_ms = asset_orchard_timing_elapsed_ms(stage_start);
    timing.post_proof_ms = timing.action_assembly_sighash_signature_ms;
    timing.total_ms = asset_orchard_timing_elapsed_ms(total_start);
    timing.result = "ok".to_string();
    record_asset_orchard_private_egress_action_build_timing(timing);
    Ok(AssetOrchardPrivateEgressBuildResult { action, anchor })
}

pub fn build_asset_orchard_swap_action(
    chain_id: &str,
    genesis_hash: [u8; 32],
    protocol_version: u32,
    input_notes: [AssetOrchardWalletNote; ASSET_ORCHARD_LEG_COUNT],
    output_note_seed_hexes: [String; ASSET_ORCHARD_LEG_COUNT],
    pool_output_commitments: &[String],
    pricing_claim: crate::asset_orchard::AssetOrchardPricingClaim,
) -> Result<AssetOrchardSwapBuildResult, AssetOrchardError> {
    if input_notes[0].output_commitment == input_notes[1].output_commitment {
        return Err(AssetOrchardError::new(
            "duplicate_asset_orchard_input_note",
            "asset-orchard swap inputs must be distinct notes",
        ));
    }
    let pool_domain =
        AssetOrchardSwapAction::expected_pool_domain(chain_id, genesis_hash, protocol_version)?;
    for note in &input_notes {
        validate_wallet_note_for_swap(note, pool_domain)?;
    }

    let (anchor, merkle_witnesses) = asset_orchard_merkle_witnesses_from_commitments(
        pool_output_commitments,
        [
            input_notes[0].output_commitment.as_hex(),
            input_notes[1].output_commitment.as_hex(),
        ],
    )?;
    let input_signing_keys = [
        asset_orchard_spend_signing_key(&input_notes[0])?,
        asset_orchard_spend_signing_key(&input_notes[1])?,
    ];
    let input_witnesses = [
        wallet_note_to_swap_witness(
            pool_domain,
            &input_notes[0],
            Some(merkle_witnesses[0].clone()),
            Some(spend_authority_from_wallet_note(
                &input_notes[0],
                &input_signing_keys[0],
            )?),
        )?,
        wallet_note_to_swap_witness(
            pool_domain,
            &input_notes[1],
            Some(merkle_witnesses[1].clone()),
            Some(spend_authority_from_wallet_note(
                &input_notes[1],
                &input_signing_keys[1],
            )?),
        )?,
    ];
    let anchor_field = anchor.to_field()?;
    let input_nullifiers = asset_orchard_input_nullifiers(pool_domain, &input_witnesses)?;
    let randomized_keys = note_swap_randomized_verification_keys(&input_witnesses)?;
    let output_rhos = [
        asset_output_rho(
            pool_domain,
            anchor_field,
            input_nullifiers,
            randomized_keys,
            0,
        )?,
        asset_output_rho(
            pool_domain,
            anchor_field,
            input_nullifiers,
            randomized_keys,
            1,
        )?,
    ];
    let output_notes = [
        build_asset_orchard_wallet_note_with_rho(
            chain_id,
            genesis_hash,
            protocol_version,
            &input_notes[1].asset_id,
            input_notes[1].value,
            &output_note_seed_hexes[0],
            output_rhos[0],
        )?,
        build_asset_orchard_wallet_note_with_rho(
            chain_id,
            genesis_hash,
            protocol_version,
            &input_notes[0].asset_id,
            input_notes[0].value,
            &output_note_seed_hexes[1],
            output_rhos[1],
        )?,
    ];
    if output_notes[0].output_commitment == output_notes[1].output_commitment {
        return Err(AssetOrchardError::new(
            "duplicate_asset_orchard_output_note",
            "asset-orchard swap outputs must be distinct notes",
        ));
    }
    for output in &output_notes {
        if pool_output_commitments
            .iter()
            .any(|existing| existing == output.output_commitment.as_hex())
        {
            return Err(AssetOrchardError::new(
                "asset_orchard_output_already_exists",
                "asset-orchard swap output commitment already exists in the pool",
            ));
        }
    }
    let output_witnesses = [
        wallet_note_to_swap_witness(pool_domain, &output_notes[0], None, None)?,
        wallet_note_to_swap_witness(pool_domain, &output_notes[1], None, None)?,
    ];
    let encrypted_outputs = [
        crate::encrypt_asset_orchard_wallet_note(
            chain_id,
            genesis_hash,
            protocol_version,
            &output_notes[0],
        )?,
        crate::encrypt_asset_orchard_wallet_note(
            chain_id,
            genesis_hash,
            protocol_version,
            &output_notes[1],
        )?,
    ];
    let public_fields = asset_orchard_public_fields_from_witnesses(
        pool_domain,
        anchor_field,
        &input_witnesses,
        &output_witnesses,
        &encrypted_outputs,
        &pricing_claim,
    )?;
    let circuit = AssetOrchardSwapConservationCircuit::new_with_note_witnesses(
        input_witnesses.clone(),
        output_witnesses,
        true,
        &public_fields,
    )?;
    let proof_key = AssetOrchardSwapProvingKey::cached()?;
    let proof = proof_key.create_proof(&circuit, OsRng)?;
    let action = signed_asset_orchard_action_from_witnesses(
        chain_id,
        genesis_hash,
        protocol_version,
        &input_witnesses,
        &input_notes,
        &output_notes,
        public_fields,
        encrypted_outputs,
        [&input_signing_keys[0], &input_signing_keys[1]],
        proof,
        pricing_claim,
    )?;
    Ok(AssetOrchardSwapBuildResult {
        action,
        output_notes,
        anchor,
    })
}

fn validate_wallet_note_for_swap(
    note: &AssetOrchardWalletNote,
    pool_domain: pallas::Base,
) -> Result<(), AssetOrchardError> {
    if note.pool_id != ASSET_ORCHARD_POOL_ID_V1 {
        return Err(AssetOrchardError::new(
            "unsupported_asset_orchard_wallet_note_pool",
            format!(
                "unsupported asset-orchard wallet note pool `{}`",
                note.pool_id
            ),
        ));
    }
    if note.pool_domain.to_field()? != pool_domain {
        return Err(AssetOrchardError::new(
            "asset_orchard_wallet_note_pool_domain_mismatch",
            "asset-orchard wallet note pool domain does not match chain/genesis/protocol",
        ));
    }
    note.note.validate_for_asset(&note.asset_id, note.value)?;
    let expected_cmx = note.note.cmx(pool_domain)?;
    if expected_cmx != note.output_commitment {
        return Err(AssetOrchardError::new(
            "asset_orchard_wallet_note_cmx_mismatch",
            "asset-orchard wallet note commitment does not match note opening",
        ));
    }
    Ok(())
}

fn wallet_note_to_swap_witness(
    pool_domain: pallas::Base,
    note: &AssetOrchardWalletNote,
    merkle_witness: Option<AssetOrchardMerkleWitness>,
    spend_authority: Option<AssetOrchardSpendAuthorityWitness>,
) -> Result<AssetOrchardSwapNoteWitness, AssetOrchardError> {
    validate_wallet_note_for_swap(note, pool_domain)?;
    let mut witness = AssetOrchardSwapNoteWitness::from_note_with_nk(
        pool_domain,
        note.note.to_note_opening()?,
        note.nk.to_field()?,
    )?;
    if let Some(merkle_witness) = merkle_witness {
        witness = witness.with_merkle_witness(merkle_witness);
    }
    if let Some(spend_authority) = spend_authority {
        witness = witness.with_spend_authority(spend_authority);
    }
    Ok(witness)
}

fn asset_orchard_spend_signing_key(
    note: &AssetOrchardWalletNote,
) -> Result<SigningKey<SpendAuth>, AssetOrchardError> {
    let bytes = fixed_hex_array::<32>(
        "asset_orchard_spend_auth_signing_key",
        note.spend_auth_signing_key.as_str(),
    )?;
    SigningKey::<SpendAuth>::try_from(bytes).map_err(|_| {
        AssetOrchardError::new(
            "invalid_asset_orchard_spend_auth_signing_key",
            "asset-orchard wallet note spend authorization key is invalid",
        )
    })
}

fn spend_authority_from_wallet_note(
    note: &AssetOrchardWalletNote,
    signing_key: &SigningKey<SpendAuth>,
) -> Result<AssetOrchardSpendAuthorityWitness, AssetOrchardError> {
    let alpha = random_pallas_scalar_nonzero();
    spend_authority_from_wallet_note_with_alpha(note, signing_key, alpha)
}

fn spend_authority_from_wallet_note_with_alpha(
    note: &AssetOrchardWalletNote,
    signing_key: &SigningKey<SpendAuth>,
    alpha: pallas::Scalar,
) -> Result<AssetOrchardSpendAuthorityWitness, AssetOrchardError> {
    let verification_key = VerificationKey::from(signing_key);
    let ak = spend_auth_verification_key_affine(&verification_key)?;
    Ok(AssetOrchardSpendAuthorityWitness {
        ak,
        alpha,
        rivk: scalar_from_hex("asset_orchard_wallet_note_rivk", note.rivk.as_str())?,
    })
}

fn spend_auth_verification_key_affine(
    key: &VerificationKey<SpendAuth>,
) -> Result<pallas::Affine, AssetOrchardError> {
    let bytes = <[u8; 32]>::from(key);
    let point =
        Option::<pallas::Affine>::from(pallas::Affine::from_bytes(&bytes)).ok_or_else(|| {
            AssetOrchardError::new(
                "invalid_asset_orchard_spend_verification_key",
                "asset-orchard spend verification key is not a canonical Pallas point",
            )
        })?;
    if bool::from(point.is_identity()) {
        return Err(AssetOrchardError::new(
            "invalid_asset_orchard_spend_verification_key",
            "asset-orchard spend verification key is identity",
        ));
    }
    Ok(point)
}

fn private_egress_spend_randomizer() -> pallas::Scalar {
    random_pallas_scalar_nonzero()
}

fn asset_orchard_merkle_witnesses_from_commitments(
    commitments: &[String],
    input_commitments: [&str; ASSET_ORCHARD_LEG_COUNT],
) -> Result<
    (
        AssetOrchardFieldElement,
        [AssetOrchardMerkleWitness; ASSET_ORCHARD_LEG_COUNT],
    ),
    AssetOrchardError,
> {
    let leaves = commitments
        .iter()
        .map(|commitment| {
            AssetOrchardFieldElement::parse_hex(commitment.clone())
                .and_then(|cmx| merkle_hash_from_cmx(cmx.to_field()?))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let positions = input_commitments.map(|commitment| {
        commitments
            .iter()
            .position(|existing| existing == commitment)
            .ok_or_else(|| {
                AssetOrchardError::new(
                    "asset_orchard_input_note_not_in_pool",
                    format!("asset-orchard input note commitment `{commitment}` not found in pool"),
                )
            })
    });
    let positions = [positions[0].clone()?, positions[1].clone()?];
    if positions[0] == positions[1] {
        return Err(AssetOrchardError::new(
            "duplicate_asset_orchard_input_position",
            "asset-orchard swap inputs resolve to the same Merkle position",
        ));
    }
    let root = merkle_root_from_nodes(leaves.clone())?;
    Ok((
        AssetOrchardFieldElement::from_field(base_from_merkle_hash(&root)?),
        [
            merkle_witness_from_nodes(leaves.clone(), positions[0])?,
            merkle_witness_from_nodes(leaves, positions[1])?,
        ],
    ))
}

fn asset_orchard_merkle_witness_from_commitments(
    commitments: &[String],
    input_commitment: &str,
) -> Result<(AssetOrchardFieldElement, AssetOrchardMerkleWitness), AssetOrchardError> {
    let leaves = commitments
        .iter()
        .map(|commitment| {
            AssetOrchardFieldElement::parse_hex(commitment.clone())
                .and_then(|cmx| merkle_hash_from_cmx(cmx.to_field()?))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let position = commitments
        .iter()
        .position(|existing| existing == input_commitment)
        .ok_or_else(|| {
            AssetOrchardError::new(
                "asset_orchard_input_note_not_in_pool",
                format!(
                    "asset-orchard input note commitment `{input_commitment}` not found in pool"
                ),
            )
        })?;
    let root = merkle_root_from_nodes(leaves.clone())?;
    Ok((
        AssetOrchardFieldElement::from_field(base_from_merkle_hash(&root)?),
        merkle_witness_from_nodes(leaves, position)?,
    ))
}

fn merkle_hash_from_cmx(cmx: pallas::Base) -> Result<MerkleHashOrchard, AssetOrchardError> {
    let bytes = cmx.to_repr();
    let extracted =
        Option::<ExtractedNoteCommitment>::from(ExtractedNoteCommitment::from_bytes(&bytes))
            .ok_or_else(|| {
                AssetOrchardError::new(
            "invalid_asset_orchard_output_commitment",
            "asset-orchard output commitment is not a canonical Orchard extracted note commitment",
        )
            })?;
    Ok(MerkleHashOrchard::from_cmx(&extracted))
}

fn base_from_merkle_hash(hash: &MerkleHashOrchard) -> Result<pallas::Base, AssetOrchardError> {
    Option::<pallas::Base>::from(pallas::Base::from_repr(hash.to_bytes())).ok_or_else(|| {
        AssetOrchardError::new(
            "invalid_asset_orchard_merkle_hash",
            "asset-orchard Merkle hash is not a canonical Pallas base field element",
        )
    })
}

fn merkle_root_from_nodes(
    mut level_nodes: Vec<MerkleHashOrchard>,
) -> Result<MerkleHashOrchard, AssetOrchardError> {
    if level_nodes.is_empty() {
        return Ok(MerkleHashOrchard::empty_root(Level::from(
            ASSET_ORCHARD_MERKLE_DEPTH as u8,
        )));
    }
    for level in 0..ASSET_ORCHARD_MERKLE_DEPTH {
        let level = Level::from(level as u8);
        let empty = MerkleHashOrchard::empty_root(level);
        let mut next_level = Vec::with_capacity(level_nodes.len().div_ceil(2));
        for chunk in level_nodes.chunks(2) {
            let left = &chunk[0];
            let right = chunk.get(1).unwrap_or(&empty);
            next_level.push(MerkleHashOrchard::combine(level, left, right));
        }
        level_nodes = next_level;
    }
    level_nodes.into_iter().next().ok_or_else(|| {
        AssetOrchardError::new(
            "asset_orchard_merkle_root_empty",
            "asset-orchard Merkle root computation produced no root",
        )
    })
}

fn merkle_witness_from_nodes(
    mut level_nodes: Vec<MerkleHashOrchard>,
    position: usize,
) -> Result<AssetOrchardMerkleWitness, AssetOrchardError> {
    let mut current_index = position;
    let mut auth_path = Vec::with_capacity(ASSET_ORCHARD_MERKLE_DEPTH);
    for level in 0..ASSET_ORCHARD_MERKLE_DEPTH {
        let level = Level::from(level as u8);
        let empty = MerkleHashOrchard::empty_root(level);
        let sibling_index = if current_index % 2 == 0 {
            current_index + 1
        } else {
            current_index - 1
        };
        let sibling = level_nodes.get(sibling_index).unwrap_or(&empty);
        auth_path.push(base_from_merkle_hash(sibling)?);

        let mut next_level = Vec::with_capacity(level_nodes.len().div_ceil(2));
        for chunk in level_nodes.chunks(2) {
            let left = &chunk[0];
            let right = chunk.get(1).unwrap_or(&empty);
            next_level.push(MerkleHashOrchard::combine(level, left, right));
        }
        level_nodes = next_level;
        current_index /= 2;
    }
    let position = u32::try_from(position).map_err(|_| {
        AssetOrchardError::new(
            "asset_orchard_merkle_position_overflow",
            "asset-orchard Merkle position does not fit u32",
        )
    })?;
    Ok(AssetOrchardMerkleWitness {
        position,
        auth_path: auth_path.try_into().map_err(|_| {
            AssetOrchardError::new(
                "asset_orchard_merkle_witness_depth_mismatch",
                "asset-orchard Merkle witness auth path depth mismatch",
            )
        })?,
    })
}

fn asset_orchard_public_fields_from_witnesses(
    pool_domain: pallas::Base,
    anchor: pallas::Base,
    inputs: &[AssetOrchardSwapNoteWitness; ASSET_ORCHARD_LEG_COUNT],
    outputs: &[AssetOrchardSwapNoteWitness; ASSET_ORCHARD_LEG_COUNT],
    encrypted_outputs: &[AssetOrchardBoundedBytes; ASSET_ORCHARD_LEG_COUNT],
    pricing_claim: &crate::asset_orchard::AssetOrchardPricingClaim,
) -> Result<AssetOrchardActionPublicFields, AssetOrchardError> {
    Ok(AssetOrchardActionPublicFields {
        pool_domain,
        anchor,
        nullifiers: asset_orchard_input_nullifiers(pool_domain, inputs)?,
        randomized_verification_keys: note_swap_randomized_verification_keys(inputs)?,
        output_commitments: [outputs[0].cmx, outputs[1].cmx],
        encrypted_output_hashes: [
            encrypted_output_hash(0, &encrypted_outputs[0].to_bytes()?)?,
            encrypted_output_hash(1, &encrypted_outputs[1].to_bytes()?)?,
        ],
        pricing: crate::asset_orchard::AssetOrchardPricingPublicFields {
            base_asset_tag: AssetTag {
                lo: pricing_claim.base_asset_tag_lo,
                hi: pricing_claim.base_asset_tag_hi,
            },
            quote_asset_tag: AssetTag {
                lo: pricing_claim.quote_asset_tag_lo,
                hi: pricing_claim.quote_asset_tag_hi,
            },
            ratio_numerator: pricing_claim.ratio_numerator,
            ratio_denominator: pricing_claim.ratio_denominator,
            commitment: pricing_claim.commitment_fields()?,
        },
        fee: 0,
    })
}

fn asset_orchard_input_nullifiers(
    pool_domain: pallas::Base,
    inputs: &[AssetOrchardSwapNoteWitness; ASSET_ORCHARD_LEG_COUNT],
) -> Result<[pallas::Base; ASSET_ORCHARD_LEG_COUNT], AssetOrchardError> {
    Ok([
        asset_derive_nullifier(
            pool_domain,
            inputs[0].nk,
            inputs[0].note.rho,
            inputs[0].note.psi,
            inputs[0].cmx,
        )?,
        asset_derive_nullifier(
            pool_domain,
            inputs[1].nk,
            inputs[1].note.rho,
            inputs[1].note.psi,
            inputs[1].cmx,
        )?,
    ])
}

fn note_swap_randomized_verification_keys(
    inputs: &[AssetOrchardSwapNoteWitness; ASSET_ORCHARD_LEG_COUNT],
) -> Result<[RandomizedVerificationKeyFields; ASSET_ORCHARD_LEG_COUNT], AssetOrchardError> {
    Ok([
        RandomizedVerificationKeyFields::from_affine(randomized_verification_key_point(
            &inputs[0],
        )?)?,
        RandomizedVerificationKeyFields::from_affine(randomized_verification_key_point(
            &inputs[1],
        )?)?,
    ])
}

fn randomized_verification_key_point(
    input: &AssetOrchardSwapNoteWitness,
) -> Result<pallas::Affine, AssetOrchardError> {
    let authority = input.spend_authority.as_ref().ok_or_else(|| {
        AssetOrchardError::new(
            "missing_asset_orchard_spend_authority",
            "asset-orchard input witness is missing spend authority",
        )
    })?;
    Ok((pallas::Point::from(authority.ak) + asset_spend_auth_g() * authority.alpha).to_affine())
}

fn signed_asset_orchard_action_from_witnesses(
    chain_id: &str,
    genesis_hash: [u8; 32],
    protocol_version: u32,
    inputs: &[AssetOrchardSwapNoteWitness; ASSET_ORCHARD_LEG_COUNT],
    input_notes: &[AssetOrchardWalletNote; ASSET_ORCHARD_LEG_COUNT],
    output_notes: &[AssetOrchardWalletNote; ASSET_ORCHARD_LEG_COUNT],
    fields: AssetOrchardActionPublicFields,
    encrypted_outputs: [AssetOrchardBoundedBytes; ASSET_ORCHARD_LEG_COUNT],
    signing_keys: [&SigningKey<SpendAuth>; ASSET_ORCHARD_LEG_COUNT],
    proof: Vec<u8>,
    pricing_claim: crate::asset_orchard::AssetOrchardPricingClaim,
) -> Result<AssetOrchardSwapAction, AssetOrchardError> {
    let rk_points = [
        randomized_verification_key_point(&inputs[0])?,
        randomized_verification_key_point(&inputs[1])?,
    ];
    let binding = AssetOrchardSwapBindingHash::from_bytes(&swap_binding_hash(&fields)?);
    let (accounting_inputs, accounting_outputs) =
        asset_orchard_swap_accounting_records(input_notes, output_notes)?;
    let placeholder = AssetOrchardSpendAuthSignature::from_orchard(
        &signing_keys[0].sign(OsRng, b"asset-orchard-placeholder"),
    );
    let mut action = AssetOrchardSwapAction {
        version: ASSET_ORCHARD_ACTION_VERSION_V1,
        schema: ASSET_ORCHARD_ACTION_SCHEMA_V1.to_string(),
        pool_id: ASSET_ORCHARD_POOL_ID_V1.to_string(),
        proof_system_id: ASSET_ORCHARD_PROOF_SYSTEM_ID_V1.to_string(),
        circuit_id: ASSET_ORCHARD_CIRCUIT_ID_V1.to_string(),
        pool_domain: AssetOrchardFieldElement::from_field(fields.pool_domain),
        anchor: AssetOrchardFieldElement::from_field(fields.anchor),
        nullifiers: fields
            .nullifiers
            .into_iter()
            .map(AssetOrchardFieldElement::from_field)
            .collect(),
        randomized_verification_keys: rk_points
            .into_iter()
            .map(AssetOrchardPoint::from_affine)
            .collect::<Result<Vec<_>, _>>()?,
        output_commitments: output_notes
            .iter()
            .map(|note| note.output_commitment.clone())
            .collect(),
        encrypted_outputs: encrypted_outputs.to_vec(),
        accounting_inputs,
        accounting_outputs,
        pricing_claim,
        swap_binding_hash: binding,
        fee: 0,
        proof: AssetOrchardProofBytes::from_bytes(&proof)?,
        spend_authorization_signatures: vec![placeholder.clone(), placeholder],
    };
    let sighash = action.sighash(chain_id, genesis_hash, protocol_version)?;
    action.spend_authorization_signatures = signing_keys
        .into_iter()
        .zip(inputs.iter())
        .map(|(key, input)| {
            let alpha = input
                .spend_authority
                .as_ref()
                .ok_or_else(|| {
                    AssetOrchardError::new(
                        "missing_asset_orchard_spend_authority",
                        "asset-orchard input witness is missing spend authority",
                    )
                })?
                .alpha;
            Ok(AssetOrchardSpendAuthSignature::from_orchard(
                &key.randomize(&alpha).sign(OsRng, &sighash),
            ))
        })
        .collect::<Result<Vec<_>, AssetOrchardError>>()?;
    action.validate()?;
    Ok(action)
}
