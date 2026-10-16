import { $, h } from './00-core.js';
import { get, post } from './10-api.js';
import { S, speakerFull, speakerMode, speakerName } from './11-state.js';
import { dialog, status, toast } from './13-feedback.js';
import { bar, empty, menuButton } from './14-renderers.js';
import { go, render } from './16-tabs.js';
import { openDetailWindow } from './21-files.js';
import { toggleFullscreen } from './35-keyboard.js';
/* ---------------------------------------------------------------- icons, speaker badges, focus mode, layouts */
const ICONS = {
  win: [['rect', { x: 3, y: 8, width: 12, height: 12, rx: 2 }], ['path', { d: 'M9 8V6a2 2 0 0 1 2-2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2h-2' }]],
  full: [['path', { d: 'M4 9V4h5M20 9V4h-5M4 15v5h5M20 15v5h-5' }]],
  density: [['path', { d: 'M4 7h16M4 12h16M4 17h16' }]],
  densityC: [['path', { d: 'M4 5h16M4 9h16M4 13h16M4 17h16M4 21h16' }]],
  theme: [['circle', { cx: 12, cy: 12, r: 8 }], ['path', { d: 'M12 4v16', 'stroke-width': 1.8 }]],
  moon: [['path', { d: 'M20 14.5A8 8 0 0 1 9.5 4a8 8 0 1 0 10.5 10.5z' }]],
  sun: [['circle', { cx: 12, cy: 12, r: 4 }], ['path', { d: 'M12 2v2M12 20v2M2 12h2M20 12h2M5 5l1.5 1.5M17.5 17.5L19 19M19 5l-1.5 1.5M6.5 17.5L5 19' }]],
  focus: [['rect', { x: 4, y: 5, width: 16, height: 14, rx: 2 }], ['path', { d: 'M9 10h6v4H9z' }]],
  layout: [['rect', { x: 3, y: 4, width: 18, height: 16, rx: 2 }], ['path', { d: 'M3 14h18' }]],
  undo: [['path', { d: 'M9 7L4 12l5 5M4 12h11a5 5 0 0 1 0 10h-3' }]],
  empty: [['path', { d: 'M4 13l2-7h12l2 7v5a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1z' }], ['path', { d: 'M4 13h5l1 2h4l1-2h5' }]],
};
export function icon(name, size = 16) {
  const NS = 'http://www.w3.org/2000/svg';
  const svg = document.createElementNS(NS, 'svg');
  for (const [k, v] of Object.entries({ viewBox: '0 0 24 24', width: size, height: size, fill: 'none', stroke: 'currentColor', 'stroke-width': 1.8, 'stroke-linecap': 'round', 'stroke-linejoin': 'round', 'aria-hidden': 'true' })) svg.setAttribute(k, v);
  for (const [tag, attrs] of ICONS[name] || []) { const el = document.createElementNS(NS, tag); for (const [k, v] of Object.entries(attrs)) el.setAttribute(k, v); svg.append(el); }
  return svg;
}
export const setIcon = (id, name) => { const b = typeof id === 'string' ? $('#' + id) : id; if (b) b.replaceChildren(icon(name)); };

/* Speaker colours come from the game's Character(...) lines (when its sources are found); otherwise a stable colour is made from the code. */
export async function loadChars() { try { S.chars = (await get('game/characters')).characters || {}; } catch { S.chars = {}; } }
export function speakerColor(who) {
  const code = (who || '').trim().split(/\s+/)[0];
  if (!code) return '';
  const c = S.chars?.[code]?.color;
  if (c) return c;
  let hue = 0; for (const ch of code) hue = (hue * 31 + ch.charCodeAt(0)) % 360;
  return `hsl(${hue} 55% 52%)`;
}
export function readableOn(color) {
  const m = /^#([0-9a-f]{3}|[0-9a-f]{6})$/i.exec(color || '');
  if (!m) return '#fff';
  const x = m[1].length === 3 ? [...m[1]].map((c) => c + c).join('') : m[1];
  const [r, g, b] = [0, 2, 4].map((i) => parseInt(x.slice(i, i + 2), 16) / 255).map((v) => (v <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4));
  return 0.2126 * r + 0.7152 * g + 0.0722 * b > 0.4 ? '#111' : '#fff';
}
export function speakerBadge(who) {
  const code = (who || '').trim().split(/\s+/)[0];
  if (!code) return null;
  const name = speakerName(who) || S.chars?.[code]?.name || code;
  const ini = ([...String(name).replace(/^\[|\]$/g, '')][0] || '?').toUpperCase();
  const bg = speakerColor(who);
  return h('span', { class: 'sbadge', title: speakerFull(who), style: { background: bg, color: readableOn(bg) } }, ini);
}

