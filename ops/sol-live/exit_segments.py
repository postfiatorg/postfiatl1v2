"""Select checkpoints whose committee can authenticate the following segment."""
from exit_setup import require


def committee(header):
    return header["consensus_v2_commit"]["precommit_qc"]["domain"]["committee_root"]


def checkpoint_target(latest, blocks):
    for current, following in reversed(list(zip(blocks, blocks[1:]))):
        h, successor = current["header"], following["header"]
        require(successor["height"] == h["height"] + 1 and successor["parent_hash"] == h["block_hash"],
                "Discontinuous finalized headers.")
        if latest < h["height"] <= latest + 65 and committee(h) == committee(successor):
            return h
    return None


def select_target(latest, fetch):
    end = latest + 65
    while end > latest:
        start = max(latest + 1, end - 3)
        blocks = fetch(0, "blocks", from_height=start, limit=end - start + 2)
        require(len(blocks) == end - start + 2 and blocks[0]["header"]["height"] == start,
                "Checkpoint header range unavailable.")
        target = checkpoint_target(latest, blocks)
        if target:
            return target
        end = start - 1
    raise ValueError("No compatible checkpoint within the guest ancestry bound.")


def validate_segment(witness, prior, target, start_root):
    headers = [step["block"]["header"] for step in witness["finality_ancestry"]]
    headers.append(witness["block"]["header"])
    require(witness["prior_checkpoint_block_id"] == prior and headers[0]["parent_hash"] == prior
            and committee(headers[0]) == start_root and len(headers) <= 65
            and headers[-1] == target, "Unexpected checkpoint segment endpoints.")
    for previous, current in zip(headers, headers[1:]):
        require(current["parent_hash"] == previous["block_hash"] and current["height"] == previous["height"]+1,
                "Disconnected checkpoint witness.")
