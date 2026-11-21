"""Append vote rows and reconcile the exact account prefix before each restart."""
import base64
import json
import struct

from solders.compute_budget import set_compute_unit_limit
from bridge import DIRECTORY, SYSTEM, VOTE, Instruction, pda, persistent, pk, require, ro, rw


def record(c, program):
    while True:
        epoch = c.call("getEpochInfo", c.level)["epoch"]
        if record_epoch(c, program, epoch):
            return


def record_epoch(c, program, epoch, publication="", *, scheduled=True):
    prefix = f"{c.genesis}-{program}-{epoch}"
    plan = DIRECTORY / f"{prefix}.votes.json"
    if not plan.exists():
        # Select V4 and Some(BLS) at the recorder ABI offsets, including delinquent votes.
        matches = c.call("getProgramAccounts", str(VOTE), {
            **c.level, "encoding": "base64", "dataSlice": {"offset": 0, "length": 193},
            "filters": [{"dataSize": 3762}, {"memcmp": {"offset": 0, "bytes": "5Sxr3"}},
                        {"memcmp": {"offset": 144, "bytes": "2"}}]})
        rows = []
        for result in matches:
            data = base64.b64decode(result["account"]["data"][0], validate=True)
            require(len(data) == 193 and data[:4] == b"\3\0\0\0" and data[144] == 1,
                    "invalid vote account response")
            rows.append([result["pubkey"], data[145:].hex()])
        require(rows, "no eligible BLS vote accounts")
        tmp = plan.with_suffix(".tmp")
        tmp.write_text(json.dumps(sorted(rows)))
        tmp.replace(plan)
    rows = json.loads(plan.read_text())
    if scheduled:
        publish_epoch(c, program, epoch)
    for tag, kind, header, stride, magic in [(0, "keys", 20, 48, b"PFSOLKY1"),
                                              (1, "next-stakes" if scheduled else "stakes", 28, 120, b"PFSOLNX1" if scheduled else b"PFSOLST1")]:
        output = persistent(f"{prefix}-{kind}{publication}")
        address = output.pubkey()
        while True:
            if c.call("getEpochInfo", c.level)["epoch"] != epoch:
                c.event(record="epoch advanced", prior_epoch=epoch)
                return False
            account = c.account(address)
            count = 0
            if account:
                raw = account["data"]
                require(account["owner"] == str(program) and raw[:16] == magic + struct.pack("<Q", epoch + int(scheduled and tag == 1)),
                        "recorder header mismatch")
                count = int.from_bytes(raw[header-4:header], "little")
                require(0 < count <= len(rows) and len(raw) == header + count * stride,
                        "recorder size mismatch")
                for i, (vote, key) in enumerate(rows[:count]):
                    row = raw[header+i*stride:header+(i+1)*stride]
                    require(row[-48:] == bytes.fromhex(key) and (not tag or row[:32] == bytes(pk(vote))),
                            "vote record differs from epoch input")
            if count == len(rows):
                c.event(record=kind, address=str(address), epoch=epoch, rows=count,
                        rent_lamports=account["lamports"])
                break
            batch = rows[count:count+16]
            ix = Instruction(program, bytes([tag]), [rw(c.owner, True), rw(address, True), ro(SYSTEM),
                             *([rw(pda(program, b"funder", bytes(address)))] if not account and scheduled else []),
                             *(ro(pk(vote)) for vote, _ in batch)])
            c.send(f"recorder.{epoch}.{kind}.{count}", [set_compute_unit_limit(1_400_000), ix], output)
    return True

def publish_epoch(c, program, epoch, *, refresh=False):
    """Publish the prior epoch's syscall snapshot as the current certificate set."""
    from bridge import load
    from solders.system_program import transfer
    path=DIRECTORY/f"{c.genesis}-{program}-{epoch-1}-next-stakes.json"
    if not path.exists():
        return False
    addr=load(path).pubkey()
    state=c.account(addr)
    if not state: return False
    require(state["owner"]==str(program) and state["data"][8:16]==struct.pack("<Q",epoch),"invalid scheduled epoch")
    if state["data"][:8]==b"PFSOLNX1":
        c.send(f"recorder.{epoch}.publish",[Instruction(program,b"\2",[rw(addr)])])
    elif state["data"][:8]==b"PFSOLST1":
        if refresh:
            # A fresh authenticated account change allows a later direct certificate
            # when the publication slot was only finalized by descent.
            c.send(f"recorder.{epoch}.refresh",[transfer(dict(from_pubkey=c.owner,to_pubkey=addr,lamports=1))])
    else:
        raise RuntimeError("unexpected stake record format")
    result=c.account(addr)
    require(result["data"]==b"PFSOLST1"+state["data"][8:],"published rank rows changed")
    return True
