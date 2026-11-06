# Deploy sheet template: two-user validator signer

Copy into a release's `DEPLOY-SHEET.md` when its packet is staged with
`deployment-validator-units-stage --signer-shape two-user`. The units and the
rules are on the [operator page](public-rpc-operator-policy.md#two-user-units).
Replace `<validator>`, `<release>`, `<chain-id>`, `<genesis-hash>` and `N`.

The signer serves both rpc-serve and `transport-validator-serve`, so the key
moves to the signer's directory and no copy stays in the data directory.

## Per validator (operator's go)

The steps are the operator page's, in the same order and words.

1. [ ] Create `postfiat-signer` and `postfiat-signer-clients`, add `postfiat` to the group, and write `/etc/postfiat/signer/<validator>.env` with `POSTFIAT_SIGNER_ALLOW_PEER_UID=$(id -u postfiat)`.

    ```sh
    useradd --system --no-create-home postfiat-signer
    groupadd --system postfiat-signer-clients
    usermod -aG postfiat-signer-clients postfiat-signer
    usermod -aG postfiat-signer-clients postfiat
    install -d -m 0755 /etc/postfiat/signer
    echo "POSTFIAT_SIGNER_ALLOW_PEER_UID=$(id -u postfiat)" > /etc/postfiat/signer/<validator>.env
    install -d -o postfiat-signer -g postfiat-signer-clients -m 0700 /var/lib/postfiat-signer/<validator>
    ```

2. [ ] Stop both old units, `postfiat-<validator>.service` and `postfiat-<validator>-rpc.service`: both sign with the key.

    ```sh
    systemctl stop postfiat-<validator>.service postfiat-<validator>-rpc.service
    ```

3. [ ] Move the key to `/var/lib/postfiat-signer/<validator>/validator_keys.json` (`0600`, owned by `postfiat-signer`); no copy stays in the data directory.

    ```sh
    install -o postfiat-signer -m 0600 /var/lib/postfiat/<validator>/validator_keys.json \
      /var/lib/postfiat-signer/<validator>/validator_keys.json
    shred -u /var/lib/postfiat/<validator>/validator_keys.json
    ```

4. [ ] Choose the floor `N` by the rule above, with both old units stopped.

    The rule: the larger of the store's finalized height plus one (`status`)
    and the highest height in `block_proposal_vote_locks` plus one. If the old
    units may have voted or timed out on a height not final yet, read the tip
    after the other validators finalize past it. Record `N`.

5. [ ] Initialize the guard once as `postfiat-signer` with `--initialize-state --min-height N` and a throwaway `--socket` under the signer's directory; stop it after its ready line.

    ```sh
    sudo -u postfiat-signer /opt/postfiat/releases/<release>/postfiat-node validator-signer-serve \
      --key-file /var/lib/postfiat-signer/<validator>/validator_keys.json \
      --state-dir /var/lib/postfiat-signer/<validator>/guard \
      --chain-id <chain-id> --genesis-hash <genesis-hash> \
      --initialize-state --min-height N \
      --socket /var/lib/postfiat-signer/<validator>/initialize.sock \
      --allow-peer-uid "$(id -u postfiat)"
    ```

6. [ ] Start the signer unit, then the transport and rpc units.

    ```sh
    systemctl daemon-reload
    systemctl start postfiat-<validator>-signer.service
    systemctl start postfiat-<validator>.service postfiat-<validator>-rpc.service
    ```

7. [ ] Run the deployment check.

    `python -m postfiat_ops.validator_signer_check` with the operator page's
    arguments; record its `report.json`.

8. [ ] Read `private_readiness` and record `custody_reasons`.
