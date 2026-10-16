import { tn, t as T, tx } from './05-i18n.js';
import { $, fmtN, h, nf, plural } from './00-core.js';
import { get, post } from './10-api.js';
import { S } from './11-state.js';
import { pref, setPref } from './12-chrome.js';
import { confirmDialog, fail, status, toast } from './13-feedback.js';
import { SEV_LABEL, buildTable, chip, empty, fmtText, fname, loading } from './14-renderers.js';
import { activeChips, filterGroup, pillRow } from './15-filterbar.js';
import { go } from './16-tabs.js';
import { refreshHeader } from './17-project.js';
import { openEditor } from './21-files.js';
import { fixDialog } from './23-fixes.js';
import { IGN_STATE, ignoreDialog, unignore } from './28-ignore.js';
import { unifyDialog } from './30-unify.js';
/* ================================================================ VALIDATION */
export async function viewValidation() {
  const o = S.goOpts; S.goOpts = null;
  if (o?.file !== undefined) setPref('validation.file', o.file);
  if (o?.ignored !== undefined) setPref('validation.ignored', o.ignored);
  const settings = await get('settings');
  const root = h('div');
  const list = h('div'), counter = h('span', { class: 'muted' });
  const files = S.files || (S.files = (await get('files')).files);
  const v = pref('validation', {});
  const SEV_OPTS = [['', T('Errors + warnings')], ['error', T('Errors')], ['warning', T('Warnings')], ['info', T('Minor things')]];
  const CAT_OPTS = [['', T('All kinds')], ['syntax', T('Tags and variables')], ['structure', T('Structure (speaker, blocks)')], ['length', T('Text length')], ['text', T('Text (numbers, English, typos)')]];
  const cat = h('select', { title: T('Kind of finding'), onchange: (ev) => { setPref('validation.category', ev.target.value); load(true); } }, CAT_OPTS.map(([k, l]) => h('option', { value: k }, l)));
  cat.value = v.category || '';
  const fsel = h('select', { title: T('File'), style: { maxWidth: '320px' }, onchange: (ev) => { setPref('validation.file', ev.target.value); load(true); } }, h('option', { value: '' }, T('All files')), files.map((f) => h('option', { value: f.name }, f.name)));
  fsel.value = v.file || '';
  const search = h('input', { type: 'search', class: 'search', id: 'search', placeholder: T('Search in the messages and the translation… (Ctrl+F)'), value: v.q || '', oninput: (ev) => { setPref('validation.q', ev.target.value); clearTimeout(S.valTimer); S.valTimer = setTimeout(() => load(true), 250); } });
  const info = h('label', { class: 'check', title: T('For example a different number of pauses {w} / {p}') }, h('input', { type: 'checkbox', checked: !!settings.validation.show_info, onchange: async (ev) => { try { await post('settings', { validation: { show_info: ev.target.checked } }); load(true); } catch (e) { fail(e); } } }), T('Show minor things too'));
  const findBtn = h('button', { title: T('Findings of the translation check') }, T('Findings'));
  const consBtn = h('button', { title: T('The same English line translated differently') }, T('Different translations'));
  const ignBtn = h('button', { title: T('Findings you let ignore on some lines') }, T('Ignored findings'));
  const lintBtn = h('button', { title: T("Runs the Ren'Py lint of the whole game (needs the Ren'Py SDK)") }, T('Game lint'));
  const consOn = () => !!pref('validation.consistency', false);
  const lintOn = () => !!pref('validation.lint', false);
  const ignOn = () => pref('validation.ignored', 'hide') === 'only';
  const special = () => consOn() || lintOn() || ignOn();
  const paintIgnBtn = () => { const n = S.overview?.validation?.ignored || 0; ignBtn.textContent = T('Ignored findings ({n})', { n }); ignBtn.classList.toggle('on', ignOn()); };
  const paintConsBtn = () => consBtn.classList.toggle('on', consOn());
  const paintLintBtn = () => { lintBtn.classList.toggle('on', lintOn()); findBtn.classList.toggle('on', !special()); };
  const setMode = (m) => { setPref('validation.consistency', m === 'cons'); setPref('validation.lint', m === 'lint'); setPref('validation.ignored', m === 'ign' ? 'only' : 'hide'); paintIgnBtn(); paintConsBtn(); paintLintBtn(); load(true); };
  findBtn.onclick = () => setMode(''); consBtn.onclick = () => setMode('cons'); ignBtn.onclick = () => setMode('ign'); lintBtn.onclick = () => setMode('lint');
  const modeSeg = h('div', { class: 'seg', role: 'group', 'aria-label': T('What to show') }, findBtn, consBtn, ignBtn, lintBtn);
  const undoFix = h('button', { class: 'btn small', style: { display: 'none' } });
  async function loadLast() {
    try {
      const r = await get('fix/last');
      undoFix.style.display = r.last ? '' : 'none';
      if (r.last) { undoFix.textContent = T('↶ Undo: {what}', { what: r.last.label || T('the last change') }); undoFix.title = T('Done {time}. The files are restored from the backup, but only if they have not changed since the fix.', { time: r.last.time.replace('T', ' ') }); }
    } catch { undoFix.style.display = 'none'; }
  }
  undoFix.onclick = async () => {
    if (!await confirmDialog(T('Undo the last file change?'), T('The files changed by the last change (a speaker fix or a text edit) are restored to the state before it, from the backup. If they have changed since, the restore is refused.'), T('Undo the change'))) return;
    try { const r = await post('fix/undo'); toast(T('Undone: {what} ({n} lines).', { what: r.label || T('last change'), n: r.lines })); S.files = null; await refreshHeader(); load(true); } catch (e) { fail(e); }
  };
  const fixAll = h('button', { class: 'btn small', title: T('Safe fixes: speaker / attributes from the original, double spaces, typos. Shows a preview where you tick each line.'), onclick: () => fixDialog({ file: pref('validation.file', '') }, () => load(true)) }, T('Fix…'));
  const pillsBox = h('div', { class: 'frow pills', role: 'group', 'aria-label': T('Severity') }), chipsBox = h('div', { class: 'frow fchips' });
  function resetFilters() {
    for (const k of ['file', 'category', 'q', 'severity']) setPref('validation.' + k, '');
    search.value = ''; fsel.value = ''; cat.value = '';
    load(true);
  }
  function paintFilters() {
    const ignoredMode = special();
    const sc = S.val.sevCounts;
    const showInfo = !!settings.validation.show_info;
    const counts = sc ? { '': sc.error + sc.warning + (showInfo ? sc.info : 0), error: sc.error, warning: sc.warning, info: sc.info } : null;
    pillsBox.replaceChildren(...(ignoredMode ? [] : pillRow(SEV_OPTS, pref('validation.severity', ''), counts, (k) => { setPref('validation.severity', k); load(true); })));
    const active = [];
    const lab = (list, k) => (list.find(([x]) => x === k) || [k, k])[1];
    if (pref('validation.file', '')) active.push([T('File: ') + pref('validation.file', '').split(/[\\/]/).pop(), () => { setPref('validation.file', ''); fsel.value = ''; load(true); }]);
    if (pref('validation.category', '')) active.push([T('Kind: ') + lab(CAT_OPTS, pref('validation.category', '')), () => { setPref('validation.category', ''); cat.value = ''; load(true); }]);
    if (pref('validation.severity', '')) active.push([T('Severity: ') + lab(SEV_OPTS, pref('validation.severity', '')), () => { setPref('validation.severity', ''); load(true); }]);
    if (pref('validation.q', '')) active.push([T('Search: “{q}”', { q: pref('validation.q', '') }), () => { setPref('validation.q', ''); search.value = ''; load(true); }]);
    chipsBox.replaceChildren(...activeChips(active, resetFilters));
  }
  root.append(h('div', { class: 'filterbar' },
      h('div', { class: 'frow' }, filterGroup(T('Show'), modeSeg), h('span', { class: 'spacer' }), undoFix, fixAll),
      h('div', { class: 'frow' }, search, filterGroup(T('File'), fsel), filterGroup(T('Kind'), cat), info),
      h('div', { class: 'frow' }, pillsBox, h('span', { class: 'spacer' }), counter), chipsBox),
    h('p', { class: 'muted' }, tx("One line of the translation = one item with all its findings below each other. The check never rewrites anything by itself; only the button {fix} next to a speaker / attributes error writes the fix from the original into the file after you confirm (with a backup). {error} = Ren'Py would probably crash or the structure of the script is broken. {warning} = suspicious, but it may be intended (for example the translation lacks a tag, the text is much longer). It is always compared with the original, not with fixed rules.", { fix: h('b', null, T('Fix')), error: h('b', null, T('Error')), warning: h('b', null, T('Warning')) })), list);
  S.val.items = [];
  async function loadLint() {
    let st, cfg;
    try { [st, cfg] = await Promise.all([get('lint/status'), get('settings')]); } catch (e) { return list.replaceChildren(h('div', { class: 'notice bad' }, e.message)); }
    paintFilters();
    const sdk = h('input', { type: 'text', value: cfg.lint?.sdk || '', placeholder: T("Ren'Py SDK folder, e.g. C:\\renpy-8.1.1-sdk"), style: { flex: 1, minWidth: '320px' } });
    const run = h('button', { class: 'btn primary', disabled: st.state === 'running', onclick: async () => { try { await post('settings', { lint: { sdk: sdk.value } }); await post('lint/start'); loadLint(); } catch (e) { fail(e); } } }, st.state === 'running' ? T('Running…') : T('Save the path and run lint'));
    const head = h('div', null, h('div', { class: 'notice' }, T("Ren'Py lint checks the whole game (missing images, unknown labels, broken translations) and finds things our checks cannot see. It needs the Ren'Py SDK folder and the source files of the game (game/…) above the translation folder. It can take a minute or longer; you can keep using the app meanwhile.")),
      h('div', { class: 'row', style: { margin: '8px 0 12px' } }, sdk, run));
    let body;
    if (st.state === 'running') { counter.textContent = ''; body = loading(T('Lint is running… {n} s', { n: Math.round(st.elapsed || 0) })); S.lintTimer = setTimeout(() => { if (lintOn() && S.view === 'validation') loadLint(); }, 1500); }
    else if (st.state === 'error') { counter.textContent = ''; body = h('div', { class: 'notice bad' }, st.error); }
    else if (st.state === 'done') {
      counter.textContent = tn(st.issues.length, '{n} finding', '{n} findings', { n: fmtN(st.issues.length) }) + ' · ' + st.seconds + ' s';
      const rows = st.issues.map((i) => h('tr', null, h('td', null, fname(i.file), h('span', { class: 'mono muted' }, T('line {n}', { n: i.line }))), h('td', null, i.message),
        h('td', null, i.rel ? h('button', { class: 'btn small', onclick: () => go('entries', { file: i.rel, line: i.line, status: 'all' }) }, T('Show')) : null)));
      const t = h('table', { class: 'data' }, h('colgroup', null, [320, 700, 110].map((w) => h('col', { style: { width: w + 'px' } }))), h('thead', null, h('tr', null, [T('File : line'), T('Message'), ''].map((x) => h('th', null, x)))), h('tbody', null, rows));
      body = h('div', null, h('div', { class: 'notice ' + (st.ok ? '' : 'warn') }, st.ok ? T('Lint finished without an error.') : T('Lint ended with code {code} – see the output below.', { code: st.exit_code })),
        rows.length ? h('div', { class: 'tablewrap' }, t) : h('div', { class: 'muted' }, T('Lint reports no line of the form file:line.')),
        h('details', { style: { marginTop: '10px' } }, h('summary', null, T('The whole lint output')), h('pre', { class: 'ctxcode', style: { maxHeight: '320px' } }, st.raw || '')));
    } else { counter.textContent = ''; body = h('div', { class: 'muted' }, T('Lint has not been run yet.')); }
    list.replaceChildren(head, body);
  }
  async function loadConsistency() {
    list.replaceChildren(loading());
    try {
      const loose = !!pref('consistency.loose', false), bySp = !!pref('consistency.bySpeaker', false);
      const r = await get('consistency', { loose: loose ? '1' : '', by_speaker: bySp ? '1' : '' });
      const hidden = new Set(pref('consistency.hidden', []));
      const showHidden = !!pref('consistency.showHidden', false);
      const groups = r.groups.filter((g) => showHidden || !hidden.has(g.key));
      counter.textContent = T('{originals} with several translations ({n} lines in total)', { originals: tn(groups.length, '{n} original', '{n} originals', { n: fmtN(groups.length) }), n: fmtN(r.lines) });
      paintFilters();
      const opt = (key, def, label, title) => h('label', { class: 'check', title }, h('input', { type: 'checkbox', checked: !!pref(key, def), onchange: (e) => { setPref(key, e.target.checked); loadConsistency(); } }), ' ' + label);
      const note = h('div', { class: 'notice' },
        h('div', null, tx('The same English line translated differently. {exact} – “Yeah.”, “Yeah!” and “YEAH!!!” are different lines and rightly translated differently, so they are never merged. The same text can also mean something else or have another speaker, so nothing is unified by itself: click {unify} at a variant, choose the lines concerned, and confirm (it can be undone). {keep} hides the group.', { exact: h('b', null, T('The exact text of the original is compared')), unify: h('b', null, T('Unify…')), keep: h('b', null, T('Leave as it is')) })),
        h('div', { class: 'row', style: { marginTop: '6px', gap: '16px' } },
          opt('consistency.bySpeaker', false, T('Distinguish the speaker'), T('Lines of different characters are never merged into one group')),
          opt('consistency.loose', false, T('Looser comparison (words only)'), T('Ignores case and punctuation – also finds “Yeah.” vs “YEAH!!!”, only as a hint, you usually do not want to unify them')),
          opt('consistency.showHidden', false, T('Show hidden ones too ({n})', { n: hidden.size }), '')));
      if (!groups.length) return list.replaceChildren(note, empty(T('Everything is consistent'), T('No repeated original has several different translations.')));
      const t = buildTable({ key: 'consistency', cols: [{ k: 'src', w: 300, label: T('Original') }, { k: 'var', w: 900, label: T('Translation variants') }, { k: 'act', w: 140, label: '', resize: false }] });
      for (const g of groups) {
        const variants = g.variants.map((v) => h('div', { class: 'variant' },
          h('div', { class: 'row' }, h('span', { class: 'chip none' }, `×${v.count}`), h('span', { class: 'txt grow' }, fmtText(v.target)),
            h('button', { class: 'btn small', title: T('You choose lines and change them to this variant'), onclick: () => unifyDialog(g, v, loose, bySp, () => loadConsistency()) }, T('Unify…'))),
          h('div', { class: 'vlines' }, v.lines.slice(0, 6).map((l) => h('a', { href: '#', class: 'mono', title: l.who ? T('speaker: {who}', { who: l.who }) : '', onclick: (ev) => { ev.preventDefault(); go('entries', { file: l.file, line: l.line, status: 'all' }); } }, `${l.file.split(/[\\/]/).pop()}:${l.line}${l.who ? ' (' + l.who.split(' ')[0] + ')' : ''}`)), v.lines.length > 6 ? ` … +${v.lines.length - 6}` : '')));
        t.tbody.append(h('tr', null,
          h('td', { class: 'txt' }, (g.sources && g.sources.length > 1) ? g.sources.map((x) => h('div', null, fmtText(x))) : fmtText(g.source), h('div', { class: 'muted' }, T('{n}× in total', { n: g.total }))),
          h('td', null, ...variants),
          h('td', null, h('button', { class: 'btn small ghost', title: T('This group is fine, hide it from the list'), onclick: () => { const hs = new Set(pref('consistency.hidden', [])); hs.has(g.key) ? hs.delete(g.key) : hs.add(g.key); setPref('consistency.hidden', [...hs]); loadConsistency(); } }, hidden.has(g.key) ? T('Watch again') : T('Leave as it is')))));
      }
      list.replaceChildren(note, h('div', { class: 'tablewrap' }, t.table));
    } catch (e) { list.replaceChildren(h('div', { class: 'notice bad' }, e.message)); }
  }
  async function loadIgnored() {
    list.replaceChildren(loading());
    try {
      const r = await get('ignore/list');
      counter.textContent = tn(r.items.length, '{n} ignored finding', '{n} ignored findings', { n: fmtN(r.items.length) });
      paintFilters();
      if (!r.items.length) return list.replaceChildren(empty(T('You are not ignoring anything'), T('Findings in the Check list have an Ignore button. Ignored ones appear here.')));
      const t = buildTable({ key: 'ignored', cols: [{ k: 'st', w: 130, label: T('State') }, { k: 'loc', w: 280, label: T('File : line') }, { k: 'msg', w: 380, label: T('Finding') }, { k: 'note', w: 260, label: T('Note') }, { k: 'tgt', w: 320, label: T('Translation') }, { k: 'act', w: 170, label: '', resize: false }] });
      for (const it of r.items) {
        const [lab, cls, tip] = IGN_STATE[it.state];
        t.tbody.append(h('tr', null, h('td', { title: tip }, chip(cls, lab)), h('td', null, fname(it.file), h('span', { class: 'mono muted' }, it.line ? T('line {n}', { n: it.line }) : '')),
          h('td', null, it.message || h('span', { class: 'muted' }, it.code)), h('td', null, it.note || h('span', { class: 'muted' }, '—'), it.date ? h('div', { class: 'muted', style: { fontSize: '11.5px' } }, it.date) : null),
          h('td', { class: 'txt' }, fmtText(it.target)), h('td', null,
            it.line ? h('button', { class: 'btn small', onclick: () => go('entries', { file: it.file, line: it.line, status: 'all' }) }, T('Show')) : null, ' ',
            h('button', { class: 'btn small', title: T('The finding is reported again'), onclick: () => unignore([it], () => { loadIgnored(); }) }, T('Stop ignoring')))));
      }
      list.replaceChildren(h('div', { class: 'tablewrap' }, t.table));
    } catch (e) { list.replaceChildren(h('div', { class: 'notice bad' }, e.message)); }
  }
  async function load(reset) {
    paintIgnBtn(); paintConsBtn(); paintLintBtn();
    clearTimeout(S.lintTimer);
    if (lintOn()) return loadLint();
    if (consOn()) return loadConsistency();
    if (pref('validation.ignored', 'hide') === 'only') return loadIgnored();
    if (reset) { S.val.items = []; list.replaceChildren(loading()); loadLast(); }
    try {
      const q = { severity: pref('validation.severity', ''), category: pref('validation.category', ''), file: pref('validation.file', ''), q: pref('validation.q', ''), offset: S.val.items.length, limit: 200, group: 'line' };
      const r = await get('validation', q);
      S.val.items.push(...r.items); S.val.total = r.total; S.val.sevCounts = r.sev_counts;
      const nf = r.findings ?? r.total;
      counter.textContent = T('{lines} with a finding · {findings}', { lines: tn(r.total, '{n} line', '{n} lines', { n: fmtN(r.total) }), findings: tn(nf, '{n} finding', '{n} findings', { n: fmtN(nf) }) }) + (S.val.items.length < r.total ? ' ' + T('(shown {n})', { n: fmtN(S.val.items.length) }) : '');
      paintFilters();
      paintList();
    } catch (e) { list.replaceChildren(h('div', { class: 'notice bad' }, e.message)); }
  }
  function findingRow(g, x) {
    return h('div', { class: 'finding ' + x.severity },
      chip(x.severity, SEV_LABEL[x.severity]), h('span', { class: 'fmsg' }, x.message),
      h('span', { class: 'fact' },
        x.extra?.fixable ? h('button', { class: 'btn small primary', title: x.extra.kind === 'speaker' ? T('Copies the speaker and attributes from the original (“{who}”)', { who: g.src_who }) : T('Shows a preview of the fix'), onclick: () => fixDialog({ items: [{ file: g.file, line: g.line }], kinds: [x.extra.kind || 'speaker'] }, () => load(true)) }, T('Fix')) : null,
        g.line && x.code !== 'duplicate-id' ? h('button', { class: 'btn small ghost', title: T('Hides this finding on this line (can be undone)'), onclick: () => ignoreDialog([{ file: g.file, line: g.line, code: x.code, severity: x.severity, message: x.message }], () => load(true)) }, T('Ignore…')) : null));
  }
  function paintList() {
    if (!S.val.items.length) return list.replaceChildren(empty(T('No findings'), pref('validation.q', '') || pref('validation.file', '') || pref('validation.category', '') || pref('validation.severity', '') ? T('No problem was found for these filters. Try clearing the filters.') : T('The translation has no problem.')));
    const t = buildTable({ key: 'validation2', cols: [{ k: 'sev', w: 110, label: T('Severity') }, { k: 'loc', w: 220, label: T('File : line') }, { k: 'msg', w: 560, label: T('Findings on the line') }, { k: 'src', w: 300, label: T('Original') }, { k: 'tgt', w: 300, label: T('Translation') }, { k: 'act', w: 150, label: '', resize: false }] });
    for (const g of S.val.items) {
      const ignorable = g.issues.filter((x) => g.line && x.code !== 'duplicate-id');
      t.tbody.append(h('tr', { class: g.severity === 'error' ? 'has-error' : '' },
        h('td', null, chip(g.severity, SEV_LABEL[g.severity]), g.issues.length > 1 ? h('div', { class: 'muted', style: { fontSize: '11.5px', marginTop: '3px' } }, tn(g.issues.length, '{n} finding', '{n} findings')) : null),
        h('td', null, fname(g.file), h('span', { class: 'mono muted' }, T('line {n}', { n: g.line }))),
        h('td', null, ...g.issues.map((x) => findingRow(g, x)),
          ignorable.length > 1 ? h('div', { class: 'fall' }, h('button', { class: 'btn small ghost', title: T('Hides all findings of this line at once'), onclick: () => ignoreDialog(ignorable.map((x) => ({ file: g.file, line: g.line, code: x.code, severity: x.severity, message: x.message })), () => load(true)) }, T('Ignore all {n}…', { n: ignorable.length }))) : null),
        h('td', { class: 'txt' }, fmtText(g.source)), h('td', { class: 'txt' }, fmtText(g.target)),
        h('td', null, h('button', { class: 'btn small', title: T('Show in the list of lines (where the text can be edited at once)'), onclick: () => go('entries', { file: g.file, line: g.line, status: 'all' }) }, T('Show')), ' ',
          h('button', { class: 'btn small ghost', title: T('Open in an external editor'), onclick: () => openEditor(g.file, g.line) }, T('Editor')))));
    }
    list.replaceChildren(h('div', { class: 'tablewrap' }, t.table), S.val.items.length < S.val.total ? h('div', { class: 'more' }, h('button', { class: 'btn', onclick: () => load(false) }, T('Load more ({n} of {total} lines)', { n: fmtN(S.val.items.length), total: fmtN(S.val.total) }))) : null);
  }
  root.afterMount = () => load(true);
  return root;
}

