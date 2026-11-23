import unittest
from unittest.mock import Mock
import exit_payout as p
import exit_canary as canary

class BudgetCanaryTest(unittest.TestCase):
    def test_shared_custody_reserves_and_fee_floor(self):
        b=dict(source_domain='same',outstanding_vault_bridge_atoms=20000,redemption_queue_atoms=20000,
               nav_subscription_allocations_atoms=0,other_allocations_atoms=0,counted_value_atoms=40000)
        self.assertEqual(p.shared_liabilities([b,dict(b,redemption_queue_atoms=0,counted_value_atoms=20000),dict(b,source_domain='other')],'same'),60000)
        c=Mock(owner=p.FEE_PAYER)
        c.rent.side_effect=lambda n:100+n
        c.call.side_effect=lambda method,*args: {'value':{'blockhash':str(p.Hash.default())}} if method=='getLatestBlockhash' else {'value':10000 if method=='getFeeForMessage' else p.FLOOR}
        debit=p.payout_budget(c,1)
        self.assertGreater(debit,10000+c.rent(2048))
        with self.assertRaises(ValueError):p.balance(c,debit)

    def test_complete_signed_fields_bind_fee_sequence_and_domain(self):
        plan=dict(chain_id='net',genesis_hash='00'*48,source=canary.OWNER,fee=23,sequence=12,protocol_version=1,operation={'operation':'vault_bridge_burn_to_redeem','amount_atoms':20000})
        u=canary.expected_unsigned(plan)
        self.assertEqual(u['amount_atoms'],20000)
        for k in u:
            with self.assertRaises(ValueError):
                canary.check_unsigned(dict(u,**{k:'different'}),plan)

    def test_canary_uses_transaction_local_token_deltas(self):
        recipient=str(p.pda(p.ATA,bytes(canary.PHANTOM),bytes(p.TOKEN),bytes(p.MINT)))
        custody=str(p.pda(p.VAULT,b'custody'))
        def row(i,n):
            return dict(accountIndex=i,mint=str(p.MINT),uiTokenAmount={'amount':str(n),'decimals':6})
        tx={'transaction':{'message':{'accountKeys':[custody,recipient]}},'meta':{
            'err':None,'preTokenBalances':[row(0,80000)],'postTokenBalances':[row(0,60000),row(1,20000)]}}
        self.assertEqual(canary.token_deltas(tx),{'custody':-20000,'recipient':20000})
        tx['meta']['postTokenBalances'][0]=row(0,59999)
        with self.assertRaises(ValueError):canary.token_deltas(tx)
