#!/bin/bash
set -euo pipefail
umask 077
# Install the independent spend safeguard before package/network work.
cat >/etc/systemd/system/yolo-qualification-expire.service <<'UNIT'
[Unit]
Description=Terminate the temporary YOLO qualification parent
[Service]
Type=oneshot
ExecStart=/usr/bin/systemctl poweroff
UNIT
cat >/etc/systemd/system/yolo-qualification-expire.timer <<'UNIT'
[Unit]
Description=YOLO cumulative spend safeguard
[Timer]
OnCalendar=@BUDGET_STOP_AT@
Persistent=true
AccuracySec=1s
[Install]
WantedBy=timers.target
UNIT
systemctl daemon-reload
systemctl enable --now yolo-qualification-expire.timer
dnf install -y aws-nitro-enclaves-cli aws-nitro-enclaves-cli-devel docker python3-pip
usermod -aG ne,docker ec2-user
cat >/etc/nitro_enclaves/allocator.yaml <<'ALLOCATOR'
---
memory_mib: @ENCLAVE_MEMORY_MIB@
cpu_count: @ENCLAVE_CPUS@
ALLOCATOR
systemctl enable --now nitro-enclaves-allocator.service docker
touch /var/lib/yolo-qualification-ready
