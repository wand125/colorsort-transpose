#!/usr/bin/env python3
"""Slide permutation-invariant beam windows over a verified full solution.

Build at repository root:
  CARGO_TARGET_DIR=beam/target-wbeam cargo build --manifest-path beam/Cargo.toml --release --bin wbeam --offline -j 2
  python3 tools/wscan.py N MOVES --len 32 --step 8 --width 10000 --jobs 2
Every subprocess inherits nice >= 10, and uses at most two Rayon threads.
"""
import argparse
from concurrent.futures import FIRST_COMPLETED, ThreadPoolExecutor, wait
import json
import os
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
DIGITS = '0123456789abcdefghijklmnopqrstuvwxyz'
sys.path.insert(0, str(ROOT / 'tools'))
from verify_all import replay as verify_solution


def parse_moves(n, text):
    if len(text) % 2:
        raise ValueError('MOVES must have even length')
    moves = [(DIGITS.index(text[i]), DIGITS.index(text[i + 1]))
             for i in range(0, len(text), 2)]
    if any(a > n or b > n or a == b for a, b in moves):
        raise ValueError('invalid lane in MOVES')
    return moves


def move_string(moves):
    return ''.join(DIGITS[a] + DIGITS[b] for a, b in moves)


def states(n, moves):
    lanes = [[]] + [list(range(1, n + 1)) for _ in range(n)]
    out = [tuple(map(tuple, lanes))]
    for t, (a, b) in enumerate(moves):
        if a == b or not (0 <= a <= n and 0 <= b <= n) or not lanes[a] or len(lanes[b]) >= n:
            raise ValueError(f'illegal move at t={t}: {a}->{b}')
        lanes[b].append(lanes[a].pop())
        out.append(tuple(map(tuple, lanes)))
    return out


def lane_string(lanes):
    return ','.join(''.join(DIGITS[c] for c in lane) or '.' for lane in lanes)


def run_window(n, start, target, length, width):
    binary = ROOT / 'beam/target-wbeam/release/wbeam'
    command = [str(binary), str(n), '--from', lane_string(start), '--to', lane_string(target),
               '--max', str(length), '--width', str(width), '--threads', '2']
    proc = subprocess.run(command, capture_output=True, text=True, check=True)
    result = json.loads(proc.stdout)
    if result.get('status') not in ('FOUND', 'NOT_FOUND'):
        raise RuntimeError(f'invalid wbeam status: {result}')
    return result


def replace_window(n, moves, t, length, result):
    replacement = parse_moves(n, result['moves'])
    if len(replacement) != result['length'] or len(replacement) >= length:
        raise RuntimeError('invalid replacement length')
    permutation = result['permutation']
    if any(type(i) is not int for i in permutation) or sorted(permutation) != list(range(n + 1)):
        raise RuntimeError('invalid target-to-endpoint permutation')
    original = states(n, moves)
    lanes = [list(lane) for lane in original[t]]
    for a, b in replacement:
        if not lanes[a] or len(lanes[b]) >= n:
            raise RuntimeError('illegal replacement move')
        lanes[b].append(lanes[a].pop())
    if any(tuple(lanes[permutation[i]]) != lane for i, lane in enumerate(original[t + length])):
        raise RuntimeError('replacement endpoint does not match target permutation')
    candidate = moves[:t] + replacement + [(permutation[a], permutation[b]) for a, b in moves[t + length:]]
    if not verify_solution(n, move_string(candidate)):
        raise RuntimeError('replacement failed tools/verify_all.py replay')
    return candidate


def checked_solution(n, moves):
    text = move_string(moves)
    if not verify_solution(n, text):
        raise RuntimeError('full solution failed replay')
    print(f'moves={len(moves)} replay=OK\n{text}', flush=True)


def scan(n, moves, length, step, width, jobs, from_t):
    if not verify_solution(n, move_string(moves)):
        raise ValueError('MOVES must solve the puzzle')
    todo = set(range(from_t, len(moves) - length + 1, step))
    revision = 0
    snapshot = states(n, moves)
    with ThreadPoolExecutor(max_workers=jobs) as pool:
        pending = {}
        while todo or pending:
            todo = {t for t in todo if t + length <= len(moves)}
            while todo and len(pending) < jobs:
                t = min(todo)
                todo.remove(t)
                print(f'window [{t},{t + length}) B={length - 1} revision={revision}', flush=True)
                future = pool.submit(run_window, n, snapshot[t], snapshot[t + length], length, width)
                pending[future] = (t, revision)
            if not pending:
                break
            done, _ = wait(pending, return_when=FIRST_COMPLETED)
            for future in sorted(done, key=lambda f: pending[f][0]):
                t, version = pending.pop(future)
                result = future.result()
                if version != revision:
                    # Running work is allowed to finish; only its own result is
                    # discarded. Retry this index against the adopted solution.
                    todo.add(t)
                    print(f'window t={t} stale revision={version}; retry revision={revision}', flush=True)
                    continue
                print(f"{result['status']} t={t} h0={result['h0']} depth={result['depth']} "
                      f"truncated={result['truncated']} total={result['seconds']:.3f}s", flush=True)
                if result['status'] == 'FOUND':
                    old = len(moves)
                    moves = replace_window(n, moves, t, length, result)
                    revision += 1
                    snapshot = states(n, moves)
                    print(f'★ 改善 {old} -> {len(moves)}', flush=True)
                    checked_solution(n, moves)
    checked_solution(n, moves)
    return moves


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('n', type=int)
    parser.add_argument('moves')
    parser.add_argument('--len', dest='length', type=int, required=True)
    parser.add_argument('--step', type=int, required=True)
    parser.add_argument('--width', type=int, required=True)
    parser.add_argument('--jobs', type=int, default=1)
    parser.add_argument('--from', dest='from_t', type=int, default=0)
    args = parser.parse_args(argv)
    if not 1 <= args.n <= 24 or min(args.length, args.step, args.width) <= 0 or not 1 <= args.jobs <= 2:
        parser.error('require N in 1..24, positive len/step/width, jobs in 1..2')
    if not 0 <= args.from_t <= len(args.moves) // 2:
        parser.error('--from must be in 0..len(MOVES)')
    # Apply once to the driver; every beam child inherits it, without preexec_fn
    # (which would be unsafe in a multithreaded Python process).
    priority = os.getpriority(os.PRIO_PROCESS, 0)
    if priority < 10:
        os.nice(10 - priority)
    scan(args.n, parse_moves(args.n, args.moves), args.length, args.step,
         args.width, args.jobs, args.from_t)
    return 0


if __name__ == '__main__':
    try:
        sys.exit(main())
    except (ValueError, RuntimeError, OSError, subprocess.CalledProcessError) as exc:
        print(f'wscan: {exc}', file=sys.stderr)
        sys.exit(1)
