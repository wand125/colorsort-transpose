import { env as bindings } from 'cloudflare:workers';
import { createExecutionContext, waitOnExecutionContext } from 'cloudflare:test';
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';
import worker from '../src/index';
import { base64url, cookie, consumeState, createState, readCookie, sameOrigin, SESSION_AGE, SESSION_COOKIE, signSession, STATE_COOKIE, unbase64url, verifySession } from '../src/session';
import { createAppJWT, loginUser, parseBounds, submissionPath, writeSubmission } from '../src/github';
import { deliverPending, recordSubmission } from '../src/database';
import { replay } from '../src/replay';
import type { Env, Submission } from '../src/types';

const secret = 'test-only-session-secret-32-bytes-minimum';
const user = { github_id: 12345, login: 'test-user' };
let pem: string, publicKey: CryptoKey;
let env: Env;
const seq = '102012';
const valid = replay(2, seq);
const bounds = bindings.TEST_BOUNDS;

async function invoke(path: string, init: RequestInit = {}) {
  const ctx = createExecutionContext();
  const response = await worker.fetch(new Request('https://judge.example' + path, init), env, ctx);
  await waitOnExecutionContext(ctx);
  return response;
}
async function submit(moves: string, n = 2, login = user) {
  return invoke('/api/submit', { method: 'POST', headers: {
    Origin: 'https://judge.example', 'Content-Type': 'application/json',
    Cookie: `${SESSION_COOKIE}=${await signSession(login, secret)}`,
  }, body: JSON.stringify({ n, moves }) });
}
function mockBounds(text = bounds) {
  return vi.spyOn(globalThis, 'fetch').mockImplementation(async input => {
    if (String(input).includes('/contents/bounds.tsv?')) return new Response(text);
    throw new Error('No external network in tests');
  });
}
async function stored() { return (await env.DB.prepare('SELECT rowid AS sequence,* FROM submissions ORDER BY rowid').all<Submission>()).results; }

beforeAll(async () => {
  const pair = await crypto.subtle.generateKey({ name: 'RSASSA-PKCS1-v1_5', modulusLength: 2048, publicExponent: new Uint8Array([1, 0, 1]), hash: 'SHA-256' }, true, ['sign', 'verify']);
  publicKey = pair.publicKey;
  pem = '-----BEGIN PRIVATE KEY-----\n' + btoa(String.fromCharCode(...new Uint8Array(await crypto.subtle.exportKey('pkcs8', pair.privateKey)))) + '\n-----END PRIVATE KEY-----';
  const statements = bindings.TEST_SCHEMA.replace(/--[^\n]*/g, '').split(';').filter(s => s.trim());
  await bindings.DB.batch(statements.map(s => bindings.DB.prepare(s)));
});
beforeEach(async () => {
  env = { ...bindings, SESSION_SECRET: secret, GITHUB_APP_ID: '1', GITHUB_INSTALLATION_ID: '2', GITHUB_APP_PRIVATE_KEY: pem, GITHUB_CLIENT_ID: 'test-client', GITHUB_CLIENT_SECRET: 'test-secret' };
  await env.DB.batch(['submissions', 'oauth_states', 'delivery_lock'].map(table => env.DB.prepare(`DELETE FROM ${table}`)));
});
afterEach(() => { vi.restoreAllMocks(); });

describe('sessions and CSRF', () => {
  it('signs only the specified identity and issue time, with secure cookie attributes', async () => {
    const token = await signSession(user, secret, 1000);
    expect(JSON.parse(new TextDecoder().decode(unbase64url(token.split('.')[0])))).toEqual({ ...user, issued_at: 1000 });
    expect(await verifySession(token, secret, 1000)).toEqual({ ...user, issued_at: 1000 });
    const c = cookie(SESSION_COOKIE, token, SESSION_AGE);
    for (const attr of ['HttpOnly', 'Secure', 'SameSite=Lax', 'Path=/', 'Max-Age=604800']) expect(c).toContain(attr);
    expect(readCookie(new Request('https://judge.example', { headers: { Cookie: c } }), SESSION_COOKIE)).toBe(token);
  });
  it('detects tampering, malformed signatures, wrong keys, expiry and future issue times', async () => {
    const token = await signSession(user, secret, 1000);
    const payload = base64url(new TextEncoder().encode(JSON.stringify({ ...user, login: 'attacker', issued_at: 1000 })));
    for (const bad of [null, 'bad', 'a.b.c', payload + '.' + token.split('.')[1], token + '.', token.split('.')[0] + '.@']) expect(await verifySession(bad, secret, 1000)).toBeNull();
    expect(await verifySession(token, secret + 'different', 1000)).toBeNull();
    expect(await verifySession(token, secret, 1000 + SESSION_AGE - 1)).not.toBeNull();
    expect(await verifySession(token, secret, 1000 + SESSION_AGE)).toBeNull();
    expect(await verifySession(token, secret, 999)).toBeNull();
  });
  it('rejects missing, null, foreign and prefix-matching origins', async () => {
    for (const origin of [null, 'null', 'https://evil.example', 'https://judge.example.evil', 'http://judge.example']) {
      const headers: Record<string, string> = origin === null ? {} : { Origin: origin };
      const request = new Request('https://judge.example/api/submit', { method: 'POST', headers });
      expect(sameOrigin(request)).toBe(false);
      expect((await invoke('/api/submit', { method: 'POST', headers })).status).toBe(403);
    }
    expect((await invoke('/api/submit', { method: 'POST', headers: { Origin: 'https://judge.example' } })).status).toBe(401);
  });
  it('logs out only on same-origin POST and expires the session', async () => {
    expect((await invoke('/logout', { method: 'POST' })).status).toBe(403);
    const r = await invoke('/logout', { method: 'POST', headers: { Origin: 'https://judge.example' } });
    expect(r.status).toBe(204); expect(r.headers.get('Set-Cookie')).toContain('Max-Age=0');
  });
});