/* Focus mode: only the list and the detail (top bar, tabs and status bar hidden), with a small floating bar. Quick translating = focus + film layout. */
export function setFocus(on) {
  S.focus = !!on;
  document.body.classList.toggle('focus', S.focus);
  if (!S.focus) S.quick = false;
  paintFocusBar();
}
export function paintFocusBar() {
  let bar = $('#focusbar');
  if (!S.focus) { bar?.remove(); return; }
  if (!bar) { bar = h('div', { id: 'focusbar', class: 'focusbar', role: 'toolbar', 'aria-label': 'Soustředění' }); document.body.append(bar); }
  const rest = S.ent?.counts?.todo, rev = S.ent?.counts?.review;
  bar.replaceChildren(
    h('button', { class: 'btn small', title: 'Vrátit horní lištu a záložky (Ctrl+Shift+F)', onclick: () => setFocus(false) }, '⤢ Zobrazit vše'),
    layoutMenu(),
    S.quick ? h('div', { class: 'seg', role: 'group', 'aria-label': 'Rychlý režim' },
      h('button', { class: S.quick === 'tr' ? 'on' : '', title: 'Jen nepřeložené řádky, editor se zaměří sám', onclick: () => S.quick !== 'tr' && enterQuick() }, 'Překlad'),
      h('button', { class: S.quick === 'proof' ? 'on' : '', title: 'Jen řádky ke kontrole, P zkontroluje a skočí dál', onclick: () => S.quick !== 'proof' && enterProof() }, 'Korektura')) : null,
    h('button', { class: 'btn small icon', title: 'Celá obrazovka (F11)', onclick: () => toggleFullscreen() }, icon('full', 14)),
    h('button', { class: 'btn small icon', title: 'Vrátit poslední změnu (Ctrl+Z)', onclick: () => $('#undoBtn')?.click() }, icon('undo', 14)),
    ...(S.quick === 'proof' ? [
      h('button', { class: 'btn small primary', title: 'Označí řádek jako zkontrolovaný a přejde na další (P)', onclick: () => S.ent?.api?.toggleProof() }, '✓ Zkontrolováno'),
      h('button', { class: 'btn small', title: 'Označí řádek k opravě (F)', onclick: () => S.ent?.api?.flagFix() }, '⚑ K opravě'),
      h('button', { class: 'btn small icon', title: 'Předchozí řádek (↑)', onclick: () => S.ent?.api?.select(S.ent.sel - 1) }, '↑'),
      h('button', { class: 'btn small icon', title: 'Další řádek (↓)', onclick: () => S.ent?.api?.select(S.ent.sel + 1) }, '↓'),
      h('span', { class: 'muted' }, 'Korektura' + (rev != null ? ` · zbývá zkontrolovat ${rev}` : ''))]
      : S.quick ? [h('span', { class: 'muted' }, 'Rychlé překládání' + (rest != null ? ` · zbývá přeložit ${rest}` : '') + (rev != null ? ` · zkontrolovat ${rev}` : ''))] : []));
}
export function enterQuick() {
  setPref('entries.layout', 'film'); setPref('ui.detailTab', 'check');
  setPref('entries.status', 'todo');
  S.quick = 'tr';
  setFocus(true);
  go('entries', { status: 'todo', file: pref('entries.file', ''), select: true });
}
/* Quick proofreading: film layout + focus mode + only lines still to be checked; P marks a line as checked and moves on, F flags it for a fix. */
export function enterProof() {
  setPref('entries.layout', 'film'); setPref('ui.detailTab', 'proof');
  setPref('entries.status', 'review');
  S.quick = 'proof';
  setFocus(true);
  go('entries', { status: 'review', file: pref('entries.file', ''), select: true });
}

