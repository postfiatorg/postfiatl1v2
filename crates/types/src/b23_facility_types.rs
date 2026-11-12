/// The two credits released by one primary redemption, denominated in the same
/// exact settlement asset. Absent on legacy receipts to preserve their bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PftlUniswapRedemptionSettlement {
    pub settlement_asset_id: String,
    pub settlement_recipient: String,
    pub settlement_value_atoms: u64,
    pub spread_recipient: String,
    pub spread_value_atoms: u64,
}

impl PftlUniswapRedemptionSettlement {
    pub fn validate(&self) -> Result<(), String> {
        validate_lower_hex_len(
            "redemption_settlement.settlement_asset_id",
            &self.settlement_asset_id,
            ISSUED_ASSET_ID_HEX_LEN,
        )?;
        for (field, value) in [
            (
                "redemption_settlement.settlement_recipient",
                &self.settlement_recipient,
            ),
            (
                "redemption_settlement.spread_recipient",
                &self.spread_recipient,
            ),
        ] {
            validate_text_field(field, value)?;
            if value.contains(['\n', '\r']) {
                return Err(format!("{field} must not contain line breaks"));
            }
        }
        if self.settlement_value_atoms == 0
            || self
                .settlement_value_atoms
                .checked_add(self.spread_value_atoms)
                .is_none()
        {
            return Err(
                "redemption settlement must be positive and total must fit u64".to_string(),
            );
        }
        Ok(())
    }

    pub fn canonical_fields(&self) -> String {
        format!(
            "redemption_settlement.settlement_asset_id={}\nredemption_settlement.settlement_recipient={}\nredemption_settlement.settlement_value_atoms={}\nredemption_settlement.spread_recipient={}\nredemption_settlement.spread_value_atoms={}\n",
            self.settlement_asset_id, self.settlement_recipient, self.settlement_value_atoms,
            self.spread_recipient, self.spread_value_atoms,
        )
    }
}
