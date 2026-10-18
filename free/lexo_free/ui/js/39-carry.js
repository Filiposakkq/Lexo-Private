import { tn, t as T } from './05-i18n.js';
import { h } from './00-core.js';
import { post } from './10-api.js';
import { S } from './11-state.js';
import { dialog, fail, toast } from './13-feedback.js';
import { chip, fname } from './14-renderers.js';
import { refreshHeader } from './17-project.js';
import { diffSpans } from './23-fixes.js';
import { requireFeature } from './38-pro.js';
/* Carry the translation of the previous version of the game over to the open project (a Pro feature): choose the old folder, look at what was found, tick, write.
   Lines with the very same original are ticked; lines whose original changed a little ("near") are ticked only when they are almost the same and have the same variables. */
export async function carryDialog(done) {
  if (!await requireFeature('carry.over')) return;
  const path = h('input', { type: 'text', placeholder: T('Folder of the previous (translated) version'), style: { flex: 1 } });
  const info = h('span', { class: 'grow' }), results = h('div'), go = h('button', { class: 'btn primary', disabled: true });
  const sel = new Set();
  let data = null;
  const key = (it) => `${it.file}|${it.line}`;
  const recommended = (it) => it.kind === 'exact' || (it.ratio >= 0.9 && !it.tags_differ);
  function paintInfo() {
    go.disabled = !sel.size;
    go.textContent = tn(sel.size, 'Carry over {n} line', 'Carry over {n} lines');
    info.textContent = data ? T('Selected {n} of {total}', { n: sel.size, total: data.items.length }) : '';
  }
  function paint() {
    if (!data) return results.replaceChildren();
    const c = data.counts;
    const rows = data.items.slice(0, 600).map((it) => {
      const d = it.kind === 'near' ? diffSpans(it.old_source, it.source) : null;
      return h('tr', null,
        h('td', { class: 'center' }, h('input', { type: 'checkbox', checked: sel.has(key(it)), onchange: (e) => { e.target.checked ? sel.add(key(it)) : sel.delete(key(it)); paintInfo(); } })),
        h('td', null, fname(it.file), h('span', { class: 'mono muted' }, T('line {n}', { n: it.line })),
          h('div', null, it.kind === 'exact' ? chip('done', T('same')) : chip('todo', T('changed {pct} %', { pct: Math.round(it.ratio * 100) })), it.tags_differ ? chip('warning', T('other variables')) : null)),
        h('td', { class: 'txt' }, d ? d.after : it.source, d ? h('div', { class: 'muted', style: { fontSize: '12px' } }, d.before) : null),
        h('td', { class: 'txt' }, it.target));
    });
    const t = h('table', { class: 'data' }, h('colgroup', null, [44, 190, 340, 340].map((w) => h('col', { style: { width: w + 'px' } }))),
      h('thead', null, h('tr', null, ['', T('Where'), T('Original (the old one below if it changed)'), T('Translation from the previous version')].map((x) => h('th', null, x)))), h('tbody', null, rows));
    t.style.width = '914px';
    results.replaceChildren(
      h('p', null, T('Found {exact} identical and {near} changed lines of {total}; {none} have no match.', { exact: c.exact, near: c.near, total: c.total, none: c.none })),
      data.limited ? h('div', { class: 'notice warn' }, T('The demo carries over only the first {limit} lines ({total} were found). A licence removes the limit.', { limit: data.limited.limit, total: data.limited.total })) : null,
      h('div', { class: 'toolbar' },
        h('button', { class: 'btn small', onclick: () => { data.items.forEach((it) => sel.add(key(it))); paint(); } }, T('Select all')),
        h('button', { class: 'btn small', onclick: () => { sel.clear(); paint(); } }, T('None')),
        h('button', { class: 'btn small', onclick: () => { sel.clear(); data.items.filter(recommended).forEach((it) => sel.add(key(it))); paint(); } }, T('Only recommended'))),
      data.items.length > 600 ? h('p', { class: 'muted' }, T('Showing the first 600 lines.')) : null,
      h('div', { class: 'tablewrap', style: { maxHeight: '44vh' } }, t));
    paintInfo();
  }
  async function find() {
    if (!path.value.trim()) return toast(T('Enter the path to the folder.'), 'bad');
    results.replaceChildren(h('p', { class: 'muted' }, T('Looking for translations…')));
    try { data = await post('carry/preview', { old: path.value.trim() }); } catch (e) { data = null; results.replaceChildren(h('div', { class: 'notice bad' }, e.message)); return paintInfo(); }
    sel.clear();
    data.items.filter(recommended).forEach((it) => sel.add(key(it)));
    if (!data.items.length) { results.replaceChildren(h('p', null, T('Nothing could be carried over from that folder.'))); return paintInfo(); }
    paint();
  }
  go.onclick = async () => {
    const selections = data.items.filter((it) => sel.has(key(it))).map((it) => ({ file: it.file, line: it.line }));
    try {
      const r = await post('carry/apply', { old: path.value.trim(), selections });
      toast(tn(r.changed, 'Carried over {n} line', 'Carried over {n} lines') + '. ' + T('Undo: “Undo”.'), 'ok', 8000);
      S.files = null; await refreshHeader(); done?.();
    } catch (e) { fail(e); return false; }
  };
  const browse = h('button', { class: 'btn', onclick: async () => {
    try { const r = await post('carry/pick', {}); if (!r.cancelled) { path.value = r.path; find(); } } catch (e) { fail(e); }
  } }, T('Browse…'));
  path.addEventListener('keydown', (e) => { if (e.key === 'Enter') find(); });
  paintInfo();
  dialog({ title: T('Carry the translation over from the previous version'),
    body: h('div', null,
      h('p', { class: 'muted' }, T('The open project is the NEW version of the game. Choose the folder of the OLD, translated version: the translation of every line whose original is the same (or almost the same) is looked up there. Nothing is written until you confirm; the change can be undone.')),
      h('div', { class: 'toolbar' }, path, browse, h('button', { class: 'btn primary', onclick: find }, T('Find'))),
      results),
    buttons: [{ label: T('Cancel') }, { node: h('span', { style: { display: 'contents' } }, info, go) }] });
}
