import { t } from './05-i18n.js';
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
  { id: 'focus', label: 'Režim soustředění (skrýt horní lištu a záložky)', keys: ['Ctrl+Shift+F'], typing: true, run: () => setFocus(!S.focus) },
  { id: 'quick', label: 'Rychlé překládání (scéna + editor, jen nepřeložené)', keys: [], run: () => enterQuick() },
  { id: 'proofmode', label: 'Rychlá korektura (scéna + editor, jen ke kontrole)', keys: [], run: () => enterProof() },
  { id: 'fullscreen', label: 'Celá obrazovka (zapnout / vypnout)', keys: ['F11'], typing: true, run: () => toggleFullscreen() },
  { id: 'palette', label: 'Paleta příkazů (hledání akcí a souborů)', keys: ['Ctrl+K'], typing: true, run: () => openPalette() },
  { id: 'search', label: 'Hledat v aktuální sekci', keys: ['Ctrl+F'], typing: true, run: () => { const el = $('#search'); if (el) { el.focus(); el.select(); } } },
  { id: 'search_slash', label: 'Hledat v aktuální sekci (lomítko)', keys: ['/'], run: () => $('#search')?.focus() },
  { id: 'refresh', label: 'Obnovit soubory', keys: ['F5', 'Ctrl+R'], typing: true, run: () => doRefresh() },
  { id: 'undo', label: 'Vrátit poslední změnu (schválení, korektura, slovník)', keys: ['Ctrl+Z'], run: () => doUndo() },
  { id: 'help', label: 'Ukázat zkratky', keys: ['?'], run: () => helpDialog() },
  ...TABS.map(([k, label], i) => ({ id: 'tab_' + k, get label() { return t('Go to: {name}', { name: t(label) }); }, keys: [`Alt+${i + 1}`], typing: true, run: () => go(k) })),
  { id: 'next', label: 'Řádky: další řádek', keys: ['ArrowDown', 'J'], entries: true, run: () => moveSel(1) },
  { id: 'prev', label: 'Řádky: předchozí řádek', keys: ['ArrowUp', 'K'], entries: true, run: () => moveSel(-1) },
  { id: 'pagedown', label: 'Řádky: o stránku dolů', keys: ['PageDown'], entries: true, run: () => moveSel(15) },
  { id: 'pageup', label: 'Řádky: o stránku nahoru', keys: ['PageUp'], entries: true, run: () => moveSel(-15) },
  { id: 'first', label: 'Řádky: první řádek', keys: ['Home'], entries: true, run: () => entriesApi()?.select(0) },
  { id: 'last', label: 'Řádky: poslední řádek', keys: ['End'], entries: true, run: () => entriesApi()?.select(S.ent.items.length - 1) },
  { id: 'approve', label: 'Řádky: schválit / zrušit schválení řádku', keys: ['A'], entries: true, run: () => entriesApi()?.toggleApprove() },
  { id: 'approve_all', label: 'Řádky: schválit všechny shodné… (s náhledem)', keys: ['Shift+A'], entries: true, run: () => entriesApi()?.globalApprove() },
  { id: 'proof', label: 'Řádky: korektura – zkontrolováno (a další)', keys: ['P'], entries: true, run: () => entriesApi()?.toggleProof() },
  { id: 'proof_upto', label: 'Řádky: zkontrolovat vše až po tento řádek', keys: ['Shift+P'], entries: true, run: () => entriesApi()?.proofUpto() },
  { id: 'unproof_upto', label: 'Řádky: zrušit zkontrolování až po tento řádek', keys: ['Shift+U'], entries: true, run: () => entriesApi()?.unproofUpto() },
  { id: 'flag', label: 'Řádky: označit k opravě + poznámka', keys: ['F'], entries: true, run: () => entriesApi()?.flagFix() },
  { id: 'open_editor', label: 'Řádky: otevřít řádek v externím editoru', keys: ['E', 'Enter'], entries: true, run: () => { const it = S.ent.items[S.ent.sel]; if (it) openEditor(it.file, it.line); } },
  { id: 'replace', label: 'Najít a nahradit v překladech…', keys: [], run: () => replaceDialog(async () => { S.files = null; await refreshHeader(); if (S.view === 'entries') S.ent.api?.reload(); else render(); }, S.fl.picked.size ? [...S.fl.picked] : []) },
  { id: 'rule', label: 'Schválit podle pravidla…', keys: [], run: () => { if (!S.files) get('files').then((r) => { S.files = r.files; }).finally(() => ruleApproveDialog(async () => { S.files = null; await refreshHeader(); render(); }, [])); else ruleApproveDialog(async () => { S.files = null; await refreshHeader(); render(); }, []); } },
  { id: 'fix', label: 'Opravit (mluvčí, mezery, překlepy)…', keys: [], run: () => fixDialog({}, async () => { S.files = null; await refreshHeader(); render(); }) },
  { id: 'consistency', label: 'Kontrola: rozdílné překlady stejného originálu', keys: [], run: () => goMode({ ...VAL_MODES, 'validation.consistency': true }) },
  { id: 'ignored', label: 'Kontrola: ignorované nálezy', keys: [], run: () => goMode({ ...VAL_MODES, 'validation.ignored': 'only' }) },
  { id: 'lint', label: 'Kontrola: lint hry (Ren\'Py)', keys: [], run: () => goMode({ ...VAL_MODES, 'validation.lint': true }) },
  { id: 'findings', label: 'Kontrola: nálezy', keys: [], run: () => goMode({ ...VAL_MODES }) },
  { id: 'snapshot', label: 'Vytvořit zálohu souborů překladu teď', keys: [], run: async () => { try { const r = await post('snapshots/create'); toast(`Záloha vytvořena (${r.snapshot.files} souborů, ${fmtSize(r.snapshot.bytes)}).`); } catch (e) { fail(e); } } },
  { id: 'theme', label: 'Přepnout vzhled (automaticky / tmavý / světlý)', keys: [], run: () => cycleTheme() },
  { id: 'density', label: 'Přepnout hustotu tabulek', keys: [], run: () => toggleDensity() },
  { id: 'new_window', label: 'Otevřít druhé okno s detailem řádku', keys: [], run: () => openDetailWindow() },
  { id: 'change_project', label: 'Změnit složku s překladem…', keys: [], run: () => pickProject() },
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
    ...list.flatMap((a) => [h('dt', null, ...(keysOf(a).length ? keysOf(a).map((k, i) => [i ? ' nebo ' : '', h('kbd', null, k)]).flat() : [h('span', { class: 'muted' }, '—')])), h('dd', { style: { textAlign: 'left' } }, a.label)])];
  const fixed = [['Ctrl+Enter', 'V poli překladu: uložit do souboru a přejít na další nepřeložený řádek'], ['Esc', 'V poli překladu: zahodit změny; jinde zruší výběr pole']];
  dialog({ title: 'Klávesové zkratky', body: h('div', null,
    h('p', { class: 'muted' }, 'Zkratky jde změnit v ', h('a', { href: '#', onclick: (ev) => { ev.preventDefault(); closeAllDialogs(); go('settings'); } }, 'Nastavení → Klávesové zkratky'), '. Bez zkratky jsou akce dostupné přes paletu příkazů ', h('kbd', null, 'Ctrl+K'), '.'),
    h('dl', { class: 'kv' }, ...group('Obecné', ACTIONS.filter((a) => !a.entries && keysOf(a).length)), ...group('Řádky', ACTIONS.filter((a) => a.entries)),
      h('dt', { style: { gridColumn: '1 / -1', marginTop: '8px' } }, h('b', null, 'Pevné')), ...fixed.flatMap(([k, d]) => [h('dt', null, h('kbd', null, k)), h('dd', { style: { textAlign: 'left' } }, d)]))),
    buttons: [{ label: 'Zavřít', kind: 'primary' }] });
}
export const closeAllDialogs = () => { const d = $('#dlg'); if (d.open) d.close(); };

