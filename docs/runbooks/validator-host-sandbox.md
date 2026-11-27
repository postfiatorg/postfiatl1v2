# Validator host: sandbox drop-ins for the RPC and transport services

The RPC (`rpc-serve`) and transport (`transport-validator-serve`) units in
`systemd/postfiat-rpc.service.example`, `systemd/postfiat-validator-transport.service.example`
and the units `crates/node/src/batch_snapshot.rs` writes share one directive set
(`NoNewPrivileges` ... `AmbientCapabilities=`). `systemd/postfiat-cobalt-shadow.service.example`
goes further. `ops/validator-host/systemd/` has opt-in drop-ins that add the missing
directives to the two validator services without editing their unit files or the
generator. Nothing restarts unless you ask for it.

## What the drop-ins add

| Directive | Effect inside the service | Shown by |
| --- | --- | --- |
| `PrivateDevices=true` | private `/dev` with only pseudo devices (`null`, `zero`, `urandom`, ...) | `/dev/tty1` absent |
| `ProtectClock=true` | system clock calls denied (`adjtimex`, `clock_settime`, ...) | `adjtimex` returns `EPERM` |
| `ProtectHostname=true` | own UTS namespace; hostname cannot change for the host | different `/proc/self/ns/uts` |
| `ProtectProc=invisible` | other users' processes hidden in `/proc` | `/proc/1` absent |
| `ProcSubset=pid` | only `/proc/<pid>` entries; no `/proc/meminfo`, `/proc/sys`, ... | `/proc/meminfo` absent |
| `RestrictNamespaces=true` | cannot create namespaces | `unshare --user true` fails |
| `RestrictAddressFamilies=AF_UNIX AF_INET AF_INET6` | no netlink, packet or other socket families | `AF_NETLINK` socket fails with `EAFNOSUPPORT` |
| `MemoryDenyWriteExecute=true` | no writable and executable memory | `mmap(PROT_WRITE\|PROT_EXEC)` fails with `EPERM` |
| `SystemCallFilter=@system-service` | allow list of syscalls for ordinary services | `ptrace` denied |
| `SystemCallErrorNumber=EPERM` | a denied syscall returns `EPERM` instead of killing the process (`SIGSYS`) | denied `ptrace` returns, does not kill |

On systemd 249 (Ubuntu 22.04) with the release binary `postfiat-node` from
`cobalt-operator-v2-20260824`, `systemd-analyze security` for a unit with the example
directive set reports exposure **5.0**; with the drop-in, **1.8**, for both services
(`scripts/test-validator-host-sandbox` measures it each run).

Considered and left out:

- `RemoveIPC=true`: both services and other PostFiat units run as `postfiat`. On stop it
  removes every System V / POSIX IPC object of that user, including a sibling service's.
- `PrivateUsers=true`: `batch_snapshot.rs` units run `ExecStartPre=+/usr/bin/install -o postfiat ...`
  as root; with a user namespace, file ownership seen by the service changes. Not tested.
- `IPAddressAllow=`/`IPAddressDeny=`, `PrivateNetwork=`: network policy depends on the
  deployment (transport binds the private overlay address, RPC binds loopback).
- `MemoryMax=`, `CPUQuota=`: resource sizing, not sandboxing; size per host.
- `DynamicUser=true`: state directories are owned by `postfiat`.

## Install

Run as root on the validator host. `KIND` is `rpc` or `transport`; `UNIT` is the
service name on the host: `postfiat-rpc.service` / `postfiat-validator-transport.service`
for the example units, `postfiat-validator-N-rpc.service` / `postfiat-validator-N.service`
for the units `batch_snapshot.rs` writes.

```bash
ops/validator-host/systemd/install.sh --dry-run install rpc postfiat-validator-0-rpc.service
ops/validator-host/systemd/install.sh install rpc postfiat-validator-0-rpc.service
ops/validator-host/systemd/install.sh install transport postfiat-validator-0.service
```

`install` writes `/etc/systemd/system/UNIT.d/50-postfiat-sandbox.conf`, runs
`systemctl daemon-reload` and `systemd-analyze verify` on the unit, then reads every
directive back with `systemctl show`. If the unit does not verify or a directive does
not show, it removes the drop-in again and exits 1. The running service keeps its old
sandbox until its next restart; restart it in your normal maintenance window, one
validator at a time:

```bash
systemctl restart postfiat-validator-0-rpc.service   # or pass --restart to install.sh
```

## Check

```bash
systemctl show postfiat-validator-0-rpc.service -p DropInPaths \
  -p PrivateDevices -p ProtectClock -p ProtectHostname -p ProtectProc -p ProcSubset \
  -p RestrictNamespaces -p RestrictAddressFamilies -p MemoryDenyWriteExecute -p SystemCallErrorNumber
systemd-analyze security postfiat-validator-0-rpc.service | tail -1
grep Seccomp_filters "/proc/$(systemctl show -p MainPID --value postfiat-validator-0-rpc.service)/status"
```

`DropInPaths` must list `50-postfiat-sandbox.conf`. After the restart, check the RPC
answers (`status`) and writes its event log, and that transport writes its ready file and
listens on its bind address. A syscall the filter denies returns `EPERM` to the service
instead of killing it; look for unexpected `Operation not permitted` errors in its stderr log.

## Roll back

```bash
ops/validator-host/systemd/install.sh uninstall rpc postfiat-validator-0-rpc.service
systemctl restart postfiat-validator-0-rpc.service
```

## Tests

```bash
scripts/test-validator-host-sandbox            # static part: drop-ins and install.sh with a stub systemctl
sudo SANDBOX_NODE_BIN=/path/to/postfiat-node SANDBOX_NODE_SHA256=<published sha256> \
  scripts/test-validator-host-sandbox          # + live part with throwaway transient units
sudo SANDBOX_NODE_BIN=... SANDBOX_NODE_SHA256=... \
  scripts/test-validator-host-sandbox --negative-controls
```

The live part never touches an installed PostFiat unit: it starts throwaway transient
units (`systemd-run`, `postfiat-sbxtest-*`, user `nobody`) over a throwaway data dir,
installs the drop-in with `install.sh` into `/run/systemd/system` for those units,
compares RPC replies and transport readiness before and after, checks each directive with
a probe (example set plus that directive vs the example set alone), rolls back, and
removes everything. The negative controls run it with a drop-in that blocks `AF_INET`, a
drop-in missing `ProtectProc`, and a wrong binary hash; each must fail. Set
`SANDBOX_NODE_LIBDIR` to run the release binary through another glibc's
`ld-linux-x86-64.so.2` on hosts older than its build (glibc 2.39), and `TMPDIR` if `/tmp`
is mounted `noexec`.
