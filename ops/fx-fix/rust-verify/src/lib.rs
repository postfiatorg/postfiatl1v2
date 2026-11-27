//! Cross-language Rust verification tests confirming Python `fx_fix_executor`
//! produces byte-identical `FxFixPacketV1` hashes, `fx_fix_register_v1`
//! operations, and `fx_fix_reservation_id` values matching `postfiat-types`.

use postfiat_types::FxFixRegisterOperationV1;

pub fn verify_fx_fix_register_json(raw: &str) -> Result<FxFixRegisterOperationV1, String> {
    let op: FxFixRegisterOperationV1 =
        serde_json::from_str(raw).map_err(|err| format!("json decode failed: {err}"))?;
    op.validate()?;
    Ok(op)
}

#[cfg(test)]
mod tests {
    use super::*;
    use postfiat_types::{
        fx_fix_reservation_id, FxFixPacketV1, FxFixReservationCreateOperationV1,
        FX_FIX_PACKET_SCHEMA_V1, FX_FIX_PACKET_VERSION_V1,
    };
    use std::process::Command;

    #[test]
    fn python_fx_fix_packet_and_reservation_match_rust_canonical_types() {
        let operator = "pf1111111111111111111111111111111111111111";
        let base_asset_id = "aa".repeat(48);
        let quote_asset_id = "bb".repeat(48);
        let source_commitment = "11".repeat(48);
        let governance_policy_hash = "22".repeat(48);

        let mut rust_packet = FxFixPacketV1 {
            version: FX_FIX_PACKET_VERSION_V1,
            schema: FX_FIX_PACKET_SCHEMA_V1.to_string(),
            operator: operator.to_string(),
            base_asset_id: base_asset_id.clone(),
            quote_asset_id: quote_asset_id.clone(),
            epoch: 1,
            ratio_numerator: 21,
            ratio_denominator: 2_000_000,
            band_bps: 15,
            fee_bps: 10,
            valid_from_height: 100_000,
            expires_at_height: 100_900,
            minimum_base_atoms: 20_000_000,
            capacity_base_atoms: 100_000_000,
            capacity_quote_atoms: 2_000,
            max_fills: 8,
            source_label: "london_1600_wm_fix".to_string(),
            source_observation_commitment: source_commitment.clone(),
            governance_policy_hash: governance_policy_hash.clone(),
            previous_fix_hash: None,
            packet_hash: String::new(),
        };
        rust_packet.packet_hash = rust_packet.canonical_hash();
        rust_packet.validate().expect("rust packet valid");

        let action_binding = "33".repeat(64);
        let wallet_intent = "34".repeat(48);
        let nonce = "44".repeat(48);
        let rust_res_id = fx_fix_reservation_id(
            &rust_packet.packet_hash,
            operator,
            &action_binding,
            20_000_000,
            210,
            &wallet_intent,
            &nonce,
        )
        .expect("rust reservation id");

        // Invoke Python fx_fix_executor and verify exact cross-language equality.
        let py_script = format!(
            concat!(
                "import json, sys; sys.path.insert(0, '..'); ",
                "from fx_fix_executor import FxFixPacketV1, build_fx_fix_register_op, build_fx_fix_reservation_create_op; ",
                "pkt = FxFixPacketV1(",
                "version=1, schema='postfiat.fx_fix.packet.v1', operator='{op}', ",
                "base_asset_id='{base}', quote_asset_id='{quote}', epoch=1, ",
                "ratio_numerator=21, ratio_denominator=2000000, band_bps=15, fee_bps=10, ",
                "valid_from_height=100000, expires_at_height=100900, ",
                "minimum_base_atoms=20000000, capacity_base_atoms=100000000, ",
                "capacity_quote_atoms=2000, max_fills=8, source_label='london_1600_wm_fix', ",
                "source_observation_commitment='{src}', governance_policy_hash='{gov}', ",
                "previous_fix_hash=None).with_computed_hash(); ",
                "reg = build_fx_fix_register_op(pkt, nav_tracked=True, nav_fix_fee_cap_bps=25); ",
                "res_op, res_id = build_fx_fix_reservation_create_op(",
                "operator='{op}', fix_packet_hash=pkt.packet_hash, action_binding_hash='{ab}', ",
                "base_atoms=20000000, quote_atoms=210, wallet_intent_hash='{wi}', ",
                "reservation_nonce='{rn}', expires_at_height=100300); ",
                "print(json.dumps({{'register': {{'operator': reg['operator'], 'packet': reg['packet']}}, ",
                "'reservation': {{k: v for k, v in res_op.items() if k != 'operation'}}, ",
                "'reservation_id': res_id}}))"
            ),
            op = operator,
            base = base_asset_id,
            quote = quote_asset_id,
            src = source_commitment,
            gov = governance_policy_hash,
            ab = action_binding,
            wi = wallet_intent,
            rn = nonce,
        );

        let output = Command::new("python3")
            .arg("-c")
            .arg(&py_script)
            .output()
            .expect("run python3 fx_fix_executor parity check");
        assert!(
            output.status.success(),
            "python parity script failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let value: serde_json::Value =
            serde_json::from_slice(&output.stdout).expect("decode python json");

        let reg_json = serde_json::to_string(&value["register"]).expect("encode register");
        let parsed_reg = verify_fx_fix_register_json(&reg_json).expect("rust validates python op");
        assert_eq!(parsed_reg.packet.packet_hash, rust_packet.packet_hash);
        assert_eq!(parsed_reg.packet, rust_packet);

        let parsed_res: FxFixReservationCreateOperationV1 =
            serde_json::from_value(value["reservation"].clone())
                .expect("rust deserializes python reservation op");
        parsed_res
            .validate()
            .expect("rust validates python reservation op");
        assert_eq!(parsed_res.reservation_id().expect("id"), rust_res_id);
        assert_eq!(
            value["reservation_id"].as_str().expect("py res id"),
            rust_res_id
        );
    }
}
