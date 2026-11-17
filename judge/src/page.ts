import { replay } from './replay';

export function page(): string {
  return `<!doctype html>
<html lang="ja"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">
<title>Color sort — Online judge</title><link rel="stylesheet" href="/style.css"><script src="/app.js" defer></script></head>
<body><main class="page"><header class="bar"><span class="site">Color sort · Online judge</span><label><span data-i18n="language">言語</span> <select id="language" aria-label="Language"><option value="ja">日本語</option><option value="en">English</option></select></label></header>
<h1 data-i18n="title">色そろえパズルの手順を提出</h1><p class="prose" data-i18n="intro">今の上界より短い手順が新記録になります。同じ手数では記録は入れ替わりません。</p>
<p><a id="public-site" data-i18n="publicSite">パズルの規則・公開サイト</a></p>
<div class="account"><a class="btn primary" id="login" href="/login" data-i18n="login">GitHub でログイン</a><span id="identity" hidden><img id="avatar" width="32" height="32" alt=""><span id="login-name"></span> <button id="logout" type="button" data-i18n="logout">ログアウト</button></span></div>
<form id="submission"><label for="n" data-i18n="colors">色数（下界・上界・差）</label><select id="n" name="n">${Array.from({ length: 23 }, (_, i) => `<option value="${i + 2}">${i + 2}</option>`).join('')}</select>
<label for="moves" data-i18n="moves">手順（1 手 2 文字、移動元 → 移動先）</label><p class="note" id="format" data-i18n="format">レーン番号は 0–9, a–o。base36 小文字、最大 5000 手。前後の空白は取り除きます。</p>
<textarea id="moves" name="moves" rows="8" spellcheck="false" autocapitalize="off" autocomplete="off" aria-describedby="format preview"></textarea>
<output id="preview" role="status" aria-live="polite"></output><button id="submit" type="submit" disabled data-i18n="submit">提出する</button><p id="result" role="status" aria-live="polite"></p></form>
<h2 data-i18n="records">現在の記録</h2><p id="load-error" role="status"></p><div class="table-wrap"><table><thead><tr><th>N</th><th data-i18n="lower">下界</th><th data-i18n="upper">上界</th><th data-i18n="gap">差</th><th data-i18n="holder">記録保持者</th></tr></thead><tbody id="records"></tbody></table></div>
<h2 data-i18n="history">採用された提出の履歴</h2><div class="table-wrap"><table><thead><tr><th data-i18n="date">日時</th><th>N</th><th data-i18n="count">手数</th><th>GitHub</th></tr></thead><tbody id="history"></tbody></table></div>
<footer class="footer" data-i18n="footer">提出は GitHub に公開されます。1 人 1 時間に 20 件、1 日に 100 件まで。</footer></main></body></html>`;
}

