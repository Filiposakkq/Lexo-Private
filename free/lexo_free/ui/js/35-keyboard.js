import { t, t as T, tx } from './05-i18n.js';
import { $, TABS, h } from './00-core.js';
import { get, post } from './10-api.js';
import { S } from './11-state.js';
import { enterProof, enterQuick, pref, setFocus, setPref } from './12-chrome.js';
import { confirmDialog, dialog, dialogOpen, fail, status, toast } from './13-feedback.js';
import { isTyping } from './14-renderers.js';
import { cycleTheme, go, render, toggleDensity } from './16-tabs.js';
import { pickProject, refreshHeader } from './17-project.js';
import { doRefresh, doUndo } from './18-refresh.js';
import { openDetailWindow, openEditor } from './21-files.js';
import { fixDialog } from './23-fixes.js';
import { replaceDialog } from './25-replace.js';
import { ruleApproveDialog } from './26-approve-rules.js';
import { fmtSize } from './29-snapshots.js';
/* ================================================================ keyboard */
/* ================================================================ keyboard: actions, shortcuts (customizable), command palette */
const foldText = (t) => String(t || '').normalize('NFD').replace(/[\u0300-\u036f]/g, '').toLowerCase();
const goMode = (prefs) => { for (const [k, v] of Object.entries(prefs)) setPref(k, v); go('validation'); };
const VAL_MODES = { 'validation.consistency': false, 'validation.lint': false, 'validation.ignored': 'hide' };
const entriesApi = () => (S.view === 'entries' && S.ent.api ? S.ent.api : null);
const moveSel = (n) => { const api = entriesApi(); if (!api) return; const i = S.ent.sel, last = S.ent.items.length - 1; api.select(Math.max(0, Math.min(last, i < 0 ? 0 : i + n))); };
/* id (no dots: it is a preference key), label, default keys, typing (works while typing in a field), entries (only in Řádky) */
/* Full screen: the desktop window is asked through the app (F11 does not reach it), a browser tab uses the Fullscreen API. */
export async function toggleFullscreen() {
  try { const r = await post('window/fullscreen', {}); if (r.ok) return; } catch { /* use the browser way */ }
  try { if (document.fullscreenElement) await document.exitFullscreen(); else await document.documentElement.requestFullscreen(); } catch (e) { fail(e); }
}
export const ACTIONS = [
  { id: 'focus', get label() { return T('Focus mode (hide the top bar and the tabs)'); }, keys: ['Ctrl+Shift+F'], typing: true, run: () => setFocus(!S.focus) },
  { id: 'quick', get label() { return T('Quick translating (scene + editor, untranslated only)'); }, keys: [], run: () => enterQuick() },
  { id: 'proofmode', get label() { return T('Quick proofreading (scene + editor, only lines to check)'); }, keys: [], run: () => enterProof() },
  { id: 'fullscreen', get label() { return T('Full screen (on / off)'); }, keys: ['F11'], typing: true, run: () => toggleFullscreen() },
  { id: 'palette', get label() { return T('Command palette (search actions and files)'); }, keys: ['Ctrl+K'], typing: true, run: () => openPalette() },
  { id: 'search', get label() { return T('Search in the current section'); }, keys: ['Ctrl+F'], typing: true, run: () => { const el = $('#search'); if (el) { el.focus(); el.select(); } } },
  { id: 'search_slash', get label() { return T('Search in the current section (slash)'); }, keys: ['/'], run: () => $('#search')?.focus() },
  { id: 'refresh', get label() { return T('Refresh files'); }, keys: ['F5', 'Ctrl+R'], typing: true, run: () => doRefresh() },
  { id: 'undo', get label() { return T('Undo the last change (approval, proofreading, glossary)'); }, keys: ['Ctrl+Z'], run: () => doUndo() },
  { id: 'help', get label() { return T('Show shortcuts'); }, keys: ['?'], run: () => helpDialog() },
  ...TABS.map(([k, label], i) => ({ id: 'tab_' + k, get label() { return t('Go to: {name}', { name: t(label) }); }, keys: [`Alt+${i + 1}`], typing: true, run: () => go(k) })),
  { id: 'next', get label() { return T('Lines: next line'); }, keys: ['ArrowDown', 'ArrowRight', 'J'], entries: true, run: () => moveSel(1) },
  { id: 'prev', get label() { return T('Lines: previous line'); }, keys: ['ArrowUp', 'ArrowLeft', 'K'], entries: true, run: () => moveSel(-1) },
  { id: 'pagedown', get label() { return T('Lines: a page down'); }, keys: ['PageDown'], entries: true, run: () => moveSel(15) },
  { id: 'pageup', get label() { return T('Lines: a page up'); }, keys: ['PageUp'], entries: true, run: () => moveSel(-15) },
  { id: 'first', get label() { return T('Lines: first line'); }, keys: ['Home'], entries: true, run: () => entriesApi()?.select(0) },
  { id: 'last', get label() { return T('Lines: last line'); }, keys: ['End'], entries: true, run: () => entriesApi()?.select(S.ent.items.length - 1) },
  { id: 'approve', get label() { return T('Lines: approve / unapprove the line'); }, keys: ['A'], entries: true, run: () => entriesApi()?.toggleApprove() },
  { id: 'approve_all', get label() { return T('Lines: approve all identical… (with a preview)'); }, keys: ['Shift+A'], entries: true, run: () => entriesApi()?.globalApprove() },
  { id: 'proof', get label() { return T('Lines: proofreading – checked (and the next one)'); }, keys: ['P'], entries: true, run: () => entriesApi()?.toggleProof() },
  { id: 'proof_upto', get label() { return T('Lines: mark everything up to this line as checked'); }, keys: ['Shift+P'], entries: true, run: () => entriesApi()?.proofUpto() },
  { id: 'unproof_upto', get label() { return T('Lines: remove the checked mark up to this line'); }, keys: ['Shift+U'], entries: true, run: () => entriesApi()?.unproofUpto() },
  { id: 'flag', get label() { return T('Lines: flag for a fix + note'); }, keys: ['F'], entries: true, run: () => entriesApi()?.flagFix() },
  { id: 'open_editor', get label() { return T('Lines: open the line in an external editor'); }, keys: ['E', 'Enter'], entries: true, run: () => { const it = S.ent.items[S.ent.sel]; if (it) openEditor(it.file, it.line); } },
  { id: 'replace', get label() { return T('Find and replace in the translations…'); }, keys: [], run: () => replaceDialog(async () => { S.files = null; await refreshHeader(); if (S.view === 'entries') S.ent.api?.reload(); else render(); }, S.fl.picked.size ? [...S.fl.picked] : []) },
  { id: 'rule', get label() { return T('Approve by rule…'); }, keys: [], run: () => { if (!S.files) get('files').then((r) => { S.files = r.files; }).finally(() => ruleApproveDialog(async () => { S.files = null; await refreshHeader(); render(); }, [])); else ruleApproveDialog(async () => { S.files = null; await refreshHeader(); render(); }, []); } },
  { id: 'fix', get label() { return T('Fix (speaker, spaces, typos)…'); }, keys: [], run: () => fixDialog({}, async () => { S.files = null; await refreshHeader(); render(); }) },
  { id: 'consistency', get label() { return T('Check: different translations of the same original'); }, keys: [], run: () => goMode({ ...VAL_MODES, 'validation.consistency': true }) },
  { id: 'ignored', get label() { return T('Check: ignored findings'); }, keys: [], run: () => goMode({ ...VAL_MODES, 'validation.ignored': 'only' }) },
  { id: 'lint', get label() { return T("Check: game lint (Ren'Py)"); }, keys: [], run: () => goMode({ ...VAL_MODES, 'validation.lint': true }) },
  { id: 'findings', get label() { return T('Check: findings'); }, keys: [], run: () => goMode({ ...VAL_MODES }) },
  { id: 'snapshot', get label() { return T('Create a backup of the translation files now'); }, keys: [], run: async () => { try { const r = await post('snapshots/create'); toast(T('Backup created ({files} files, {size}).', { files: r.snapshot.files, size: fmtSize(r.snapshot.bytes) })); } catch (e) { fail(e); } } },
  { id: 'theme', get label() { return T('Switch the appearance (automatic / dark / light)'); }, keys: [], run: () => cycleTheme() },
  { id: 'density', get label() { return T('Switch the table density'); }, keys: [], run: () => toggleDensity() },
  { id: 'new_window', get label() { return T('Open a second window with the line detail'); }, keys: [], run: () => openDetailWindow() },
  { id: 'change_project', get label() { return T('Change the translation folder…'); }, keys: [], run: () => pickProject() },
];
export const keysOf = (a) => { const o = pref('keys', {}); return Array.isArray(o[a.id]) ? o[a.id] : a.keys; };
export function comboOf(e) {
  const k = e.key;
  if (!k || ['Control', 'Shift', 'Alt', 'Meta', 'AltGraph', 'CapsLock', 'Dead'].includes(k)) return '';
  const letter = k.length === 1 && /\p{L}/u.test(k);
  const parts = [];
  if (e.ctrlKey || e.metaKey) parts.push('Ctrl');
  if (e.altKey) parts.push('Alt');
  if (e.shiftKey && (letter || k.length > 1)) parts.push('Shift');
  parts.push(k === ' ' ? 'Space' : k.length === 1 ? k.toUpperCase() : k);
  return parts.join('+');
}
export function findAction(combo, typing) {
  const inEntries = !!entriesApi();
  return ACTIONS.find((a) => keysOf(a).includes(combo) && (!typing || a.typing) && (!a.entries || inEntries));
}
document.addEventListener('keydown', (e) => {
  if (dialogOpen()) return;
  const combo = comboOf(e);
  if (!combo) return;
  const typing = isTyping(e.target);
  if (e.target.closest?.('.splitter')) return;      // the divider uses the arrow keys itself
  if (typing && e.key === 'Escape') { e.target.blur(); return; }
  const act = findAction(combo, typing);
  if (!act) return;
  e.preventDefault();
  act.run(e);
});