describe('one-use OAuth state and GitHub login', () => {
  it('compares the cookie, expires after 10 minutes, and consumes exactly once including races', async () => {
    const s = await createState(env.DB, 1000);
    expect(await consumeState(env.DB, s, 'wrong', 1001)).toBe(false);
    expect(await consumeState(env.DB, s, null, 1001)).toBe(false);
    const results = await Promise.all([consumeState(env.DB, s, s, 1599), consumeState(env.DB, s, s, 1599)]);
    expect(results.sort()).toEqual([false, true]);
    const expired = await createState(env.DB, 2000);
    expect(await consumeState(env.DB, expired, expired, 2600)).toBe(false);
  });
  it('does not request OAuth scopes and handles callback without storing the user token or personal details', async () => {
    const login = await invoke('/login');
    expect(login.status).toBe(302);
    const auth = new URL(login.headers.get('Location')!);
    expect(auth.origin).toBe('https://github.com'); expect(auth.searchParams.has('scope')).toBe(false);
    const state = auth.searchParams.get('state')!;
    const stateCookie = `${STATE_COOKIE}=${state}`;
    const mock = vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      if (String(input).endsWith('/login/oauth/access_token')) {
        expect(JSON.parse(String(init!.body))).toMatchObject({ code: 'test-code', client_id: 'test-client' });
        return Response.json({ access_token: 'ephemeral-user-token', refresh_token: 'discard-this' });
      }
      expect(String(input)).toBe('https://api.github.com/user');
      expect(new Headers(init?.headers).get('Authorization')).toBe('Bearer ephemeral-user-token');
      return Response.json({ id: user.github_id, login: user.login, avatar_url: 'https://avatars.githubusercontent.com/u/12345', name: 'Not retained', email: 'not-retained@example.com' });
    });
    const callback = await invoke(`/callback?state=${state}&code=test-code`, { headers: { Cookie: stateCookie } });
    expect(callback.status).toBe(302); expect(mock).toHaveBeenCalledTimes(2);
    const cookies = callback.headers.get('Set-Cookie')!;
    const token = cookies.match(/__Host-session=([^;]+)/)![1];
    expect(await verifySession(token, secret)).toMatchObject(user);
    expect(cookies).not.toContain('ephemeral-user-token'); expect(cookies).not.toContain('Not retained');
    const me = await invoke('/api/me', { headers: { Cookie: `${SESSION_COOKIE}=${token}` } });
    expect(await me.json()).toEqual({ user: { login: user.login, avatar: 'https://avatars.githubusercontent.com/u/12345?s=80' }, public_site: env.PUBLIC_SITE_URL });
    expect((await invoke(`/callback?state=${state}&code=test-code`, { headers: { Cookie: stateCookie } })).status).toBe(400);
  });
  it('rejects a mismatched state without calling GitHub and consumes state even when exchange fails', async () => {
    const state = await createState(env.DB);
    const mock = vi.spyOn(globalThis, 'fetch').mockResolvedValue(Response.json({ error: 'bad_code' }));
    expect((await invoke(`/callback?state=${state}&code=x`)).status).toBe(400); expect(mock).not.toHaveBeenCalled();
    const init = { headers: { Cookie: `${STATE_COOKIE}=${state}` } };
    expect((await invoke(`/callback?state=${state}&code=x`, init)).status).toBe(502);
    expect((await invoke(`/callback?state=${state}&code=x`, init)).status).toBe(400);
  });
});

