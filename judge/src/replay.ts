export type ReplayResult = {
  ok: boolean;
  length: number;
  step: number | null;
  reason: string | null;
};

// Kept self-contained so the browser executes this exact function too.
export function replay(n: number, moves: string): ReplayResult {
  const length = moves.length / 2;
  const bad = (reason: string, step: number | null = null): ReplayResult =>
    ({ ok: false, length, step, reason });
  if (!Number.isInteger(n) || n < 2 || n > 24) return bad('n');
  if (moves.length % 2) return bad('odd_length');
  if (length < 1 || length > 5000) return bad('length');
  if (!/^[0-9a-z]+$/.test(moves)) {
    return bad('characters', Math.floor(moves.search(/[^0-9a-z]/) / 2) + 1);
  }
  const lanes: number[][] = [[], ...Array.from({ length: n }, () =>
    Array.from({ length: n }, (_, i) => i + 1))];
  for (let i = 0; i < moves.length; i += 2) {
    const a = parseInt(moves[i], 36), b = parseInt(moves[i + 1], 36);
    if (a > n || b > n || a === b || !lanes[a].length || lanes[b].length >= n) {
      return bad('illegal_move', i / 2 + 1);
    }
    lanes[b].push(lanes[a].pop()!);
  }
  if (!lanes.every(l => !l.length || (l.length === n && l.every(c => c === l[0])))) {
    return bad('unfinished');
  }
  return { ok: true, length, step: null, reason: null };
}
