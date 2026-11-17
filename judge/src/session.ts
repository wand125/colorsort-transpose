export const SESSION_AGE = 7 * 24 * 3600;
export const SESSION_COOKIE = '__Host-session';
export const STATE_COOKIE = '__Host-oauth';
export type Identity = { github_id: number; login: string };
export type Session = Identity & { issued_at: number };
const encoder = new TextEncoder();
export function base64url(bytes: Uint8Array): string {
  return btoa(String.fromCharCode(...bytes)).replace(/=/g, '').replace(/\+/g, '-').replace(/\//g, '_');
}
export function unbase64url(s: string): Uint8Array<ArrayBuffer> {
  if (!/^[A-Za-z0-9_-]+$/.test(s)) throw new Error('base64');
  return Uint8Array.from(atob(s.replace(/-/g, '+').replace(/_/g, '/')), c => c.charCodeAt(0));
}
export function validIdentity(value: Identity): boolean {
  return Number.isSafeInteger(value.github_id) && value.github_id > 0 &&
    typeof value.login === 'string' && /^[a-z\d](?:[a-z\d-]{0,37}[a-z\d])?$/i.test(value.login);
}
async function key(secret: string): Promise<CryptoKey> {
  if (!secret || encoder.encode(secret).length < 32) throw new Error('session configuration');
  return crypto.subtle.importKey('raw', encoder.encode(secret), { name: 'HMAC', hash: 'SHA-256' }, false, ['sign', 'verify']);
}
export async function signSession(identity: Identity, secret: string, now = Math.floor(Date.now() / 1000)): Promise<string> {
  if (!validIdentity(identity)) throw new Error('identity');
  const payload = base64url(encoder.encode(JSON.stringify({ github_id: identity.github_id, login: identity.login, issued_at: now })));
  const signature = await crypto.subtle.sign('HMAC', await key(secret), encoder.encode(payload));
  return `${payload}.${base64url(new Uint8Array(signature))}`;
}
export async function verifySession(token: string | null, secret: string, now = Math.floor(Date.now() / 1000)): Promise<Session | null> {
  if (!token || token.length > 1024) return null;
  try {
    const parts = token.split('.');
    if (parts.length !== 2 || !await crypto.subtle.verify('HMAC', await key(secret), unbase64url(parts[1]), encoder.encode(parts[0]))) return null;
    const data: Session = JSON.parse(new TextDecoder().decode(unbase64url(parts[0])));
    if (!data || !validIdentity(data) || !Number.isInteger(data.issued_at) || data.issued_at > now || now - data.issued_at >= SESSION_AGE) return null;
    if (Object.keys(data).sort().join(',') !== 'github_id,issued_at,login') return null;
    return data;
  } catch { return null; }
}
export function cookie(name: string, value: string, maxAge: number): string {
  return `${name}=${value}; Path=/; HttpOnly; Secure; SameSite=Lax; Max-Age=${maxAge}`;
}
export function readCookie(request: Request, name: string): string | null {
  const matches = (request.headers.get('Cookie') || '').split(';').map(v => v.trim()).filter(v => v.startsWith(name + '='));
  return matches.length === 1 ? matches[0].slice(name.length + 1) : null;
}
export function sameOrigin(request: Request): boolean {
  return request.headers.get('Origin') === new URL(request.url).origin;
}
export async function createState(db: D1Database, now = Math.floor(Date.now() / 1000)): Promise<string> {
  const state = base64url(crypto.getRandomValues(new Uint8Array(32)));
  await db.batch([
    db.prepare('DELETE FROM oauth_states WHERE expires_at <= ?').bind(now),
    db.prepare('INSERT INTO oauth_states(state, expires_at) VALUES (?, ?)').bind(state, now + 600),
  ]);
  return state;
}
export async function consumeState(db: D1Database, supplied: string | null, stored: string | null, now = Math.floor(Date.now() / 1000)): Promise<boolean> {
  if (!supplied || !stored || supplied !== stored || !/^[A-Za-z0-9_-]{43}$/.test(supplied)) return false;
  return !!await db.prepare('DELETE FROM oauth_states WHERE state = ? AND expires_at > ? RETURNING state').bind(supplied, now).first();
}
