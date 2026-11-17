import { installationToken, writeSubmission } from './github';
import type { Env, Submission } from './types';
import type { Identity } from './session';
import type { ReplayResult } from './replay';

export async function recordSubmission(db: D1Database, user: Identity, n: number, moves: string, result: ReplayResult, repoUpper: number): Promise<Submission | null> {
  // Reading the minimum and rate counts inside INSERT prevents concurrent ties
  // claiming two records or requests exceeding the rolling limits.
  return db.prepare(`
    WITH current AS (
      SELECT MIN(?, COALESCE(MIN(length), ?)) AS upper FROM submissions
      WHERE n = ? AND status = 'accepted'
    )
    INSERT INTO submissions(id, created_at, github_id, login, n, moves, length, status, reason, commit_sha)
    SELECT ?, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), ?, ?, ?, ?, ?,
      CASE WHEN ? = 0 THEN 'invalid' WHEN ? < upper THEN 'accepted' ELSE 'not_shorter' END,
      json_set(?, '$.upper', upper), NULL
    FROM current
    WHERE (SELECT COUNT(*) FROM submissions WHERE github_id = ? AND created_at > strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-1 hour')) < 20
      AND (SELECT COUNT(*) FROM submissions WHERE github_id = ? AND created_at > strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-1 day')) < 100
    RETURNING rowid AS sequence, *
  `).bind(repoUpper, repoUpper, n, crypto.randomUUID(), user.github_id, user.login, n, moves,
    Math.floor(result.length), result.ok ? 1 : 0, result.length,
    JSON.stringify({ code: result.reason, step: result.step }), user.github_id, user.github_id).first<Submission>();
}

export async function deliverPending(env: Env): Promise<void> {
  const owner = crypto.randomUUID();
  const claim = await env.DB.prepare(`INSERT INTO delivery_lock(id, owner, expires_at) VALUES (1, ?, unixepoch() + 120)
    ON CONFLICT(id) DO UPDATE SET owner=excluded.owner, expires_at=excluded.expires_at
    WHERE delivery_lock.expires_at <= unixepoch() RETURNING owner`).bind(owner).first();
  if (!claim) return;
  try {
    const pending = await env.DB.prepare('SELECT rowid AS sequence, * FROM submissions WHERE commit_sha IS NULL ORDER BY rowid LIMIT 10').all<Submission>();
    if (!pending.results.length) return;
    const token = await installationToken(env);
    for (const submission of pending.results) {
      const renewed = await env.DB.prepare('UPDATE delivery_lock SET expires_at = unixepoch() + 120 WHERE id=1 AND owner=? RETURNING owner').bind(owner).first();
      if (!renewed) return;
      const sha = await writeSubmission(env, submission, token);
      await env.DB.prepare('UPDATE submissions SET commit_sha=? WHERE id=? AND commit_sha IS NULL').bind(sha, submission.id).run();
    }
  } finally {
    await env.DB.prepare('DELETE FROM delivery_lock WHERE id=1 AND owner=?').bind(owner).run();
  }
}
export async function retryDelivery(env: Env): Promise<void> {
  try { await deliverPending(env); }
  catch { console.error(JSON.stringify({ event: 'submission_delivery_failed', retry: 'next cron' })); }
}
