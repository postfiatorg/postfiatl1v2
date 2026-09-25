//! Deployed egress identities are immutable, even when host dependencies change.
use anyhow::{ensure, Context, Result};
use clap::ValueEnum;
use serde::Deserialize;
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, Default, ValueEnum)]
pub enum EgressRelease {
    ArcV2,
    #[default]
    PfethV1,
}

#[derive(Debug, Deserialize)]
pub struct ProgramIdentity {
    pub elf_sha256: String,
    pub program_vkey: String,
}

impl EgressRelease {
    pub fn identity(self) -> Result<ProgramIdentity> {
        #[derive(Deserialize)]
        struct Manifest {
            egress: ProgramIdentity,
        }
        let text = match self {
            Self::ArcV2 => include_str!("../../../docs/evidence/arc-mvp-20260828/program-info.current-v2-docker-20260902.json"),
            Self::PfethV1 => include_str!("../../../programs/pfusdc-egress/program-info.pfeth-v1.json"),
        };
        Ok(serde_json::from_str::<Manifest>(text)
            .context("decode pinned egress identity")?
            .egress)
    }
}

impl ProgramIdentity {
    pub fn verify_elf(&self, bytes: &[u8]) -> Result<()> {
        let actual = hex::encode(Sha256::digest(bytes));
        ensure!(actual == self.elf_sha256,
            "egress ELF identity mismatch: expected {}, received {}; select the retained ELF for the verifier's release",
            self.elf_sha256, actual);
        Ok(())
    }

    pub fn verify_key(&self, actual: &str) -> Result<()> {
        ensure!(
            actual == self.program_vkey,
            "egress verifying key mismatch: expected {}, received {}",
            self.program_vkey,
            actual
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retained_pfeth_identity_rejects_mutation_and_arc_substitution() {
        let bytes = include_bytes!("../../../programs/pfusdc-egress/elf/pfusdc-egress-program");
        let pfeth = EgressRelease::PfethV1.identity().unwrap();
        let arc = EgressRelease::ArcV2.identity().unwrap();
        pfeth.verify_elf(bytes).unwrap();
        assert!(arc.verify_elf(bytes).is_err());
        let mut changed = bytes.to_vec();
        changed[0] ^= 1;
        assert!(pfeth.verify_elf(&changed).is_err());
        pfeth.verify_key(&pfeth.program_vkey).unwrap();
        arc.verify_key(&arc.program_vkey).unwrap();
        assert!(pfeth.verify_key(&arc.program_vkey).is_err());
        assert!(arc.verify_key(&pfeth.program_vkey).is_err());
    }
}
