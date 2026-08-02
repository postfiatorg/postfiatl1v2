use near_sdk::borsh::{
    schema::{add_definition, Declaration, Definition},
    BorshDeserialize, BorshSchema, BorshSerialize,
};
use near_sdk::json_types::{Base64VecU8, U128};
use near_sdk::serde::Serialize;
use near_sdk::{
    env, ext_contract, near, require, AccountId, Gas, NearToken, Promise, PromiseError,
};
use std::io::Write;

pub const READER_EVENT_STANDARD: &str = "postfiat-nav";
pub const READER_EVENT_VERSION: &str = "1.0.0";
pub const READER_EVENT_NAME: &str = "NearStakeSnapshot";
pub const SNAPSHOT_SALT_LEN: usize = 32;
const BALANCE_READ_GAS: Gas = Gas::from_tgas(12);
const CALLBACK_GAS: Gas = Gas::from_tgas(24);

/// Public, stateless reader for a standard NEAR staking-pool account position.
///
/// The reader obtains the staked and unstaked balances from the pool at the
/// same source receipt, emits their canonical commitment, and returns the raw
/// canonical payload. The public reserve-proof verifier checks receipt
/// inclusion, payload/event equality, deployed reader and pool code hashes,
/// owner authorization, and the governed PFTL checkpoint independently.
#[near(contract_state)]
#[derive(Default)]
pub struct NearStakeReader {}

#[ext_contract(ext_staking_pool)]
#[allow(dead_code)]
trait ExtStakingPool {
    fn get_account_staked_balance(&self, account_id: AccountId) -> U128;
    fn get_account_unstaked_balance(&self, account_id: AccountId) -> U128;
}

#[near]
impl NearStakeReader {
    #[init]
    pub fn new() -> Self {
        Self {}
    }

    /// Reads a position without accepting custody or attached value.
    pub fn snapshot(
        &self,
        pool_id: AccountId,
        account_id: AccountId,
        salt: Base64VecU8,
    ) -> Promise {
        require!(
            salt.0.len() == SNAPSHOT_SALT_LEN,
            "salt must be exactly 32 bytes"
        );
        require!(
            env::attached_deposit().is_zero(),
            "snapshot must not attach funds"
        );
        let staked = ext_staking_pool::ext(pool_id.clone())
            .with_static_gas(BALANCE_READ_GAS)
            .with_attached_deposit(NearToken::from_yoctonear(0))
            .get_account_staked_balance(account_id.clone());
        let unstaked = ext_staking_pool::ext(pool_id.clone())
            .with_static_gas(BALANCE_READ_GAS)
            .with_attached_deposit(NearToken::from_yoctonear(0))
            .get_account_unstaked_balance(account_id.clone());
        staked.and(unstaked).then(
            Self::ext(env::current_account_id())
                .with_static_gas(CALLBACK_GAS)
                .with_attached_deposit(NearToken::from_yoctonear(0))
                .on_snapshot(pool_id, account_id, salt),
        )
    }

    #[private]
    #[result_serializer(borsh)]
    pub fn on_snapshot(
        &self,
        pool_id: AccountId,
        account_id: AccountId,
        salt: Base64VecU8,
        #[callback_result] staked: Result<U128, PromiseError>,
        #[callback_result] unstaked: Result<U128, PromiseError>,
    ) -> RawPayload {
        require!(
            salt.0.len() == SNAPSHOT_SALT_LEN,
            "salt must be exactly 32 bytes"
        );
        let staked = staked.unwrap_or_else(|_| env::panic_str("staked balance read failed"));
        let unstaked = unstaked.unwrap_or_else(|_| env::panic_str("unstaked balance read failed"));
        let salt = fixed_salt(&salt.0);
        let block_timestamp = env::block_timestamp();
        let payload = encode_snapshot_payload(
            &account_id,
            &pool_id,
            staked.0,
            unstaked.0,
            block_timestamp,
            salt,
        );
        let commitment = env::sha256(&payload);
        emit_snapshot_event(&commitment, block_timestamp, &payload);
        RawPayload(payload)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RawPayload(pub Vec<u8>);

impl BorshSerialize for RawPayload {
    fn serialize<W: Write>(&self, writer: &mut W) -> std::io::Result<()> {
        writer.write_all(&self.0)
    }
}

impl BorshDeserialize for RawPayload {
    fn deserialize_reader<R: std::io::Read>(reader: &mut R) -> std::io::Result<Self> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes)?;
        Ok(Self(bytes))
    }
}

