//! This module provides an implementation of a variant of (Turbo)[PLONK][plonk]
//! that is designed specifically for the polynomial commitment scheme described
//! in the [Halo][halo] paper.
//!
//! [halo]: https://eprint.iacr.org/2019/1021
//! [plonk]: https://eprint.iacr.org/2019/953

use blake2b_simd::Params as Blake2bParams;
use group::ff::{Field, FromUniformBytes, PrimeField};

use crate::arithmetic::CurveAffine;
use crate::helpers::CurveRead;
use crate::poly::{
    Coeff, EvaluationDomain, ExtendedLagrangeCoeff, LagrangeCoeff, PinnedEvaluationDomain,
    Polynomial,
};
use crate::transcript::{ChallengeScalar, EncodedChallenge, Transcript};

mod assigned;
mod circuit;
mod error;
mod keygen;
mod lookup;
pub(crate) mod permutation;
mod vanishing;

mod prover;
mod verifier;

pub use assigned::*;
pub use circuit::*;
pub use error::*;
pub use keygen::*;
pub use prover::*;
pub use verifier::*;

use std::io;

/// This is a verifying key which allows for the verification of proofs for a
/// particular circuit.
#[derive(Clone, Debug)]
pub struct VerifyingKey<C: CurveAffine> {
    domain: EvaluationDomain<C::Scalar>,
    fixed_commitments: Vec<C>,
    permutation: permutation::VerifyingKey<C>,
    cs: ConstraintSystem<C::Scalar>,
    /// Cached maximum degree of `cs` (which doesn't change after construction).
    cs_degree: usize,
    /// The representative of this `VerifyingKey` in transcripts.
    transcript_repr: C::Scalar,
}

impl<C: CurveAffine> VerifyingKey<C>
where
    C::Scalar: FromUniformBytes<64>,
{
    fn from_parts(
        domain: EvaluationDomain<C::Scalar>,
        fixed_commitments: Vec<C>,
        permutation: permutation::VerifyingKey<C>,
        cs: ConstraintSystem<C::Scalar>,
    ) -> Self {
        // Compute cached values.
        let cs_degree = cs.degree();

        let mut vk = Self {
            domain,
            fixed_commitments,
            permutation,
            cs,
            cs_degree,
            // Temporary, this is not pinned.
            transcript_repr: C::Scalar::ZERO,
        };

        let mut hasher = Blake2bParams::new()
            .hash_length(64)
            .personal(b"Halo2-Verify-Key")
            .to_state();

        let s = format!("{:?}", vk.pinned());

        hasher.update(&(s.len() as u64).to_le_bytes());
        hasher.update(s.as_bytes());

        // Hash in final Blake2bState
        vk.transcript_repr = C::Scalar::from_uniform_bytes(hasher.finalize().as_array());

        vk
    }
}

impl<C: CurveAffine> VerifyingKey<C> {
    /// Hashes a verification key into a transcript.
    pub fn hash_into<E: EncodedChallenge<C>, T: Transcript<C, E>>(
        &self,
        transcript: &mut T,
    ) -> io::Result<()> {
        transcript.common_scalar(self.transcript_repr)?;

        Ok(())
    }

    /// Obtains a pinned representation of this verification key that contains
    /// the minimal information necessary to reconstruct the verification key.
    pub fn pinned(&self) -> PinnedVerificationKey<'_, C> {
        PinnedVerificationKey {
            base_modulus: C::Base::MODULUS,
            scalar_modulus: C::Scalar::MODULUS,
            domain: self.domain.pinned(),
            fixed_commitments: &self.fixed_commitments,
            permutation: &self.permutation,
            cs: self.cs.pinned(),
        }
    }
}

/// Minimal representation of a verification key that can be used to identify
/// its active contents.
#[allow(dead_code)]
#[derive(Debug)]
pub struct PinnedVerificationKey<'a, C: CurveAffine> {
    base_modulus: &'static str,
    scalar_modulus: &'static str,
    domain: PinnedEvaluationDomain<'a, C::Scalar>,
    cs: PinnedConstraintSystem<'a, C::Scalar>,
    fixed_commitments: &'a Vec<C>,
    permutation: &'a permutation::VerifyingKey<C>,
}

/// Bounded, serializable verifier-key assembly for reconstructing a pinned
/// [`VerifyingKey`] without re-running circuit synthesis and fixed-column
/// commitments.
#[derive(Clone, Debug)]
pub struct VerifyingKeyPinnedAssembly<C: CurveAffine> {
    /// Fixed-column and compressed-selector polynomial commitments in final
    /// verifier order.
    pub fixed_commitments: Vec<C>,
    /// Permutation commitments in the final constraint-system permutation order.
    pub permutation_commitments: Vec<C>,
    /// Pre-compression selector activation rows, indexed selector first.
    pub selectors: Vec<Vec<bool>>,
}

/// Limits used when reading a [`VerifyingKeyPinnedAssembly`] from bytes.
#[derive(Clone, Copy, Debug)]
pub struct VerifyingKeyPinnedAssemblyLimits {
    /// Maximum number of fixed commitments accepted.
    pub max_fixed_commitments: usize,
    /// Maximum number of permutation commitments accepted.
    pub max_permutation_commitments: usize,
    /// Maximum number of selectors accepted.
    pub max_selectors: usize,
    /// Maximum number of rows accepted per selector.
    pub max_selector_rows: usize,
}

impl<C: CurveAffine> VerifyingKeyPinnedAssembly<C> {
    const MAGIC: &'static [u8] = b"halo2.verifying_key_pinned_assembly.v1\n";