export function helpDialog() {
  const group = (title, list) => [h('dt', { style: { gridColumn: '1 / -1', marginTop: '8px' } }, h('b', null, title)),
    ...list.flatMap((a) => [h('dt', null, ...(keysOf(a).length ? keysOf(a).map((k, i) => [i ? T(' or ') : '', h('kbd', null, k)]).flat() : [h('span', { class: 'muted' }, '—')])), h('dd', { style: { textAlign: 'left' } }, a.label)])];
  const fixed = [['Ctrl+Enter', T('In the translation field: save to the file and go to the next untranslated line')], ['Esc', T('In the translation field: discard changes; elsewhere it clears the field selection')]];
  dialog({ title: T('Keyboard shortcuts'), body: h('div', null,
    h('p', { class: 'muted' }, tx('The shortcuts can be changed in {where}. Actions without a shortcut are available through the command palette {palette}.', { where: h('a', { href: '#', onclick: (ev) => { ev.preventDefault(); closeAllDialogs(); go('settings'); } }, T('Settings → Keyboard shortcuts')), palette: h('kbd', null, 'Ctrl+K') })),
    h('dl', { class: 'kv' }, ...group(T('General'), ACTIONS.filter((a) => !a.entries && keysOf(a).length)), ...group(T('Lines'), ACTIONS.filter((a) => a.entries)),
      h('dt', { style: { gridColumn: '1 / -1', marginTop: '8px' } }, h('b', null, T('Fixed'))), ...fixed.flatMap(([k, d]) => [h('dt', null, h('kbd', null, k)), h('dd', { style: { textAlign: 'left' } }, d)]))),
    buttons: [{ label: T('Close'), kind: 'primary' }] });
}
export const closeAllDialogs = () => { const d = $('#dlg'); if (d.open) d.close(); };