impl BorshSchema for RawPayload {
    fn add_definitions_recursively(
        definitions: &mut std::collections::BTreeMap<Declaration, Definition>,
    ) {
        add_definition(
            Self::declaration(),
            Definition::Sequence {
                length_width: 0,
                length_range: 0..=u64::MAX,
                elements: <u8 as BorshSchema>::declaration(),
            },
            definitions,
        );
        <u8 as BorshSchema>::add_definitions_recursively(definitions);
    }

    fn declaration() -> Declaration {
        "RawPayloadBytes".to_owned()
    }
}

#[derive(Serialize)]
#[serde(crate = "near_sdk::serde")]
struct Nep297Event<'a> {
    standard: &'static str,
    version: &'static str,
    event: &'static str,
    data: [NearStakeSnapshotEvent<'a>; 1],
}

#[derive(Serialize)]
#[serde(crate = "near_sdk::serde")]
struct NearStakeSnapshotEvent<'a> {
    commitment: String,
    block_timestamp: u64,
    payload: Base64Payload<'a>,
}

struct Base64Payload<'a>(&'a [u8]);

impl Serialize for Base64Payload<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: near_sdk::serde::Serializer,
    {
        near_sdk::serde::Serializer::serialize_str(serializer, &base64_encode(self.0))
    }
}

pub fn encode_snapshot_payload(
    account_id: &AccountId,
    pool_id: &AccountId,
    staked: u128,
    unstaked: u128,
    block_timestamp: u64,
    salt: [u8; 32],
) -> Vec<u8> {
    let mut out = Vec::with_capacity(account_id.as_str().len() + pool_id.as_str().len() + 84);
    put_string(&mut out, account_id.as_str());
    put_string(&mut out, pool_id.as_str());
    out.extend_from_slice(&staked.to_le_bytes());
    out.extend_from_slice(&unstaked.to_le_bytes());
    out.extend_from_slice(&block_timestamp.to_le_bytes());
    out.extend_from_slice(&salt);
    out
}

fn emit_snapshot_event(commitment: &[u8], block_timestamp: u64, payload: &[u8]) {
    let event = Nep297Event {
        standard: READER_EVENT_STANDARD,
        version: READER_EVENT_VERSION,
        event: READER_EVENT_NAME,
        data: [NearStakeSnapshotEvent {
            commitment: bs58_encode(commitment),
            block_timestamp,
            payload: Base64Payload(payload),
        }],
    };
    let json = near_sdk::serde_json::to_string(&event).expect("event serialization cannot fail");
    env::log_str(&format!("EVENT_JSON:{json}"));
}

fn fixed_salt(salt: &[u8]) -> [u8; 32] {
    if salt.len() != SNAPSHOT_SALT_LEN {
        env::panic_str("salt must be exactly 32 bytes");
    }
    let mut out = [0u8; SNAPSHOT_SALT_LEN];
    out.copy_from_slice(salt);
    out
}

fn put_string(out: &mut Vec<u8>, value: &str) {
    let len = u32::try_from(value.len()).unwrap_or_else(|_| env::panic_str("string too long"));
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(value.as_bytes());
}

fn bs58_encode(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 58] = b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
    if bytes.is_empty() {
        return String::new();
    }
    let zeroes = bytes.iter().take_while(|byte| **byte == 0).count();
    let mut digits = vec![0u8; bytes.len() * 138 / 100 + 1];
    let mut length = 0usize;
    for byte in bytes {
        let mut carry = u32::from(*byte);
        for digit in digits.iter_mut().rev().take(length) {
            carry += u32::from(*digit) << 8;
            *digit = (carry % 58) as u8;
            carry /= 58;
        }
        while carry > 0 {
            let index = digits.len() - length - 1;
            digits[index] = (carry % 58) as u8;
            length += 1;
            carry /= 58;
        }
    }
    let mut out = String::with_capacity(zeroes + length);
    for _ in 0..zeroes {
        out.push('1');
    }
    for digit in &digits[digits.len() - length..] {
        out.push(ALPHABET[usize::from(*digit)] as char);
    }
    out
}

