# Native-v1 signer-origin matrix

Locations are custody references only. No signing material is present in this directory.

| Leg | Native action | Signing identity / authority | Custody leaf location |
|---|---|---|---|
| 0 | Read-only fleet/EVM preflight | none | none |
| 1 | EVM deposit; PFTL relay/claim | agentd wallet; PFTL proposer/finalizer/claimer | `/home/postfiat/repos/StakeHub/stakehub/agentd.py`; `/home/postfiat/tmp/navswap-ce22-venue-rebuild-20260719/private/` |
| 2a | `pftl_uniswap_order_reserve` | holder/subscriber `live_demo_primary_subscriber`; PFTL validator/proposal | `/home/postfiat/.stakehub/live-demo-holder-custody/`; `/home/postfiat/tmp/navswap-ce22-venue-rebuild-20260719/private/` |
| 2b | `pftl_uniswap_primary_subscribe_v2` | holder/subscriber `live_demo_primary_subscriber`; PFTL validator/proposal | same holder vault and PFTL directory |
| 3a | `pftl_uniswap_export_debit` | holder/subscriber; PFTL validator/proposal | same holder vault and PFTL directory |
| 3b0 | exact ETH funding | agentd `0x1455Bd7F…`; target constrained signer `0xe01eaf76…` | `/home/postfiat/repos/StakeHub/stakehub/agentd.py` |
| 3b | proof-gated accept/mint | constrained signer `0xe01eaf76…` | `/run/user/1000/postfiat-constrained-signer/a666-signer.sock` |
| 3c | wA666 approval | agentd `0x1455Bd7F…` | `/home/postfiat/repos/StakeHub/stakehub/agentd.py` |
| 3d | Permit2 wA666 approval | agentd `0x1455Bd7F…` | `/home/postfiat/repos/StakeHub/stakehub/agentd.py` |
| 3e | wA666 to USDC swap | agentd `0x1455Bd7F…` | `/home/postfiat/repos/StakeHub/stakehub/agentd.py` |
| 3f | USDC approval | agentd `0x1455Bd7F…` | `/home/postfiat/repos/StakeHub/stakehub/agentd.py` |
| 3g | Permit2 USDC approval | agentd `0x1455Bd7F…` | `/home/postfiat/repos/StakeHub/stakehub/agentd.py` |
| 3h | USDC to wA666 swap | agentd `0x1455Bd7F…` | `/home/postfiat/repos/StakeHub/stakehub/agentd.py` |
| 4 | return burn; PFTL return import | constrained signer; PFTL operator/validator | `/run/user/1000/postfiat-constrained-signer/a666-signer.sock`; `/home/postfiat/tmp/navswap-ce22-venue-rebuild-20260719/private/` |
| 5a | `pftl_uniswap_primary_redeem` | holder/subscriber `live_demo_primary_subscriber`; PFTL validator/proposal | same holder vault and PFTL directory |
| 5b | PFTL exit/burn/settle and EVM withdrawal | PFTL owner/settlement; agentd withdrawal leaf | `/home/postfiat/tmp/navswap-ce22-venue-rebuild-20260719/private/`; `/home/postfiat/repos/StakeHub/stakehub/agentd.py` |
