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
            route: hashv(&[f[6], f[7], f[23], f[26], f[32], f[33], f[34], f[35], f[36]]).to_bytes(),
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
