import unittest
from unittest.mock import Mock
class SegmentTest(unittest.TestCase):
    def test_rotation_is_inside_next_segment(self):
        from exit_segments import checkpoint_target,validate_segment
        def block(h,root):
            return {'header':dict(height=h,block_hash=f'{h:096x}',parent_hash=f'{h-1:096x}',consensus_v2_commit={'precommit_qc':{'domain':{'committee_root':root}}})}
        old,new='ab'*48,'cd'*48
        blocks=[block(h, old if h<=1818 else new) for h in range(1817,1820)]
        self.assertEqual(checkpoint_target(1753,blocks)['height'],1817)
        w=dict(prior_checkpoint_block_id=f'{1817:096x}',finality_ancestry=[{'block':blocks[1]}],block=blocks[2])
        validate_segment(w,w['prior_checkpoint_block_id'],blocks[2]['header'],old)
        with self.assertRaises(ValueError):validate_segment(w,w['prior_checkpoint_block_id'],blocks[2]['header'],new)
        with self.assertRaises(ValueError):validate_segment(w,w['prior_checkpoint_block_id'],blocks[1]['header'],old)
