//! Length-delimited witness and proof transport on a private Unix socket.
use anyhow::{bail, ensure, Context, Result};
use pfusdc_sol_ingress_program::{
    shards, verify_deposit_witness, verify_handoff_witness, verify_key_table_witness,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sp1_sdk::{CudaProver, HashableKey, ProveRequest, Prover, SP1Proof, SP1Stdin};
use std::{fs, os::unix::fs::PermissionsExt, path::Path, sync::Arc, time::Instant};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    net::{UnixListener, UnixStream},
    sync::{Mutex, Semaphore},
    task::JoinSet,
};

type Key = <CudaProver as Prover>::ProvingKey;
const NAMES: [&str; 5] = [
    "pfusdc-sol-ingress-program",
    "pfusdc-sol-key-table-proof",
    "pfusdc-sol-handoff-proof",
    "pfusdc-sol-delta-shard",
    "pfusdc-sol-deposit-aggregate",
];

/// Select contiguous work by estimated hashing cost. The ingress parser locates
/// and validates the record stream. The scheduler only needs record lengths.
fn balanced_ranges(witness: &[u8], workers: usize) -> Result<Vec<&[u8]>> {
    let validated = shards::partition(witness, 1).map_err(|e| anyhow::anyhow!("{e:?}"))?;
    let records = *validated.first().context("empty deposit delta")?;
    let mut boundaries = vec![(0usize, 0usize)];
    let mut remaining = records;
    while !remaining.is_empty() {
        let start = remaining.len();
        remaining = remaining.get(32..).context("truncated address")?;
        for _ in 0..2 {
            // lamports, executable, owner, then u32 length and account data.
            let (header, tail) = remaining
                .split_at_checked(45)
                .context("truncated account")?;
            let len = u32::from_le_bytes(header[41..45].try_into()?) as usize;
            remaining = tail.get(len..).context("truncated data")?;
        }
        let cost = boundaries.last().unwrap().1 + start - remaining.len()
            + 2 * pfusdc_sol_ingress_program::LT_BYTES;
        boundaries.push((records.len() - remaining.len(), cost));
    }
    let count = boundaries.len() - 1;
    // On small deltas one large account dominates leaf latency. Extra tiny
    // leaves add recursive verifications without shortening that critical leaf.
    let workers = workers.min(records.len().div_ceil(192 * 1024).max(1));
    let total = boundaries[count].1;
    let mut first = 0;
    let mut ranges = Vec::new();
    for slots in (1..=workers.min(count)).rev() {
        let current = boundaries[first].1;
        let target = current + (total - current).div_ceil(slots);
        let candidates = &boundaries[first + 1..=count + 1 - slots];
        let right = candidates.partition_point(|&(_, weight)| weight < target);
        let mut next = first + 1 + right.min(candidates.len() - 1);
        if next > first + 1
            && boundaries[next - 1].1.abs_diff(target) < boundaries[next].1.abs_diff(target)
        {
            next -= 1;
        }
        ranges.push(&records[boundaries[first].0..boundaries[next].0]);
        first = next;
    }
    Ok(ranges)
}

struct Device {
    client: CudaProver,
    leaf: Key,
    busy: Arc<Mutex<()>>,
}

struct Service {
    devices: Vec<Device>,
    keys: Vec<Key>,
}

impl Service {
    async fn load(dir: &Path, n: usize) -> Result<Self> {
        ensure!(
            (1..=shards::MAX_SHARDS).contains(&n),
            "invalid device count"
        );
        let elf = fs::read(dir.join(NAMES[3]))?;
        let mut devices = Vec::new();
        for id in 0..n {
            let client = sp1_sdk::ProverClient::builder()
                .cuda()
                .with_device_id(id as u32)
                .build()
                .await;
            let leaf = client.setup(elf.clone().into()).await?;
            devices.push(Device {
                client,
                leaf,
                busy: Arc::new(Mutex::new(())),
            });
        }
        let mut keys = Vec::new();
        for name in [NAMES[0], NAMES[1], NAMES[2], NAMES[4]] {
            let gpu = if name == NAMES[2] { n - 1 } else { 0 };
            keys.push(
                devices[gpu]
                    .client
                    .setup(fs::read(dir.join(name))?.into())
                    .await?,
            );
        }
        Ok(Self { devices, keys })
    }

