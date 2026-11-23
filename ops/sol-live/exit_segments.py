"""Select checkpoints whose committee can authenticate the following segment."""

from exit_setup import require


def committee(header):
    return header["consensus_v2_commit"]["precommit_qc"]["domain"]["committee_root"]


def checkpoint_target(latest, blocks):
    for current, following in reversed(list(zip(blocks, blocks[1:]))):
        h, successor = current["header"], following["header"]
        require(
            successor["height"] == h["height"] + 1
            and successor["parent_hash"] == h["block_hash"],
            "Discontinuous finalized headers.",
        )
        if latest < h["height"] <= latest + 65 and committee(h) == committee(successor):
            return h
    return None


def select_target(latest, fetch):
    end = latest + 65
    while end > latest:
        start = max(latest + 1, end - 3)
        blocks = fetch(0, "blocks", from_height=start, limit=end - start + 2)
        require(
            len(blocks) == end - start + 2 and blocks[0]["header"]["height"] == start,
            "Checkpoint header range unavailable.",
        )
        target = checkpoint_target(latest, blocks)
        if target:
            return target
        end = start - 1
    raise ValueError("No compatible checkpoint within the guest ancestry bound.")


def validate_segment(witness, prior, target, start_root):
    headers = [step["block"]["header"] for step in witness.get("finality_ancestry", [])]
    headers.append(witness["block"]["header"])
    require(
        witness["prior_checkpoint_block_id"] == prior
        and headers[0]["parent_hash"] == prior
        and committee(headers[0]) == start_root
        and len(headers) <= 65
        and headers[-1] == target,
        "Unexpected checkpoint segment endpoints.",
    )
    for previous, current in zip(headers, headers[1:]):
        require(
            current["parent_hash"] == previous["block_hash"]
            and current["height"] == previous["height"] + 1,
            "Disconnected checkpoint witness.",
        )


def public_checkpoint(fetch, height, prior, target, members, domain):
    """Use allowlisted public blocks and the original signed governance payload."""
    import json

    require(0 < target["height"] - height <= 65, "Checkpoint outside guest range.")
    blocks = []
    for first in range(height + 1, target["height"] + 1, 8):
        count = min(8, target["height"] - first + 1)
        page = fetch(0, "blocks", from_height=first, limit=count)
        require(
            len(page) == count and page[0]["header"]["height"] == first,
            "Incomplete checkpoint page.",
        )
        blocks.extend(page)
    steps = []
    for current, following in zip(blocks, blocks[1:]):
        h = current["header"]
        d = h["consensus_v2_commit"]["precommit_qc"]["domain"]
        nd = following["header"]["consensus_v2_commit"]["precommit_qc"]["domain"]
        step = dict(
            block=current, committee=members, committee_epoch=d["committee_epoch"]
        )
        if d["committee_root"] != nd["committee_root"]:
            require(
                h["batch_kind"] == "governance",
                "Rotation requires archived governance.",
            )
            (batch,) = fetch(
                0,
                "batch_archive",
                batch_kind="governance",
                batch_id=h["batch_id"],
                limit=1,
            )
            require(batch["batch_id"] == h["batch_id"], "Wrong governance archive.")
            payload = batch["payload_json"]
            (update,) = json.loads(payload)["validator_registry_updates"]
            require(
                update["activation_height"] == h["height"],
                "Unsupported deferred rotation.",
            )
            records = {v["node_id"]: v for v in members}
            if update.get("new_record"):
                entry = update["new_record"]
                records[entry["node_id"]] = entry
            members = [
                records[i]
                for i in (update.get("new_validators") or update["validators"])
            ]
            step.update(
                governance_payload_json=payload,
                next_committee=members,
                next_committee_epoch=nd["committee_epoch"],
            )
        steps.append(step)
    w = dict(
        schema="postfiat.pfusdc.checkpoint_proof_witness.v1",
        **{k: domain[k] for k in ("chain_id", "genesis_hash", "protocol_version")},
        prior_checkpoint_block_id=prior,
        block=blocks[-1],
        committee=members,
        committee_epoch=target["consensus_v2_commit"]["precommit_qc"]["domain"][
            "committee_epoch"
        ],
    )
    if steps:
        w["finality_ancestry"] = steps
    validate_segment(w, prior, target, committee(blocks[0]["header"]))
    return w


class RotationScan:
    """Check bounded pages of finalized governance payloads before advertising a new burn."""

    def __init__(self):
        self.anchor = None
        self.height = 0
        self.tip = None
        self.blocked = False

    def check(self, fetch, latest, anchor, target):
        import json

        if self.anchor != anchor:
            self.anchor, self.height, self.tip, self.blocked = (
                anchor,
                latest - 1,
                None,
                False,
            )
        if self.blocked or target < self.height:
            return False
        count = min(8, target - self.height)
        if count == 0:
            return True
        page = fetch(0, "blocks", from_height=self.height + 1, limit=count)
        require(len(page) == count, "Incomplete committee history.")
        for block in page:
            h = block["header"]
            require(
                h["height"] == self.height + 1
                and (
                    h["parent_hash"] == self.tip
                    if self.tip
                    else h["block_hash"] == anchor
                ),
                "Committee history differs from the stored checkpoint.",
            )
            if h["batch_kind"] == "governance":
                (batch,) = fetch(
                    0,
                    "batch_archive",
                    batch_kind="governance",
                    batch_id=h["batch_id"],
                    limit=1,
                )
                require(batch["batch_id"] == h["batch_id"], "Wrong governance batch.")
                payload = json.loads(batch["payload_json"])
                updates = payload.get("validator_registry_updates", [])
                if updates:
                    u = updates[0]
                    signed = len(u.get("signed_authorizations", []))
                    # The frozen native verifier requires V2 old-quorum signatures and a single update.
                    if (
                        h["height"] == latest
                        or len(updates) != 1
                        or u["activation_height"] != h["height"]
                        or signed < (2 * len(u["validators"])) // 3 + 1
                        or signed != len(u["votes"])
                        or signed != len(u["support"])
                        or any(
                            payload.get(k)
                            for k in (
                                "amendments",
                                "governance_agent_dry_runs",
                                "fastswap_bootstraps",
                                "fastpay_recovery_bootstraps",
                                "vault_bridge_route_profile_activations",
                            )
                        )
                    ):
                        self.blocked = True
                        return False
            self.height, self.tip = h["height"], h["block_hash"]
        return self.height == target
