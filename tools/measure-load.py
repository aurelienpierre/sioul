#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright © 2026 Aurélien Pierre

"""How much Sioul takes, while it runs: the window's process and every
process under it (Qt WebEngine's zygotes, the sites' renderers, its utility
processes), and, if given, the reminder watcher. Every five seconds: CPU from
/proc/<pid>/stat, memory as PSS (each shared page counted once), USS and RSS
from smaps_rollup, threads, context switches, disk I/O. Linux only.

    tools/measure-load.py <sioul-app pid> [<watcher pid>|0] [seconds] [out prefix]

Writes <out>.csv (one line per process and sample) and <out>.json (the
summary), and prints the summary. The figures in the Install guide
("Resources needed") were made with it.
"""

import json, os, sys, time
ROOT = int(sys.argv[1]); WATCHER = int(sys.argv[2]) if len(sys.argv) > 2 and sys.argv[2] != '0' else None
SECONDS = int(sys.argv[3]) if len(sys.argv) > 3 else 300; STEP = 5
OUT = sys.argv[4] if len(sys.argv) > 4 else 'sioul-load'
HZ = os.sysconf('SC_CLK_TCK'); PAGE = os.sysconf('SC_PAGE_SIZE')

def tree(root):
    children = {}
    for d in os.listdir('/proc'):
        if not d.isdigit(): continue
        try:
            with open(f'/proc/{d}/stat') as f: s = f.read()
            ppid = int(s[s.rfind(')') + 2:].split()[1]); children.setdefault(ppid, []).append(int(d))
        except OSError: pass
    out, todo = [], [root]
    while todo:
        p = todo.pop(); out.append(p); todo.extend(children.get(p, []))
    return out

def kind(pid):
    """"sioul-app", "QtWebEngineProcess renderer"…: Chromium rewrites its
    command line into one string, so the type is looked for in all of it."""
    try:
        line = open(f'/proc/{pid}/cmdline', 'rb').read().replace(b'\0', b' ').decode('utf-8', 'replace')
        name = os.path.basename(line.split(' ', 1)[0]) or '?'
        found = [part.split('=', 1)[1] for part in line.split() if part.startswith(('--type=', '--utility-sub-type='))]
        return ' '.join([name] + [f.split('.')[-1] for f in found])
    except OSError:
        return '?'


def read(pid):
    try:
        with open(f'/proc/{pid}/stat') as f: s = f.read()
        fields = s[s.rfind(')') + 2:].split()
        cpu = int(fields[11]) + int(fields[12]); threads = int(fields[17])
        mem = {}
        with open(f'/proc/{pid}/smaps_rollup') as f:
            for line in f:
                k, _, v = line.partition(':')
                if k in ('Rss', 'Pss', 'Private_Clean', 'Private_Dirty', 'Swap'): mem[k] = int(v.split()[0])
        st = {}
        with open(f'/proc/{pid}/status') as f:
            for line in f:
                k, _, v = line.partition(':')
                if k in ('voluntary_ctxt_switches', 'nonvoluntary_ctxt_switches'): st[k] = int(v)
        io = {}
        try:
            with open(f'/proc/{pid}/io') as f:
                for line in f:
                    k, _, v = line.partition(':')
                    if k in ('read_bytes', 'write_bytes'): io[k] = int(v)
        except OSError: pass
        return dict(cpu=cpu, threads=threads, rss=mem.get('Rss', 0), pss=mem.get('Pss', 0), uss=mem.get('Private_Clean', 0) + mem.get('Private_Dirty', 0), swap=mem.get('Swap', 0), ctx=st.get('voluntary_ctxt_switches', 0) + st.get('nonvoluntary_ctxt_switches', 0), rd=io.get('read_bytes', 0), wr=io.get('write_bytes', 0))
    except OSError: return None

def total_cpu():
    with open('/proc/stat') as f: v = [int(x) for x in f.readline().split()[1:]]
    return sum(v), sum(v) - v[3] - v[4]

rows, first, prev = [], {}, {}
t0 = time.time(); cpu_all0, busy0 = total_cpu(); ncpu = os.cpu_count()
with open(OUT + '.csv', 'w') as csv:
    csv.write('t,pid,kind,cpu_pct_of_one_core,pss_kb,uss_kb,rss_kb,threads,ctx_per_s\n')
    while time.time() - t0 < SECONDS + 0.5:
        now = time.time(); pids = tree(ROOT) + ([WATCHER] if WATCHER else [])
        for pid in pids:
            r = read(pid)
            if not r: continue
            r['kind'] = kind(pid); first.setdefault(pid, (now, r))
            if pid in prev:
                pt, pr = prev[pid]; dt = now - pt
                cpu = (r['cpu'] - pr['cpu']) / HZ / dt * 100; ctx = (r['ctx'] - pr['ctx']) / dt
                csv.write(f"{now - t0:.0f},{pid},{r['kind'].replace(',', ' ')},{cpu:.2f},{r['pss']},{r['uss']},{r['rss']},{r['threads']},{ctx:.1f}\n")
                rows.append(dict(t=now - t0, pid=pid, kind=r['kind'], cpu=cpu, pss=r['pss'], uss=r['uss'], rss=r['rss'], threads=r['threads'], ctx=ctx))
            prev[pid] = (now, r)
        csv.flush(); time.sleep(STEP)
cpu_all1, busy1 = total_cpu(); elapsed = time.time() - t0
# Per process: mean and peak CPU, last memory; and the whole.
summary = {}
for pid, (t1, r1) in prev.items():
    tf, rf = first[pid]; dt = t1 - tf
    mine = [x for x in rows if x['pid'] == pid]
    summary[pid] = dict(kind=r1['kind'], cpu_mean=(r1['cpu'] - rf['cpu']) / HZ / dt * 100 if dt > 0 else 0, cpu_peak=max((x['cpu'] for x in mine), default=0), pss_mb=r1['pss'] / 1024, uss_mb=r1['uss'] / 1024, rss_mb=r1['rss'] / 1024, threads=r1['threads'], ctx_per_s=(r1['ctx'] - rf['ctx']) / dt if dt > 0 else 0, read_mb=(r1['rd'] - rf['rd']) / 1048576, written_mb=(r1['wr'] - rf['wr']) / 1048576)
times = sorted({round(x['t']) for x in rows})
whole = [sum(x['cpu'] for x in rows if round(x['t']) == t and x['pid'] != WATCHER) for t in times]
pss_whole = [sum(x['pss'] for x in rows if round(x['t']) == t and x['pid'] != WATCHER) / 1024 for t in times]
result = dict(seconds=elapsed, cpus=ncpu, system_busy_pct=(busy1 - busy0) / (cpu_all1 - cpu_all0) * 100,
              sioul_cpu_mean_pct_one_core=sum(v['cpu_mean'] for p, v in summary.items() if p != WATCHER),
              sioul_cpu_peak_pct_one_core=max(whole, default=0), sioul_pss_mb_mean=sum(pss_whole) / max(1, len(pss_whole)), sioul_pss_mb_peak=max(pss_whole, default=0),
              processes=len([p for p in summary if p != WATCHER]), loadavg=open('/proc/loadavg').read().split()[:3], per_process=summary)
json.dump(result, open(OUT + '.json', 'w'), indent=1, default=str)
print(json.dumps({k: v for k, v in result.items() if k != 'per_process'}, default=str))
