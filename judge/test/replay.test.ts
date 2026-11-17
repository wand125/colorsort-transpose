import { env } from 'cloudflare:workers';
import { describe, expect, it } from 'vitest';
import { replay } from '../src/replay';

declare global {
  namespace Cloudflare { interface Env { TEST_CASES: string; TEST_SCHEMA: string; TEST_BOUNDS: string } }
}
describe('Python verify_all.replay parity', () => {
  const cases: { n: number; moves: string; ok: boolean }[] = JSON.parse(env.TEST_CASES);
  it(`matches all best.tsv rows and six corruptions (${cases.length} cases)`, () => {
    for (const c of cases) expect(replay(c.n, c.moves).ok, `N=${c.n}, ${c.moves.slice(0, 16)}`).toBe(c.ok);
  });
  it('replays the original single-ball rules', () => {
    expect(replay(2, '102012').ok).toBe(true);
    expect(replay(2, '10')).toMatchObject({ ok: false, reason: 'unfinished', length: 1 });
  });
  it('enforces input limits and reports the first bad move', () => {
    for (const n of [1, 25, 2.5, NaN]) expect(replay(n, '102012').reason).toBe('n');
    expect(replay(2, '').reason).toBe('length');
    expect(replay(2, '10'.repeat(5001)).reason).toBe('length');
    expect(replay(2, '10'.repeat(5000)).reason).not.toBe('length');
    expect(replay(2, '10212').reason).toBe('odd_length');
    expect(replay(2, '10A0')).toMatchObject({ reason: 'characters', step: 2 });
    for (const moves of ['00', '01', '12', '30']) expect(replay(2, moves)).toMatchObject({ reason: 'illegal_move', step: 1 });
    expect(replay(2, '10201200')).toMatchObject({ reason: 'illegal_move', step: 4 });
  });
});
