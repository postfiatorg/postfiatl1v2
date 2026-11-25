"""Operator integration for a historical checkpoint and paged native witness RPC."""
import json
import tempfile
import unittest
from pathlib import Path
from unittest.mock import Mock, patch

from web3 import Web3

import service
from eth_journal import durable_json


def blocks_between(low, high):
    return {h: {'header': {'height':h, 'block_hash':f'{h:096x}', 'parent_hash':f'{h-1:096x}'}}
            for h in range(low, high+1)}


class BatchTests(unittest.TestCase):
    def test_second_burn_uses_an_accepted_prior_after_first_moves_the_tip(self):
        fixture = json.loads((Path(__file__).with_name('fixtures')/'witness.json').read_text())
        first = fixture['withdrawal_packet']
        second = dict(first, burn_tx_id='77'*48, withdrawal_id='88'*48, amount_atoms=42)
        h = first['finalized_height']
        prior = h - 3
        blocks = blocks_between(prior, h)
        blocks[h]['receipt_ids'] = [second['burn_tx_id'], '99'*48, first['burn_tx_id']]
        exits = [second, first]
        redemptions = [dict(created_at_height=h, withdrawal_packet=p) for p in exits]
        archive = {'ledger':{'vault_bridge_redemptions':redemptions}, 'blocks':{'blocks':list(blocks.values())}}
        reads = {'latestFinalizedHeight': prior, 'routeEpoch':1}
        audit = {'block':100, 'contracts':{'PFTLFinalityVerifierV1':{'address':service.VERIFIER, 'reads':reads}}}
        client = Mock()
        client.eth.chain_id = 1
        lookup = client.eth.contract.return_value.functions.acceptedCheckpointCommitment

        def accepted(digest):
            def check(block_identifier):
                self.assertEqual(block_identifier, 100)
                return digest == Web3.keccak(bytes.fromhex(blocks[prior]['header']['block_hash']))
            return Mock(call=check)

        lookup.side_effect = accepted

        def receipt(burn):
            return {'accepted':True, 'code':'accepted', 'tx_id':burn}

        requests = []
        def witness(method, params):
            self.assertEqual(method, 'pfusdc_egress_witness')
            self.assertEqual(params['prior_checkpoint'], blocks[prior]['header']['block_hash'])
            index, packet = next((i,p) for i,p in enumerate(exits) if p['withdrawal_id']==params['withdrawal_id'])
            requests.append(packet['burn_tx_id'])
            return dict(block=blocks[h], prior_checkpoint_block_id=params['prior_checkpoint'],
                        finality_ancestry=[{'block':blocks[i]} for i in range(prior+1,h)],
                        withdrawal_packet=packet, receipt=receipt(packet['burn_tx_id']),
                        route_profile={'profile':{'route_epoch':1}},
                        merkle_proof={'leaf_index':index,'leaf_count':2,'siblings':['42'*48]})

        def proof(kind, path, directory, name):
            self.assertEqual(kind, 'withdrawal')
            doc = json.loads(path.read_text())
            packet = doc['withdrawal_packet']
            self.assertEqual(doc['merkle_proof']['leaf_index'], exits.index(packet))
            self.assertEqual(doc['merkle_proof']['siblings'], ['42'*48])

        def payout(mode, proof_dir, out):
            self.assertEqual(mode, 'withdraw')
            reads['latestFinalizedHeight'] = h
            tx = {'mode':mode, 'tx':requests[-1][:64], 'block':99, 'status':1}
            durable_json(out, tx)
            return tx

        with tempfile.TemporaryDirectory() as tmp, patch.object(service, 'JOBS', Path(tmp)), \
             patch.object(service, 'audit', return_value=audit), patch.object(service, 'Web3', wraps=Web3) as web, \
             patch.object(service, 'call', side_effect=witness), patch.object(service, 'prove', side_effect=proof), \
             patch.object(service, 'eth_send', side_effect=payout), \
             patch.object(service, 'read_six', side_effect=lambda burn:[{'receipts':[receipt(burn)]}]*6), \
             patch.object(service, 'export_bundle', side_effect=lambda d:(d/'archive',archive)):
            web.return_value = client
            for packet in (first,second):
                job = dict(burn_tx_id=packet['burn_tx_id'])
                service.process(job)
                self.assertEqual(job['stage'], 'paid')
            self.assertEqual(requests, [first['burn_tx_id'],second['burn_tx_id']])
            self.assertEqual(lookup.call_count, h - prior)

    def test_paging_preserves_governance_transitions_and_allows_small_frames(self):
        blocks = blocks_between(10, 40)
        def witness(method, params):
            low = int(params['prior_checkpoint'], 16)
            end = int(params['target_block'],16) if method=='pfusdc_checkpoint_witness' else 40
            if end-low>4:
                raise RuntimeError('rpc_response_too_large')
            return dict(block=blocks[end], prior_checkpoint_block_id=params['prior_checkpoint'],
                        finality_ancestry=[dict(block=blocks[h], next_committee_epoch=h)
                                          for h in range(low+1,end)])
        with patch.object(service, 'call', side_effect=witness):
            result = service.native_witness(blocks, 10, 40, 'withdrawal-id')
        self.assertEqual(result['prior_checkpoint_block_id'],blocks[10]['header']['block_hash'])
        self.assertEqual([step['next_committee_epoch'] for step in result['finality_ancestry']],list(range(11,40)))
