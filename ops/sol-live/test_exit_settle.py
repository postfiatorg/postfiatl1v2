import unittest
from copy import deepcopy
import base58
import exit_canary as c
import exit_payout as p


class SettlementTest(unittest.TestCase):
    def test_finalized_ordinal_accounts_and_draft_match_paid_record(self):
        sig = base58.b58encode(bytes(range(64))).decode()
        burn = "ab"*48
        packet = "cd"*48
        ata=p.pda(p.ATA,bytes(c.PHANTOM),bytes(p.TOKEN),bytes(p.MINT))
        accounts=[str(x) for x in [p.FEE_PAYER,p.LIGHT,"prior","next","buffer",p.SYSTEM,p.marker(burn),
                  p.VAULT,p.pda(p.VERIFIER,b"egress-release",bytes(p.VAULT)),p.pda(p.VAULT,b"config"),
                  p.pda(p.VAULT,b"custody"),ata,c.PHANTOM,p.MINT,p.TOKEN]]
        ix=dict(programIdIndex=15,accounts=list(range(15)),data=base58.b58encode(b"\2"+bytes(356)).decode())
        def row(i,n):
            return dict(accountIndex=i,mint=str(p.MINT),uiTokenAmount=dict(amount=str(n),decimals=6))
        tx=dict(slot=9,transaction=dict(signatures=[sig],message=dict(accountKeys=accounts+[str(p.VERIFIER)],instructions=[ix])),
                meta=dict(err=None,innerInstructions=[],preTokenBalances=[row(10,20000)],postTokenBalances=[row(10,0),row(11,20000)]))
        paid=dict(burn_tx_id=burn, finalized_slot=9,solana_tx=sig,withdrawal_packet_hash=packet)
        ref=c.finalized_reference(paid,tx)
        self.assertEqual(ref["kind"],"solana_exit_r8_paid:"+packet+":9:0:"+bytes(range(64)).hex())
        for field,value in [("finalized_slot",10),("solana_tx","different")]:
            with self.assertRaises(ValueError):
                c.finalized_reference(dict(paid,**{field:value}),tx)
        for key in ["duplicate","inner","wrong owner"]:
            altered=deepcopy(tx)
            if key=="duplicate":
                altered["transaction"]["message"]["instructions"].insert(0,ix)
            elif key=="inner":
                altered["meta"]["innerInstructions"]=[dict(index=0,instructions=[ix])]
            else:
                altered["transaction"]["message"]["instructions"][0]["accounts"][12]=0
            with self.assertRaises(ValueError):
                c.finalized_reference(paid,altered)
        with self.assertRaises(ValueError):
            c.match_reference(dict(kind=ref["kind"].replace(":9:0:",":9:1:")),ref)
