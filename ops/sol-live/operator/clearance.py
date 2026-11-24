import json, subprocess
from exit_govern import *
hosts="64.176.220.75 95.179.184.122 66.42.48.39 149.28.63.106 95.179.179.206 45.32.110.170".split()
head=fleet()
for i,h in enumerate(hosts):
    doc=dict(validator=f"validator-{i}",profile_hash=EXIT,revision=head["build_git_revision"],parent=parent_of(head),hold_through_open=True,outstanding_signed_rotation_rounds=[])
    subprocess.run(["ssh","-oBatchMode=yes",f"root@{h}","umask 022; f=/etc/postfiat/sol-exit-rotation-clearance.json; cat > $f.tmp && chown root:root $f.tmp && chmod 644 $f.tmp && mv $f.tmp $f"],input=json.dumps(doc,indent=1).encode(),check=True)
rotation_clearance(head); print("CLEARANCE OK at", head["block_height"])