describe('atomic D1 decisions and rolling rate limits', () => {
  it('accepts strictly shorter moves, counts invalid and equal moves, and takes the smaller repository/pending upper', async () => {
    expect((await recordSubmission(env.DB, user, 2, seq, valid, 5))!.status).toBe('accepted');
    expect((await recordSubmission(env.DB, user, 2, seq, valid, 5))!.status).toBe('not_shorter');
    const invalid = await recordSubmission(env.DB, user, 2, '00', replay(2, '00'), 5);
    expect(invalid!.status).toBe('invalid');
    expect(JSON.parse(invalid!.reason!).upper).toBe(3);
    const smallerRepo = await recordSubmission(env.DB, user, 2, seq, valid, 2);
    expect(smallerRepo!.status).toBe('not_shorter'); expect(JSON.parse(smallerRepo!.reason!).upper).toBe(2);
    expect((await stored()).length).toBe(4);
  });
  it('serializes equal concurrent submissions, permitting exactly 20 of 25 requests', async () => {
    const results = await Promise.all(Array.from({ length: 25 }, () => recordSubmission(env.DB, user, 2, seq, valid, 5)));
    expect(results.filter(r => r?.status === 'accepted')).toHaveLength(1);
    expect(results.filter(r => r?.status === 'not_shorter')).toHaveLength(19);
    expect(results.filter(r => r === null)).toHaveLength(5);
    expect(await recordSubmission(env.DB, { github_id: 99, login: 'other' }, 2, seq, valid, 5)).not.toBeNull();
  });
  it('limits 100 per rolling day and releases entries older than a day', async () => {
    const template = env.DB.prepare(`INSERT INTO submissions VALUES (?, strftime('%Y-%m-%dT%H:%M:%fZ','now','-2 hours'), ?, ?, 2, '00', 1, 'invalid', NULL, NULL)`);
    await env.DB.batch(Array.from({ length: 100 }, () => template.bind(crypto.randomUUID(), user.github_id, user.login)));
    expect(await recordSubmission(env.DB, user, 2, seq, valid, 5)).toBeNull();
    await env.DB.prepare(`UPDATE submissions SET created_at=strftime('%Y-%m-%dT%H:%M:%fZ','now','-25 hours')`).run();
    expect(await recordSubmission(env.DB, user, 2, seq, valid, 5)).not.toBeNull();
  });
});

describe('HTTP submission and records', () => {
  it('normalizes edge whitespace and records equal and invalid solutions', async () => {
    mockBounds();
    expect(await (await submit(' \n102012\t ')).json()).toMatchObject({ status: 'not_shorter', length: 3, upper: 3 });
    expect(await (await submit('00')).json()).toMatchObject({ status: 'invalid', step: 1 });
    expect(await (await submit('10A0')).json()).toMatchObject({ status: 'invalid', step: 2 });
    expect(await (await submit('10'.repeat(5001))).json()).toMatchObject({ status: 'invalid', reason: 'length' });
    expect((await stored()).map(s => s.status)).toEqual(['not_shorter', 'invalid', 'invalid', 'invalid']);
  });
  it('rejects extra fields, malformed input and oversized bodies', async () => {
    const headers = { Origin: 'https://judge.example', 'Content-Type': 'application/json', Cookie: `${SESSION_COOKIE}=${await signSession(user, secret)}` };
    for (const payload of [{ n: 1, moves: seq }, { n: '2', moves: seq }, { n: 2, moves: seq, login: 'forged' }, { n: 2, moves: null }]) expect((await invoke('/api/submit', { method: 'POST', headers, body: JSON.stringify(payload) })).status).toBe(400);
    expect((await invoke('/api/submit', { method: 'POST', headers, body: '{bad' })).status).toBe(400);
    expect((await invoke('/api/submit', { method: 'POST', headers, body: JSON.stringify({ n: 2, moves: '0'.repeat(70000) }) })).status).toBe(400);
  });
  it('exposes pending records and the first holder, but no private identity or moves in public history', async () => {
    const cases: { n: number; moves: string; ok: boolean }[] = JSON.parse(bindings.TEST_CASES);
    const c = cases.find(c => c.n === 7 && c.ok)!;
    const mock = mockBounds(bounds.replace(/(7\t[^\n]*?\t)78\t/, '$180\t'));
    expect(await (await submit(c.moves, 7)).json()).toMatchObject({ status: 'accepted', previous_upper: 80, length: 78 });
    expect(await (await submit(c.moves, 7, { github_id: 99, login: 'second' })).json()).toMatchObject({ status: 'not_shorter', upper: 78 });
    const data = await (await invoke('/api/records')).json<{ records: { n: number; upper: number; holder: string }[]; history: unknown[] }>();
    expect(data.records.find(r => r.n === 7)).toMatchObject({ upper: 78, holder: user.login });
    expect(data.history).toHaveLength(1);
    expect(JSON.stringify(data.history)).not.toContain('github_id'); expect(JSON.stringify(data.history)).not.toContain('moves');
    expect(mock).toHaveBeenCalled();
  });
  it('returns 429 for the next invalid request after 20 recorded attempts', async () => {
    mockBounds();
    for (let i = 0; i < 20; i++) expect((await submit('00')).status).toBe(200);
    expect((await submit('00')).status).toBe(429);
  });
  it('serves same-origin assets with CSP and no-store, and anonymous identity', async () => {
    for (const path of ['/', '/app.js', '/style.css', '/api/me']) {
      const r = await invoke(path); expect(r.status).toBe(200);
      expect(r.headers.get('Cache-Control')).toBe('no-store'); expect(r.headers.get('Content-Security-Policy')).toContain("script-src 'self'");
      if (path === '/') expect(await r.text()).toContain('aria-live="polite"');
    }
    expect(await (await invoke('/api/me')).json()).toMatchObject({ user: null });
  });
});

