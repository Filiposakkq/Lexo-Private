import { $, clamp01, h } from './00-core.js';
import { S } from './11-state.js';
import { icon, pref, setPref } from './12-chrome.js';
/* ---------------------------------------------------------------- shared renderers */
export function fname(rel, { link = false, onclick } = {}) {
  const p = String(rel);
  const i = Math.max(p.lastIndexOf('\\'), p.lastIndexOf('/'));
  const wrapBreaks = (s) => s.split(/([\\/_])/).map((t) => (/[\\/_]/.test(t) ? [t, h('wbr')] : t));
  const el = h('div', { class: 'fname' + (link ? ' clickable' : ''), onclick },
    i >= 0 ? [h('span', { class: 'dir' }, wrapBreaks(p.slice(0, i + 1))), wrapBreaks(p.slice(i + 1))] : wrapBreaks(p));
  return el;
}
const TOKEN_RE = /(\{\{|\[\[|\{[^}]*\}|\[[^\]]*\]|\\n|\\")/g;
export function fmtText(text) {
  const frag = document.createDocumentFragment();
  for (const part of String(text ?? '').split(TOKEN_RE)) {
    if (!part) continue;
    if (part.startsWith('{') && part !== '{{') frag.append(h('span', { class: 'tok-tag' }, part));
    else if (part.startsWith('[') && part !== '[[') frag.append(h('span', { class: 'tok-var' }, part));
    else if (part === '\\n' || part === '\\"') frag.append(h('span', { class: 'tok-esc' }, part));
    else frag.append(part);
  }
  return frag;
}
/* Renders Ren'Py text markup as it would read in the game: {i}{b}{u}{s}{color=..}, \n and {p} as line breaks, [variables] as XXXXX;
   waits and other tags are dropped. Only text nodes and a few styled elements are created, so nothing from the text is ever run as HTML. */
export function gameText(text) {
  const root = document.createElement('span');
  const stack = [root];
  const top = () => stack[stack.length - 1];
  const open = (tag, color) => { const el = document.createElement(tag); if (color) el.style.color = color; el.dataset.tag = tag; top().append(el); stack.push(el); };
  const close = (tag) => { for (let i = stack.length - 1; i > 0; i--) if (stack[i].dataset.tag === tag) { stack.length = i; return; } };
  const re = /\{\{|\[\[|\{(\/?)(i|b|u|s)\}|\{color=([^}]*)\}|\{\/color\}|\{p(?:=[^}]*)?\}|\{[^}]*\}|\[[^\]]*\]|\\n|\\"|\\\\|\\./g;
  let last = 0, m;
  while ((m = re.exec(text))) {
    if (m.index > last) top().append(text.slice(last, m.index));
    const t = m[0];
    if (t === '{{') top().append('{');
    else if (t === '[[') top().append('[');
    else if (m[2]) { if (m[1]) close(m[2]); else open(m[2]); }
    else if (m[3] !== undefined) { const c = m[3].trim(); open('span', /^#?[0-9a-fA-F]{3,8}$/.test(c) ? (c[0] === '#' ? c : '#' + c) : /^[a-z]+$/i.test(c) ? c : ''); }
    else if (t === '{/color}') close('span');
    else if (t === '\\n' || t.startsWith('{p')) top().append(document.createElement('br'));
    else if (t === '\\"') top().append('"');
    else if (t === '\\\\') top().append('\\');
    else if (t[0] === '[') top().append('XXXXX');
    else if (t[0] === '\\') top().append(t[1]);
    /* any other {tag}: waits, speed, comments ... are invisible */
    last = re.lastIndex;
  }
  if (last < text.length) top().append(text.slice(last));
  return root;
}
export const STATUS_LABEL = { done: 'Hotovo', rev: 'Korektura', proof: 'V korekci', tr: 'Přeloženo', wip: 'Rozpracováno', none: 'Nezačato', planned: 'Plánováno' };
export const ENTRY_LABEL = { translated: 'Přeloženo', approved: 'Schváleno', todo: 'Nepřeloženo' };
export const KIND_LABEL = { empty: 'prázdný', english: 'anglicky', same: 'shodné', symbols: 'jen znaky' };
export const SEV_LABEL = { error: 'Chyba', warning: 'Varování', info: 'Info' };
export const proofChip = (p) => (p === 'ok' ? chip('proof', '✓ Zkontrolováno') : p === 'fix' ? chip('warning', '⚑ k opravě')
  : p === 'stale' ? chip('info', '↻ změněno po korektuře') : null);
export const chip = (cls, text) => h('span', { class: 'chip ' + cls }, text);
export function bar(pct, cls = '') { return h('div', { class: 'bar ' + cls }, h('i', { style: { width: clamp01(pct) + '%' } })); }
export function empty(title, text) { return h('div', { class: 'empty' }, h('div', { class: 'empty-ico' }, icon('empty', 28)), h('b', null, title), text); }
export function loading(text = 'Načítám…') { return h('div', { class: 'loading' }, h('span', { class: 'spin' }), ' ' + text); }
export const isTyping = (t) => t && (['INPUT', 'TEXTAREA', 'SELECT'].includes(t.tagName) || t.isContentEditable);

/* resizable, sortable table with a persisted column layout */
/* Draggable divider between a list and its detail pane. axis 'v': detail below (height), 'h': detail on the right
   (width). The size is clamped (min px, max share of the area), saved as a preference, double-click resets it,
   arrow keys resize it when it has focus. */
export function makeSplitter(root, axis, { key, def, min, maxFrac }) {
  const vertical = axis === 'v';
  const prop = vertical ? '--detail-h' : '--detail-w';
  const bar = h('div', { class: 'splitter ' + (vertical ? 'sp-v' : 'sp-h'), role: 'separator', tabindex: '0',
    'aria-orientation': vertical ? 'horizontal' : 'vertical', title: 'Tažením změníš velikost detailu, dvojklik ji vrátí. S fokusem fungují šipky.' });
  const clampTo = (v) => {
    const total = vertical ? root.clientHeight : root.clientWidth;
    const max = Math.max(min, Math.floor(total * maxFrac));
    return Math.round(Math.max(min, Math.min(max, v)));
  };
  const apply = (v, save) => { const c = clampTo(v); root.style.setProperty(prop, c + 'px'); if (save) setPref(key, c); return c; };
  const current = () => parseInt(root.style.getPropertyValue(prop), 10) || def;
  const fromPointer = (ev) => { const r = root.getBoundingClientRect(); return vertical ? r.bottom - ev.clientY : r.right - ev.clientX; };
  bar.addEventListener('pointerdown', (e) => {
    e.preventDefault(); bar.setPointerCapture(e.pointerId); bar.classList.add('drag');
    const move = (ev) => apply(fromPointer(ev), false);
    const up = (ev) => { bar.classList.remove('drag'); bar.removeEventListener('pointermove', move); bar.removeEventListener('pointerup', up); apply(fromPointer(ev), true); };
    bar.addEventListener('pointermove', move); bar.addEventListener('pointerup', up);
  });
  bar.addEventListener('dblclick', () => apply(def, true));
  bar.addEventListener('keydown', (e) => {
    const step = e.shiftKey ? 80 : 20;
    const grow = e.key === (vertical ? 'ArrowUp' : 'ArrowLeft'), shrink = e.key === (vertical ? 'ArrowDown' : 'ArrowRight');
    if (grow || shrink) { e.preventDefault(); apply(current() + (grow ? step : -step), true); }
    else if (e.key === 'Home') { e.preventDefault(); apply(def, true); }
  });
  bar.sync = () => apply(pref(key, def), false);      // re-clamp after the window changed size
  S.splitSync = () => bar.sync();
  return bar;
}
window.addEventListener('resize', () => S.splitSync?.());

export function buildTable({ key, cols, sort, onSort, className = '' }) {
  const saved = pref(key + '.colw', {});
  const colgroup = h('colgroup', null, cols.map((c) => h('col', { style: { width: (saved[c.k] || c.w) + 'px' } })));
  const total = () => cols.reduce((s, c, i) => s + parseInt(colgroup.children[i].style.width, 10), 0);
  const table = h('table', { class: 'data ' + className });
  const fit = () => { table.style.width = Math.max(total(), 0) + 'px'; };
  const head = h('tr', null, cols.map((c, i) => {
    const th = h('th', { class: (c.sortable ? 'sortable ' : '') + (c.center ? 'center' : ''), title: c.title || '',
      onclick: c.sortable ? () => onSort?.(c.k) : undefined, style: { position: 'sticky' } },
      c.node || c.label, sort && sort.k === c.k ? h('span', { class: 'arrow' }, sort.dir > 0 ? ' ▲' : ' ▼') : null);
    if (c.resize !== false) {
      const rz = h('span', { class: 'rz', title: 'Tažením změníš šířku sloupce' });
      rz.addEventListener('click', (e) => e.stopPropagation());
      rz.addEventListener('mousedown', (e) => {
        e.preventDefault(); e.stopPropagation();
        const x0 = e.clientX, w0 = parseInt(colgroup.children[i].style.width, 10);
        rz.classList.add('drag');
        const move = (ev) => { colgroup.children[i].style.width = Math.max(40, w0 + ev.clientX - x0) + 'px'; fit(); };
        const up = () => {
          rz.classList.remove('drag');
          removeEventListener('mousemove', move); removeEventListener('mouseup', up);
          const w = {}; cols.forEach((cc, j) => { w[cc.k] = parseInt(colgroup.children[j].style.width, 10); });
          setPref(key + '.colw', w);
        };
        addEventListener('mousemove', move); addEventListener('mouseup', up);
      });
      th.append(rz);
    }
    return th;
  }));
  const tbody = h('tbody'); const tfoot = h('tfoot');
  table.append(colgroup, h('thead', null, head), tbody, tfoot);
  fit();
  return { table, tbody, tfoot };
}

/* A button that opens a small menu of actions (keeps toolbars short). items: [{ label, hint?, run }] */
export function menuButton(label, items, { primary = false, title = '' } = {}) {
  const wrap = h('span', { class: 'menuwrap' });
  const btn = h('button', { class: 'btn small' + (primary ? ' primary' : ''), 'aria-haspopup': 'menu', 'aria-expanded': 'false', title }, label, ' ▾');
  const menu = h('div', { class: 'menu', role: 'menu' });
  menu.hidden = true;
  const onDoc = (e) => { if (!wrap.contains(e.target)) close(); };
  function close() { menu.hidden = true; btn.setAttribute('aria-expanded', 'false'); document.removeEventListener('mousedown', onDoc, true); }
  btn.onclick = (e) => {
    e.stopPropagation();
    if (!menu.hidden) return close();
    menu.replaceChildren(...items.map((it) => h('button', { class: 'menuitem', role: 'menuitem', onclick: () => { close(); it.run(); } }, h('span', null, it.label), it.hint ? h('span', { class: 'muted' }, it.hint) : null)));
    menu.hidden = false; btn.setAttribute('aria-expanded', 'true');
    menu.classList.remove('flip-x', 'flip-y');           // keep the menu on the screen: open to the left / upwards when it would leave it
    const box = menu.getBoundingClientRect();
    if (box.right > innerWidth - 8) menu.classList.add('flip-x');
    if (box.bottom > innerHeight - 8 && btn.getBoundingClientRect().top > box.height + 16) menu.classList.add('flip-y');
    document.addEventListener('mousedown', onDoc, true);
  };
  wrap.addEventListener('keydown', (e) => { if (e.key === 'Escape') { close(); btn.focus(); } });
  wrap.append(btn, menu);
  return wrap;
}

