import json, pathlib, re
C = pathlib.Path(__file__).parent
hosts, cur, sec = [], None, None
for line in (C / 'logs/fleet-disk.stdout').read_text().splitlines():
    if line.startswith('=== '):
        _, vid, ip = line.split()
        cur = dict(validator=vid, host=ip, releases_top=[], snapshots_top=[], var_lib_postfiat_top=[]); hosts.append(cur); sec = 'df'; continue
    if line.startswith('--'):
        sec = line[2:]; continue
    f = line.split()
    if sec == 'df' and f[0].startswith('/dev/'):
        if f[1].endswith('G'):
            cur.update(size=f[1], used=f[2], avail=f[3], use_percent=f[4])
        else:
            cur.update(size_bytes=int(f[1]), used_bytes=int(f[2]), avail_bytes=int(f[3]))
    elif sec in ('releases', 'snapshots', 'varlib') and len(f) == 2:
        key = {'releases': 'releases', 'snapshots': 'snapshots', 'varlib': 'var_lib_postfiat'}[sec]
        root = {'releases': '/opt/postfiat/releases', 'snapshots': '/var/lib/postfiat/pre-rollout-snapshots', 'varlib': None}[sec]
        if root and f[1] == root:
            cur[f'{key}_total'] = f[0]
        else:
            cur[f'{key}_top'].append(dict(size=f[0], path=f[1]))
def section(name):
    return (C / name).read_text().splitlines()
v1 = dict(
    note='Read-only du of validator-1. Explains the 96% root use: /var/log/postfiat/validator-1 13G, /var/backups/postfiat 10G, pre-rollout-snapshots 9.0G (33 finalized-checkpoint exports, 36 staged candidate binaries, three 460M .a666-checkpoint-verifier-experimental.incoming* entries), gate931 5.7G, gate926 4.3G, /opt/postfiat/releases 4.1G.',
    root_top=[l for l in section('logs/validator-1-detail.stdout') if l.startswith(('68G', '52G', '4.2G\t/opt', '3.3G'))],
    var_top=section('logs/validator-1-var.stdout'),
    log_and_backups=section('logs/validator-1-logs-backups.stdout'),
    snapshot_entries=sum(1 for l in section('logs/validator-1-detail.stdout') if '/pre-rollout-snapshots/' in l),
)
out = dict(
    schema='postfiat.release-repair.fleet-disk.v1',
    collected_at=(C / 'logs/fleet-disk.collected_at').read_text().strip(),
    mode='read-only: df -h /, df -B1 /, du -sh on /opt/postfiat/releases and /var/lib/postfiat/pre-rollout-snapshots (top entries); nothing written, deleted or restarted',
    hosts=hosts, validator_1_detail=v1,
    raw_logs=['canary-backup-1036/logs/fleet-disk.stdout', 'canary-backup-1036/logs/validator-1-detail.stdout',
              'canary-backup-1036/logs/validator-1-var.stdout', 'canary-backup-1036/logs/validator-1-logs-backups.stdout'],
)
print(json.dumps(out, indent=2))
