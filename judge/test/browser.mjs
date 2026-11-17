import { chromium } from 'playwright';
import assert from 'node:assert/strict';
import { readFile, mkdir } from 'node:fs/promises';

const origin = process.env.JUDGE_URL || 'http://127.0.0.1:18787';
const browser = await chromium.launch({
  executablePath: process.env.CHROME_PATH || '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
  headless: true,
});
const bounds = (await readFile('../bounds.tsv', 'utf8')).trim().split('\n').slice(1).map(line => {
  const f = line.split('\t');
  return { n: Number(f[0]), lower: Number(f[1]), upper: Number(f[3]), gap: Number(f[3])-Number(f[1]), holder: null };
});
const best = (await readFile('../solutions/best.tsv', 'utf8')).trim().split('\n').slice(1).map(line => line.split('\t'));
const n7 = best.find(row => row[0] === '7');
await mkdir('test/artifacts', { recursive: true });
const errors = [];
let assertions = 0;
try {
  for (const [locale, colorScheme, width, loggedIn] of [
    ['ja-JP', 'light', 1100, false], ['en-US', 'dark', 390, false],
    ['ja-JP', 'light', 1100, true], ['en-US', 'dark', 390, true],
  ]) {
    const context = await browser.newContext({ locale, colorScheme, viewport: { width, height: 900 } });
    const page = await context.newPage();
    page.on('pageerror', error => errors.push(String(error)));
    let submissions = 0;
    const records = structuredClone(bounds);
    records.find(row => row.n === 7).upper = Number(n7[1]) + 2;
    records.find(row => row.n === 7).gap += 2;
    const history = [];
    await page.route('https://avatars.githubusercontent.com/**', route => route.fulfill({
      contentType: 'image/svg+xml', body: '<svg xmlns="http://www.w3.org/2000/svg" width="32" height="32"><rect width="32" height="32" fill="#23407a"/></svg>',
    }));
    await page.route('**/api/**', async route => {
      const path = new URL(route.request().url()).pathname;
      if (path === '/api/me') return route.fulfill({ json: { user: loggedIn ? { login: 'test-user', avatar: 'https://avatars.githubusercontent.com/u/12345?s=80' } : null, public_site: 'https://wand125.github.io/colorsort-transpose/' } });
      if (path === '/api/records') return route.fulfill({ json: { records, history } });
      if (path === '/api/submit') {
        const payload = route.request().postDataJSON();
        assert.equal(payload.n, 7); assert.equal(payload.moves, n7[4]);
        submissions++;
        if (submissions === 1) {
          const record = records.find(row => row.n === 7);
          record.upper = Number(n7[1]); record.gap = record.upper - record.lower; record.holder = 'test-user';
          history.push({ n: 7, length: Number(n7[1]), login: 'test-user', created_at: '2026-11-17T00:00:00Z' });
          return route.fulfill({ json: { status: 'accepted', previous_upper: Number(n7[1])+2, length: Number(n7[1]) } });
        }
        return route.fulfill({ json: { status: 'not_shorter', upper: Number(n7[1]), length: Number(n7[1]) } });
      }
      return route.abort();
    });
    await page.goto(origin, { waitUntil: 'networkidle' });
    const lang = locale.startsWith('ja') ? 'ja' : 'en';
    assert.equal(await page.locator('html').getAttribute('lang'), lang); assertions++;
    assert.equal(await page.locator('#login').isVisible(), !loggedIn); assertions++;
    await page.waitForFunction(() => document.querySelectorAll('#records tr').length === 23, { timeout: 10000 });
    assert.equal(await page.locator('#records tr').count(), 23, errors.join('\n')); assertions++;
    assert.equal(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), true); assertions++;
    assert.equal(await page.locator('#public-site').getAttribute('href'), 'https://wand125.github.io/colorsort-transpose/'); assertions++;
    await page.selectOption('#n', '7');
    await page.fill('#moves', '00');
    assert.match(await page.locator('#preview').textContent(), lang === 'ja' ? /1 手目/ : /step 1/); assertions++;
    assert.equal(await page.locator('#submit').isDisabled(), true); assertions++;
    await page.fill('#moves', ' \n' + n7[4] + '\n ');
    assert.match(await page.locator('#preview').textContent(), lang === 'ja' ? /完成/ : /Complete/); assertions++;
    assert.equal(await page.locator('#submit').isDisabled(), !loggedIn); assertions++;
    if (loggedIn) {
      await page.click('#submit');
      await page.waitForFunction(() => /新記録|New record/.test(document.querySelector('#result').textContent));
      assert.equal(submissions, 1); assertions++;
      assert.match(await page.locator('#history').textContent(), /test-user/); assertions++;
      assert.match(await page.locator('#records').textContent(), /@test-user/); assertions++;
      await page.click('#submit');
      await page.waitForFunction(() => /短くありません|Not shorter/.test(document.querySelector('#result').textContent));
      assert.equal(submissions, 2); assertions++;
    }
    await page.screenshot({ path: `test/artifacts/${lang}-${colorScheme}-${loggedIn ? 'signed-in' : 'anonymous'}.png`, fullPage: true });
    await page.selectOption('#language', lang === 'ja' ? 'en' : 'ja');
    assert.equal(await page.locator('html').getAttribute('lang'), lang === 'ja' ? 'en' : 'ja'); assertions++;
    await context.close();
  }
  assert.deepEqual(errors, []);
  console.log(`Browser checks passed: ${assertions} assertions, four locale/theme/viewport/login combinations, no JavaScript errors.`);
} finally {
  await browser.close();
}
