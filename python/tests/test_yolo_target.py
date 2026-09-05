import copy
import json
from pathlib import Path
import tempfile
import unittest

from postfiat_rpc.yolo_target import registration, registration_id, decode_public_values, submit_operation, main, report_html

FIXTURE = Path(__file__).parents[2] / 'crates/execution/testdata/yolo-target-v1-synthetic-proof.json'


def inputs():
    fixture = json.loads(FIXTURE.read_text())
    public, proof = bytes.fromhex(fixture['publicValuesHex']), bytes.fromhex(fixture['proofCalldataHex'])
    values = decode_public_values(public)
    config = {key:values[key] for key in ('program_sha256','methodology_sha256','parameter_manifest_sha256',
        'collection_manifest_sha256','series_id_sha256','underlier_id_sha256','epoch_sha256','prior_state_sha256')}
    config.update(registrant='synthetic-registrant',submitter='synthetic-submitter',
        sp1_program_vkey=fixture['program']['programVkey'],replay_id_sha256='ab'*32,activation_height=12)
    return config, public, proof


class YoloTargetTest(unittest.TestCase):
    def test_real_public_proof_operation_preserves_all_bytes(self):
        config, public, proof = inputs()
        operation = submit_operation(config, public, proof)
        self.assertEqual(bytes(operation['sp1_proof_bytes']), proof)
        self.assertEqual(bytes(operation['sp1_public_values']), public)
        self.assertEqual(operation['values'], decode_public_values(public))
        self.assertEqual(registration_id(dict(reversed(list(config.items())))), registration_id(config))

    def test_each_required_registration_field_affects_identity(self):
        config, _, _ = inputs()
        for field in config:
            changed = copy.deepcopy(config)
            changed[field] = 13 if field == 'activation_height' else ('0x'+'ee'*32 if field == 'sp1_program_vkey' else 'ee'*32)
            self.assertNotEqual(registration_id(config), registration_id(changed), field)
            del changed[field]
            with self.assertRaises(ValueError): registration(changed)

    def test_prior_state_and_manifest_substitution_reject(self):
        config, public, proof = inputs()
        for field in ('prior_state_sha256','collection_manifest_sha256','program_sha256','parameter_manifest_sha256','epoch_sha256'):
            changed = {**config,field:'ee'*32}
            with self.assertRaises(ValueError): submit_operation(changed,public,proof)

    def test_unknown_versions_lengths_and_counts_reject(self):
        _, public, _ = inputs()
        for altered in (public[:-1],public+b'0',b'UNKNOWN!'+public[8:],public[:11]+b'\2'+public[12:], public[:400]+b'\0'*4+public[404:]):
            with self.assertRaises(ValueError): decode_public_values(altered)

    def test_cli_outputs_are_create_only_and_viewer_escapes_node_text(self):
        config, public, proof = inputs()
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory); (root/'config.json').write_text(json.dumps(config)); (root/'public.bin').write_bytes(public); (root/'proof.bin').write_bytes(proof)
            args=['prepare-submit','--config',str(root/'config.json'),'--public-values',str(root/'public.bin'),'--proof-calldata',str(root/'proof.bin'),'--output',str(root/'operation.json')]
            self.assertEqual(main(args),0)
            with self.assertRaises(FileExistsError): main(args)
            report={'schema':'postfiat.yolo.target_receipt_query.v1','chainId':'<script>alert(1)</script>','registrationId':registration_id(config),'finality':None}
            (root/'receipt.json').write_text(json.dumps(report))
            self.assertEqual(main(['html','--receipt',str(root/'receipt.json'),'--output',str(root/'receipt.html')]),0)
            page=(root/'receipt.html').read_text()
            self.assertNotIn('<script>',page);self.assertIn('&lt;script&gt;',page)


if __name__ == '__main__': unittest.main()