fn base64_encode(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0];
        let b1 = *chunk.get(1).unwrap_or(&0);
        let b2 = *chunk.get(2).unwrap_or(&0);
        out.push(ALPHABET[usize::from(b0 >> 2)] as char);
        out.push(ALPHABET[usize::from(((b0 & 0x03) << 4) | (b1 >> 4))] as char);
        if chunk.len() > 1 {
            out.push(ALPHABET[usize::from(((b1 & 0x0f) << 2) | (b2 >> 6))] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(ALPHABET[usize::from(b2 & 0x3f)] as char);
        } else {
            out.push('=');
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
    use near_sdk::test_utils::{get_logs, VMContextBuilder};
    use near_sdk::testing_env;

    #[test]
    fn payload_encoding_is_canonical() {
        let account_id: AccountId =
            "eed15bedebb4ac46d1528187a8c2f00aa59b441398d3e346c44eb2dcb2fc1d9a"
                .parse()
                .unwrap();
        let pool_id: AccountId = "astro-stakers.poolv1.near".parse().unwrap();
        let salt = [7u8; 32];
        let payload = encode_snapshot_payload(
            &account_id,
            &pool_id,
            123_456_789_000_000_000_000_000_000,
            42,
            1_781_384_407_636_935_368,
            salt,
        );
        let mut expected = Vec::new();
        expected.extend_from_slice(&(account_id.as_str().len() as u32).to_le_bytes());
        expected.extend_from_slice(account_id.as_bytes());
        expected.extend_from_slice(&(pool_id.as_str().len() as u32).to_le_bytes());
        expected.extend_from_slice(pool_id.as_bytes());
        expected.extend_from_slice(&123_456_789_000_000_000_000_000_000u128.to_le_bytes());
        expected.extend_from_slice(&42u128.to_le_bytes());
        expected.extend_from_slice(&1_781_384_407_636_935_368u64.to_le_bytes());
        expected.extend_from_slice(&salt);
        assert_eq!(payload, expected);
    }

    #[test]
    fn callback_returns_raw_payload_and_emits_event() {
        let account_id: AccountId =
            "eed15bedebb4ac46d1528187a8c2f00aa59b441398d3e346c44eb2dcb2fc1d9a"
                .parse()
                .unwrap();
        let pool_id: AccountId = "astro-stakers.poolv1.near".parse().unwrap();
        let salt = vec![9u8; SNAPSHOT_SALT_LEN];
        let timestamp = 1_781_384_407_636_935_368u64;
        let mut context = VMContextBuilder::new();
        context
            .current_account_id("reader.test.near".parse().unwrap())
            .predecessor_account_id("reader.test.near".parse().unwrap())
            .block_timestamp(timestamp);
        testing_env!(context.build());

        let raw = NearStakeReader::new().on_snapshot(
            pool_id.clone(),
            account_id.clone(),
            Base64VecU8(salt),
            Ok(U128(1000)),
            Ok(U128(200)),
        );
        let expected =
            encode_snapshot_payload(&account_id, &pool_id, 1000, 200, timestamp, [9u8; 32]);
        assert_eq!(raw.0, expected);

        let logs = get_logs();
        assert_eq!(logs.len(), 1);
        let event = logs[0].strip_prefix("EVENT_JSON:").unwrap();
        let value: near_sdk::serde_json::Value = near_sdk::serde_json::from_str(event).unwrap();
        assert_eq!(value["standard"], READER_EVENT_STANDARD);
        assert_eq!(value["version"], READER_EVENT_VERSION);
        assert_eq!(value["event"], READER_EVENT_NAME);
        assert_eq!(value["data"][0]["block_timestamp"], timestamp);
        assert_eq!(
            value["data"][0]["payload"].as_str().unwrap(),
            BASE64.encode(expected)
        );
    }

    #[test]
    fn raw_payload_borsh_serializer_writes_exact_bytes() {
        let raw = RawPayload(vec![1, 2, 3, 4]);
        let mut out = Vec::new();
        raw.serialize(&mut out).unwrap();
        assert_eq!(out, vec![1, 2, 3, 4]);
    }

    #[test]
    fn rejects_bad_salt_length() {
        let account_id: AccountId = "alice.near".parse().unwrap();
        let pool_id: AccountId = "pool.near".parse().unwrap();
        testing_env!(VMContextBuilder::new().build());
        let result = std::panic::catch_unwind(|| {
            let _ =
                NearStakeReader::new().snapshot(pool_id, account_id, Base64VecU8(vec![1, 2, 3]));
        });
        assert!(result.is_err());
    }

    #[test]
    fn rejects_attached_funds() {
        let mut context = VMContextBuilder::new();
        context.attached_deposit(NearToken::from_yoctonear(1));
        testing_env!(context.build());
        let result = std::panic::catch_unwind(|| {
            let _ = NearStakeReader::new().snapshot(
                "pool.near".parse().unwrap(),
                "alice.near".parse().unwrap(),
                Base64VecU8(vec![1; SNAPSHOT_SALT_LEN]),
            );
        });
        assert!(result.is_err());
    }
}
