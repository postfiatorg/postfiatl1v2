# Deploy sheet template: two-user validator signer

Copy into a release's `DEPLOY-SHEET.md` when its packet is staged with
`deployment-validator-units-stage --signer-shape two-user`. The units and the
rules are on the [operator page](public-rpc-operator-policy.md#two-user-units).
Replace `<validator>`, `<release>`, `<chain-id>`, `<genesis-hash>` and `N`.

Until `transport-validator-serve` signs through the signer, its unit keeps
`<data-dir>/validator_keys.json` and readiness reports
`validator_key_copy_in_data_dir`. Record that reason; do not copy the key back.

## Per validator (operator's go)

- [ ] Users and key:

    ```sh
    useradd --system --no-create-home postfiat-signer
    groupadd --system postfiat-signer-clients
    usermod -aG postfiat-signer-clients postfiat-signer
    usermod -aG postfiat-signer-clients postfiat
    install -d -m 0755 /etc/postfiat/signer
    echo "POSTFIAT_SIGNER_ALLOW_PEER_UID=$(id -u postfiat)" > /etc/postfiat/signer/<validator>.env
    install -d -o postfiat-signer -g postfiat-signer-clients -m 0700 /var/lib/postfiat-signer/<validator>
    install -o postfiat-signer -m 0600 /var/lib/postfiat/<validator>/validator_keys.json \
      /var/lib/postfiat-signer/<validator>/validator_keys.json
    ```

- [ ] Stop the old units: `systemctl stop postfiat-<validator>-rpc.service`.
- [ ] Floor `N`: the larger of the store's finalized height plus one
  (`status`) and the highest height in `block_proposal_vote_locks` plus one.
  If the old signer may have voted or timed out on a height not final yet,
  read the tip after the other validators finalize past it. Record `N`.
- [ ] Initialize the guard once (stop with Ctrl-C after the ready line):

    ```sh
    sudo -u postfiat-signer /opt/postfiat/releases/<release>/postfiat-node validator-signer-serve \
      --key-file /var/lib/postfiat-signer/<validator>/validator_keys.json \
      --state-dir /var/lib/postfiat-signer/<validator>/guard \
      --chain-id <chain-id> --genesis-hash <genesis-hash> \
      --initialize-state --min-height N \
      --socket /var/lib/postfiat-signer/<validator>/initialize.sock \
      --allow-peer-uid "$(id -u postfiat)"
    ```

- [ ] Remove the data-directory key only after the transport validator no
  longer reads it (not yet: see above).
- [ ] `systemctl daemon-reload`, start `postfiat-<validator>-signer.service`,
  then `postfiat-<validator>-rpc.service`.
- [ ] Deployment check (`python -m postfiat_ops.validator_signer_check`, the
  operator page's command) and its `report.json` recorded.
- [ ] `private_readiness`: record `custody_reasons`.
