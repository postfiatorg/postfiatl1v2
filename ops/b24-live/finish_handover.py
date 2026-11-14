#!/usr/bin/env python3
"""Publish the verified native cutover handover and enable its bounded daily timer."""
import argparse,shutil
from datetime import datetime,timezone
from sign import *
OUT=COORD/'evidence/tee/phase5c'
OLD=COORD/'evidence/tee/phase5'
def main():
    ap=argparse.ArgumentParser();ap.add_argument('--activate',action='store_true',required=True);ap.parse_args()
    result=load(OUT/'cutover-result.json');assert result['passed'] and result==load(COORD/'status/b24-cutover-done')
    assert len(result['receipts'])==14 and all(r['accepted'] and r['six_validators'] and r['full_ledger_match'] for r in result['receipts'])
    state=load(Path(result['final_snapshot']));assert not [r for r in state['ledger']['trustlines'] if r['account']==FOUNDER and r['asset_id']==B24],'Founder trustline exists; deliver replacement before publishing pending status'
    assert balance(state['ledger'],ALICE,B24)>=101000000
    source=OLD/'b24-app-config.json';backup=OUT/'handover-before.json';assert not backup.exists();shutil.copyfile(source,backup)
    original_hash=hashlib.sha256(backup.read_bytes()).hexdigest();c=load(source)
    c.update(status='LIVE',live=True,g1_passed=True,profile_note='Registered on all six validators; real 13:30 enclave reading and Groth16 proof finalized at epoch 1.',last_fully_verified_height=result['height'],state_root=result['state_root'],cutover_marker_written=True,wallet_handover_instruction='B24 native cutover is verified. WALLET may switch to this configuration after checking the matching marker; Ethereum remains unavailable until separately funded and proved.')
    c.pop('stop_reason',None);c['wallet_config_patch']['g1_passed']=True;c['opening_reading']['live']=True
    c['native_state']=dict(opening_supply_atoms=result['opening_supply_atoms'],current_supply_atoms=result['supply_atoms'],redemption_reserve_atoms=result['reserve_atoms'],non_nav_spread_atoms=result['spread_atoms'],resting_offer=result['offer'],proof_epoch=1,NAV_atoms=result['nav_per_unit'])
    c['limits']['current_cash_in_reserve_atoms']=result['reserve_atoms'];c['limits']['reserve_funding_source']='20 PFUSDC(F), funded by 20.1 PFUSDC(F) from Alice test funds; 0.1 spread; no founder funds.'
    c['consumer_actions']['market_buy_sell']='Live native PFUSDC(S) order book: Alice lists 100 B24 at NAV 1. Requires B24 and market S trustlines. Wallet deployment is a separate WALLET-lane step.'
    c['consumer_actions']['cash_in']='9995/10000 of NAV for tracked native route inventory, limited by the matching reserve: 20 PFUSDC(F) currently funded. Book purchases/transfers do not update legacy route wallet inventory, so book buyers cannot cash in. No automatic broker withdrawal.'
    c['founder_entitlement'].update(status='1 B24 held within Alice inventory, outside the 100-B24 offer, pending founder trustline; not yet delivered or escrowed.',currently_held_B24_atoms=1000000)
    here=Path(__file__).resolve().parent;python='/home/postfiat/repos/navstrategies-b23-tee-nav/.venv/bin/python'
    local=json.loads(subprocess.check_output([python,str(here/'daily.py'),'--check'],text=True));assert local['local_check'] and local['asset_id']==B24 and local['epoch']==2;save(OUT/'daily-local-check-at-enable.json',local)
    subprocess.run(['systemd-analyze','--user','verify',str(here/'b24-tee-nav-daily.service'),str(here/'b24-tee-nav-daily.timer')],check=True)
    units=Path.home()/'.config/systemd/user';units.mkdir(parents=True,exist_ok=True)
    for name in ('b24-tee-nav-daily.service','b24-tee-nav-daily.timer'):subprocess.run(['install','-m','644',str(here/name),str(units/name)],check=True)
    subprocess.run(['systemctl','--user','daemon-reload'],check=True);subprocess.run(['systemctl','--user','enable','--now','b24-tee-nav-daily.timer'],check=True)
    rows={}
    for name in ('b23-tee-nav-daily.timer','b23-tee-nav-daily.service','b24-tee-nav-daily.timer','b24-tee-nav-daily.service'):
        rows[name]=subprocess.check_output(['systemctl','--user','show',name,'-p','ActiveState','-p','SubState','-p','UnitFileState','-p','NextElapseUSecRealtime'],text=True)
    assert 'ActiveState=active' in rows['b24-tee-nav-daily.timer'] and 'UnitFileState=enabled' in rows['b24-tee-nav-daily.timer']
    assert all('ActiveState=inactive' in rows[name] for name in ('b23-tee-nav-daily.timer','b23-tee-nav-daily.service','b24-tee-nav-daily.service'))
    daily=dict(enabled_at=datetime.now(timezone.utc).isoformat(),units=rows,first_observation_UTC=datetime.fromtimestamp(local['cutoff_unix_ms']/1000,timezone.utc).isoformat(),run_cap_usd='9',month_cap_usd='250',cumulative_lane_cap_usd='100',run_now=False)
    save(OUT/'daily-enabled.json',daily);c['daily']=daily;save(source,c);save(OUT/'b24-app-config.json',c)
    save(OUT/'handover-history.json',dict(mutable_handover=str(source),previous_sha256=original_hash,previous_bytes_preserved_at=str(backup),current_sha256=hashlib.sha256(source.read_bytes()).hexdigest(),cutover_marker=str(COORD/'status/b24-cutover-done'),wallet_runtime_changed=False))
    print(json.dumps(dict(handover_live=True,g1_passed=True,daily_timer_enabled=True,first_observation_UTC=daily['first_observation_UTC'],founder_held_B24_atoms=1000000)))
if __name__=='__main__':main()
