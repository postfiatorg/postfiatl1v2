//! Bounded CBOR decoding before typed YOLO witness or Nitro interpretation.
//!
//! Map duplicates must be rejected before a general deserializer can collapse
//! them. Nitro's signed payloads also permit bounded indefinite maps/arrays.
//! Witness inputs retain definite lengths. No floats or non-minimal heads.

use serde_cbor::Value;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy)]
pub struct CborLimits {
    pub bytes: usize,
    pub items: usize,
    pub depth: usize,
}

pub const NITRO_CBOR_LIMITS: CborLimits = CborLimits {
    bytes: 64 * 1024,
    items: 4096,
    depth: 32,
};

pub fn decode_strict_cbor(input: &[u8], limits: CborLimits) -> Result<Value, String> {
    decode_cbor(input, limits, false)
}

/// Validate definite CBOR without allocating a second tree of witness values.
/// Primitive map keys have a unique encoding because heads must be minimal.
/// Borrowing those encoded keys therefore preserves duplicate-key rejection.
pub fn validate_strict_cbor(input: &[u8], limits: CborLimits) -> Result<(), String> {
    if input.is_empty() || input.len() > limits.bytes {
        return Err("CBOR input size exceeds its bounds".into());
    }
    let mut reader = Reader {
        input,
        offset: 0,
        remaining_items: limits.items,
        limits,
        indefinite_containers: false,
    };
    reader.skip_definite(0)?;
    if reader.offset != input.len() {
        return Err("CBOR input has trailing bytes".into());
    }
    Ok(())
}

pub fn decode_nitro_cbor(input: &[u8]) -> Result<Value, String> {
    decode_cbor(input, NITRO_CBOR_LIMITS, true)
}

fn decode_cbor(
    input: &[u8],
    limits: CborLimits,
    indefinite_containers: bool,
) -> Result<Value, String> {
    if input.is_empty() || input.len() > limits.bytes {
        return Err("CBOR input size exceeds its bounds".into());
    }
    let mut reader = Reader {
        input,
        offset: 0,
        remaining_items: limits.items,
        limits,
        indefinite_containers,
    };
    let value = reader.item(0)?;
    if reader.offset != input.len() {
        return Err("CBOR input has trailing bytes".into());
    }
    Ok(value)
}

struct Reader<'a> {
    input: &'a [u8],
    offset: usize,
    remaining_items: usize,
    limits: CborLimits,
    indefinite_containers: bool,
}

impl<'a> Reader<'a> {
    fn skip_definite(&mut self, depth: usize) -> Result<(), String> {
        if depth > self.limits.depth {
            return Err("CBOR nesting exceeds its bound".into());
        }
        self.remaining_items = self
            .remaining_items
            .checked_sub(1)
            .ok_or("CBOR item limit exceeded")?;
        let initial = self.take(1)?[0];
        let major = initial >> 5;
        let additional = initial & 31;
        let argument = self.argument(additional)?;
        match major {
            0 | 1 => Ok(()),
            2 | 3 => {
                let length = usize::try_from(argument).map_err(|_| "CBOR length overflow")?;
                let bytes = self.take(length)?;
                if major == 3 {
                    std::str::from_utf8(bytes).map_err(|_| "CBOR text is not UTF-8")?;
                }
                Ok(())
            }
            4 | 5 => {
                let count = usize::try_from(argument).map_err(|_| "CBOR count overflow")?;
                let child_count = count
                    .checked_mul(if major == 5 { 2 } else { 1 })
                    .ok_or("CBOR count overflow")?;
                if child_count > self.remaining_items
                    || child_count > self.input.len() - self.offset
                {
                    return Err("CBOR container exceeds remaining bounds".into());
                }
                let input = self.input;
                let mut keys = BTreeSet::new();
                for _ in 0..count {
                    if major == 5 {
                        let start = self.offset;
                        let key_major = input.get(start).ok_or("CBOR input is truncated")? >> 5;
                        if key_major > 3 {
                            return Err("CBOR map key type is unsupported".into());
                        }
                        self.skip_definite(depth + 1)?;
                        if !keys.insert(&input[start..self.offset]) {
                            return Err("CBOR map contains a duplicate key".into());
                        }
                    }
                    self.skip_definite(depth + 1)?;
                }
                Ok(())
            }
            6 if argument == 18 && depth == 0 => self.skip_definite(depth + 1),
            6 => Err("CBOR tag is unsupported here".into()),
            7 if matches!(additional, 20..=22) => Ok(()),
            7 => Err("CBOR simple or floating-point value is unsupported".into()),
            _ => Err("CBOR type is unsupported".into()),
        }
    }