/* Command palette: type to find an action or a file. Enter runs the selected one, arrows move, Esc closes. */
function openPalette() {
  const input = h('input', { type: 'search', class: 'search', placeholder: T('What do you want to do? An action, or the beginning of a file name…'), style: { width: '100%' }, 'aria-label': T('Command palette') });
  const list = h('div', { class: 'palette', role: 'listbox' });
  let items = [], sel = 0;
  const inEntries = !!entriesApi();
  const files = (S.files || []).map((f) => f.name);
  function build() {
    const q = foldText(input.value.trim());
    const acts = ACTIONS.filter((a) => (!a.entries || inEntries) && (!q || foldText(a.label).includes(q) || foldText(a.id).includes(q)))
      .map((a) => ({ label: a.label, hint: keysOf(a)[0] || '', run: () => a.run() }));
    const fl = q ? files.filter((f) => foldText(f).includes(q)).slice(0, 8).map((f) => ({ label: T('File: {name}', { name: f }), hint: T('lines'), run: () => go('entries', { file: f, status: 'all' }) })) : [];
    items = [...acts, ...fl].slice(0, 40);
    sel = Math.min(sel, Math.max(0, items.length - 1));
    list.replaceChildren(...(items.length ? items.map((it, i) => h('div', { class: 'palrow' + (i === sel ? ' on' : ''), role: 'option', onclick: () => choose(i), onmousemove: () => { if (sel !== i) { sel = i; mark(); } } }, h('span', null, it.label), it.hint ? h('kbd', null, it.hint) : null)) : [h('div', { class: 'muted', style: { padding: '10px' } }, T('Nothing found.'))]));
  }
  const mark = () => [...list.children].forEach((c, i) => c.classList.toggle('on', i === sel));
  function choose(i) { const it = items[i]; if (!it) return; closeAllDialogs(); setTimeout(() => it.run(), 0); }
  input.addEventListener('input', () => { sel = 0; build(); });
  input.addEventListener('keydown', (e) => {
    if (e.key === 'ArrowDown') { e.preventDefault(); sel = Math.min(items.length - 1, sel + 1); mark(); list.children[sel]?.scrollIntoView({ block: 'nearest' }); }
    else if (e.key === 'ArrowUp') { e.preventDefault(); sel = Math.max(0, sel - 1); mark(); list.children[sel]?.scrollIntoView({ block: 'nearest' }); }
    else if (e.key === 'Enter') { e.preventDefault(); choose(sel); }
  });
  build();
  dialog({ title: T('Command palette'), narrow: true, body: h('div', null, input, list), buttons: [] });
  setTimeout(() => input.focus(), 0);
  if (!S.files) get('files').then((r) => { S.files = r.files; files.push(...r.files.map((f) => f.name)); build(); }).catch(() => {});
}

