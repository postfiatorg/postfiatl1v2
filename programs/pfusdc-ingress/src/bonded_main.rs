#![no_main]

sp1_zkvm::entrypoint!(main);

pub fn main() {
    let encoded = sp1_zkvm::io::read_vec();
    let input: pfusdc_ingress_program::bonded::PfUsdcBondedGuestInputV1 =
        serde_cbor::from_slice(&encoded).expect("decode canonical bonded guest input");
    let canonical = match input {
        pfusdc_ingress_program::bonded::PfUsdcBondedGuestInputV1::Ingress(witness) => {
            pfusdc_ingress_program::bonded::verify_bonded_ingress_witness_v1(&witness)
                .expect("verify bonded-ingress witness")
                .canonical_bytes_without_commitment()
                .expect("encode bonded-ingress public values")
        }
        pfusdc_ingress_program::bonded::PfUsdcBondedGuestInputV1::Confirmation(witness) => {
            pfusdc_ingress_program::bonded::verify_bonded_confirmation_witness_v1(&witness)
                .expect("verify bonded-confirmation witness")
                .canonical_bytes_without_commitment()
                .expect("encode bonded-confirmation public values")
        }
        pfusdc_ingress_program::bonded::PfUsdcBondedGuestInputV1::Reversion(witness) => {
            pfusdc_ingress_program::bonded::verify_bonded_reversion_witness_v1(&witness)
                .expect("verify bonded-reversion witness")
                .canonical_bytes_without_commitment()
                .expect("encode bonded-reversion public values")
        }
        pfusdc_ingress_program::bonded::PfUsdcBondedGuestInputV1::AgeRelease(witness) => {
            pfusdc_ingress_program::bonded::verify_bonded_age_release_witness_v1(&witness)
                .expect("verify bonded-age-release witness")
                .canonical_bytes_without_commitment()
                .expect("encode bonded-age-release public values")
        }
    };
    sp1_zkvm::io::commit_slice(&canonical);
}
