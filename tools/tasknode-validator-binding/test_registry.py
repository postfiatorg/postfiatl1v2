import copy
import datetime as dt
import unittest
import tempfile
from pathlib import Path
from registry import UTC, CHAIN, atomic_json, block_summary, daily_participation, iso, registry_rows, snapshot

GENESIS = 'a' * 96
PK = 'a' * 3904


def block(height=1, parent='0' * 96, signers=('v0', 'v1')):
    identity = {'height': height, 'block_id': format(height, '096x')}
    domain = {'chain_id': CHAIN, 'genesis_hash': GENESIS, 'committee_root': 'b' * 96}
    qc = {'phase': 'precommit', 'round': {'height': height, 'view': 0}, 'block': identity,
          'domain': domain, 'quorum': 2, 'validators': ['v0', 'v1', 'v2'], 'certificate_id': 'c' * 96,
          'votes': [{'phase': 'precommit', 'round': {'height': height, 'view': 0}, 'block': identity,
                     'domain': domain, 'validator': s,
                     'signature': {'signer': s, 'algorithm_id': 'ML-DSA-65',
                                   'public_key_hex': PK, 'signature_hex': 'a' * 6618}} for s in signers]}
    return {'header': {'height': height, 'block_hash': identity['block_id'], 'parent_hash': parent,
                       'consensus_v2_commit': {'precommit_qc': qc}}}


