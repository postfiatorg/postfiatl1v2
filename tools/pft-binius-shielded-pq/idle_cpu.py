"""Pick an allowed Linux CPU using a one-second sample, including its SMT siblings."""
import os
import time
from pathlib import Path


def sample():
    counters = {}
    for line in Path("/proc/stat").read_text().splitlines():
        name, *values = line.split()
        if name.startswith("cpu") and name[3:].isdigit():
            ticks = list(map(int, values[:8]))
            counters[int(name[3:])] = (sum(ticks), ticks[3])
    return counters


def siblings(cpu, observed):
    mask = Path(f"/sys/devices/system/cpu/cpu{cpu}/topology/thread_siblings")
    bits = int(mask.read_text().strip().replace(",", ""), 16)
    return [i for i in observed if bits & (1 << i)]


if __name__ == "__main__":
    before = sample()
    time.sleep(1)
    after = sample()
    busy = {
        cpu: 1 - (idle - before[cpu][1]) / max(1, total - before[cpu][0])
        for cpu, (total, idle) in after.items() if cpu in before
    }
    print(min(os.sched_getaffinity(0), key=lambda cpu: (max(busy[i] for i in siblings(cpu, busy)), busy[cpu], cpu)))
