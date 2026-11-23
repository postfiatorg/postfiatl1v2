import unittest
from unittest.mock import Mock
from solders.hash import Hash
from solders.transaction import Transaction
import base64
import exit_setup as s

class Tag8Test(unittest.TestCase):
    def test_simulation_uses_unsigned_tag8_and_checks_returned_root(self):
        from exit_tag8 import simulate
        cfg=s.read(s.CONFIG)
        values={'prior':cfg['initial_checkpoint'],'result':'11'*48,'end':'22'*48,'height':1754,'start':cfg['initial_committee']}
        c=Mock(owner=s.MAINNET_PAYER,level={'commitment':'finalized'})
        light=s.expected_light(cfg)
        state=[{'owner':str(s.VERIFIER),'data':[base64.b64encode(d).decode(),'base64']}
               for d in [light[:-57]+bytes.fromhex(values['result'])+(1754).to_bytes(8,'little')+b'\0',bytes.fromhex(values['end'])]]
        c.call.side_effect=[{'value':{'blockhash':str(Hash.default())}},{'value':{'err':None,'accounts':state}}]
        simulate(c,values,bytes(356),s.FEE_PAYER,light)
        args=c.call.call_args.args
        self.assertEqual(args[0],'simulateTransaction')
        tx=Transaction.from_bytes(base64.b64decode(args[1]))
        self.assertFalse(any(bytes(tx.signatures[0])))
        self.assertEqual(tx.message.instructions[-1].data[0],8)
        self.assertFalse(args[2]['sigVerify'])