class RegistryTest(unittest.TestCase):
    def test_counts_actual_precommits_not_committee_members(self):
        value = block_summary(block(), CHAIN, GENESIS)
        self.assertEqual(value['precommit_signers'], ['v0', 'v1'])
        self.assertNotIn('v2', value['precommit_signers'])

    def test_reject_wrong_phase_chain_identity_duplicate_votes_and_missing_qc(self):
        for mutation in [lambda q: q.update(phase='prepare'), lambda q: q['domain'].update(chain_id='other'),
                         lambda q: q['votes'].append(q['votes'][0]), lambda q: q['block'].update(block_id='d' * 96),
                         lambda q: q.update(quorum=3), lambda q: q['votes'][0]['signature'].update(signature_hex='00')]:
            value = block()
            mutation(value['header']['consensus_v2_commit']['precommit_qc'])
            with self.assertRaises(ValueError): block_summary(value, CHAIN, GENESIS)
        value = block(); value['header'].pop('consensus_v2_commit')
        with self.assertRaises(ValueError): block_summary(value, CHAIN, GENESIS)

    def test_empty_history_cannot_grant_reward_or_invent_zero(self):
        now = dt.datetime(2026, 10, 28, 14, tzinfo=UTC)
        p = daily_participation({}, [{'validator_id': 'v0'}], now)['v0']
        self.assertFalse(p['available']); self.assertFalse(p['eligible'])
        self.assertIsNone(p['share']); self.assertIsNone(p['mean_daily_score'])
        self.assertEqual(p['reported_days'], 0)
        self.assertEqual(len(p['daily']), 30)
        self.assertTrue(all(d['score'] is None for d in p['daily']))

    def intervals(self):
        day = dt.datetime(2026, 10, 27, tzinfo=UTC)
        rows = []
        for i in range(144):
            lower = day + dt.timedelta(minutes=10*i)
            blocks = [{'height': 10, 'precommit_signers': ['v0']}, {'height': 11, 'precommit_signers': ['v1']}] if i == 30 else []
            rows.append({'lower_bound': iso(lower), 'upper_bound': iso(lower+dt.timedelta(minutes=10)), 'blocks': blocks})
        return rows

    def test_observed_day_score_and_midnight_gap_safety(self):
        now = dt.datetime(2026, 10, 28, 14, tzinfo=UTC)
        state = {'intervals': self.intervals()}
        p = daily_participation(state, [{'validator_id':'v0'}], now)['v0']
        self.assertEqual(p['reported_days'], 1)
        self.assertEqual(p['share'], 0.5); self.assertEqual(p['mean_daily_score'], 0.5)
        self.assertEqual(p['daily'][-1]['committed_blocks'], 2)
        self.assertFalse(p['eligible'])
        # Empty days cannot claim 100%; a gap or crossing-midnight block cannot be assigned.
        state['intervals'].pop(60)
        self.assertEqual(daily_participation(state, [{'validator_id':'v0'}], now)['v0']['reported_days'], 0)
        state = {'intervals': self.intervals()}
        state['intervals'][-1]['blocks'] = [{'height': 12, 'precommit_signers':['v0']}]
        self.assertEqual(daily_participation(state, [{'validator_id':'v0'}], now)['v0']['reported_days'], 0)

    def observed_history(self, scores, first_day=dt.datetime(2026, 10, 1, tzinfo=UTC)):
        intervals = []
        height = 1
        for offset, (signed, total) in enumerate(scores):
            day = first_day + dt.timedelta(days=offset)
            for step in range(144):
                lower = day + dt.timedelta(minutes=10 * step)
                blocks = []
                if step == 30:
                    for index in range(total):
                        blocks.append({'height': height, 'precommit_signers': ['v0'] if index < signed else ['v1']})
                        height += 1
                intervals.append({'lower_bound': iso(lower), 'upper_bound': iso(lower + dt.timedelta(minutes=10)), 'blocks': blocks})
        return {'intervals': intervals}

    def test_d63_twenty_five_reported_days_and_d1_daily_mean(self):
        now = dt.datetime(2026, 10, 26, 14, tzinfo=UTC)
        rows = [{'validator_id': 'v0'}]
        before = daily_participation(self.observed_history([(1, 1)] * 24), rows, now)['v0']
        self.assertEqual(before['reported_days'], 24)
        self.assertFalse(before['available']); self.assertFalse(before['eligible'])
        self.assertEqual(before['eligibility_reason'], 'insufficient_observed_days')
        # D1 is an unweighted daily average, not aggregate block share.
        above = daily_participation(self.observed_history([(1, 1)] * 24 + [(0, 100)]), rows, now)['v0']
        self.assertEqual(above['reported_days'], 25)
        self.assertTrue(above['available']); self.assertTrue(above['eligible'])
        self.assertEqual(above['mean_daily_score'], 0.96)
        self.assertLess(above['share'], 0.95)
        below = daily_participation(self.observed_history([(100, 100)] * 23 + [(0, 1)] * 2), rows, now)['v0']
        self.assertTrue(below['available']); self.assertFalse(below['eligible'])
        self.assertEqual(below['mean_daily_score'], 0.92)
        self.assertGreater(below['share'], 0.95)
        self.assertEqual(below['eligibility_reason'], 'below_participation_threshold')
        boundary = daily_participation(self.observed_history([(1, 1)] * 23 + [(3, 4), (0, 1)]), rows, now)['v0']
        self.assertEqual(boundary['mean_daily_score'], 0.95)
        self.assertTrue(boundary['eligible'])
        repeated_boundary = daily_participation(self.observed_history([(19, 20)] * 25), rows, now)['v0']
        self.assertEqual(repeated_boundary['mean_daily_score'], 0.95)
        self.assertTrue(repeated_boundary['eligible'])

    def test_d63_gaps_empty_days_and_rolling_window_cannot_inflate_coverage(self):
        now = dt.datetime(2026, 10, 26, 14, tzinfo=UTC)
        rows = [{'validator_id': 'v0'}]
        history = self.observed_history([(1, 1)] * 25)
        history['intervals'].pop(60)
        gapped = daily_participation(history, rows, now)['v0']
        self.assertEqual(gapped['reported_days'], 24)
        self.assertFalse(gapped['available']); self.assertFalse(gapped['eligible'])
        empty_day = daily_participation(self.observed_history([(1, 1)] * 24 + [(0, 0)]), rows, now)['v0']
        self.assertTrue(empty_day['daily'][-1]['complete'])
        self.assertFalse(empty_day['daily'][-1]['reported'])
        self.assertFalse(empty_day['available']); self.assertFalse(empty_day['eligible'])
        history = self.observed_history([(1, 1)] * 25)
        expired = daily_participation(history, rows, dt.datetime(2026, 11, 2, tzinfo=UTC))['v0']
        self.assertEqual(expired['reported_days'], 23)
        self.assertFalse(expired['available']); self.assertFalse(expired['eligible'])

    def source(self, port, method, params=None):
        if method == 'blocks': return [block()]
        if method == 'status':
            return {'node_id': str(port), 'chain_id': CHAIN, 'genesis_hash': GENESIS,
                    'block_height': 1, 'block_tip_hash': format(1, '096x')}
        return {'chain_id': CHAIN, 'genesis_hash': GENESIS, 'registry_root': 'c'*96,
                'validator_count': 1, 'validators': [{'node_id':'v0', 'algorithm_id':'ML-DSA-65','public_key_hex':PK}]}

    def test_consistent_snapshot_and_chain_reset_failure(self):
        now = dt.datetime(2026, 10, 28, 14, tzinfo=UTC)
        output, state = snapshot([1,2], {}, now, self.source, lambda: now)
        self.assertEqual(output['height'], 1); self.assertEqual(len(output['source_heights']), 2)
        self.assertFalse(output['validators'][0]['participation']['eligible'])
        with self.assertRaises(ValueError): snapshot([1,1], {}, now, self.source, lambda: now)
        state['genesis_hash'] = 'd'*96
        with self.assertRaises(ValueError): snapshot([1,2], state, now, self.source, lambda: now)

    def test_disagreeing_sources_preserve_previous_state(self):
        now = dt.datetime(2026, 10, 28, 14, tzinfo=UTC)
        def source(port, method, params=None):
            value = self.source(port, method, params)
            if port == 2 and method == 'validators': value['registry_root'] = 'e'*96
            return value
        state = {}; before = copy.deepcopy(state)
        with self.assertRaises(ValueError): snapshot([1,2], state, now, source, lambda: now)
        self.assertEqual(state, before)

    def test_registry_rejects_duplicate_key_and_bad_length(self):
        r = self.source(1, 'validators'); r['validators'][0]['public_key_hex']='a'*3903
        with self.assertRaises(ValueError): registry_rows(r)
        r=self.source(1,'validators'); r['validator_count']=2
        r['validators'].append({**r['validators'][0], 'node_id':'v1'})
        with self.assertRaises(ValueError): registry_rows(r)

    def test_refuses_to_write_inside_validator_data_directory(self):
        with tempfile.TemporaryDirectory() as root:
            data = Path(root) / 'validator'
            (data / 'nested').mkdir(parents=True)
            (data / 'node_state.json').write_text('{}')
            for target in [data / 'registry.json', data / 'nested' / 'observer.json']:
                with self.assertRaises(ValueError): atomic_json(target, {})
                self.assertFalse(target.exists())
            atomic_json(Path(root) / 'publish' / 'registry.json', {'ok': True})
            self.assertTrue((Path(root) / 'publish' / 'registry.json').exists())


if __name__ == '__main__': unittest.main()
