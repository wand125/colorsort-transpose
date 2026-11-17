import { cloudflareTest } from '@cloudflare/vitest-plugin';
import { defineConfig } from 'vitest/config';
import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';

// Python runs on the host, not in workerd. Only immutable fixtures enter tests.
const fixtureJSON = execFileSync('python3', ['-c', `
import csv, json, sys
sys.dont_write_bytecode = True
sys.path.insert(0, '../tools')
from verify_all import replay
rows = list(csv.DictReader(open('../solutions/best.tsv'), delimiter='\\t'))
cases = []
for row in rows:
    n, s = int(row['N']), row['moves36'].strip()
    for moves in (s, s[:-2], s+'00', '00'+s, 'zz'+s, s+'0', s[:2]):
        cases.append(dict(n=n, moves=moves, ok=replay(n,moves)))
print(json.dumps(cases))
`], { encoding: 'utf8' });

export default defineConfig({
  plugins: [cloudflareTest({
    wrangler: { configPath: './wrangler.toml' },
    miniflare: { bindings: {
      SESSION_SECRET: 'test-only-session-secret-32-bytes-minimum',
      GITHUB_CLIENT_ID: 'test-client', GITHUB_CLIENT_SECRET: 'test-secret',
      GITHUB_APP_ID: '1', GITHUB_INSTALLATION_ID: '2', GITHUB_APP_PRIVATE_KEY: '',
      TEST_SCHEMA: readFileSync('schema.sql', 'utf8'),
      TEST_BOUNDS: readFileSync('../bounds.tsv', 'utf8'),
      TEST_CASES: fixtureJSON,
    } },
  })],
  test: { maxWorkers: 1, fileParallelism: false, testTimeout: 20000 },
});
