use std::env;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::Path;

use postfiat_cobalt_adversarial_oracle::{
    build_manifest, verify_manifest, CorpusManifest, DEFAULT_CASE_COUNT, DEFAULT_SEED,
};

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message.into())
}

fn write_new_json(path: &Path, value: &impl serde::Serialize) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = File::options().write(true).create_new(true).open(path)?;
    serde_json::to_writer_pretty(&mut file, value).map_err(io::Error::other)?;
    file.write_all(b"\n")
}

fn main() -> io::Result<()> {
    let args = env::args().collect::<Vec<_>>();
    match args.get(1).map(String::as_str) {
        Some("freeze") => {
            let path = args.get(2).ok_or_else(|| invalid("usage: freeze PATH"))?;
            let manifest = build_manifest(DEFAULT_SEED, DEFAULT_CASE_COUNT);
            write_new_json(Path::new(path), &manifest)?;
            println!(
                "frozen {} cases as {} ({})",
                manifest.case_count, manifest.corpus_sha256, path
            );
        }
        Some("verify") => {
            let path = args.get(2).ok_or_else(|| invalid("usage: verify PATH"))?;
            let manifest: CorpusManifest =
                serde_json::from_slice(&fs::read(path)?).map_err(io::Error::other)?;
            let cases = verify_manifest(&manifest).map_err(invalid)?;
            println!(
                "verified {} cases as {}",
                cases.len(),
                manifest.corpus_sha256
            );
        }
        _ => {
            return Err(invalid(
                "usage: postfiat-cobalt-adversarial-oracle freeze|verify PATH",
            ))
        }
    }
    Ok(())
}
