import { cookie, consumeState, createState, readCookie, sameOrigin, SESSION_AGE, SESSION_COOKIE, signSession, STATE_COOKIE, verifySession } from './session';
import { fetchBounds, loginUser } from './github';
import { recordSubmission, retryDelivery } from './database';
import { replay } from './replay';
import { page, browserScript, stylesheet } from './page';
import type { Env } from './types';

function json(data: unknown, status = 200): Response { return Response.json(data, { status }); }

async function readInput(request: Request): Promise<{ n: number; moves: string }> {
  if (request.headers.get('Content-Type')?.split(';')[0].trim() !== 'application/json') throw new Error('input');
  const reader = request.body?.getReader();
  if (!reader) throw new Error('input');
  const chunks: Uint8Array[] = [];
  let size = 0;
  while (true) {
    const { done, value } = await reader.read();
    if (done) break;
    size += value.byteLength;
    if (size > 65536) { await reader.cancel(); throw new Error('input'); }
    chunks.push(value);
  }
  const bytes = new Uint8Array(size);
  let offset = 0;
  for (const chunk of chunks) { bytes.set(chunk, offset); offset += chunk.length; }
  const body = JSON.parse(new TextDecoder('utf-8', { fatal: true }).decode(bytes));
  if (!body || Array.isArray(body) || Object.keys(body).sort().join(',') !== 'moves,n' || typeof body.n !== 'number' || !Number.isInteger(body.n) || body.n < 2 || body.n > 24 || typeof body.moves !== 'string') throw new Error('input');
  return { n: body.n, moves: body.moves.trim() };
}

async function route(request: Request, env: Env, ctx: ExecutionContext): Promise<Response> {
  const url = new URL(request.url);
  if (request.method === 'POST' && !sameOrigin(request)) return json({ error: 'origin' }, 403);
  if (request.method === 'GET' && url.pathname === '/') return new Response(page(), { headers: { 'Content-Type': 'text/html; charset=utf-8' } });
  if (request.method === 'GET' && url.pathname === '/app.js') return new Response(browserScript(), { headers: { 'Content-Type': 'application/javascript; charset=utf-8' } });
  if (request.method === 'GET' && url.pathname === '/style.css') return new Response(stylesheet, { headers: { 'Content-Type': 'text/css; charset=utf-8' } });
  if (request.method === 'GET' && url.pathname === '/login') {
    const state = await createState(env.DB);
    const auth = new URL('https://github.com/login/oauth/authorize');
    auth.searchParams.set('client_id', env.GITHUB_CLIENT_ID);
    auth.searchParams.set('redirect_uri', `${url.origin}/callback`);
    auth.searchParams.set('state', state);
    return new Response(null, { status: 302, headers: { Location: auth.href, 'Set-Cookie': cookie(STATE_COOKIE, state, 600) } });
  }
  if (request.method === 'GET' && url.pathname === '/callback') {
    const clear = cookie(STATE_COOKIE, '', 0);
    if (!await consumeState(env.DB, url.searchParams.get('state'), readCookie(request, STATE_COOKIE))) return new Response('Invalid or expired OAuth state', { status: 400, headers: { 'Set-Cookie': clear } });
    const code = url.searchParams.get('code');
    if (!code || code.length > 512) return new Response('Missing OAuth code', { status: 400, headers: { 'Set-Cookie': clear } });
    try {
      const user = await loginUser(env, code, `${url.origin}/callback`);
      const headers = new Headers({ Location: '/' });
      headers.append('Set-Cookie', clear);
      headers.append('Set-Cookie', cookie(SESSION_COOKIE, await signSession(user, env.SESSION_SECRET), SESSION_AGE));
      return new Response(null, { status: 302, headers });
    } catch { return new Response('GitHub login failed. Please log in again.', { status: 502, headers: { 'Set-Cookie': clear } }); }
  }
  if (request.method === 'POST' && url.pathname === '/logout') return new Response(null, { status: 204, headers: { 'Set-Cookie': cookie(SESSION_COOKIE, '', 0) } });
  if (request.method === 'GET' && url.pathname === '/api/me') {
    const user = await verifySession(readCookie(request, SESSION_COOKIE), env.SESSION_SECRET);
    return json({ user: user ? { login: user.login, avatar: `https://avatars.githubusercontent.com/u/${user.github_id}?s=80` } : null, public_site: env.PUBLIC_SITE_URL });
  }
  if (request.method === 'GET' && url.pathname === '/api/records') {
    const bounds = await fetchBounds(env);
    const history = await env.DB.withSession('first-primary').prepare(`SELECT id, created_at, login, n, length, commit_sha FROM submissions WHERE status='accepted' ORDER BY rowid`).all<{ id: string; created_at: string; login: string; n: number; length: number; commit_sha: string | null }>();
    const records = bounds.map(b => {
      let upper = b.upper;
      let holder: string | null = b.source.match(/^外部の提出: @([a-z\d-]+)$/i)?.[1] || null;
      for (const h of history.results) if (h.n === b.n && h.length < upper) { upper = h.length; holder = h.login; }
      return { n: b.n, lower: b.lower, upper, gap: upper - b.lower, holder, source: b.source };
    });
    return json({ records, history: history.results });
  }
  if (request.method === 'POST' && url.pathname === '/api/submit') {
    const user = await verifySession(readCookie(request, SESSION_COOKIE), env.SESSION_SECRET);
    if (!user) return json({ error: 'authentication' }, 401);
    let input: { n: number; moves: string };
    try { input = await readInput(request); } catch { return json({ error: 'input' }, 400); }
    const result = replay(input.n, input.moves);
    const bound = (await fetchBounds(env)).find(b => b.n === input.n)!;
    const submission = await recordSubmission(env.DB, user, input.n, input.moves, result, bound.upper);
    if (!submission) return json({ error: 'rate_limit', hourly: 20, daily: 100 }, 429);
    ctx.waitUntil(retryDelivery(env));
    const previous = JSON.parse(submission.reason!).upper;
    return json({ id: submission.id, status: submission.status, length: submission.length, step: result.step,
      reason: result.reason, previous_upper: previous, upper: submission.status === 'accepted' ? submission.length : previous });
  }
  return json({ error: 'not_found' }, 404);
}
export default {
  async fetch(request: Request, env: Env, ctx: ExecutionContext): Promise<Response> {
    let response: Response;
    try { response = await route(request, env, ctx); }
    catch { console.error(JSON.stringify({ event: 'request_failed', path: new URL(request.url).pathname })); response = json({ error: 'unavailable' }, 503); }
    response.headers.set('Cache-Control', 'no-store');
    response.headers.set('X-Content-Type-Options', 'nosniff');
    response.headers.set('Referrer-Policy', 'no-referrer');
    response.headers.set('Content-Security-Policy', "default-src 'none'; script-src 'self'; style-src 'self'; connect-src 'self'; img-src https://avatars.githubusercontent.com; base-uri 'none'; frame-ancestors 'none'; form-action 'self'");
    return response;
  },
  async scheduled(_event: ScheduledController, env: Env): Promise<void> { await retryDelivery(env); },
} satisfies ExportedHandler<Env>;
