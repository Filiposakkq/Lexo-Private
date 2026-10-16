import { tn, t as T } from './05-i18n.js';
import { $, fmtN, fmtPct, h } from './00-core.js';
import { get, post } from './10-api.js';
import { S } from './11-state.js';
import { COMPANION, pref, setPref } from './12-chrome.js';
import { dialog, fail, status, toast } from './13-feedback.js';
import { STATUS_LABEL, bar, buildTable, chip, empty, fname, makeSplitter } from './14-renderers.js';
import { filterGroup } from './15-filterbar.js';
import { go, render } from './16-tabs.js';
import { refreshHeader } from './17-project.js';
/* ================================================================ FILES */
const FL_COLS = () => [
  { k: 'pick', w: 38, label: '', resize: false }, { k: 'name', w: 430, label: T('File'), sortable: true },
  { k: 'assign', w: 140, label: T('Assignment'), sortable: true }, { k: 'tp', w: 250, label: T('Translation'), sortable: true },
  { k: 'cp', w: 230, label: T('Proofreading'), sortable: true }, { k: 'todo', w: 110, label: T('Left to translate'), sortable: true }, { k: 'rev', w: 130, label: T('Left to check'), sortable: true },
  { k: 'order', w: 112, label: T('State'), sortable: true }, { k: 'warn', w: 84, label: T('Check'), sortable: true },
];
const assignKey = (a) => (!a ? 1000 : a === 'gen' ? 900 : parseInt(a.slice(3), 10));
function fileView() {
  const f = pref('files', {});
  const q = (f.q || '').toLowerCase();
  let rows = S.files.filter((r) => (!q || r.name.toLowerCase().includes(q)) &&
    (f.assign === undefined || f.assign === '*' || f.assign === r.assign) &&
    ((f.filter || 'all') === 'all' || (f.filter === 'open' && r.todo > 0) || (f.filter === 'rev' && r.todo === 0 && r.corr < r.total) || (f.filter === 'done' && r.corr >= r.total)));
  const sk = f.sortk || 'name', dir = f.sortdir || 1;
  const val = { name: (r) => r.name.toLowerCase(), assign: (r) => assignKey(r.assign), tp: (r) => r.done / r.total, cp: (r) => r.corr / r.total,
    todo: (r) => r.todo, rev: (r) => Math.max(0, r.total - r.corr), order: (r) => r.order, warn: (r) => r.errors * 1000 + r.warnings };
  rows = rows.slice().sort((a, b) => { const x = val[sk](a), y = val[sk](b); return (x < y ? -1 : x > y ? 1 : a.name.localeCompare(b.name, 'cs', { numeric: true })) * dir; });
  return rows;
}
export async function viewFiles() {
  S.files = await get('files').then((r) => r.files);
  S.overview = S.overview || await get('overview');
  const f = pref('files', {});
  const root = h('div', { class: 'split cols', style: { '--detail-w': pref('files.detailw', 350) + 'px' } });
  const main = h('div', { class: 'pane' });
  const detail = h('div', { class: 'detail' });
  const counter = h('span', { class: 'muted' });
  const pickBtn = h('button', { class: 'btn', onclick: () => bulkAssignDialog([...S.fl.picked]) }, T('Assign selected…'));
  const search = h('input', { type: 'search', class: 'search', id: 'search', placeholder: T('Search files… (Ctrl+F)'), value: f.q || '',
    oninput: (e) => { setPref('files.q', e.target.value); draw(); } });
  const seg = h('div', { class: 'seg' }, [['all', T('All')], ['open', T('Unfinished')], ['rev', T('To proofread')], ['done', T('Finished')]].map(([k, l]) =>
    h('button', { class: (f.filter || 'all') === k ? 'on' : '', onclick: () => { setPref('files.filter', k); seg.querySelectorAll('button').forEach((b) => b.classList.toggle('on', b === event.currentTarget)); draw(); } }, l)));
  const asel = h('select', { title: T('Filter by assignment'), onchange: (e) => { setPref('files.assign', e.target.value); draw(); } },
    h('option', { value: '*' }, T('Assignment: all')), h('option', { value: '' }, T('Unassigned')), h('option', { value: 'gen' }, T('General')),
    ...[1, 2, 3, 4, 5, 6, 7, 8].map((n) => h('option', { value: 'ep:' + n }, T('Episode {n}', { n }))));
  asel.value = f.assign === undefined ? '*' : f.assign;
  main.append(h('div', { class: 'pane-head' }, h('div', { class: 'toolbar filterbar' }, search, filterGroup(T('State'), seg), filterGroup(T('Assignment'), asel),
    h('button', { class: 'btn', onclick: () => bulkAssignDialog(null) }, T('Suggest assignment…')), pickBtn, h('span', { class: 'spacer' }), counter)));
  const wrap = h('div', { class: 'pane-body' });
  main.append(wrap);
  root.append(main, makeSplitter(root, 'h', { key: 'files.detailw', def: 350, min: 260, maxFrac: 0.6 }), detail);

  function draw() {
    const rows = fileView();
    const sk = pref('files.sortk', 'name'), dir = pref('files.sortdir', 1);
    const cols = FL_COLS().map((c) => c.k === 'pick' ? { ...c, node: h('input', { type: 'checkbox', title: T('Select all shown'), checked: rows.length && rows.every((r) => S.fl.picked.has(r.name)),
      onchange: (e) => { rows.forEach((r) => e.target.checked ? S.fl.picked.add(r.name) : S.fl.picked.delete(r.name)); draw(); } }) } : c);
    const t = buildTable({ key: 'files', cols, sort: { k: sk, dir }, onSort: (k) => { setPref('files.sortdir', k === sk ? -dir : 1); setPref('files.sortk', k); draw(); } });
    const tw = h('div', { class: 'tablewrap' }, t.table);
    if (!rows.length) tw.append(empty(T('No files'), T('Change the filter or the search text.')));
    let d = 0, tot = 0, c = 0, rem = 0, remRev = 0;
    rows.forEach((r, idx) => {
      d += r.done; tot += r.total; c += r.corr; rem += r.todo; remRev += Math.max(0, r.total - r.corr);
      const picked = S.fl.picked.has(r.name);
      const tr = h('tr', { class: (S.fl.sel === r.name ? 'sel ' : '') + (picked ? 'picked' : ''), onclick: () => { S.fl.sel = r.name; draw(); drawDetail(); } },
        h('td', { class: 'center' }, h('input', { type: 'checkbox', checked: picked, 'aria-label': T('Select ') + r.name, onclick: (e) => {
          e.stopPropagation();
          if (e.shiftKey && S.fl.lastPick !== null) { const [a, b] = [S.fl.lastPick, idx].sort((x, y) => x - y); rows.slice(a, b + 1).forEach((x) => S.fl.picked.add(x.name)); }
          else if (e.target.checked) S.fl.picked.add(r.name); else S.fl.picked.delete(r.name);
          S.fl.lastPick = idx; draw();
        } })),
        h('td', null, fname(r.name, { link: true, onclick: (e) => { if (e.detail === 2) go('entries', { file: r.name }); } }), r.proof_stale ? chip('info', `↻ ${r.proof_stale}`) : null),
        h('td', null, h('select', { class: 'asg', value: r.assign, onclick: (e) => e.stopPropagation(), onchange: async (e) => {
          try { await post('assign', { name: r.name, v: e.target.value }); r.assign = e.target.value; toast(T('Assignment: {name} → {value}', { name: r.name.split(/[\\/]/).pop(), value: e.target.selectedOptions[0].text })); refreshHeader(); draw(); drawDetail(); } catch (err) { fail(err); e.target.value = r.assign; }
        } }, h('option', { value: '' }, T('Unassigned')), h('option', { value: 'gen' }, T('General')), [1, 2, 3, 4, 5, 6, 7, 8].map((n) => h('option', { value: 'ep:' + n }, T('Episode {n}', { n }))))),
        h('td', null, numCell(r, 't')), h('td', null, numCell(r, 'c')),
        h('td', { class: 'num' }, fmtN(r.todo)), h('td', { class: 'num' }, fmtN(Math.max(0, r.total - r.corr))), h('td', null, chip(r.status, STATUS_LABEL[r.status])),
        h('td', null, r.errors ? h('span', { class: 'badge', title: T('Errors') }, r.errors) : null, ' ', r.warnings ? h('span', { class: 'badge warn', title: T('Warnings') }, r.warnings) : null));
      t.tbody.append(tr);
    });
    t.tfoot.append(h('tr', null, h('td'), h('td', null, T('Total ({n} shown)', { n: rows.length })), h('td'),
      h('td', { class: 'num' }, `${fmtN(d)} / ${fmtN(tot)}  (${fmtPct(tot ? d / tot * 100 : 0)})`), h('td', { class: 'num' }, `${fmtN(c)} (${fmtPct(tot ? c / tot * 100 : 0)})`), h('td', { class: 'num' }, fmtN(rem)), h('td', { class: 'num' }, fmtN(remRev)), h('td'), h('td')));
    const keep = wrap.scrollTop;
    wrap.replaceChildren(tw); wrap.scrollTop = keep;
    counter.textContent = T('Showing {n} of {total} files', { n: rows.length, total: S.files.length });
    pickBtn.disabled = !S.fl.picked.size;
    pickBtn.textContent = S.fl.picked.size ? T('Assign selected ({n})…', { n: S.fl.picked.size }) : T('Assign selected…');
  }
  function numCell(r, which) {
    const isT = which === 't';
    const val = isT ? r.done : r.corr;
    const inp = h('input', { type: 'text', class: 'ed' + (isT && r.done !== r.auto ? ' ov' : ''), value: String(val), inputmode: 'numeric',
      title: isT ? T('Override the number of translated lines by hand (automatic: {auto}). A number, % or “ano”.', { auto: r.auto }) : T('Number of proofread lines (a number, % or “ano”)'),
      onclick: (e) => e.stopPropagation(), onfocus: (e) => e.target.select(),
      oninput: (e) => { e.target.dataset.dirty = '1'; },
      onkeydown: (e) => { if (e.key === 'Enter') e.target.blur(); if (e.key === 'Escape') { e.target.value = String(val); delete e.target.dataset.dirty; e.target.blur(); } },
      onchange: async (e) => {
        delete e.target.dataset.dirty;
        const c = isT ? String(r.corr) : e.target.value, t = isT ? e.target.value.trim() : (r.done !== r.auto ? String(r.done) : '');
        try { await post('file/save', { name: r.name, c, t: isT && e.target.value.trim() === '' ? '' : t }); toast(T('Saved ✓')); await reloadFiles(); } catch (err) { fail(err); }
      } });
    const pct = r.total ? (val / r.total) * 100 : 0;
    return h('div', { class: 'cell-num' }, inp, bar(pct, isT ? '' : 'alt'), h('span', { class: 'pct' }, pct.toFixed(1).replace('.', ',') + ' %'),
      h('span', { class: 'ico-slot' },
        isT && r.done !== r.auto ? h('button', { class: 'ico', title: T('Restore the automatic value ({auto})', { auto: r.auto }), onclick: async (e) => { e.stopPropagation(); try { await post('file/save', { name: r.name, c: String(r.corr), t: '' }); await reloadFiles(); } catch (err) { fail(err); } } }, '↺') : null,
        !isT && r.corr < r.total ? h('button', { class: 'ico', title: T('Mark the whole file as proofread'), onclick: async (e) => { e.stopPropagation(); try { await post('file/save', { name: r.name, c: 'ano', t: r.done !== r.auto ? String(r.done) : '' }); await reloadFiles(); } catch (err) { fail(err); } } }, '✓') : null));
  }
  async function reloadFiles() { S.files = (await get('files')).files; await refreshHeader(); draw(); drawDetail(); }
  function drawDetail() {
    const r = S.files.find((x) => x.name === S.fl.sel);
    if (!r) return detail.replaceChildren(h('h4', null, T('File detail')), h('p', { class: 'muted' }, T('Select a file in the table. Double-click opens its lines.')));
    const total = r.total;
    detail.replaceChildren(h('h4', null, T('File')), h('div', { class: 'path' }, r.name),
      h('div', { class: 'actions' },
        h('button', { class: 'btn small primary', onclick: () => go('entries', { file: r.name }) }, T('Show lines')),
        h('button', { class: 'btn small', onclick: () => go('entries', { file: r.name, status: 'todo' }) }, T('Only untranslated')),
        r.proof_stale ? h('button', { class: 'btn small', title: T('Lines that changed after proofreading'), onclick: () => go('entries', { file: r.name, status: 'stale' }) }, T('Changed after proofreading ({n})', { n: r.proof_stale })) : null,
        h('button', { class: 'btn small primary', onclick: () => go('entries', { file: r.name, status: 'review', select: true }) }, T('Continue proofreading')),
        h('button', { class: 'btn small', onclick: () => openEditor(r.name, 1) }, T('Open in the editor')),
        h('button', { class: 'btn small', onclick: () => copy(r.name) }, T('Copy the path'))),
      h('dl', { class: 'kv' },
        h('dt', null, T('Assignment')), h('dd', null, r.assign === '' ? T('Unassigned') : r.assign === 'gen' ? T('General') : T('Episode ') + r.assign.slice(3)),
        h('dt', null, T('Lines in total')), h('dd', null, fmtN(total)),
        h('dt', null, T('Translated')), h('dd', null, `${fmtN(r.done)} (${fmtPct(total ? r.done / total * 100 : 0)})`),
        h('dt', null, T('of which approved by hand')), h('dd', null, fmtN(r.approved)),
        h('dt', null, T('Counted automatically')), h('dd', null, fmtN(r.auto)),
        h('dt', null, T('Left to translate')), h('dd', null, fmtN(r.todo)),
        h('dt', null, T('Left to check')), h('dd', null, fmtN(Math.max(0, total - r.corr))),
        h('dt', null, T('Proofread')), h('dd', null, `${fmtN(r.corr)} (${fmtPct(total ? r.corr / total * 100 : 0)})`),
        h('dt', null, T('of which line by line')), h('dd', null, fmtN(r.proof_marked)),
        h('dt', null, T('To fix')), h('dd', null, r.proof_fix ? h('button', { class: 'btn small', onclick: () => go('entries', { file: r.name, status: 'fix' }) }, fmtN(r.proof_fix)) : '0'),
        h('dt', null, T('Errors / warnings')), h('dd', null, `${r.errors} / ${r.warnings}`)),
      r.errors + r.warnings ? h('div', { class: 'actions' }, h('button', { class: 'btn small', onclick: () => go('validation', { file: r.name }) }, T('Show the problems'))) : null);
  }
  root.afterMount = () => {
    if (S.goOpts?.file) { S.fl.sel = S.goOpts.file; }
    draw(); drawDetail();
  };
  return root;
}
async function bulkAssignDialog(names) {
  let data;
  try { data = await post('assign/suggest', { names: names || [] }); } catch (e) { return fail(e); }
  const items = names ? data.items : data.items.filter((i) => !i.current || i.state !== 'keep');
  if (!items.length) return toast(T('Nothing to suggest.'), 'bad');
  const rows = items.map((i) => ({ ...i, value: i.suggested || i.current || '', apply: i.state === 'new' || i.state === 'same' ? i.state === 'new' : false, overwrite: false }));
  S.dlgDirty = true;
  const STATE = { new: [T('New'), 'tr'], same: [T('Unchanged'), 'none'], conflict: [T('Conflict'), 'error'], ambiguous: [T('Ambiguous'), 'warning'], unknown: [T('Unknown'), 'none'], keep: [T('Kept'), 'none'] };
  const CONF = { high: 'vysoká', medium: 'střední', low: 'nízká', none: '—' };
  const summary = h('span');
  const upd = () => { const n = rows.filter((r) => r.apply && r.value !== r.current).length; summary.textContent = tn(n, 'To write: {n} file', 'To write: {n} files'); applyBtn.disabled = !n; };
  const filterSel = h('select', { onchange: () => paint() }, h('option', { value: 'all' }, T('Show all')), h('option', { value: 'change' }, T('Only changes')), h('option', { value: 'problem' }, T('Only conflicts and unclear ones')));
  const tbody = h('tbody');
  const table = h('table', { class: 'data' }, h('colgroup', null, [40, 400, 130, 190, 90, 120, 330].map((w) => h('col', { style: { width: w + 'px' } }))),
    h('thead', null, h('tr', null, ['', T('File'), T('So far'), T('Suggested assignment'), T('Confidence'), T('State'), T('Reason')].map((t, i) => h('th', { class: i === 0 ? 'center' : '' }, t)))), tbody);
  table.style.width = '1300px';
  function paint() {
    tbody.replaceChildren();
    for (const r of rows) {
      const kind = filterSel.value;
      if ((kind === 'change' && r.value === r.current) || (kind === 'problem' && !['conflict', 'ambiguous', 'unknown'].includes(r.state))) continue;
      const needsOw = r.current && r.value !== r.current;
      tbody.append(h('tr', null,
        h('td', { class: 'center' }, h('input', { type: 'checkbox', checked: r.apply, title: needsOw ? T('Overwrites an assignment set by hand – ticking confirms the overwrite') : T('Apply'), onchange: (e) => { r.apply = e.target.checked; r.overwrite = e.target.checked && !!needsOw; upd(); } })),
        h('td', null, fname(r.name)), h('td', null, r.current ? optLabel(data.options, r.current) : h('span', { class: 'muted' }, 'nepřiřazeno')),
        h('td', null, h('select', { value: r.value, onchange: (e) => { r.value = e.target.value; r.apply = r.value !== r.current; r.overwrite = !!(r.current && r.apply); paint(); upd(); } },
          data.options.map((o) => h('option', { value: o.value }, o.label)))),
        h('td', null, CONF[r.confidence]), h('td', null, chip(STATE[r.state][1], STATE[r.state][0]), needsOw ? h('div', { class: 'sugg-state muted' }, T('overwrites a manual value')) : null),
        h('td', null, h('span', { class: 'reason' }, r.reason))));
    }
  }
  const applyBtn = h('button', { class: 'btn primary', onclick: async () => {
    const send = rows.filter((r) => r.apply && r.value !== r.current).map((r) => ({ name: r.name, value: r.value, overwrite: r.overwrite }));
    try {
      const res = await post('assign/apply', { items: send });
      toast(tn(res.applied.length, 'Assigned {n} file', 'Assigned {n} files') + (res.skipped.length ? ', ' + T('skipped {n}', { n: res.skipped.length }) : '') + T('. Undo: Ctrl+Z'), res.skipped.length ? 'bad' : 'ok');
      S.files = null; S.fl.picked.clear(); await refreshHeader(); render(); closeFn();
    } catch (e) { fail(e); }
  } }, T('Write the assignment'));
  let closeFn = () => {};
  paint(); upd();
  closeFn = dialog({ title: names ? tn(rows.length, 'Assignment of {n} selected file', 'Assignment of {n} selected files') : T('Suggested assignment of files to episodes'),
    body: h('div', null, h('p', { class: 'muted' }, T('The suggestion is based on file and folder names (epN, updateN, folder seasonN) and on the assignment of the other files in the same folder. Nothing is written until you confirm. A manually set assignment is overwritten only in the rows you tick yourself.')),
      h('div', { class: 'toolbar' }, filterSel, summary), h('div', { class: 'tablewrap', style: { maxHeight: '55vh' } }, table)),
    buttons: [{ label: T('Cancel') }, { node: applyBtn }] });
}
const optLabel = (opts, v) => (opts.find((o) => o.value === v) || { label: v }).label;
/* Another window of the app on the same project (e.g. the scene or the file list next to the editor). Falls back to a browser window. */
async function openWindow(opts = {}) {
  const q = { view: S.view, ...opts };
  try {
    const r = await post('window/open', q);
    if (!r.ok) throw new Error(r.error || T('The window could not be opened.'));
  } catch (e) {
    const u = new URLSearchParams({ win: '2', ...Object.fromEntries(Object.entries(q).filter(([, v]) => v !== undefined && v !== '')) });
    if (!window.open(`${location.origin}/?${u}`, '_blank')) fail(e);
  }
}
/* Companion window: follows the line open in the main window (polls the shared focus once a second; edits stay safe: an unsaved text is never replaced). */
export function startFollowing() {
  let seen = -1, pending = null;
  const apply = (f) => {
    if (!f || !f.file) return;
    S.lastFocus = f;
    if (S.follow === false || S.view !== 'entries') return;
    pending = null;
    if (typeof f.text !== 'string' && $('#editTarget')?.dataset.dirty === '1') { pending = f; return; }   // keep what is being typed here; retried below
    const cur = S.ent?.items?.[S.ent.sel];
    if (cur && cur.file === f.file && cur.line === f.line && typeof f.text === 'string') { S.compDraft?.(f.text); return; }     // same line, only the typed text changed (a plain re-announcement, e.g. after saving, reloads it)
    const key = f.file + '	' + f.line;
    if (S.followTarget === key && Date.now() - S.followAt < 500) { if (typeof f.text === 'string') S.compDraft?.(f.text); return; }      // the channel and the server both announce a change: draw it once
    if (!S.ent?.api) { pending = f; return; }
    S.followTarget = key; S.followAt = Date.now();
    S.ent.api.follow(f.file, f.line);
  };
  S.followApply = apply;
  if (FOCUS_BC) FOCUS_BC.onmessage = (e) => apply(e.data);                      // instant, when the windows share a browser process
  (async function loop() {                                                      // otherwise the server answers as soon as the focus changes
    for (;;) {
      try { const f = await get('focus', { after: seen, wait: 1 }); if (f.seq !== seen) { seen = f.seq; apply(f); } } catch { await new Promise((r) => setTimeout(r, 1500)); }
    }
  })();
  setInterval(() => { if (pending && $('#editTarget')?.dataset.dirty !== '1') apply(pending); }, 800);
}
const FOCUS_BC = typeof BroadcastChannel !== 'undefined' ? new BroadcastChannel('badik-focus') : null;
export function announceFocus(it) {
  FOCUS_BC?.postMessage({ file: it.file, line: it.line, text: null });
  post('focus', { file: it.file, line: it.line }).catch(() => {});
}
/* What is typed in the main editor goes to the detail window's preview (debounced for the server path, at once through the channel). */
export function announceDraft(d, text) {
  FOCUS_BC?.postMessage({ file: d.file, line: d.line, text });
  clearTimeout(S.draftT);
  S.draftT = setTimeout(() => post('focus', { file: d.file, line: d.line, text }).catch(() => {}), 150);
}
/* The only extra window: the line detail alone (scene, check, proofreading…), following the line chosen in the main window. */
export function openDetailWindow() {
  const cur = S.ent?.items?.[S.ent.sel];
  if (!COMPANION) { S.companionOpen = true; S.ent?.api?.redraw?.(); }       // the next ping corrects it when the window did not open
  openWindow({ view: 'entries', companion: '1', tab: 'scene', status: 'all', ...(cur ? { file: cur.file, line: cur.line } : {}) });
}
export async function openEditor(file, line) {
  try { const r = await post('open-editor', { file, line }); toast(r.message, r.ok ? 'ok' : 'bad', 7000); } catch (e) { fail(e); }
}
export async function copy(text) {
  try { await navigator.clipboard.writeText(text); toast(T('Copied.')); } catch { toast(T('Copying failed.'), 'bad'); }
}

