mod local;
mod live;
mod public;
mod relay_deposit;
mod relay_transaction;
mod witness;

use anyhow::{Result, bail};
use std::{env, fs, path::Path};

fn main() -> Result<()> {
    let args: Vec<_> = env::args().skip(1).collect();
    match args.as_slice() {
        [cmd, dir, keys, vault, recorder] if cmd == "persistent" => live::run(Path::new(dir), Path::new(keys), vault.parse()?, recorder.parse()?),
        [cmd, dir] if cmd == "local" => local::run(Path::new(dir), None, None),
        [cmd, dir, slot] if cmd == "local" => local::run(Path::new(dir), Some(slot.parse()?), None),
        [cmd, dir, elf] if cmd == "local-deposit" => {
            local::run(Path::new(dir), None, Some(Path::new(elf)))
        }
        [cmd, dir, slot, output] if cmd == "build" => {
            witness::build(Path::new(dir), slot.parse()?, Path::new(output))
        }
        [cmd, dir, slot, signature] if cmd == "transaction" => {
            relay_transaction::read(Path::new(dir), slot.parse()?, signature.parse()?)
        }
        [cmd, target, carrier, dir, vault, route, extra @ ..]
            if cmd == "public" && extra.len() <= 1 =>
        {
            public::archive(
                Path::new(target),
                Path::new(carrier),
                Path::new(dir),
                vault.parse::<solana_pubkey::Pubkey>()?.to_bytes(),
                route.parse::<solana_hash::Hash>()?.to_bytes(),
                extra.first().map(|s| s.parse()).transpose()?,
            )
        }
        [cmd, dir] if cmd == "cert-target" => public::cert_target(Path::new(dir)),
        [cmd, archive, slot, finalized_id, vault, route] if cmd == "verify-receipt-block" => {
            witness::verify_receipt_block(
                Path::new(archive),
                slot.parse()?,
                finalized_id.parse()?,
                vault.parse()?,
                route.parse()?,
            )
        }
        [cmd, path] if cmd == "verify" => witness::inspect(&fs::read(path)?).map(|_| ()),
        _ => bail!(
            "usage: pfusdc-sol-capture local ARCHIVE [SLOT] | local-deposit ARCHIVE VAULT_SBF | build ARCHIVE SLOT WITNESS | transaction ARCHIVE SLOT SIGNATURE | verify WITNESS"
        ),
    }
}