describe('GitHub App signing and restricted durable delivery', () => {
  it('signs RS256 JWT verifiable by a freshly generated public key', async () => {
    const jwt = await createAppJWT('123', pem, 1000);
    const [head, body, signature] = jwt.split('.');
    expect(JSON.parse(new TextDecoder().decode(unbase64url(head)))).toEqual({ alg: 'RS256', typ: 'JWT' });
    expect(JSON.parse(new TextDecoder().decode(unbase64url(body)))).toEqual({ iss: '123', iat: 940, exp: 1540 });
    expect(await crypto.subtle.verify('RSASSA-PKCS1-v1_5', publicKey, unbase64url(signature), new TextEncoder().encode(head + '.' + body))).toBe(true);
    expect(await crypto.subtle.verify('RSASSA-PKCS1-v1_5', publicKey, unbase64url(signature), new TextEncoder().encode(head + '.' + body + 'x'))).toBe(false);
  });
  it('disallows path traversal and rejects malformed bounds', () => {
    for (const id of ['../bounds.tsv', 'x/y', 'a'.repeat(36)]) expect(() => submissionPath(id)).toThrow();
    expect(() => parseBounds('N\tupper\n2\t3')).toThrow();
    expect(parseBounds(bounds)).toHaveLength(23);
  });
  it('writes only records/submissions JSON with App bot author and coauthor trailer', async () => {
    await recordSubmission(env.DB, user, 2, seq, valid, 5);
    await recordSubmission(env.DB, user, 2, '00', replay(2, '00'), 5);
    const writes: string[] = [];
    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);
      if (url.endsWith('/access_tokens')) {
        expect(JSON.parse(String(init?.body))).toEqual({ repositories: [env.REPO_NAME], permissions: { contents: 'write' } });
        return Response.json({ token: 'installation-token' });
      }
      expect(init?.method).toBe('PUT'); expect(url).toMatch(/\/contents\/records\/submissions\/[a-f0-9-]+\.json$/);
      const body = JSON.parse(String(init?.body)); expect(body.author).toBeUndefined(); expect(body.committer).toBeUndefined(); expect(body.sha).toBeUndefined();
      expect(body.message).toMatch(/\n\nCo-authored-by: test-user <12345\+test-user@users\.noreply\.github\.com>$/);
      const data = JSON.parse(atob(body.content)); writes.push(data.id);
      expect(data).not.toHaveProperty('commit_sha');
      return Response.json({ commit: { sha: 'commit-' + writes.length } }, { status: 201 });
    });
    await Promise.all([deliverPending(env), deliverPending(env)]);
    expect(writes).toHaveLength(2);
    expect((await stored()).map(s => s.commit_sha)).toEqual(['commit-1', 'commit-2']);
  });
  it('retains failed deliveries, retries oldest first and recovers a lost successful response without overwriting', async () => {
    const row = (await recordSubmission(env.DB, user, 2, seq, valid, 5))!;
    const mock = vi.spyOn(globalThis, 'fetch').mockImplementation(async input => {
      if (String(input).endsWith('/access_tokens')) return Response.json({ token: 'test-installation' });
      return new Response('failure', { status: 503 });
    });
    await expect(deliverPending(env)).rejects.toThrow();
    expect((await stored())[0].commit_sha).toBeNull();
    const { commit_sha: _sha, ...saved } = row;
    mock.mockImplementation(async (input, init) => {
      const url = String(input);
      if (url.endsWith('/access_tokens')) return Response.json({ token: 'test-installation' });
      if (init?.method === 'PUT') return new Response('', { status: 422 });
      if (url.includes('/commits?')) return Response.json([{ sha: 'recovered-commit' }]);
      return Response.json({ content: btoa(JSON.stringify({ ...saved, result: 'accepted' })) });
    });
    await deliverPending(env);
    expect((await stored())[0].commit_sha).toBe('recovered-commit');
  });
});
