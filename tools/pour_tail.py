"""Re-solve tails with pour; iterate_big-compatible improvement lines.

python3 tools/pour_tail.py N MOVES_OR_FILE WIDTH [PARTS] [--cuts 100,200]
All artifacts, including best_N.txt, go to --out (default beam/pour-runs/tail).
"""
import argparse
import json
import os
import signal
from pathlib import Path
import subprocess
import time
from verify_all import D, replay

ROOT = Path(__file__).resolve().parents[1]


def timed_run(command, timeout):
    """Terminate only the dedicated session created for this invocation."""
    with subprocess.Popen(command, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                          text=True, stdin=subprocess.DEVNULL, start_new_session=True) as child:
        try:
            stdout, stderr = child.communicate(timeout=timeout)
            return stdout, stderr, child.returncode, False
        except subprocess.TimeoutExpired:
            # Keep /usr/bin/time alive long enough to report its child's RSS.
            # Filter by the session's process group; never signal other jobs.
            supervised = '/usr/bin/time' in command
            if supervised:
                rows = subprocess.check_output(['ps', '-axo', 'pid,pgid'], text=True)
                targets = [int(row.split()[0]) for row in rows.splitlines()[1:]
                           if len(row.split()) == 2 and int(row.split()[1]) == child.pid
                           and int(row.split()[0]) != child.pid]
            else:
                targets = []
            try:
                if targets:
                    for pid in targets:
                        try:
                            os.kill(pid, signal.SIGTERM)
                        except ProcessLookupError:
                            pass
                else:
                    os.killpg(child.pid, signal.SIGTERM)
            except ProcessLookupError:
                pass
            try:
                stdout, stderr = child.communicate(timeout=10)
            except subprocess.TimeoutExpired:
                try:
                    os.killpg(child.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
                stdout, stderr = child.communicate()
            return stdout, stderr, child.returncode, True


def read_moves(value):
    path = Path(value)
    try:
        is_file = path.is_file()
    except OSError:
        is_file = False
    if not is_file:
        return value.strip()
    text = path.read_text().strip()
    if text.startswith('['):
        return ''.join(D[a] + D[b] for a, b in json.loads(text))
    lines = text.splitlines()
    if lines[0].startswith('N='):
        return lines[1].strip() if len(lines) > 1 else ''
    return lines[-1].split()[-1]


def state_at(n, moves, t):
    lanes = [[]] + [list(range(1, n + 1)) for _ in range(n)]
    for i in range(0, 2 * t, 2):
        a, b = D.index(moves[i]), D.index(moves[i + 1])
        lanes[b].append(lanes[a].pop())
    return ','.join(''.join(D[c] for c in lane) or '.' for lane in lanes)


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('n', type=int)
    p.add_argument('solution')
    p.add_argument('width', type=int)
    p.add_argument('parts', type=int, nargs='?', default=8)
    p.add_argument('--cuts', help='specific prefix lengths, tried in given order')
    p.add_argument('--threads', type=int, choices=range(1, 5), default=4)
    p.add_argument('--seed', type=int, default=1)
    p.add_argument('--a', type=float, default=.25)
    p.add_argument('--b', type=float, default=.5)
    p.add_argument('--d', type=float, default=1.)
    p.add_argument('--lb', choices=['m', 'hd', 'j'], default='m')
    p.add_argument('--eval-lb', action='store_true')
    p.add_argument('--upper', action='store_true', help='require a strictly shorter suffix')
    p.add_argument('--timeout', type=float, default=600)
    p.add_argument('--max-attempts', type=int, default=0, help='0: iterate until no improvement')
    p.add_argument('--out', type=Path, default=ROOT / 'beam/pour-runs/tail')
    p.add_argument('--bin', type=Path, default=ROOT / 'beam/target-pour/release/pour')
    args = p.parse_args()
    if args.parts < 3 or args.width < 1 or args.timeout <= 0:
        p.error('parts >= 3, width >= 1, timeout > 0 required')
    args.out.mkdir(parents=True, exist_ok=True)
    mv = read_moves(args.solution)
    assert replay(args.n, mv), 'invalid input solution'
    tried = set()
    attempts = 0
    improved = True
    while improved:
        improved = False
        total = len(mv) // 2
        cuts = ([int(x) for x in args.cuts.split(',')] if args.cuts else
                [round(total * (.15 + .7 * (j - 1) / (args.parts - 2)))
                 for j in range(args.parts - 1, 0, -1)])
        for t in cuts:
            if not 0 <= t < total or (total, t) in tried:
                continue
            if args.max_attempts and attempts >= args.max_attempts:
                break
            tried.add((total, t))
            attempts += 1
            command = ['nice', '-n', '10', str(args.bin.resolve()), str(args.n), str(args.width),
                       '--start', state_at(args.n, mv, t), '--threads', str(args.threads),
                       '--seed', str(args.seed), '--a', str(args.a), '--b', str(args.b),
                       '--d', str(args.d), '--lb', args.lb]
            if args.eval_lb:
                command.append('--eval-lb')
            if args.upper:
                command += ['--upper', str(total - t)]
            clock = time.monotonic()
            prefix = args.out / f'n{args.n}_try{attempts:03d}_T{total}_t{t}'
            stdout, stderr, code, timed_out = timed_run(command, args.timeout)
            elapsed = time.monotonic() - clock
            prefix.with_suffix('.stdout').write_text(stdout)
            prefix.with_suffix('.stderr').write_text(stderr)
            record = dict(command=command, seconds=elapsed, timeout=timed_out, returncode=code,
                          n=args.n, total=total, t=t)
            lines = stdout.splitlines()
            if code == 0 and len(lines) >= 2 and lines[0].startswith('N='):
                seq = mv[:2*t] + lines[1].strip()
                assert replay(args.n, seq), 'invalid returned tail'
                length = len(seq) // 2
                assert int(lines[0].split('moves=')[1]) == len(lines[1].strip()) // 2
                record.update(moves=length, verified=True)
                print(f'N={args.n} T={total} t={t} -> {length} {elapsed:.0f}s', flush=True)
                if length < total:
                    mv = seq
                    improved = True
                    with (args.out / f'best_{args.n}.txt').open('a') as f:
                        f.write(f'{length}\t{mv}\n')
                    print(f'★ N={args.n} 改善 {total} -> {length}', flush=True)
            else:
                print(f'N={args.n} T={total} t={t} 見つからず {elapsed:.0f}s' +
                      (' (timeout)' if timed_out else ''), flush=True)
            prefix.with_suffix('.json').write_text(json.dumps(record, ensure_ascii=False, indent=2) + '\n')
            if improved:
                break
    print(f'N={args.n} 最終 {len(mv)//2}', flush=True)


if __name__ == '__main__':
    main()
