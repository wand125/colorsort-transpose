export type Env = Cloudflare.Env & {
  GITHUB_APP_ID: string;
  GITHUB_APP_PRIVATE_KEY: string;
  GITHUB_CLIENT_ID: string;
  GITHUB_CLIENT_SECRET: string;
  GITHUB_INSTALLATION_ID: string;
  SESSION_SECRET: string;
};
export type Submission = {
  id: string; sequence: number; created_at: string; github_id: number; login: string;
  n: number; moves: string; length: number;
  status: 'accepted' | 'not_shorter' | 'invalid';
  reason: string | null; commit_sha: string | null;
};
export type Bound = { n: number; lower: number; upper: number; source: string };
