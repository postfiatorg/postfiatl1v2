# Operator notes from the Solana service checkout

These are historical orders and observations from the private checkout at `a60d93baa`. Their spending permissions, machine state and commands belong to that earlier operation.

- The operator removed `.relaunches` (one line containing 40) and `CONTINUE.md` (the four-line worker resume instruction). Both files are absent on main.
- The working `PROMPT.md` replaced the Task T orders with Task M mainnet activation orders. The untracked `docs/review/task-t-orders.md` was a byte-for-byte copy of the previous prompt. Both texts are preserved here.
- The status file gained the two paragraphs below. They describe the observer snapshot restart and the old deposit and status commands. Main has its own repository-wide status file.
- `worker.log` gained 11,945 lines and lost four. It is a raw operator transcript. Its live working blob ID was `cfa82510e71c74349b90be07261940ec42521ceb`. The original stays in the service checkout. No unit imports these files.

## Addition to WORKER-STATUS.md

The completed full snapshot was 126,818,750,888 bytes. A second incremental for the same base, slot 471325217 (536,095,538 bytes), passed `zstd -t` before startup. At 11:51:35 UTC the tmux supervisor, PID 438, launched `validator-main-custody`, PID 1940. At 11:51:49 its Agave log reported 7,715 slot storages rebuilt.

Run `ops/sol-live/mainnet-go.sh` from this checkout for the idempotent first deposit. Run `python3 ops/sol-live/mainnet-status.py --watch` to follow its proof job and re-read all six accepted receipts.
