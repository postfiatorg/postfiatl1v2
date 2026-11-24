//! Decode the frozen guest's canonical big-endian TLV, as in PFTLFinalityVerifierV1.
use crate::Error;
use solana_program_error::ProgramError;
use solana_sha256_hasher::hashv;

pub const EGRESS: &[u8] = b"postfiat.pfusdc.egress_public_values.v1";
pub const CHECKPOINT: &[u8] = b"postfiat.pfusdc.checkpoint_public_values.v1";
const MAGIC: &[u8] = b"PFTL-PFUSDC-TIER4";

pub struct Values<'a> {
    pub chain: [u8; 32],
    pub prior: &'a [u8],
    pub result: &'a [u8],
    pub committee: &'a [u8],
    pub start_committee: &'a [u8],
    pub height: u64,
    pub exit: Option<Exit<'a>>,
}

pub struct Exit<'a> {
    pub route: [u8; 32],
    pub vault_alias: &'a [u8],
    pub mint_alias: &'a [u8],
    pub burn: &'a [u8],
    pub amount: u64,
    pub destination_hash: &'a [u8],
}

fn take<'a>(b: &mut &'a [u8], n: usize) -> Result<&'a [u8], ProgramError> {
    let (v, rest) = b.split_at_checked(n).ok_or(Error::Encoding)?;
    *b = rest;
    Ok(v)
}

fn u32be(b: &mut &[u8]) -> Result<usize, ProgramError> {
    Ok(u32::from_be_bytes(take(b, 4)?.try_into().unwrap()) as usize)
}

/// The exit route over fields 6, 7, 23 and 32 to 36: descriptor, family and custody.
pub fn custody_route(fields: [&[u8]; 8]) -> [u8; 32] {
    hashv(&fields).to_bytes()
}