function browserMain(replayFn: typeof replay): void {
  type RecordRow = { n: number; lower: number; upper: number; gap: number; holder: string | null };
  type HistoryRow = { n: number; length: number; login: string; created_at: string };
  const messages = {
    ja: {
      language: '言語', title: '色そろえパズルの手順を提出', intro: '今の上界より短い手順が新記録になります。同じ手数では記録は入れ替わりません。',
      publicSite: 'パズルの規則・公開サイト', login: 'GitHub でログイン', logout: 'ログアウト', colors: '色数（下界・上界・差）',
      moves: '手順（1 手 2 文字、移動元 → 移動先）', format: 'レーン番号は 0–9, a–o。base36 小文字、最大 5000 手。前後の空白は取り除きます。',
      submit: '提出する', records: '現在の記録', lower: '下界', upper: '上界', gap: '差', holder: '記録保持者', history: '採用された提出の履歴', date: '日時', count: '手数',
      footer: '提出は GitHub に公開されます。1 人 1 時間に 20 件、1 日に 100 件まで。',
      complete: '完成', unfinished: '未完成', empty: '手順を貼り付けてください。', odd_length: '文字数は偶数にしてください。', length: '手数は 1〜5000 にしてください。', characters: 'base36 小文字以外が含まれています。',
      illegal_move: '不正な手順', n: '色数は 2〜24 にしてください。', repository: 'リポジトリの記録', noHistory: 'まだ提出はありません。',
      unavailable: '読み込みに失敗しました。時間をおいてページを再読み込みしてください。', sending: '送信中…', rate_limit: '提出件数の制限に達しました。時間をおいて再提出してください。', authentication: 'GitHub でログインし直してください。', input: '入力形式を確認してください。', origin: '同じサイトから提出してください。',
    },
    en: {
      language: 'Language', title: 'Submit a color sort solution', intro: 'A solution strictly shorter than the current upper bound becomes a new record. Ties keep the earlier record.',
      publicSite: 'Puzzle rules and public site', login: 'Log in with GitHub', logout: 'Log out', colors: 'Colors (lower / upper / gap)',
      moves: 'Moves (two characters per move, source → destination)', format: 'Lanes: 0–9, a–o. Lowercase base36, at most 5000 moves. Leading and trailing whitespace is removed.',
      submit: 'Submit', records: 'Current records', lower: 'Lower', upper: 'Upper', gap: 'Gap', holder: 'Record holder', history: 'Accepted submission history', date: 'Date', count: 'Moves',
      footer: 'Submissions are published on GitHub. Limit: 20 per hour and 100 per day per person.',
      complete: 'Complete', unfinished: 'Unfinished', empty: 'Paste your moves above.', odd_length: 'The character count must be even.', length: 'Use between 1 and 5000 moves.', characters: 'Use lowercase base36 characters only.',
      illegal_move: 'Illegal move', n: 'Choose 2–24 colors.', repository: 'Repository record', noHistory: 'No submissions yet.',
      unavailable: 'Could not load data. Please reload the page later.', sending: 'Submitting…', rate_limit: 'Submission limit reached. Please try again later.', authentication: 'Please log in with GitHub again.', input: 'Please check the input format.', origin: 'Submit from this site.',
    },
  };
  type Key = keyof typeof messages.en;
  const el = <T = HTMLElement>(id: string) => document.getElementById(id) as T;
  const langSelect = el<HTMLSelectElement>('language');
  let language: 'ja' | 'en' = (navigator.languages?.[0] || navigator.language || '').toLowerCase().startsWith('ja') ? 'ja' : 'en';
  let records: RecordRow[] = [], history: HistoryRow[] = [];
  let loggedIn = false, sending = false, loaded = false, loadError = false;
  let lastResult: { status?: string; length?: number; previous_upper?: number; upper?: number; step?: number | null; reason?: Key; error?: Key } | null = null;
  const t = (key: Key) => messages[language][key];
  const movesInput = el<HTMLTextAreaElement>('moves');
  const nInput = el<HTMLSelectElement>('n');
  const stepText = (step: number) => language === 'ja' ? `（${step} 手目）` : ` (step ${step})`;
  const countText = (n: number) => language === 'ja' ? `${n} 手` : `${n} moves`;
  function preview(): void {
    const moves = movesInput.value.trim(), result = replayFn(Number(nInput.value), moves);
    el('preview').textContent = !moves ? t('empty') : `${countText(Math.floor(result.length))} · ${result.ok ? t('complete') : t(result.reason as Key)}${result.step ? stepText(result.step) : ''}`;
    el<HTMLButtonElement>('submit').disabled = !loggedIn || !result.ok || sending || !loaded;
  }
  function renderResult(): void {
    let text = '';
    if (sending) text = t('sending');
    else if (lastResult) {
      const r = lastResult;
      if (r.error) text = t(r.error) || t('unavailable');
      else if (r.status === 'accepted') text = language === 'ja' ? `新記録（${r.previous_upper} → ${r.length}）。数分でサイトに反映されます。` : `New record (${r.previous_upper} → ${r.length}). The public site will update in a few minutes.`;
      else if (r.status === 'not_shorter') text = language === 'ja' ? `今の上界 ${r.upper} より短くありません。` : `Not shorter than the current upper bound ${r.upper}.`;
      else text = t('illegal_move') + (r.step ? stepText(r.step) : ' · ' + t(r.reason || 'unfinished'));
    }
    el('result').textContent = text;
  }
  function renderTables(): void {
    const selected = nInput.value;
    if (records.length) {
      nInput.replaceChildren(...records.map(r => new Option(`${r.n} — ${r.lower} / ${r.upper} / ${r.gap}`, String(r.n))));
      nInput.value = selected;
    }
    const row = (values: (string | number)[]) => {
      const tr = document.createElement('tr');
      for (const v of values) { const td = document.createElement('td'); td.textContent = String(v); tr.appendChild(td); }
      return tr;
    };
    el('records').replaceChildren(...records.map(r => row([r.n, r.lower, r.upper, r.gap, r.holder ? '@' + r.holder : t('repository')])));
    el('history').replaceChildren(...(history.length ? [...history].reverse().map(h => row([new Date(h.created_at).toLocaleString(language), h.n, h.length, '@' + h.login])) : [row([t('noHistory')])]));
    preview();
  }
  function translate(): void {
    document.documentElement.lang = language;
    document.title = t('title') + ' · Color sort'; langSelect.value = language;
    document.querySelectorAll<HTMLElement>('[data-i18n]').forEach(node => { node.textContent = t(node.dataset.i18n as Key); });
    el('load-error').textContent = loadError ? t('unavailable') : '';
    renderTables(); renderResult();
  }
  async function loadRecords(): Promise<void> {
    const response = await fetch('/api/records');
    if (!response.ok) throw new Error('records');
    const data = await response.json<{ records: RecordRow[]; history: HistoryRow[] }>(); records = data.records; history = data.history; loaded = true; loadError = false;
    el('load-error').textContent = ''; renderTables();
  }
  langSelect.addEventListener('change', () => { language = langSelect.value === 'ja' ? 'ja' : 'en'; translate(); });
  movesInput.addEventListener('input', preview); nInput.addEventListener('change', preview);
  el('logout').addEventListener('click', async () => {
    try { const r = await fetch('/logout', { method: 'POST' }); if (!r.ok) throw new Error('logout'); location.reload(); }
    catch { lastResult = { error: 'unavailable' }; renderResult(); }
  });
  el('submission').addEventListener('submit', async event => {
    event.preventDefault(); if (el<HTMLButtonElement>('submit').disabled) return;
    sending = true; preview(); renderResult();
    try {
      const response = await fetch('/api/submit', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ n: Number(nInput.value), moves: movesInput.value.trim() }) });
      lastResult = await response.json();
      if (!response.ok && !lastResult?.error) lastResult = { error: 'unavailable' };
      try { await loadRecords(); } catch { loadError = true; el('load-error').textContent = t('unavailable'); }
    } catch { lastResult = { error: 'unavailable' }; }
    finally { sending = false; preview(); renderResult(); }
  });
  translate();
  void Promise.all([
    (async () => {
      const response = await fetch('/api/me'); if (!response.ok) throw new Error('me');
      const data = await response.json<{ public_site: string; user: { login: string; avatar: string } | null }>(); const link = el<HTMLAnchorElement>('public-site');
      if (/^https:\/\//.test(data.public_site)) link.href = data.public_site;
      loggedIn = !!data.user; el('login').hidden = loggedIn; el('identity').hidden = !loggedIn;
      if (data.user) { el('login-name').textContent = '@' + data.user.login; el<HTMLImageElement>('avatar').src = data.user.avatar; }
      preview();
    })(), loadRecords(),
  ]).catch(() => { loadError = true; el('load-error').textContent = t('unavailable'); });
}
export function browserScript(): string { return `(${browserMain.toString()})(${replay.toString()});`; }
export const stylesheet = `:root{--paper:#fff;--panel:#f5f6f7;--rule:#dadde1;--ink:#16181b;--muted:#5c6168;--accent:#23407a;--accent-soft:#e6eaf2;color-scheme:light}*{box-sizing:border-box}body{margin:0;background:var(--paper);color:var(--ink);font:17px/1.6 Georgia,"Times New Roman",serif}.page{max-width:860px;margin:auto;padding:0 20px 64px}.bar{display:flex;justify-content:space-between;align-items:center;gap:16px;flex-wrap:wrap;padding:18px 0;border-bottom:1px solid var(--rule);margin-bottom:36px;font-size:15px}.site{font-weight:600}h1{font-size:clamp(26px,4vw,34px);line-height:1.25;font-weight:600}h2{font-size:22px;margin-top:44px}.prose{max-width:68ch}a{color:var(--accent);text-underline-offset:2px}button,.btn,select,textarea{font:inherit;border:1px solid var(--rule);border-radius:4px;background:var(--paper);color:var(--ink)}button,.btn{display:inline-block;cursor:pointer;padding:8px 14px;color:var(--accent);border-color:var(--accent);font-size:15px;text-decoration:none}.primary,button[type=submit]{background:var(--accent);color:var(--paper)}button:disabled{opacity:.5;cursor:default}a:focus-visible,button:focus-visible,select:focus-visible,textarea:focus-visible{outline:2px solid var(--accent);outline-offset:3px}.account{margin:24px 0}#identity{align-items:center;gap:10px}#identity:not([hidden]){display:inline-flex}img{border-radius:50%}form{max-width:68ch;border-top:1px solid var(--rule);padding-top:24px}form>label{display:block;margin:16px 0 8px}select{padding:6px;max-width:100%}textarea{display:block;width:100%;padding:12px;resize:vertical;font:14px/1.5 ui-monospace,monospace;background:var(--panel)}output{display:block;min-height:3em;padding:12px 0}#result{margin-top:16px}.note,.footer{font-size:14px;color:var(--muted)}.table-wrap{overflow-x:auto}table{border-collapse:collapse;font-size:15px;width:100%}th,td{text-align:left;border-bottom:1px solid var(--rule);padding:8px 16px 8px 0;white-space:nowrap}.footer{border-top:1px solid var(--rule);margin-top:56px;padding-top:16px}[hidden]{display:none!important}@media(prefers-color-scheme:dark){:root{--paper:#121314;--panel:#1b1c1e;--rule:#34363a;--ink:#e6e7e9;--muted:#9da1a8;--accent:#9db4e0;--accent-soft:#252c3a;color-scheme:dark}}`;