    async fn request(&self, kind: &str, mode: &str, witness: Vec<u8>) -> Result<(Value, Vec<u8>)> {
        ensure!(
            matches!(mode, "compressed" | "groth16"),
            "unknown proof mode"
        );
        let start = Instant::now();
        let witness_sha = format!("{:x}", Sha256::digest(&witness));
        // Leaves prove runs of records in address order.
        let witness = if kind == "parallel" {
            shards::canonical(&witness).map_err(|e| anyhow::anyhow!("{e:?}"))?
        } else {
            witness
        };
        let (idx, expected) = match kind {
            "deposit" | "parallel" => (
                if kind == "deposit" { 0 } else { 3 },
                verify_deposit_witness(&witness)
                    .map_err(|e| anyhow::anyhow!("{e:?}"))?
                    .encode(),
            ),
            "key_table" => (
                1,
                verify_key_table_witness(&witness)
                    .map_err(|e| anyhow::anyhow!("{e:?}"))?
                    .encode(),
            ),
            "handoff" => (
                2,
                verify_handoff_witness(&witness)
                    .map_err(|e| anyhow::anyhow!("{e:?}"))?
                    .encode(),
            ),
            _ => bail!("unknown guest"),
        };
        let native = start.elapsed().as_secs_f64();
        let mut stdin = SP1Stdin::new();
        let mut leaves = Vec::new();
        let mut shard_seconds = 0.0;
        if kind == "parallel" {
            let partition_start = Instant::now();
            let ranges = balanced_ranges(&witness, self.devices.len())?;
            let (header, whole) = shards::inputs(&witness, 1).map_err(|e| anyhow::anyhow!("{e:?}"))?;
            let binding = &whole[0][..64];
            let mut jobs = JoinSet::new();
            for (index, (records, device)) in ranges.iter().zip(&self.devices).enumerate() {
                let bytes = records.len();
                let records = [binding, records].concat();
                let client = device.client.clone();
                let key = device.leaf.clone();
                let busy = device.busy.clone();
                jobs.spawn(async move {
                    let _device = busy.lock().await;
                    let begin = Instant::now();
                    let reference = shards::leaf(&records).map_err(|e| anyhow::anyhow!("{e:?}"))?;
                    let proof = client
                        .prove(&key, SP1Stdin::from(&records))
                        .compressed()
                        .await?;
                    ensure!(
                        proof.public_values.as_slice() == reference,
                        "leaf journal mismatch"
                    );
                    client.verify(&proof, key.verifying_key(), None)?;
                    let SP1Proof::Compressed(inner) = proof.proof else {
                        bail!("expected compressed shard")
                    };
                    Ok::<_, anyhow::Error>((
                        index,
                        bytes,
                        reference,
                        inner,
                        begin.elapsed().as_secs_f64(),
                    ))
                });
            }
            // Drain every SDK exchange before returning a failed request. Cancelling
            // another device mid-response would desynchronize its persistent socket.
            let mut completed = Vec::new();
            while let Some(result) = jobs.join_next().await {
                completed.push(result.map_err(anyhow::Error::from).and_then(|r| r));
            }
            let mut results = completed.into_iter().collect::<Result<Vec<_>>>()?;
            results.sort_by_key(|r| r.0);
            let mut frames = Vec::new();
            for (index, len, journal, proof, elapsed) in results {
                frames.extend_from_slice(&u32::try_from(journal.len())?.to_le_bytes());
                frames.extend_from_slice(&journal);
                stdin.write_proof(*proof, self.devices[index].leaf.verifying_key().vk.clone());
                leaves.push(json!({"gpu":index,"record_bytes":len,"seconds":elapsed,"proof_verified":true,"public_values_match":true}));
            }
            stdin.write_vec(frames);
            // The aggregate reads no account record.
            stdin.write_vec(header.to_vec());
            shard_seconds = partition_start.elapsed().as_secs_f64();
        } else {
            stdin.write_vec(witness);
        }
        let leaves_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs_f64();
        let key = &self.keys[idx];
        // The handoff guest gets the last GPU while small deposits use the first
        // devices. Serialize SDK exchanges on each persistent device connection.
        let device = &self.devices[if kind == "handoff" {
            self.devices.len() - 1
        } else {
            0
        }];
        let _device = device.busy.lock().await;
        let client = &device.client;
        let proving = Instant::now();
        let req = client.prove(key, stdin);
        let proof = match mode {
            "compressed" => req.compressed().await?,
            _ => req.groth16().await?,
        };
        let aggregate_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs_f64();
        let prove_seconds = proving.elapsed().as_secs_f64();
        let verifying = Instant::now();
        ensure!(
            proof.public_values.as_slice() == expected,
            "proof and native journals differ"
        );
        client.verify(&proof, key.verifying_key(), None)?;
        let verify_seconds = verifying.elapsed().as_secs_f64();
        let wire = if mode == "groth16" {
            proof.bytes()
        } else {
            bincode::serialize(&proof.proof)?
        };
        let header = json!({"guest":kind,"mode":mode,"witness_sha256":witness_sha,"program_vkey":key.verifying_key().bytes32(),
            "public_values": proof.public_values.as_slice(), "public_values_sha256":format!("{:x}",Sha256::digest(expected)),
            "proof_bytes":wire.len(), "proof_verified":true,"public_values_match":true,"shards":leaves, "leaves_at":leaves_at, "aggregate_at":aggregate_at,
            "seconds":{"native":native,"shards_wall":shard_seconds,"prove_final":prove_seconds,"verify_final":verify_seconds,"request":start.elapsed().as_secs_f64()}});
        Ok((header, wire))
    }

