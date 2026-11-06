use super::*;

/// Inputs for `stage_deployment_validator_units` with a six-validator
/// private-overlay topology.
pub(super) fn stage_inputs(root: &Path) -> DeploymentValidatorUnitsStageOptions {
    std::fs::create_dir_all(root).expect("stage test root");
    let topology_file = root.join("topology.json");
    let peers = (0_u16..6)
        .map(|index| {
            serde_json::json!({
                "node_id": format!("validator-{index}"),
                "host": format!("10.0.0.{}", index + 1),
                "p2p_port": 26_650_u16 + index * 2,
                "rpc_port": 27_650_u16 + index,
                "p2p_address": format!("/ip4/10.0.0.{}/tcp/{}", index + 1, 26_650_u16 + index * 2),
            })
        })
        .collect::<Vec<_>>();
    atomic_write(
        &topology_file,
        format!(
            "{}\n",
            serde_json::json!({
                "topology_id": "stage-test-topology",
                "chain_id": "postfiat-stage-test",
                "genesis_hash": "11".repeat(48),
                "protocol_version": 1,
                "peers": peers,
            })
        ),
    )
    .expect("topology");
    for (name, bytes) in [
        ("postfiat-node", &b"release-binary"[..]),
        ("swap.metadata.json", b"swap-metadata"),
        ("private-egress.metadata.json", b"private-egress-metadata"),
    ] {
        atomic_write(root.join(name), bytes).expect("stage input");
    }
    DeploymentValidatorUnitsStageOptions {
        release_id: "release-test-1".to_string(),
        topology_file,
        binary_file: root.join("postfiat-node"),
        swap_circuit_metadata_file: root.join("swap.metadata.json"),
        private_egress_circuit_metadata_file: root.join("private-egress.metadata.json"),
        output_dir: root.join("candidate"),
        signer_shape: DeploymentSignerShape::KeyFile,
    }
}

