"""Random search of pour weights at small width, confirmation at larger width.

python3 tools/pour_tune.py 13 --trials 12 --width 2000 --confirm-width 10000
Trials are sequential, nice 10, and use at most four threads.
"""
import argparse
import json
from pathlib import Path
import random
import subprocess
import time
from verify_all import replay
from pour_tail import timed_run

ROOT = Path(__file__).resolve().parents[1]


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('n', type=int, nargs='+')
    p.add_argument('--trials', type=int, default=12)
    p.add_argument('--width', type=int, default=2000)
    p.add_argument('--confirm-width', type=int, default=10000)
    p.add_argument('--top', type=int, default=3)
    p.add_argument('--threads', type=int, choices=range(1, 5), default=4)
    p.add_argument('--seed', type=int, default=1)
    p.add_argument('--search-seed', type=int, default=1)
    p.add_argument('--timeout', type=float, default=600)
    p.add_argument('--out', type=Path, default=ROOT / 'beam/pour-runs/tune')
    p.add_argument('--bin', type=Path, default=ROOT / 'beam/target-pour/release/pour')
    args = p.parse_args()
    if min(args.trials, args.width, args.confirm_width, args.top) < 1 or args.timeout <= 0:
        p.error('counts, widths, timeout must be positive')
    args.out.mkdir(parents=True, exist_ok=True)
    rng = random.Random(args.search_seed)
    for n in args.n:
        weights = [(.25, .5, 1.)] + [(rng.uniform(0, 1.5), rng.uniform(0, 1.5),
                                           rng.uniform(0, 2.5)) for _ in range(args.trials - 1)]
        records = []

        def run(stage, trial, w, values):
            a, b, d = values
            command = ['nice', '-n', '10', '/usr/bin/time', '-l', str(args.bin.resolve()),
                       str(n), str(w), '--a', str(a), '--b', str(b), '--d', str(d),
                       '--seed', str(args.seed), '--threads', str(args.threads)]
            clock = time.monotonic()
            out, err, code, timed_out = timed_run(command, args.timeout)
            seconds = time.monotonic() - clock
            prefix = args.out / f'n{n}_{stage}_{trial:03d}'
            prefix.with_suffix('.stdout').write_text(out)
            prefix.with_suffix('.stderr').write_text(err)
            record = dict(n=n, stage=stage, trial=trial, width=w, a=a, b=b, d=d,
                          seed=args.seed, seconds=seconds, timeout=timed_out,
                          returncode=code, command=command)
            lines = out.splitlines()
            if code == 0 and len(lines) >= 2:
                moves = int(lines[0].split('moves=')[1])
                assert len(lines[1]) // 2 == moves and replay(n, lines[1])
                record.update(moves=moves, verified=True)
            for line in err.splitlines():
                if 'maximum resident set size' in line:
                    record['rss_bytes'] = int(line.split()[0])
            records.append(record)
            prefix.with_suffix('.json').write_text(json.dumps(record, indent=2) + '\n')
            print(json.dumps(record), flush=True)
            return record

        candidates = [run('small', i, args.width, values) for i, values in enumerate(weights)]
        candidates = sorted((r for r in candidates if 'moves' in r),
                            key=lambda r: (r['moves'], r['seconds'], r['trial']))[:args.top]
        for rank, record in enumerate(candidates):
            run('confirm', rank, args.confirm_width, (record['a'], record['b'], record['d']))
        (args.out / f'n{n}_summary.json').write_text(json.dumps(records, indent=2) + '\n')


if __name__ == '__main__':
    main()