    async fn connection(&self, stream: &mut UnixStream) -> Result<()> {
        let mut input = BufReader::new(&mut *stream);
        let mut header = String::new();
        (&mut input).take(256).read_line(&mut header).await?;
        ensure!(header.ends_with('\n'), "incomplete request header");
        let fields: Vec<_> = header.split_whitespace().collect();
        ensure!(fields.len() == 3, "expected guest mode byte_length");
        let n = fields[2].parse::<usize>()?;
        ensure!(n <= 512 * 1024 * 1024, "service witness limit exceeded");
        let mut witness = vec![0; n];
        input.read_exact(&mut witness).await?;
        let (reply, bytes) = self.request(fields[0], fields[1], witness).await?;
        stream.write_all(format!("{reply}\n").as_bytes()).await?;
        stream.write_all(&bytes).await?;
        Ok(())
    }
}

pub async fn serve(args: &[String]) -> Result<()> {
    ensure!(
        args.len() == 4,
        "serve ELF_DIR SOCKET GPU_COUNT PRIME_DEPOSIT_WITNESS|--measure-cold"
    );
    let started = Instant::now();
    let n = args[2].parse()?;
    let service = Arc::new(Service::load(Path::new(&args[0]), n).await?);
    eprintln!(
        "{}",
        json!({"loaded_keys_seconds":started.elapsed().as_secs_f64(),"gpus":n})
    );
    if args[3] != "--measure-cold" {
        let (report, _) = service
            .request("deposit", "groth16", fs::read(&args[3])?)
            .await?;
        eprintln!("{}", json!({"priming_proof":report}));
    }
    let socket = Path::new(&args[1]);
    let listener = UnixListener::bind(socket).context("bind in a private service directory")?;
    fs::set_permissions(socket, fs::Permissions::from_mode(0o600))?;
    eprintln!(
        "{}",
        json!({"ready_seconds":started.elapsed().as_secs_f64()})
    );
    let capacity = Arc::new(Semaphore::new(2));
    loop {
        let permit = capacity.clone().acquire_owned().await?;
        let (mut stream, _) = listener.accept().await?;
        let service = service.clone();
        tokio::spawn(async move {
            let _permit = permit;
            if let Err(e) = service.connection(&mut stream).await {
                let reply = json!({"error":format!("{e:#}")});
                let _ = stream.write_all(format!("{reply}\n").as_bytes()).await;
            }
        });
    }
}
