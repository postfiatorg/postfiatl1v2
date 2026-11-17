import copy,json,unittest
from pathlib import Path
from exit_profile import select

class ExitProfileTests(unittest.TestCase):
    def setUp(self):
        root=Path(__file__).parent/'fixtures'
        self.w=json.loads((root/'witness.json').read_text());self.s=json.loads((root/'snapshot.json').read_text());self.a=json.loads((root/'audit.json').read_text())
    def test_epoch11_leaf_is_preserved_and_only_exit_descriptor_changes(self):
        result=select(self.w,self.s,self.a)
        self.assertEqual(result['route_profile']['profile']['route_epoch'],10)
        self.assertEqual(result['merkle_proof']['leaf']['route_epoch'],11)
        self.assertEqual([k for k in self.w if self.w[k]!=result[k]],['route_profile'])
    def test_wrong_recipient_and_amount_are_rejected(self):
        for key,value in [('recipient','0x'+'11'*20),('amount_atoms',500001)]:
            w=copy.deepcopy(self.w);w['withdrawal_packet'][key]=value
            with self.subTest(key=key),self.assertRaisesRegex(ValueError,'leaf/packet'):select(w,self.s,self.a)
    def test_foreign_vault_or_family_is_rejected(self):
        for key,value in [('vault_address','0x'+'11'*20),('vault_bridge_asset_id','11'*48)]:
            w=copy.deepcopy(self.w);w['withdrawal_packet'][key]=value
            with self.subTest(key=key),self.assertRaisesRegex(ValueError,'destination mismatch'):select(w,self.s,self.a)
    def test_bucket_of_another_vault_cannot_use_exit(self):
        self.s['ledger']['vault_bridge_bucket_states'][0]['source_domain']='erc20_bridge_vault:1:other-vault:other-token'
        with self.assertRaisesRegex(ValueError,'another vault'):select(self.w,self.s,self.a)
    def test_wrong_ingress_epoch_cannot_be_relabelled(self):
        self.w['merkle_proof']['leaf']['route_epoch']=10
        with self.assertRaisesRegex(ValueError,'ingress epoch'):select(self.w,self.s,self.a)
    def test_wrong_native_chain_and_expired_profile_fail(self):
        w=copy.deepcopy(self.w);w['chain_id']='other-chain'
        with self.assertRaisesRegex(ValueError,'native chain'):select(w,self.s,self.a)
        self.w['block']['header']['height']=1000000
        with self.assertRaisesRegex(ValueError,'validity interval'):select(self.w,self.s,self.a)
    def test_unknown_program_paused_or_insolvent_vault_fail(self):
        for name,value in [('paused',True),('totalObligations',1)]:
            a=copy.deepcopy(self.a);a['contracts']['ERC20BridgeVaultL1']['reads'][name]=value
            with self.subTest(name=name),self.assertRaises(ValueError):select(self.w,self.s,a)
        self.a['contracts']['PFTLFinalityVerifierV1']['reads']['programVKey']='0x'+'11'*32
        with self.assertRaisesRegex(ValueError,'unqualified frozen program'):select(self.w,self.s,self.a)

if __name__=='__main__':unittest.main()
