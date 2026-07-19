# Failed local V2 representative proof

This directory is an archived, incomplete Phase-2 benchmark attempt. It is not
a proof receipt and must never be submitted or counted toward a gate.

- Unit: `pfusdc-tier4-v2-benchmark-proof-20260718.service`
- Started: 2026-07-18 22:06:41 UTC
- Cgroup OOM: 2026-07-18 22:08:51 UTC
- Result: `oom-kill`; main process exit status 9
- `MemoryMax`: 64,424,509,440 bytes (60 GiB)
- `MemoryPeak`: 64,424,509,440 bytes
- `MemorySwapPeak`: 4,602,269,696 bytes
- CPU usage: 2,670,549,733,000 ns
- SP1 execution: 520,023,827 cycles in 13,230 ms
- Receipt: absent

Per the founder directive, this result prohibits another proof attempt on the
122 GiB local host. The next attempt must use a rented high-memory host and
must still run under systemd with `MemoryMax`.