/* Command palette: type to find an action or a file. Enter runs the selected one, arrows move, Esc closes. */
function openPalette() {
  const input = h('input', { type: 'search', class: 'search', placeholder: 'Co chceš udělat? Akce, nebo začátek názvu souboru…', style: { width: '100%' }, 'aria-label': 'Paleta příkazů' });
  const list = h('div', { class: 'palette', role: 'listbox' });
  let items = [], sel = 0;
  const inEntries = !!entriesApi();
  const files = (S.files || []).map((f) => f.name);
  function build() {
    const q = foldText(input.value.trim());
    const acts = ACTIONS.filter((a) => (!a.entries || inEntries) && (!q || foldText(a.label).includes(q) || foldText(a.id).includes(q)))
      .map((a) => ({ label: a.label, hint: keysOf(a)[0] || '', run: () => a.run() }));
    const fl = q ? files.filter((f) => foldText(f).includes(q)).slice(0, 8).map((f) => ({ label: `Soubor: ${f}`, hint: 'řádky', run: () => go('entries', { file: f, status: 'all' }) })) : [];
    items = [...acts, ...fl].slice(0, 40);
    sel = Math.min(sel, Math.max(0, items.length - 1));
    list.replaceChildren(...(items.length ? items.map((it, i) => h('div', { class: 'palrow' + (i === sel ? ' on' : ''), role: 'option', onclick: () => choose(i), onmousemove: () => { if (sel !== i) { sel = i; mark(); } } }, h('span', null, it.label), it.hint ? h('kbd', null, it.hint) : null)) : [h('div', { class: 'muted', style: { padding: '10px' } }, 'Nic nenalezeno.')]));
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
  dialog({ title: 'Paleta příkazů', narrow: true, body: h('div', null, input, list), buttons: [] });
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
    dialog({ title: 'Nová zkratka', narrow: true, body: h('p', null, 'Stiskni klávesu nebo kombinaci, kterou chceš použít. ', h('kbd', null, 'Esc'), ' zruší.'), buttons: [{ label: 'Zrušit', onclick: () => { finish(null); return false; } }], onclose: () => finish(null) });
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
    if (a.typing && bare) return toast('Tahle akce funguje i při psaní, takže potřebuje Ctrl nebo Alt (nebo klávesu F1–F12).', 'bad');
    const map = current();
    const other = ACTIONS.find((o) => o.id !== a.id && map[o.id].includes(c));
    if (other && !await confirmDialog('Zkratka už je použitá', `${c} je teď „${other.label}“. Přesunout ji k „${a.label}“?`, 'Přesunout')) return;
    if (other) map[other.id] = map[other.id].filter((k) => k !== c);
    map[a.id] = add ? [...new Set([...map[a.id], c])] : [c];
    save(map);
  }
  function paint() {
    const o = pref('keys', {});
    const rows = ACTIONS.map((a) => h('tr', null, h('td', null, a.label), h('td', null, ...(keysOf(a).length ? keysOf(a).map((k) => h('kbd', { style: { marginRight: '4px' } }, k)) : [h('span', { class: 'muted' }, 'bez zkratky')])),
      h('td', null, h('button', { class: 'btn small', onclick: () => assign(a, false) }, 'Změnit'), ' ', h('button', { class: 'btn small ghost', title: 'Přidat druhou zkratku', onclick: () => assign(a, true) }, '+'), ' ',
        keysOf(a).length ? h('button', { class: 'btn small ghost', title: 'Odebrat všechny zkratky této akce', onclick: () => { const m = current(); m[a.id] = []; save(m); } }, 'Odebrat') : null, ' ',
        Array.isArray(o[a.id]) ? h('button', { class: 'btn small ghost', title: 'Vrátit výchozí', onclick: () => { const m = { ...pref('keys', {}) }; delete m[a.id]; save(m); } }, 'Výchozí') : null)));
    const t = h('table', { class: 'data' }, h('colgroup', null, [420, 200, 300].map((w) => h('col', { style: { width: w + 'px' } }))), h('thead', null, h('tr', null, ['Akce', 'Zkratka', ''].map((x) => h('th', null, x)))), h('tbody', null, rows));
    t.style.width = '920px';
    box.replaceChildren(h('p', { class: 'muted' }, 'Ukládá se hned. Všechny akce jdou vyhledat i v paletě příkazů ', h('kbd', null, 'Ctrl+K'), '. Pevné zůstávají ', h('kbd', null, 'Ctrl+Enter'), ' a ', h('kbd', null, 'Esc'), ' v poli překladu.'),
      h('div', { class: 'tablewrap', style: { maxHeight: '340px' } }, t),
      h('div', { class: 'row', style: { marginTop: '8px' } }, h('button', { class: 'btn small', onclick: async () => { if (await confirmDialog('Vrátit všechny zkratky?', 'Všechny zkratky se vrátí na výchozí.', 'Vrátit')) save({}); } }, 'Vrátit všechny výchozí')));
  }
  paint();
  return box;
}

