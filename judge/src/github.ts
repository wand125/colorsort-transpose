import { base64url, validIdentity } from './session';
import type { Bound, Env, Submission } from './types';

const encoder = new TextEncoder();
const API = 'https://api.github.com';

async function api(url: string, init: RequestInit = {}): Promise<Response> {
  const headers = new Headers(init.headers);
  headers.set('User-Agent', 'colorsort-judge');
  if (!headers.has('Accept')) headers.set('Accept', 'application/vnd.github+json');
  headers.set('X-GitHub-Api-Version', '2022-11-28');
  return fetch(url, { ...init, headers, signal: AbortSignal.timeout(10000), redirect: 'error' });
}

function repoURL(env: Env): string {
  if (!/^[A-Za-z\d-]+$/.test(env.REPO_OWNER) || !/^[A-Za-z\d_.-]+$/.test(env.REPO_NAME)) throw new Error('repository configuration');
  return `${API}/repos/${env.REPO_OWNER}/${env.REPO_NAME}`;
}

export async function createAppJWT(appId: string, pem: string, now = Math.floor(Date.now() / 1000)): Promise<string> {
  if (!/^\d+$/.test(appId)) throw new Error('app configuration');
  const normalized = pem.replace(/\\n/g, '\n');
  if (!normalized.includes('-----BEGIN PRIVATE KEY-----')) throw new Error('PKCS8 required');
  const der = Uint8Array.from(atob(normalized.replace(/-----[A-Z ]+-----/g, '').replace(/\s/g, '')), c => c.charCodeAt(0));
  const key = await crypto.subtle.importKey('pkcs8', der, { name: 'RSASSA-PKCS1-v1_5', hash: 'SHA-256' }, false, ['sign']);
  const head = base64url(encoder.encode(JSON.stringify({ alg: 'RS256', typ: 'JWT' })));
  const body = base64url(encoder.encode(JSON.stringify({ iat: now - 60, exp: now + 540, iss: appId })));
  const data = `${head}.${body}`;
  return `${data}.${base64url(new Uint8Array(await crypto.subtle.sign('RSASSA-PKCS1-v1_5', key, encoder.encode(data))))}`;
}

export async function loginUser(env: Env, code: string, redirectUri: string): Promise<{ github_id: number; login: string; avatar: string }> {
  const exchanged = await api('https://github.com/login/oauth/access_token', {
    method: 'POST', headers: { Accept: 'application/json', 'Content-Type': 'application/json' },
    body: JSON.stringify({ client_id: env.GITHUB_CLIENT_ID, client_secret: env.GITHUB_CLIENT_SECRET, code, redirect_uri: redirectUri }),
  });
  if (!exchanged.ok) throw new Error('oauth exchange');
  const token = await exchanged.json<{ access_token?: string }>();
  if (!token.access_token) throw new Error('oauth token');
  const response = await api(`${API}/user`, { headers: { Authorization: `Bearer ${token.access_token}` } });
  if (!response.ok) throw new Error('oauth user');
  const user = await response.json<{ id: number; login: string; avatar_url: string }>();
  if (!validIdentity({ github_id: user.id, login: user.login })) throw new Error('oauth identity');
  return { github_id: user.id, login: user.login, avatar: `https://avatars.githubusercontent.com/u/${user.id}?s=80` };
}

export function parseBounds(text: string): Bound[] {
  const lines = text.trim().split(/\r?\n/).map(l => l.split('\t'));
  const header = lines.shift()!;
  const col = (name: string) => header.indexOf(name);
  if (['N', 'lower', 'upper', 'upper_source'].some(k => col(k) < 0)) throw new Error('bounds header');
  const bounds = lines.map(row => ({ n: Number(row[col('N')]), lower: Number(row[col('lower')]), upper: Number(row[col('upper')]), source: row[col('upper_source')] }));
  if (bounds.length !== 23 || new Set(bounds.map(b => b.n)).size !== 23 || bounds.some(b => !Number.isInteger(b.n) || b.n < 2 || b.n > 24 || !Number.isInteger(b.lower) || !Number.isInteger(b.upper) || b.lower < 1 || b.upper < b.lower)) throw new Error('bounds data');
  return bounds.sort((a, b) => a.n - b.n);
}

export async function fetchBounds(env: Env): Promise<Bound[]> {
  const response = await api(`${repoURL(env)}/contents/bounds.tsv?ref=${encodeURIComponent(env.BRANCH)}`, { headers: { Accept: 'application/vnd.github.raw+json', 'Cache-Control': 'no-cache' }, cache: 'no-store' });
  if (!response.ok) throw new Error('bounds fetch');
  return parseBounds(await response.text());
}

export async function installationToken(env: Env): Promise<string> {
  if (!/^\d+$/.test(env.GITHUB_INSTALLATION_ID)) throw new Error('installation configuration');
  const response = await api(`${API}/app/installations/${env.GITHUB_INSTALLATION_ID}/access_tokens`, {
    method: 'POST', headers: { Authorization: `Bearer ${await createAppJWT(env.GITHUB_APP_ID, env.GITHUB_APP_PRIVATE_KEY)}`, 'Content-Type': 'application/json' },
    body: JSON.stringify({ repositories: [env.REPO_NAME], permissions: { contents: 'write' } }),
  });
  if (!response.ok) throw new Error('installation token');
  const data = await response.json<{ token: string }>();
  if (!data.token) throw new Error('installation token missing');
  return data.token;
}

export function submissionPath(id: string): string {
  if (!/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/.test(id)) throw new Error('submission id');
  return `records/submissions/${id}.json`;
}

export async function writeSubmission(env: Env, submission: Submission, token: string): Promise<string> {
  if (!validIdentity(submission)) throw new Error('submission identity');
  const url = `${repoURL(env)}/contents/${submissionPath(submission.id)}`;
  const { commit_sha: _sha, ...data } = submission;
  const bytes = encoder.encode(JSON.stringify(data, null, 2) + '\n');
  const content = btoa(Array.from(bytes, b => String.fromCharCode(b)).join(''));
  const headers = { Authorization: `Bearer ${token}`, 'Content-Type': 'application/json' };
  const response = await api(url, {
    method: 'PUT', headers,
    body: JSON.stringify({
      branch: env.BRANCH, content,
      message: `Record N=${submission.n} submission (${submission.length} moves)\n\nCo-authored-by: ${submission.login} <${submission.github_id}+${submission.login}@users.noreply.github.com>`,
    }),
  });
  if (response.ok) return (await response.json<{ commit: { sha: string } }>()).commit.sha;
  if (response.status === 409 || response.status === 422) {
    const existing = await api(`${url}?ref=${encodeURIComponent(env.BRANCH)}`, { headers });
    if (existing.ok) {
      const file = await existing.json<{ content: string }>();
      const saved = JSON.parse(new TextDecoder().decode(Uint8Array.from(atob(file.content.replace(/\s/g, '')), c => c.charCodeAt(0))));
      if (Object.entries(data).every(([k, v]) => saved[k] === v)) {
        const commits = await api(`${repoURL(env)}/commits?path=${submissionPath(submission.id)}&sha=${encodeURIComponent(env.BRANCH)}&per_page=1`, { headers });
        if (commits.ok) {
          const history = await commits.json<{ sha: string }[]>();
          if (history[0]?.sha) return history[0].sha;
        }
      }
    }
  }
  throw new Error('submission delivery');
}