/* Settings: change the shortcuts. A press of the new key is captured; a conflict asks before taking the key from another action. */
function captureCombo() {
  return new Promise((resolve) => {
    let done = false;
    const finish = (v) => { if (done) return; done = true; document.removeEventListener('keydown', onKey, true); resolve(v); closeAllDialogs(); };
    const onKey = (e) => { const c = comboOf(e); if (!c) return; e.preventDefault(); e.stopPropagation(); finish(c === 'Escape' ? null : c); };
    document.addEventListener('keydown', onKey, true);
    dialog({ title: T('New shortcut'), narrow: true, body: h('p', null, tx('Press the key or combination you want to use. {esc} cancels.', { esc: h('kbd', null, 'Esc') })), buttons: [{ label: T('Cancel'), onclick: () => { finish(null); return false; } }], onclose: () => finish(null) });
  });
}
export function keysPanel() {
  const box = h('div');
  const save = (map) => { setPref('keys', map); paint(); };
  const current = () => { const m = {}; ACTIONS.forEach((a) => { m[a.id] = [...keysOf(a)]; }); return m; };
  async function assign(a, add) {
    const c = await captureCombo();
    if (!c) return;
    const bare = !/^(Ctrl|Alt)\+/.test(c) && !/^F\d+$/.test(c);
    if (a.typing && bare) return toast(T('This action also works while typing, so it needs Ctrl or Alt (or an F1–F12 key).'), 'bad');
    const map = current();
    const other = ACTIONS.find((o) => o.id !== a.id && map[o.id].includes(c));
    if (other && !await confirmDialog(T('The shortcut is already used'), T('{c} is now “{other}”. Move it to “{action}”?', { c, other: other.label, action: a.label }), T('Move'))) return;
    if (other) map[other.id] = map[other.id].filter((k) => k !== c);
    map[a.id] = add ? [...new Set([...map[a.id], c])] : [c];
    save(map);
  }
  function paint() {
    const o = pref('keys', {});
    const rows = ACTIONS.map((a) => h('tr', null, h('td', null, a.label), h('td', null, ...(keysOf(a).length ? keysOf(a).map((k) => h('kbd', { style: { marginRight: '4px' } }, k)) : [h('span', { class: 'muted' }, T('no shortcut'))])),
      h('td', null, h('button', { class: 'btn small', onclick: () => assign(a, false) }, T('Change')), ' ', h('button', { class: 'btn small ghost', title: T('Add a second shortcut'), onclick: () => assign(a, true) }, '+'), ' ',
        keysOf(a).length ? h('button', { class: 'btn small ghost', title: T('Remove all shortcuts of this action'), onclick: () => { const m = current(); m[a.id] = []; save(m); } }, T('Remove')) : null, ' ',
        Array.isArray(o[a.id]) ? h('button', { class: 'btn small ghost', title: T('Restore the default'), onclick: () => { const m = { ...pref('keys', {}) }; delete m[a.id]; save(m); } }, T('Default')) : null)));
    const t = h('table', { class: 'data' }, h('colgroup', null, [420, 200, 300].map((w) => h('col', { style: { width: w + 'px' } }))), h('thead', null, h('tr', null, [T('Action'), T('Shortcut'), ''].map((x) => h('th', null, x)))), h('tbody', null, rows));
    t.style.width = '920px';
    box.replaceChildren(h('p', { class: 'muted' }, tx('Saved at once. All actions can also be found in the command palette {palette}. The fixed ones are {enter} and {esc} in the translation field.', { palette: h('kbd', null, 'Ctrl+K'), enter: h('kbd', null, 'Ctrl+Enter'), esc: h('kbd', null, 'Esc') })),
      h('div', { class: 'tablewrap', style: { maxHeight: '340px' } }, t),
      h('div', { class: 'row', style: { marginTop: '8px' } }, h('button', { class: 'btn small', onclick: async () => { if (await confirmDialog(T('Restore all shortcuts?'), T('All shortcuts return to the default.'), T('Restore'))) save({}); } }, T('Restore all defaults'))));
  }
  paint();
  return box;
}