fn sha256_file(path: &Path) -> String {
    Sha256::digest(std::fs::read(path).expect("read staged file"))
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// The default (single-user) units, byte for byte as before the two-user
/// shape existed: the digests were measured on `5a69b481a`.
#[test]
fn default_stage_writes_the_single_user_units_unchanged() {
    let root = unique_test_dir("postfiat-two-user-default");
    let report = stage_deployment_validator_units(stage_inputs(&root)).expect("stage");
    let row = &report.validators[0];
    let digests = [
        sha256_file(&row.rpc_unit_file),
        sha256_file(&row.rpc_environment_file),
        sha256_file(&row.transport_unit_file),
        sha256_file(&row.transport_environment_file),
        sha256_file(&row.runtime_bindings_file),
    ];
    assert_eq!(
        digests,
        [
            "84a1b103f7d3ebd5278a23d0d53c9f66c8fafb7d8e10c520f938020061e95680",
            "3f010ff03c5430bff047d0e443ab00ec9991480d57af9ac2fa81066c14b32698",
            "2e3fadc4599e64dc560f61c166eafd8d7d1ffe0d862a9ccc7e44a669e999e391",
            "c7c02c1bfe86ca1f76d893820f7c0db0a11079664d56907ac57a5d8096f3d1d0",
            "579174bdbeb27cbcac0d804750add15da014d23e30c8f69c6e5ff86ae32dc3f9",
        ]
    );
    std::fs::remove_dir_all(root).expect("cleanup");
}

/// Every validator's default transport unit, byte for byte as before the
/// transport process could sign through the signer: the digests were
/// measured on `2ef121d54`.
#[test]
fn default_stage_writes_every_transport_unit_unchanged() {
    let root = unique_test_dir("postfiat-two-user-default-transport");
    let report = stage_deployment_validator_units(stage_inputs(&root)).expect("stage");
    let digests = report
        .validators
        .iter()
        .map(|row| {
            assert!(row.signer_unit_file.is_none());
            sha256_file(&row.transport_unit_file)
        })
        .collect::<Vec<_>>();
    assert_eq!(digests, PINNED_DEFAULT_TRANSPORT_UNITS);
    std::fs::remove_dir_all(root).expect("cleanup");
}

const PINNED_DEFAULT_TRANSPORT_UNITS: [&str; 6] = [
    "2e3fadc4599e64dc560f61c166eafd8d7d1ffe0d862a9ccc7e44a669e999e391",
    "19a85a5001aaff9c49da2481e4817dca6e78a4664df1651eff58fc25c90ddb12",
    "7ae012d184819cb2e988d58a70fcb6a1f8d2b3c69eb0a9f0d00d876fbed184f1",
    "727574007721be97fdd03cf821e9fbf11d441219fdfe102a626d95e4b70a2a11",
    "ff4aa08bff18bf28f41b089982f2193498a1a4bad2e48838307b51e0a2d76427",
    "fdd5a3412b9f155ca36df62e9f2f236b63ca7dd9da86ad63a730b6c0b316a265",
];

#[test]
fn two_user_stage_writes_a_signer_unit_and_an_rpc_unit_without_key_files() {
    let root = unique_test_dir("postfiat-two-user-units");
    let mut options = stage_inputs(&root);
    options.signer_shape = DeploymentSignerShape::TwoUser;
    let report = stage_deployment_validator_units(options).expect("stage");
    let row = &report.validators[0];
    let signer_file = row.signer_unit_file.as_ref().expect("signer unit");
    assert!(signer_file.ends_with("etc/systemd/system/postfiat-validator-0-signer.service"));
    let signer = std::fs::read_to_string(signer_file).expect("signer unit");
    let rpc = std::fs::read_to_string(&row.rpc_unit_file).expect("rpc unit");
    let release = "/etc/postfiat/releases/release-test-1";
    let binary = "/opt/postfiat/releases/release-test-1/postfiat-node";
    let verify = format!("{binary} deployment-manifest-verify --manifest-file {release}/deployment-manifest.json --trusted-publisher-key-file {release}/deployment.public.json --validator-id validator-0 --validator-bindings-file {release}/validator-0.bindings.json --runtime-binary-file {binary} --runtime-topology-file {release}/topology.json --runtime-swap-circuit-metadata-file {release}/swap.metadata.json --runtime-private-egress-circuit-metadata-file {release}/private-egress.metadata.json");
    let hardening = "NoNewPrivileges=true\nPrivateTmp=true\nProtectSystem=strict\nProtectHome=true\nProtectControlGroups=true\nProtectKernelTunables=true\nProtectKernelModules=true\nProtectKernelLogs=true\nRestrictSUIDSGID=true\nLockPersonality=true\nRestrictRealtime=true\nSystemCallArchitectures=native\nCapabilityBoundingSet=\nAmbientCapabilities=\n";
    assert_eq!(
        signer,
        format!(
            "[Unit]\nDescription=PostFiat L1 validator signer validator-0\nBefore=postfiat-validator-0.service postfiat-validator-0-rpc.service\nStartLimitIntervalSec=300\nStartLimitBurst=5\n\n[Service]\nType=simple\nUser=postfiat-signer\nGroup=postfiat-signer-clients\nEnvironmentFile=/etc/postfiat/signer/validator-0.env\nExecStartPre={verify}\nExecStart={binary} validator-signer-serve --key-file /var/lib/postfiat-signer/validator-0/validator_keys.json --state-dir /var/lib/postfiat-signer/validator-0/guard --chain-id postfiat-stage-test --genesis-hash {genesis} --socket /run/postfiat-signer/validator-0.sock --socket-group postfiat-signer-clients --allow-peer-uid ${{POSTFIAT_SIGNER_ALLOW_PEER_UID}}\nRuntimeDirectory=postfiat-signer\nRuntimeDirectoryMode=0750\nStateDirectory=postfiat-signer/validator-0\nStateDirectoryMode=0700\n{hardening}PrivateNetwork=true\nUMask=0077\nLimitCORE=0\nTasksMax=64\nKillMode=control-group\nRestart=on-failure\nRestartSec=5\nTimeoutStopSec=30\n\n[Install]\nWantedBy=multi-user.target\n",
            genesis = "11".repeat(48),
        )
    );
    assert!(rpc.contains(
        "\nAfter=network-online.target postfiat-validator-0.service postfiat-validator-0-signer.service\nRequires=postfiat-validator-0-signer.service\n"
    ));
    assert!(rpc.contains("\nUser=postfiat\nGroup=postfiat\nSupplementaryGroups=postfiat-signer-clients\n"));
    assert!(rpc.contains(" --finality-signer-socket /run/postfiat-signer/validator-0.sock "));
    assert!(rpc.contains(hardening));
    for unit in [&signer, &rpc] {
        assert!(!unit.contains("--finality-key-file"));
        assert!(!unit.contains("--finality-proposal-key-file"));
        assert!(!unit.contains("/var/lib/postfiat/validator-0/validator_keys.json"));
        assert!(!unit.contains("CAP_"));
        assert!(!unit.contains("--initialize-state"));
        assert!(!unit.contains("--min-height"));
    }
    // Everything else the packet signs is as in the default shape.
    let default_root = unique_test_dir("postfiat-two-user-units-default");
    let default = stage_deployment_validator_units(stage_inputs(&default_root)).expect("stage");
    let default_row = &default.validators[0];
    assert!(default_row.signer_unit_file.is_none());
    let rewritten = std::fs::read_to_string(&default_row.rpc_unit_file)
        .unwrap()
        .replace(
            "postfiat-validator-0.service\n",
            "postfiat-validator-0.service postfiat-validator-0-signer.service\nRequires=postfiat-validator-0-signer.service\n",
        )
        .replace("Group=postfiat\n", "Group=postfiat\nSupplementaryGroups=postfiat-signer-clients\n")
        .replace(
            "--finality-key-file /var/lib/postfiat/validator-0/validator_keys.json --finality-proposal-key-file /var/lib/postfiat/validator-0/validator_keys.json",
            "--finality-signer-socket /run/postfiat-signer/validator-0.sock",
        );
    assert_eq!(rpc, rewritten);
    // The transport unit signs through the same socket: no key file and no
    // file-signer acknowledgement, the socket's group, the signer first.
    let transport = std::fs::read_to_string(&row.transport_unit_file).expect("transport unit");
    let rewritten = std::fs::read_to_string(&default_row.transport_unit_file)
        .unwrap()
        .replace(
            "\nAfter=network-online.target\n",
            "\nAfter=network-online.target postfiat-validator-0-signer.service\nRequires=postfiat-validator-0-signer.service\n",
        )
        .replace("Group=postfiat\n", "Group=postfiat\nSupplementaryGroups=postfiat-signer-clients\n")
        .replace(" --unsafe-devnet-file-signer", "")
        .replace(
            "--key-file /var/lib/postfiat/validator-0/validator_keys.json",
            "--finality-signer-socket /run/postfiat-signer/validator-0.sock",
        );
    assert_eq!(transport, rewritten);
    assert!(transport.contains(" --finality-signer-socket /run/postfiat-signer/validator-0.sock "));
    assert!(!transport.contains("--key-file"));
    assert!(!transport.contains("--unsafe-devnet-file-signer"));
    assert!(!transport.contains("validator_keys.json"));
    for (two_user, single) in [
        (&row.rpc_environment_file, &default_row.rpc_environment_file),
        (&row.transport_environment_file, &default_row.transport_environment_file),
        (&row.runtime_bindings_file, &default_row.runtime_bindings_file),
    ] {
        assert_eq!(std::fs::read(two_user).unwrap(), std::fs::read(single).unwrap());
    }
    std::fs::remove_dir_all(root).expect("cleanup");
    std::fs::remove_dir_all(default_root).expect("cleanup");
}