    /// Writes the pinned assembly using a deterministic little-endian binary
    /// format. This contains verifier material only, not proving material.
    pub fn write<W: io::Write>(&self, writer: &mut W) -> io::Result<()> {
        writer.write_all(Self::MAGIC)?;
        write_usize_as_u64(writer, self.fixed_commitments.len())?;
        for commitment in &self.fixed_commitments {
            writer.write_all(commitment.to_bytes().as_ref())?;
        }
        write_usize_as_u64(writer, self.permutation_commitments.len())?;
        for commitment in &self.permutation_commitments {
            writer.write_all(commitment.to_bytes().as_ref())?;
        }
        write_usize_as_u64(writer, self.selectors.len())?;
        for selector in &self.selectors {
            write_usize_as_u64(writer, selector.len())?;
            for enabled in selector {
                writer.write_all(&[u8::from(*enabled)])?;
            }
        }
        Ok(())
    }

    /// Reads the pinned assembly with caller-provided bounds to avoid
    /// attacker-controlled allocation growth.
    pub fn read_with_limits<R: io::Read>(
        reader: &mut R,
        limits: VerifyingKeyPinnedAssemblyLimits,
    ) -> io::Result<Self> {
        let mut magic = vec![0u8; Self::MAGIC.len()];
        reader.read_exact(&mut magic)?;
        if magic != Self::MAGIC {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid Halo2 pinned VK assembly magic",
            ));
        }

        let fixed_count = read_bounded_usize(reader, limits.max_fixed_commitments, "fixed")?;
        let mut fixed_commitments = Vec::with_capacity(fixed_count);
        for _ in 0..fixed_count {
            fixed_commitments.push(C::read(reader)?);
        }

        let permutation_count =
            read_bounded_usize(reader, limits.max_permutation_commitments, "permutation")?;
        let mut permutation_commitments = Vec::with_capacity(permutation_count);
        for _ in 0..permutation_count {
            permutation_commitments.push(C::read(reader)?);
        }

        let selector_count = read_bounded_usize(reader, limits.max_selectors, "selectors")?;
        let mut selectors = Vec::with_capacity(selector_count);
        for _ in 0..selector_count {
            let selector_len =
                read_bounded_usize(reader, limits.max_selector_rows, "selector rows")?;
            let mut selector = Vec::with_capacity(selector_len);
            for _ in 0..selector_len {
                let mut byte = [0u8; 1];
                reader.read_exact(&mut byte)?;
                match byte[0] {
                    0 => selector.push(false),
                    1 => selector.push(true),
                    _ => {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            "invalid selector boolean byte",
                        ));
                    }
                }
            }
            selectors.push(selector);
        }

        Ok(Self {
            fixed_commitments,
            permutation_commitments,
            selectors,
        })
    }
}

fn write_usize_as_u64<W: io::Write>(writer: &mut W, value: usize) -> io::Result<()> {
    let value = u64::try_from(value).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "length does not fit in u64 for pinned VK assembly",
        )
    })?;
    writer.write_all(&value.to_le_bytes())
}

fn read_bounded_usize<R: io::Read>(
    reader: &mut R,
    limit: usize,
    label: &'static str,
) -> io::Result<usize> {
    let mut bytes = [0u8; 8];
    reader.read_exact(&mut bytes)?;
    let value = usize::try_from(u64::from_le_bytes(bytes)).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{label} length does not fit in usize"),
        )
    })?;
    if value > limit {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{label} length exceeds pinned VK assembly limit"),
        ));
    }
    Ok(value)
}
/// This is a proving key which allows for the creation of proofs for a
/// particular circuit.
#[derive(Clone, Debug)]
pub struct ProvingKey<C: CurveAffine> {
    vk: VerifyingKey<C>,
    l0: Polynomial<C::Scalar, ExtendedLagrangeCoeff>,
    l_blind: Polynomial<C::Scalar, ExtendedLagrangeCoeff>,
    l_last: Polynomial<C::Scalar, ExtendedLagrangeCoeff>,
    fixed_values: Vec<Polynomial<C::Scalar, LagrangeCoeff>>,
    fixed_polys: Vec<Polynomial<C::Scalar, Coeff>>,
    fixed_cosets: Vec<Polynomial<C::Scalar, ExtendedLagrangeCoeff>>,
    permutation: permutation::ProvingKey<C>,
}

impl<C: CurveAffine> ProvingKey<C> {
    /// Get the underlying [`VerifyingKey`].
    pub fn get_vk(&self) -> &VerifyingKey<C> {
        &self.vk
    }
}

impl<C: CurveAffine> VerifyingKey<C> {
    /// Get the underlying [`EvaluationDomain`].
    pub fn get_domain(&self) -> &EvaluationDomain<C::Scalar> {
        &self.domain
    }
}

#[derive(Clone, Copy, Debug)]
struct Theta;
type ChallengeTheta<F> = ChallengeScalar<F, Theta>;

#[derive(Clone, Copy, Debug)]
struct Beta;
type ChallengeBeta<F> = ChallengeScalar<F, Beta>;

#[derive(Clone, Copy, Debug)]
struct Gamma;
type ChallengeGamma<F> = ChallengeScalar<F, Gamma>;

#[derive(Clone, Copy, Debug)]
struct Y;
type ChallengeY<F> = ChallengeScalar<F, Y>;

#[derive(Clone, Copy, Debug)]
struct X;
type ChallengeX<F> = ChallengeScalar<F, X>;
