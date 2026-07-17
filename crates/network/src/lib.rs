use std::collections::BTreeSet;
use std::net::{Ipv4Addr, Ipv6Addr};

use postfiat_crypto_provider::hash_hex;
use serde::{Deserialize, Serialize};

pub const CRATE_PURPOSE: &str = "local validator networking and message framing";
pub const DEFAULT_BASE_PORT: u16 = 26_650;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetworkDomain {
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeerInfo {
    pub node_id: String,
    pub host: String,
    pub p2p_port: u16,
    pub rpc_port: u16,
    pub p2p_address: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetworkTopology {
    pub topology_id: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub peers: Vec<PeerInfo>,
}

impl NetworkTopology {
    pub fn peer(&self, node_id: &str) -> Option<&PeerInfo> {
        self.peers.iter().find(|peer| peer.node_id == node_id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FramedMessage {
    pub message_id: String,
    pub from: String,
    pub to: Option<String>,
    pub topic: String,
    pub payload_hash: String,
    pub payload_len: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct FaultPlan {
    pub drop_message_ids: Vec<String>,
    pub duplicate_message_ids: Vec<String>,
    pub delay_message_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FaultDelivery {
    pub delivered: Vec<FramedMessage>,
    pub dropped_message_ids: Vec<String>,
    pub duplicated_message_ids: Vec<String>,
    pub delayed_message_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkError {
    message: String,
}

impl NetworkError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for NetworkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for NetworkError {}

pub fn local_topology(
    domain: NetworkDomain,
    validator_count: u32,
    base_port: u16,
) -> Result<NetworkTopology, NetworkError> {
    if validator_count == 0 {
        return Err(NetworkError::new("validator_count must be positive"));
    }

    let hosts = vec!["127.0.0.1".to_string(); validator_count as usize];
    topology_from_hosts(domain, &hosts, base_port, None)
}

pub fn remote_topology(
    domain: NetworkDomain,
    hosts: &[String],
    base_port: u16,
    rpc_base_port: u16,
) -> Result<NetworkTopology, NetworkError> {
    topology_from_hosts(domain, hosts, base_port, Some(rpc_base_port))
}

fn topology_from_hosts(
    domain: NetworkDomain,
    hosts: &[String],
    base_port: u16,
    rpc_base_port: Option<u16>,
) -> Result<NetworkTopology, NetworkError> {
    validate_domain(&domain)?;
    if hosts.is_empty() {
        return Err(NetworkError::new("validator_count must be positive"));
    }

    let mut peers = Vec::with_capacity(hosts.len());
    let mut used_host_ports = BTreeSet::new();
    for (index, host) in hosts.iter().enumerate() {
        validate_host(host)?;
        let port_index = u16::try_from(index)
            .map_err(|_| NetworkError::new("validator_count exceeds local port range"))?;
        let p2p_offset = port_index
            .checked_mul(2)
            .ok_or_else(|| NetworkError::new("validator_count exceeds local port range"))?;
        let p2p_port = base_port
            .checked_add(p2p_offset)
            .ok_or_else(|| NetworkError::new("p2p port range overflow"))?;
        let rpc_port = match rpc_base_port {
            Some(base) => base
                .checked_add(port_index)
                .ok_or_else(|| NetworkError::new("rpc port range overflow"))?,
            None => p2p_port
                .checked_add(1)
                .ok_or_else(|| NetworkError::new("rpc port range overflow"))?,
        };
        for (kind, port) in [("p2p", p2p_port), ("rpc", rpc_port)] {
            if !used_host_ports.insert((host.as_str(), port)) {
                return Err(NetworkError::new(format!(
                    "{kind} port {port} collides on host `{host}`"
                )));
            }
        }
        peers.push(PeerInfo {
            node_id: format!("validator-{index}"),
            p2p_address: multiaddr(host, p2p_port),
            host: host.clone(),
            p2p_port,
            rpc_port,
        });
    }

    topology_with_peers(domain, peers)
}

fn topology_with_peers(
    domain: NetworkDomain,
    peers: Vec<PeerInfo>,
) -> Result<NetworkTopology, NetworkError> {
    let topology_bytes = serde_json::to_vec(&(
        domain.chain_id.as_str(),
        domain.genesis_hash.as_str(),
        domain.protocol_version,
        &peers,
    ))
    .map_err(|error| NetworkError::new(error.to_string()))?;
    let topology_id = hash_hex("postfiat.network.topology.v1", &topology_bytes);
    Ok(NetworkTopology {
        topology_id,
        chain_id: domain.chain_id,
        genesis_hash: domain.genesis_hash,
        protocol_version: domain.protocol_version,
        peers,
    })
}

fn validate_host(host: &str) -> Result<(), NetworkError> {
    if host.trim() != host || host.is_empty() {
        return Err(NetworkError::new(
            "topology host must be nonempty and trimmed",
        ));
    }
    if host
        .bytes()
        .any(|byte| byte.is_ascii_whitespace() || byte == b'/' || byte == b',')
    {
        return Err(NetworkError::new(format!(
            "topology host `{host}` contains an invalid character"
        )));
    }
    if host.contains(':') && host.parse::<Ipv6Addr>().is_err() {
        return Err(NetworkError::new(format!(
            "topology host `{host}` is not a valid IPv6 address"
        )));
    }
    Ok(())
}

fn multiaddr(host: &str, port: u16) -> String {
    if let Ok(address) = host.parse::<Ipv4Addr>() {
        format!("/ip4/{address}/tcp/{port}")
    } else if let Ok(address) = host.parse::<Ipv6Addr>() {
        format!("/ip6/{address}/tcp/{port}")
    } else {
        format!("/dns4/{host}/tcp/{port}")
    }
}

pub fn frame_message(
    domain: &NetworkDomain,
    from: impl Into<String>,
    to: Option<String>,
    topic: impl Into<String>,
    payload: &[u8],
) -> Result<FramedMessage, NetworkError> {
    validate_domain(domain)?;
    let from = from.into();
    let topic = topic.into();
    let payload_hash = network_payload_hash(domain, payload)?;
    let message_id = hash_hex(
        "postfiat.network.message.v1",
        format!(
            "chain_id={}\ngenesis_hash={}\nprotocol_version={}\nfrom={from}\nto={}\ntopic={topic}\npayload_hash={payload_hash}\npayload_len={}\n",
            domain.chain_id,
            domain.genesis_hash,
            domain.protocol_version,
            to.as_deref().unwrap_or("*"),
            payload.len()
        )
        .as_bytes(),
    );

    Ok(FramedMessage {
        message_id,
        from,
        to,
        topic,
        payload_hash,
        payload_len: payload.len() as u64,
    })
}

pub fn verify_message_payload(
    domain: &NetworkDomain,
    message: &FramedMessage,
    payload: &[u8],
) -> bool {
    let Ok(payload_hash) = network_payload_hash(domain, payload) else {
        return false;
    };
    let Ok(expected) = frame_message(
        domain,
        message.from.clone(),
        message.to.clone(),
        message.topic.clone(),
        payload,
    ) else {
        return false;
    };
    message.payload_hash == payload_hash
        && message.payload_len == payload.len() as u64
        && expected.message_id == message.message_id
}

fn network_payload_hash(domain: &NetworkDomain, payload: &[u8]) -> Result<String, NetworkError> {
    validate_domain(domain)?;
    let prefix = format!(
        "chain_id={}\ngenesis_hash={}\nprotocol_version={}\npayload_len={}\n",
        domain.chain_id,
        domain.genesis_hash,
        domain.protocol_version,
        payload.len()
    );
    let mut preimage = Vec::with_capacity(prefix.len() + payload.len());
    preimage.extend_from_slice(prefix.as_bytes());
    preimage.extend_from_slice(payload);
    Ok(hash_hex("postfiat.network.payload.v2", &preimage))
}

fn validate_domain(domain: &NetworkDomain) -> Result<(), NetworkError> {
    if domain.chain_id.trim().is_empty() {
        return Err(NetworkError::new("network domain chain_id is empty"));
    }
    if domain.genesis_hash.trim().is_empty() {
        return Err(NetworkError::new("network domain genesis_hash is empty"));
    }
    if !is_lower_hex_len(&domain.genesis_hash, 96) {
        return Err(NetworkError::new(
            "network domain genesis_hash must be 96 lowercase hex characters",
        ));
    }
    if domain.protocol_version == 0 {
        return Err(NetworkError::new(
            "network domain protocol_version must be nonzero",
        ));
    }
    Ok(())
}

fn is_lower_hex_len(value: &str, expected_len: usize) -> bool {
    value.len() == expected_len
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

pub fn apply_fault_plan(
    messages: &[FramedMessage],
    plan: &FaultPlan,
) -> Result<FaultDelivery, NetworkError> {
    let known_message_ids = checked_message_ids(messages)?;
    let drop_ids = checked_fault_ids(&known_message_ids, "drop", &plan.drop_message_ids)?;
    let duplicate_ids =
        checked_fault_ids(&known_message_ids, "duplicate", &plan.duplicate_message_ids)?;
    let delay_ids = checked_fault_ids(&known_message_ids, "delay", &plan.delay_message_ids)?;

    if let Some(message_id) = drop_ids.intersection(&duplicate_ids).next() {
        return Err(NetworkError::new(format!(
            "message `{message_id}` cannot be both dropped and duplicated",
        )));
    }
    if let Some(message_id) = drop_ids.intersection(&delay_ids).next() {
        return Err(NetworkError::new(format!(
            "message `{message_id}` cannot be both dropped and delayed",
        )));
    }

    let mut delivered = Vec::new();
    let mut delayed_delivery = Vec::new();
    let mut dropped_message_ids = Vec::new();
    let mut duplicated_message_ids = Vec::new();
    let mut delayed_message_ids = Vec::new();

    for message in messages {
        let message_id = message.message_id.as_str();
        if drop_ids.contains(message_id) {
            dropped_message_ids.push(message.message_id.clone());
            continue;
        }

        let target = if delay_ids.contains(message_id) {
            if !delayed_message_ids.iter().any(|found| found == message_id) {
                delayed_message_ids.push(message.message_id.clone());
            }
            &mut delayed_delivery
        } else {
            &mut delivered
        };

        target.push(message.clone());
        if duplicate_ids.contains(message_id) {
            target.push(message.clone());
            duplicated_message_ids.push(message.message_id.clone());
        }
    }

    delivered.extend(delayed_delivery);
    Ok(FaultDelivery {
        delivered,
        dropped_message_ids,
        duplicated_message_ids,
        delayed_message_ids,
    })
}

fn checked_message_ids(messages: &[FramedMessage]) -> Result<BTreeSet<&str>, NetworkError> {
    let mut message_ids = BTreeSet::new();
    for message in messages {
        if message.message_id.trim().is_empty() {
            return Err(NetworkError::new("message id must be nonempty"));
        }
        if !message_ids.insert(message.message_id.as_str()) {
            return Err(NetworkError::new(format!(
                "duplicate message id `{}`",
                message.message_id
            )));
        }
    }
    Ok(message_ids)
}

fn checked_fault_ids<'a>(
    known_message_ids: &BTreeSet<&str>,
    label: &str,
    message_ids: &'a [String],
) -> Result<BTreeSet<&'a str>, NetworkError> {
    let mut fault_ids = BTreeSet::new();
    for message_id in message_ids {
        if message_id.trim().is_empty() {
            return Err(NetworkError::new(format!(
                "{label} fault message id must be nonempty",
            )));
        }
        if !known_message_ids.contains(message_id.as_str()) {
            return Err(NetworkError::new(format!(
                "{label} fault references unknown message `{message_id}`",
            )));
        }
        if !fault_ids.insert(message_id.as_str()) {
            return Err(NetworkError::new(format!(
                "{label} fault repeats message `{message_id}`",
            )));
        }
    }
    Ok(fault_ids)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_domain() -> NetworkDomain {
        NetworkDomain {
            chain_id: "postfiat-local".to_string(),
            genesis_hash: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
            protocol_version: 1,
        }
    }

    #[test]
    fn builds_local_topology() {
        let domain = test_domain();
        let topology = local_topology(domain.clone(), 4, DEFAULT_BASE_PORT).expect("topology");

        assert_eq!(topology.peers.len(), 4);
        assert_eq!(topology.chain_id, domain.chain_id);
        assert_eq!(topology.genesis_hash, domain.genesis_hash);
        assert_eq!(topology.protocol_version, domain.protocol_version);
        assert_eq!(topology.peer("validator-2").expect("peer").p2p_port, 26654);
        assert!(topology.peer("validator-4").is_none());
        assert!(!topology.topology_id.is_empty());
    }

    #[test]
    fn builds_remote_topology_from_hosts() {
        let domain = test_domain();
        let hosts = vec![
            "10.0.0.10".to_string(),
            "validator-1.internal".to_string(),
            "2001:db8::1".to_string(),
        ];
        let topology = remote_topology(domain.clone(), &hosts, 26000, 27000).expect("topology");

        assert_eq!(topology.peers.len(), 3);
        assert_eq!(topology.chain_id, domain.chain_id);
        assert_eq!(topology.genesis_hash, domain.genesis_hash);
        assert_eq!(topology.protocol_version, domain.protocol_version);

        let validator_0 = topology.peer("validator-0").expect("validator-0");
        assert_eq!(validator_0.p2p_port, 26000);
        assert_eq!(validator_0.rpc_port, 27000);
        assert_eq!(validator_0.p2p_address, "/ip4/10.0.0.10/tcp/26000");

        let validator_1 = topology.peer("validator-1").expect("validator-1");
        assert_eq!(
            validator_1.p2p_address,
            "/dns4/validator-1.internal/tcp/26002"
        );

        let validator_2 = topology.peer("validator-2").expect("validator-2");
        assert_eq!(validator_2.p2p_address, "/ip6/2001:db8::1/tcp/26004");
    }

    #[test]
    fn remote_topology_rejects_bad_hosts_and_same_host_port_collisions() {
        let domain = test_domain();
        let malformed_hosts = vec!["validator 0".to_string()];
        assert!(remote_topology(domain.clone(), &malformed_hosts, 26000, 27000).is_err());

        for malformed_ipv6 in [":::", "1::2::3", "2001:db8:::1"] {
            let malformed_hosts = vec![malformed_ipv6.to_string()];
            assert!(
                remote_topology(domain.clone(), &malformed_hosts, 26000, 27000).is_err(),
                "{malformed_ipv6} must be rejected"
            );
        }

        let colliding_hosts = vec!["127.0.0.1".to_string(), "127.0.0.1".to_string()];
        assert!(remote_topology(domain, &colliding_hosts, 26650, 26652).is_err());
    }

    #[test]
    fn frames_and_verifies_payloads() {
        let payload = br#"{"batch":"abc"}"#;
        let domain = test_domain();
        let message = frame_message(
            &domain,
            "validator-0",
            Some("validator-1".to_string()),
            "batch_reference",
            payload,
        )
        .expect("frame message");

        assert!(verify_message_payload(&domain, &message, payload));
        assert!(!verify_message_payload(&domain, &message, b"tampered"));

        let mut wrong_domain = domain;
        wrong_domain.genesis_hash = "111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111".to_string();
        assert!(!verify_message_payload(&wrong_domain, &message, payload));
    }

    #[test]
    fn payload_hash_commits_to_chain_domain() {
        let payload = br#"{"batch":"abc"}"#;
        let local_domain = test_domain();
        let mut other_domain = local_domain.clone();
        other_domain.chain_id = "postfiat-other".to_string();

        let local_message = frame_message(
            &local_domain,
            "validator-0",
            Some("validator-1".to_string()),
            "batch_reference",
            payload,
        )
        .expect("local frame");
        let other_message = frame_message(
            &other_domain,
            "validator-0",
            Some("validator-1".to_string()),
            "batch_reference",
            payload,
        )
        .expect("other frame");

        assert_ne!(local_message.payload_hash, other_message.payload_hash);
        assert_ne!(local_message.message_id, other_message.message_id);
        assert!(verify_message_payload(
            &local_domain,
            &local_message,
            payload
        ));
        assert!(!verify_message_payload(
            &other_domain,
            &local_message,
            payload
        ));
    }

    #[test]
    fn fault_plan_drops_duplicates_and_delays_messages() {
        let messages = test_messages(4);
        let plan = FaultPlan {
            drop_message_ids: vec![messages[1].message_id.clone()],
            duplicate_message_ids: vec![messages[2].message_id.clone()],
            delay_message_ids: vec![messages[3].message_id.clone()],
        };

        let outcome = apply_fault_plan(&messages, &plan).expect("fault delivery");
        let delivered_ids: Vec<&str> = outcome
            .delivered
            .iter()
            .map(|message| message.message_id.as_str())
            .collect();

        assert_eq!(
            delivered_ids,
            vec![
                messages[0].message_id.as_str(),
                messages[2].message_id.as_str(),
                messages[2].message_id.as_str(),
                messages[3].message_id.as_str()
            ]
        );
        assert_eq!(
            outcome.dropped_message_ids,
            vec![messages[1].message_id.clone()]
        );
        assert_eq!(
            outcome.duplicated_message_ids,
            vec![messages[2].message_id.clone()]
        );
        assert_eq!(
            outcome.delayed_message_ids,
            vec![messages[3].message_id.clone()]
        );
    }

    #[test]
    fn fault_plan_rejects_unknown_conflicting_and_duplicate_ids() {
        let messages = test_messages(2);

        let unknown = FaultPlan {
            drop_message_ids: vec!["missing-message".to_string()],
            duplicate_message_ids: vec![],
            delay_message_ids: vec![],
        };
        assert!(apply_fault_plan(&messages, &unknown).is_err());

        let conflicting = FaultPlan {
            drop_message_ids: vec![messages[0].message_id.clone()],
            duplicate_message_ids: vec![messages[0].message_id.clone()],
            delay_message_ids: vec![],
        };
        assert!(apply_fault_plan(&messages, &conflicting).is_err());

        let repeated_fault = FaultPlan {
            drop_message_ids: vec![
                messages[0].message_id.clone(),
                messages[0].message_id.clone(),
            ],
            duplicate_message_ids: vec![],
            delay_message_ids: vec![],
        };
        assert!(apply_fault_plan(&messages, &repeated_fault).is_err());

        let duplicate_messages = vec![messages[0].clone(), messages[0].clone()];
        assert!(apply_fault_plan(&duplicate_messages, &FaultPlan::default()).is_err());
    }

    #[test]
    fn rejects_invalid_topology_size() {
        assert!(local_topology(test_domain(), 0, DEFAULT_BASE_PORT).is_err());
    }

    #[test]
    fn rejects_malformed_network_domain() {
        let mut domain = test_domain();
        domain.chain_id.clear();
        assert!(local_topology(domain.clone(), 1, DEFAULT_BASE_PORT).is_err());
        assert!(frame_message(&domain, "validator-0", None, "topic", b"payload").is_err());
        assert!(!verify_message_payload(
            &domain,
            &FramedMessage {
                message_id: "message".to_string(),
                from: "validator-0".to_string(),
                to: None,
                topic: "topic".to_string(),
                payload_hash: "payload".to_string(),
                payload_len: 7,
            },
            b"payload"
        ));

        let mut domain = test_domain();
        domain.genesis_hash = " ".to_string();
        assert!(local_topology(domain.clone(), 1, DEFAULT_BASE_PORT).is_err());
        assert!(frame_message(&domain, "validator-0", None, "topic", b"payload").is_err());

        let mut domain = test_domain();
        domain.genesis_hash = "not-a-genesis-hash".to_string();
        assert!(local_topology(domain.clone(), 1, DEFAULT_BASE_PORT).is_err());
        assert!(frame_message(&domain, "validator-0", None, "topic", b"payload").is_err());

        let mut domain = test_domain();
        domain.protocol_version = 0;
        assert!(local_topology(domain.clone(), 1, DEFAULT_BASE_PORT).is_err());
        assert!(frame_message(&domain, "validator-0", None, "topic", b"payload").is_err());
    }

    fn test_messages(count: usize) -> Vec<FramedMessage> {
        let domain = test_domain();
        (0..count)
            .map(|index| {
                frame_message(
                    &domain,
                    format!("validator-{index}"),
                    Some(format!("validator-{}", (index + 1) % count)),
                    "batch_reference",
                    format!("payload-{index}").as_bytes(),
                )
                .expect("test frame")
            })
            .collect()
    }
}
