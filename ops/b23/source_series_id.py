#!/usr/bin/env python3
"""pfusdc_source_series_id (crates/types market_nav_asset_types.rs:1141), for predicting
the credited child S of a vault-route epoch. Usage: source_series_id.py EPOCH POLICY_HASH"""
import hashlib, sys
DOMAIN = sys.argv[3] if len(sys.argv) > 3 else None
def sid(epoch, policy, domain, chain="postfiat-wan-devnet-2",
        family="02c46a36eb0da3516b4d8affea8f4028ad3f36825a3e8f0e009ea9dbbbcfb3c233f6830bd5221fe2717fb6a1a7005d7b",
        src=1, vault="0xe7a432a28b20621a70e845c7f202004cf98e7a2c", token="0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"):
    pre = (f"pftl_chain_id_bytes={len(chain)}\npftl_chain_id={chain}\nasset_family_id={family}\nsource_chain_id={src}\n"
           f"vault_address={vault}\ntoken_address={token}\nroute_epoch={epoch}\npolicy_hash={policy}\n")
    h = hashlib.sha3_384(); h.update(domain.encode()); h.update(b"\x00"); h.update(pre.encode()); return h.hexdigest()
if __name__ == "__main__":
    print(sid(int(sys.argv[1]), sys.argv[2], DOMAIN))