pub fn decode(mut b: &[u8], withdrawal: bool) -> Result<Values<'_>, ProgramError> {
    let schema = if withdrawal { EGRESS } else { CHECKPOINT };
    if take(&mut b, MAGIC.len())? != MAGIC
        || u32be(&mut b)? != schema.len()
        || take(&mut b, schema.len())? != schema
    {
        return Err(Error::Encoding.into());
    }
    // -1 means a 1..256 byte chain id; -2 is an absent or 48-byte transition root.
    let mut lengths: Vec<i16> = vec![schema.len() as i16, 4, -1, 48, 4];
    if withdrawal {
        lengths.extend([48, 8]);
    }
    lengths.extend([48, 48, 8, 48, -2, 8, 8, 48, 48, 48]);
    if withdrawal {
        lengths.extend([
            48, 8, 48, 48, 8, 48, 48, 48, 48, 8, 20, 48, 48, 8, 8, 20, 32, 20, 32, 32, 48, 32,
        ]);
    }
    let mut f = vec![&[][..]];
    for (i, n) in lengths.into_iter().enumerate() {
        if take(&mut b, 2)? != ((i + 1) as u16).to_be_bytes() {
            return Err(Error::Encoding.into());
        }
        let size = u32be(&mut b)?;
        if !match n {
            -1 => (1..=256).contains(&size),
            -2 => matches!(size, 0 | 48),
            _ => size == n as usize,
        } {
            return Err(Error::Encoding.into());
        }
        f.push(take(&mut b, size)?);
    }
    if !b.is_empty() || f[1] != schema || f[2] != 1u32.to_be_bytes() {
        return Err(Error::Encoding.into());
    }
    let h = |idx: usize| u64::from_be_bytes(f[idx].try_into().unwrap());
    let prior = if withdrawal { 8 } else { 6 };
    let height = h(prior + 5);
    if f[prior + 1] != f[prior + 7] || height == 0 {
        return Err(Error::Binding.into());
    }
    let exit = if withdrawal {
        if f[21] != f[24]
            || f[22] != b"accepted"
            || h(27) == 0
            || h(31) == 0
            || h(31) > height
            || f[32] != b"SOLANA01"
        {
            return Err(Error::Binding.into());
        }
        Some(Exit {
            // The route binds the exit descriptor, family and custody (chain tag, vault and
            // mint aliases and code hashes). Field 26, the burn's source bucket, stays proven in
            // the exit leaf; Post Fiat enforces each bucket's own limits before it is burned.
            route: custody_route([f[6], f[7], f[23], f[32], f[33], f[34], f[35], f[36]]),
            vault_alias: f[33],
            mint_alias: f[35],
            burn: f[24],
            amount: h(27),
            destination_hash: f[29],
        })
    } else {
        None
    };
    Ok(Values {
        chain: hashv(&[f[3], f[4], f[5]]).to_bytes(),
        prior: f[prior],
        result: f[prior + 1],
        committee: f[prior + 3],
        start_committee: if f[prior + 4].is_empty() {
            f[prior + 3]
        } else {
            f[prior + 4]
        },
        height,
        exit,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Encode a well-formed withdrawal TLV with one byte value per filled field.
    fn exit_values(bucket: u8, family: u8, vault: u8) -> Vec<u8> {
        let mut lengths: Vec<usize> = vec![EGRESS.len(), 4, 10, 48, 4, 48, 8];
        lengths.extend([48, 48, 8, 48, 0, 8, 8, 48, 48, 48]);
        lengths.extend([
            48, 8, 48, 48, 8, 48, 48, 48, 48, 8, 20, 48, 48, 8, 8, 20, 32, 20, 32, 32, 48, 32,
        ]);
        let mut fields: Vec<Vec<u8>> = lengths.iter().map(|n| vec![0x11; *n]).collect();
        let set =
            |fields: &mut Vec<Vec<u8>>, index: usize, value: Vec<u8>| fields[index - 1] = value;
        set(&mut fields, 1, EGRESS.to_vec());
        set(&mut fields, 2, 1u32.to_be_bytes().to_vec());
        set(&mut fields, 13, 9u64.to_be_bytes().to_vec());
        set(&mut fields, 15, vec![0x11; 48]);
        set(&mut fields, 22, b"accepted".to_vec());
        set(&mut fields, 23, vec![family; 48]);
        set(&mut fields, 26, vec![bucket; 48]);
        set(&mut fields, 27, 20_000u64.to_be_bytes().to_vec());
        set(&mut fields, 31, 9u64.to_be_bytes().to_vec());
        set(&mut fields, 32, b"SOLANA01".to_vec());
        set(&mut fields, 33, vec![vault; 20]);
        let mut out = MAGIC.to_vec();
        out.extend((EGRESS.len() as u32).to_be_bytes());
        out.extend(EGRESS);
        for (i, field) in fields.iter().enumerate() {
            out.extend(((i + 1) as u16).to_be_bytes());
            out.extend((field.len() as u32).to_be_bytes());
            out.extend(field);
        }
        out
    }

    fn route(bucket: u8, family: u8, vault: u8) -> [u8; 32] {
        decode(&exit_values(bucket, family, vault), true)
            .unwrap()
            .exit
            .unwrap()
            .route
    }

    #[test]
    fn route_binds_the_custody_and_not_one_bucket() {
        // Every revision bucket of one custody shares the route.
        assert_eq!(route(0x03, 0x22, 0x33), route(0x09, 0x22, 0x33));
        // Another family or another vault does not.
        assert_ne!(route(0x09, 0x22, 0x33), route(0x09, 0x23, 0x33));
        assert_ne!(route(0x09, 0x22, 0x33), route(0x09, 0x22, 0x34));
    }

    fn unhex(text: &str) -> Vec<u8> {
        (0..text.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
            .collect()
    }

    /// The same vector is checked by ops/sol-live/test_exit_route.py against bootstrap.py.
    #[test]
    fn route_matches_the_shared_cross_language_vector() {
        let vector = include_str!("../tests/route-vector.txt");
        let value = |key: &str| {
            vector
                .lines()
                .find_map(|l| l.strip_prefix(key).and_then(|v| v.strip_prefix('=')))
                .map(unhex)
                .unwrap()
        };
        let mut data = exit_values(0x09, 0x00, 0x00);
        // Rebuild the TLV with the vector's route fields in place of the fillers.
        let mut fields = Vec::new();
        let mut rest = &data[MAGIC.len() + 4 + EGRESS.len()..];
        while !rest.is_empty() {
            let size = u32::from_be_bytes(rest[2..6].try_into().unwrap()) as usize;
            fields.push(rest[6..6 + size].to_vec());
            rest = &rest[6 + size..];
        }
        for index in [6usize, 7, 23, 32, 33, 34, 35, 36] {
            fields[index - 1] = value(&format!("f{index}"));
        }
        data.truncate(MAGIC.len() + 4 + EGRESS.len());
        for (i, field) in fields.iter().enumerate() {
            data.extend(((i + 1) as u16).to_be_bytes());
            data.extend((field.len() as u32).to_be_bytes());
            data.extend(field);
        }
        let route = decode(&data, true).unwrap().exit.unwrap().route;
        assert_eq!(route.to_vec(), value("route"));
    }

    #[test]
    fn route_matches_the_bootstrap_preimage() {
        let data = exit_values(0x09, 0x22, 0x33);
        let decoded = decode(&data, true).unwrap().exit.unwrap();
        let mut preimage = vec![0x11u8; 48];
        preimage.extend([0x11u8; 8]);
        preimage.extend([0x22u8; 48]);
        preimage.extend(b"SOLANA01");
        preimage.extend([0x33u8; 20]);
        preimage.extend([0x11u8; 32]);
        preimage.extend([0x11u8; 20]);
        preimage.extend([0x11u8; 32]);
        assert_eq!(decoded.route, hashv(&[&preimage]).to_bytes());
    }
}