    fn take(&mut self, count: usize) -> Result<&'a [u8], String> {
        let end = self
            .offset
            .checked_add(count)
            .ok_or("CBOR length overflow")?;
        let bytes = self
            .input
            .get(self.offset..end)
            .ok_or("CBOR input is truncated")?;
        self.offset = end;
        Ok(bytes)
    }

    fn argument(&mut self, additional: u8) -> Result<u64, String> {
        let (width, minimum) = match additional {
            0..=23 => return Ok(u64::from(additional)),
            24 => (1, 24),
            25 => (2, 256),
            26 => (4, 65_536),
            27 => (8, 4_294_967_296),
            _ => return Err("indefinite or reserved CBOR head".into()),
        };
        let value = self
            .take(width)?
            .iter()
            .fold(0_u64, |n, byte| (n << 8) | u64::from(*byte));
        if value < minimum {
            return Err("CBOR head is not minimally encoded".into());
        }
        Ok(value)
    }

    fn item(&mut self, depth: usize) -> Result<Value, String> {
        if depth > self.limits.depth {
            return Err("CBOR nesting exceeds its bound".into());
        }
        self.remaining_items = self
            .remaining_items
            .checked_sub(1)
            .ok_or("CBOR item limit exceeded")?;
        let initial = self.take(1)?[0];
        let major = initial >> 5;
        let additional = initial & 31;
        let indefinite = self.indefinite_containers && additional == 31 && matches!(major, 4 | 5);
        let argument = if indefinite {
            0
        } else {
            self.argument(additional)?
        };
        match major {
            0 => Ok(Value::Integer(i128::from(argument))),
            1 => Ok(Value::Integer(-1 - i128::from(argument))),
            2 | 3 => {
                let length = usize::try_from(argument).map_err(|_| "CBOR length overflow")?;
                let bytes = self.take(length)?;
                if major == 2 {
                    Ok(Value::Bytes(bytes.to_vec()))
                } else {
                    Ok(Value::Text(
                        std::str::from_utf8(bytes)
                            .map_err(|_| "CBOR text is not UTF-8")?
                            .to_string(),
                    ))
                }
            }
            4 | 5 => {
                let count = usize::try_from(argument).map_err(|_| "CBOR count overflow")?;
                let child_count = count
                    .checked_mul(if major == 5 { 2 } else { 1 })
                    .ok_or("CBOR count overflow")?;
                if child_count > self.remaining_items
                    || child_count > self.input.len() - self.offset
                {
                    return Err("CBOR container exceeds remaining bounds".into());
                }
                if major == 4 {
                    let mut values = Vec::with_capacity(count);
                    while indefinite || values.len() < count {
                        if indefinite && self.input.get(self.offset) == Some(&0xff) {
                            self.offset += 1;
                            break;
                        }
                        values.push(self.item(depth + 1)?);
                    }
                    Ok(Value::Array(values))
                } else {
                    let mut values = BTreeMap::new();
                    while indefinite || values.len() < count {
                        if indefinite && self.input.get(self.offset) == Some(&0xff) {
                            self.offset += 1;
                            break;
                        }
                        let key = self.item(depth + 1)?;
                        if !matches!(key, Value::Integer(_) | Value::Text(_) | Value::Bytes(_)) {
                            return Err("CBOR map key type is unsupported".into());
                        }
                        if values.contains_key(&key) {
                            return Err("CBOR map contains a duplicate key".into());
                        }
                        let value = self.item(depth + 1)?;
                        values.insert(key, value);
                    }
                    Ok(Value::Map(values))
                }
            }
            6 if argument == 18 && depth == 0 => {
                Ok(Value::Tag(18, Box::new(self.item(depth + 1)?)))
            }
            6 => Err("CBOR tag is unsupported here".into()),
            7 => match additional {
                20 => Ok(Value::Bool(false)),
                21 => Ok(Value::Bool(true)),
                22 => Ok(Value::Null),
                _ => Err("CBOR simple or floating-point value is unsupported".into()),
            },
            _ => Err("CBOR type is unsupported".into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn borrowed_validation_matches_strict_decoder() {
        let limits = CborLimits {
            bytes: 4096,
            items: 1024,
            depth: 8,
        };
        let cases = [
            vec![0xa2, 0x61, b'a', 0, 0x61, b'a', 1],
            vec![0xa2, 0, 0, 0x18, 0, 1], // non-minimal duplicate integer
            vec![0xa2, 0x41, 0, 0, 0x41, 0, 1],
            vec![0xa2, 0x61, b'a', 0, 0x41, b'a', 1], // distinct text/bytes
            vec![0xa1, 0xf4, 0],
            vec![0x61, 0xff],
            vec![0x9f, 0xff],
            vec![0xd2, 0x84, 0x40, 0xa0, 0x40, 0x40],
            vec![0x81, 0xd2, 0x80],
            vec![0xfa, 0, 0, 0, 0],
        ];
        for encoded in cases {
            assert_eq!(
                validate_strict_cbor(&encoded, limits).is_ok(),
                decode_strict_cbor(&encoded, limits).is_ok(),
                "{encoded:?}"
            );
        }
        for count in 0..40 {
            let values = serde_json::json!({"rows": (0..count).map(|i|
                serde_json::json!({"id":i,"name":"é","flags":[true,false,null],"items":[1,2,3]})
            ).collect::<Vec<_>>()});
            let encoded = serde_cbor::to_vec(&values).unwrap();
            for limit in [
                limits,
                CborLimits {
                    bytes: 4096,
                    items: 12,
                    depth: 2,
                },
            ] {
                assert_eq!(
                    validate_strict_cbor(&encoded, limit).is_ok(),
                    decode_strict_cbor(&encoded, limit).is_ok()
                );
            }
        }
        let mut random = 0x9e37_79b9_u32;
        for length in 0..64 {
            for _ in 0..64 {
                let encoded: Vec<u8> = (0..length)
                    .map(|_| {
                        random ^= random << 13;
                        random ^= random >> 17;
                        random ^= random << 5;
                        random as u8
                    })
                    .collect();
                assert_eq!(
                    validate_strict_cbor(&encoded, limits).is_ok(),
                    decode_strict_cbor(&encoded, limits).is_ok(),
                    "{encoded:?}"
                );
            }
        }
    }

    #[test]
    fn nitro_indefinite_containers_preserve_all_bounds_and_map_rules() {
        let data = [0xbf, 0x61, b'a', 0x9f, 0x00, 0x01, 0xff, 0xff];
        let expected =
            decode_strict_cbor(&[0xa1, 0x61, b'a', 0x82, 0x00, 0x01], NITRO_CBOR_LIMITS).unwrap();
        assert_eq!(decode_nitro_cbor(&data).unwrap(), expected);
        assert!(decode_strict_cbor(&data, NITRO_CBOR_LIMITS).is_err());
        for bad in [
            vec![0xbf],
            vec![0xbf, 0x00, 0xff],
            vec![0xff],
            vec![0xbf, 0x00, 0x01, 0x00, 0x02, 0xff],
            vec![0x9f, 0xbf, 0x00, 0x01, 0x00, 0x02, 0xff, 0xff],
            vec![0x9f, 0x01],
            vec![0x5f, 0x41, 0x01, 0xff],
            vec![0xbf, 0xf4, 0x00, 0xff],
            vec![0x9f, 0xff, 0xff],
        ] {
            assert!(decode_nitro_cbor(&bad).is_err(), "accepted {bad:?}");
        }
        let mut too_many = vec![0x9f];
        too_many.extend(vec![0x00; 4096]);
        too_many.push(0xff);
        assert!(decode_nitro_cbor(&too_many)
            .unwrap_err()
            .contains("item limit"));
        let mut too_deep = vec![0x9f; 34];
        too_deep.extend(vec![0xff; 34]);
        assert!(decode_nitro_cbor(&too_deep)
            .unwrap_err()
            .contains("nesting"));
    }

    #[test]
    fn preserves_signed_cose_bytes_and_typed_values() {
        let encoded = [0xd2, 0x84, 0x43, 0xa1, 0x01, 0x38, 0xa0, 0x40, 0x40];
        let value = decode_strict_cbor(&encoded, NITRO_CBOR_LIMITS).unwrap();
        let Value::Tag(18, body) = value else {
            panic!("missing COSE tag")
        };
        let Value::Array(items) = *body else {
            panic!("missing COSE body")
        };
        assert_eq!(items[0], Value::Bytes(vec![0xa1, 0x01, 0x38]));
        assert_eq!(
            decode_strict_cbor(&[0x38, 0x22], NITRO_CBOR_LIMITS).unwrap(),
            Value::Integer(-35)
        );
        assert_eq!(
            decode_strict_cbor(&[0x83, 0xf4, 0xf5, 0xf6], NITRO_CBOR_LIMITS).unwrap(),
            Value::Array(vec![Value::Bool(false), Value::Bool(true), Value::Null])
        );
    }

    #[test]
    fn rejects_duplicate_keys_at_every_level() {
        for input in [
            vec![0xa2, 0x01, 0x00, 0x01, 0x01],
            vec![0xa2, 0x61, b'a', 0x00, 0x61, b'a', 0x01],
            vec![0x81, 0xa2, 0x41, b'a', 0x00, 0x41, b'a', 0x01],
        ] {
            assert!(decode_strict_cbor(&input, NITRO_CBOR_LIMITS)
                .unwrap_err()
                .contains("duplicate"));
        }
    }

    #[test]
    fn rejects_malformed_nonminimal_and_unbounded_values() {
        for input in [
            vec![],
            vec![0x00, 0x00],
            vec![0x18, 0x17],
            vec![0x19, 0x00, 0xff],
            vec![0x38, 0x00],
            vec![0x58, 0x01, 0x00],
            vec![0x9f, 0xff],
            vec![0x7f, 0xff],
            vec![0x61, 0xff],
            vec![0x44, 0x00],
            vec![0xa1, 0xf4, 0x00],
            vec![0xa1, 0x80, 0x00],
            vec![0xc0, 0x00],
            vec![0x81, 0xd2, 0x80],
            vec![0xf8, 0x20],
            vec![0xf9, 0x3c, 0x00],
            vec![0x9b, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff],
        ] {
            assert!(
                decode_strict_cbor(&input, NITRO_CBOR_LIMITS).is_err(),
                "accepted {input:?}"
            );
        }
        let limits = CborLimits {
            bytes: 4,
            items: 2,
            depth: 1,
        };
        assert!(decode_strict_cbor(&[0x82, 0, 0], limits).is_err());
        assert!(decode_strict_cbor(&[0x81, 0x81, 0], CborLimits { items: 3, ..limits }).is_err());
        assert!(decode_strict_cbor(&[0x44, 0, 0, 0, 0], limits).is_err());
    }
}