/* Layouts of the line list + detail: the built-in ones, "Dva monitory" (detail window) and the user's own saved ones. */
const LAYOUTS = [['auto', 'Automaticky', 'podle šířky okna'], ['rows', 'Detail dole', ''], ['cols', 'Detail vpravo', ''], ['film', 'Scéna a filmový pás', 'velký obrázek, editor, pás řádků']];
/* Redraw the list with another layout but keep the line that is open. */
function reopenEntries() {
  const cur = S.ent?.items?.[S.ent.sel];
  if (S.view === 'entries' && cur) go('entries', { file: pref('entries.file', ''), line: cur.line, status: pref('entries.status', 'all'), select: true });
  else render();
}
function applyLayout(l) { setPref('entries.layout', l); if (l === 'film') setPref('ui.detailTab', 'check'); reopenEntries(); }
function askName(title, label, done) {
  const inp = h('input', { type: 'text', style: { width: '100%' }, placeholder: label, 'aria-label': label });
  dialog({ title, narrow: true, body: inp, buttons: [{ label: 'Zrušit' }, { label: 'Uložit', kind: 'primary', onclick: () => { const v = inp.value.trim().slice(0, 40); if (!v) return false; done(v); } }] });
  setTimeout(() => inp.focus(), 0);
}
function savePreset() {
  askName('Uložit rozložení', 'Název rozložení', (name) => {
    setPref(`ui.presets.${name.replace(/\./g, '·')}`, { layout: pref('entries.layout', 'auto'), tab: pref('ui.detailTab', 'check'), w: pref('entries.detailw', 620), hh: pref('entries.detailh', 380), slim: pref('ui.slimMain', true) });
    toast(`Rozložení „${name}“ uloženo.`, 'ok', 2000);
  });
}
function applyPreset(name) {
  const p = pref('ui.presets', {})[name]; if (!p) return;
  setPref('entries.layout', p.layout); setPref('ui.detailTab', p.tab); setPref('entries.detailw', p.w); setPref('entries.detailh', p.hh); setPref('ui.slimMain', p.slim !== false);
  reopenEntries();
}
function deletePreset() {
  const names = Object.keys(pref('ui.presets', {}));
  const close = dialog({ title: 'Smazat vlastní rozložení', narrow: true, buttons: [{ label: 'Zavřít' }],
    body: h('div', { class: 'row', style: { flexWrap: 'wrap', gap: '6px' } }, names.map((n) => h('button', { class: 'btn small', onclick: () => { const all = { ...pref('ui.presets', {}) }; delete all[n]; setPref('ui.presets', all); close(); toast(`Rozložení „${n}“ smazáno.`, 'ok', 1800); } }, `Smazat „${n}“`))) });
}
export function layoutMenu() {
  const cur = pref('entries.layout', 'auto');
  const custom = Object.keys(pref('ui.presets', {}));
  return menuButton('Rozložení', [
    ...LAYOUTS.map(([k, label, hint]) => ({ label: (cur === k ? '✓ ' : '') + label, hint, run: () => applyLayout(k) })),
    { label: 'Rychlý režim', hint: 'překlad nebo korektura: scéna + editor, bez rušení', run: () => enterQuick() },
    { label: 'Dva monitory', hint: 'detail ve druhém okně', run: () => openDetailWindow() },
    ...custom.map((n) => ({ label: '★ ' + n, hint: 'vlastní', run: () => applyPreset(n) })),
    { label: 'Uložit aktuální rozložení…', run: savePreset },
    ...(custom.length ? [{ label: 'Smazat vlastní rozložení…', run: deletePreset }] : []),
  ], { title: 'Rozložení seznamu a detailu' });
}

export function speakerModeSelect(onchange) {
  return h('select', { title: 'Jak zobrazovat mluvčí', onchange: (ev) => { setPref('display.speaker', ev.target.value); onchange?.(); } },
    ...[['both', 'Mluvčí: kód i jméno (2 sloupce)'], ['code', 'Mluvčí: jen kód'], ['name', 'Mluvčí: jen jméno']].map(([v, l]) => h('option', { value: v, selected: speakerMode() === v }, l)));
}
/* Several windows can be open at once (each has its own list / filter state). Only the first one saves preferences, so the
   windows never overwrite each other's settings; the app quits when the last window is closed. */
export const WIN_ID = Math.random().toString(36).slice(2, 10);
export const URL_Q = new URLSearchParams(location.search);
export const SECONDARY = URL_Q.has('win');
export const COMPANION = SECONDARY && URL_Q.get('companion') === '1';      // detail-only window that follows the line open in the main window
let prefTimer = null;
export const pref = (path, def) => path.split('.').reduce((o, k) => (o && o[k] !== undefined ? o[k] : undefined), S.prefs) ?? def;
export function setPref(path, value) {
  const parts = path.split('.');
  let o = S.prefs;
  for (const k of parts.slice(0, -1)) o = o[k] = o[k] || {};
  o[parts.at(-1)] = value;
  if (SECONDARY) return;
  clearTimeout(prefTimer);
  prefTimer = setTimeout(() => post('prefs', S.prefs).catch(() => {}), 500);
}

