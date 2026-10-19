import { labels, tn, t as T, tx } from './05-i18n.js';
import { $, h, plural } from './00-core.js';
import { post } from './10-api.js';
import { S } from './11-state.js';
import { dialog, fail, toast } from './13-feedback.js';
import { chip, fname } from './14-renderers.js';
import { go } from './16-tabs.js';
import { refreshHeader } from './17-project.js';
import { requireFeature } from './38-pro.js';
/* Fix speaker / attributes / trailing code from the original (the only operation that edits the .rpy files). */
/* Shows what changed between two texts: common start / end stay plain, the middle is marked. */
export function diffSpans(before, after) {
  const n = Math.min(before.length, after.length);
  let i = 0;
  while (i < n && before[i] === after[i]) i++;
  let j = 0;
  while (j < n - i && before[before.length - 1 - j] === after[after.length - 1 - j]) j++;
  const part = (str, cls) => h('span', null, str.slice(0, i), h('mark', { class: cls }, str.slice(i, str.length - j) || '∅'), str.slice(str.length - j));
  return { before: part(before, 'diff-del'), after: part(after, 'diff-add') };
}

/* Safe fixes: speaker / attributes from the original, double spaces, typo hints. Every line is ticked or unticked
   by the user; doubtful ones start unticked. Variables and tags are never offered (their order may legitimately differ). */
const FIX_KIND_LABEL = labels({ speaker: 'Speaker / attributes', spaces: 'Double spaces', typo: 'Typos' });
const FIX_KIND_HELP = labels({
  speaker: 'The speaker, attributes and the code after the text are copied from the original (the line with #). The translation text is not changed.',
  spaces: 'Several spaces in a row between words become one (only when the original does not have them either).',
  typo: 'Suggestions from the dictionary of your own translation. Short and uncertain words are unticked because they may be a false alarm (byt / být). Tick only what you really want.',
});
export async function fixDialog({ items = [], file = '', kinds = ['speaker', 'spaces', 'typo'] }, done) {
  if (!await requireFeature('fixes.safe')) return;
  let data;
  try { data = await post('fix/preview', { items, file, kinds }); } catch (e) { return fail(e); }
  if (!data.total) return toast(T('Nothing to fix.'), 'ok');
  const key = (it) => `${it.file}|${it.line}|${it.kind}`;
  const sel = new Set(data.items.filter((it) => it.default && !it.skip).map(key));
  let only = kinds.length === 1 ? kinds[0] : '';
  const head = h('div'), listBox = h('div'), info = h('span', { class: 'grow' });
  const go = h('button', { class: 'btn primary', disabled: true });
  const visible = () => data.items.filter((it) => !only || it.kind === only).slice(0, 600);
  function paint() {
    const seg = h('div', { class: 'seg' }, h('button', { class: only === '' ? 'on' : '', onclick: () => { only = ''; paint(); } }, T('All ({n})', { n: data.total })),
      ...Object.keys(data.counts).filter((k) => data.counts[k]).map((k) => h('button', { class: only === k ? 'on' : '', onclick: () => { only = k; paint(); } }, `${FIX_KIND_LABEL[k]} (${data.counts[k]})`)));
    const vis = visible();
    head.replaceChildren(
      only ? h('p', null, FIX_KIND_HELP[only]) : h('p', { class: 'muted' }, tx('Choose what to fix. A backup of the file is made in the subfolder {folder} of the local project data before writing; the change can be undone with the button “Undo the last file change”. The files must be saved in the editor.', { folder: h('b', null, 'backups') })),
      h('div', { class: 'toolbar' }, seg, h('span', { class: 'spacer' }),
        h('button', { class: 'btn small', onclick: () => { vis.forEach((it) => { if (!it.skip) sel.add(key(it)); }); paint(); } }, T('Select all')),
        h('button', { class: 'btn small', onclick: () => { vis.forEach((it) => sel.delete(key(it))); paint(); } }, T('None')),
        h('button', { class: 'btn small', title: T('Only what is safe (uncertain suggestions stay unticked)'), onclick: () => { vis.forEach((it) => { if (it.default && !it.skip) sel.add(key(it)); else sel.delete(key(it)); }); paint(); } }, T('Only recommended'))));
    const rows = vis.map((it) => {
      const d = it.code ? null : diffSpans(it.before, it.after);
      return h('tr', null,
        h('td', { class: 'center' }, h('input', { type: 'checkbox', checked: sel.has(key(it)), disabled: !!it.skip, onchange: (e) => { e.target.checked ? sel.add(key(it)) : sel.delete(key(it)); paint2(); } })),
        h('td', null, fname(it.file), h('span', { class: 'mono muted' }, T('line {n}', { n: it.line })), only ? null : h('div', null, chip('none', FIX_KIND_LABEL[it.kind]))),
        h('td', { class: it.code ? 'mono txt' : 'txt' }, d ? d.before : it.before),
        h('td', { class: it.code ? 'mono txt' : 'txt' }, it.skip ? h('span', { class: 'muted' }, it.skip) : (d ? d.after : it.after), !it.code && it.note ? h('div', { class: 'muted', style: { fontSize: '12px' } }, it.note) : null));
    });
    const t = h('table', { class: 'data' }, h('colgroup', null, [44, 190, 340, 340].map((w) => h('col', { style: { width: w + 'px' } }))),
      h('thead', null, h('tr', null, ['', T('Where'), T('Now'), T('After the fix')].map((x) => h('th', null, x)))), h('tbody', null, rows));
    t.style.width = '914px';
    listBox.replaceChildren(vis.length < data.items.filter((it) => !only || it.kind === only).length ? h('p', { class: 'muted' }, T('Showing the first 600 lines.')) : null, h('div', { class: 'tablewrap', style: { maxHeight: '46vh' } }, t));
    paint2();
  }
  function paint2() {
    const n = sel.size;
    go.disabled = !n;
    go.textContent = tn(n, 'Fix {n} line', 'Fix {n} lines');
    info.textContent = T('Selected {n} of {total} applicable', { n, total: data.fixable });
  }
  go.onclick = async () => {
    const selections = data.items.filter((it) => sel.has(key(it))).map((it) => ({ file: it.file, line: it.line, kind: it.kind }));
    try {
      const r = await post('fix/apply', { selections });
      toast(tn(r.fixed, 'Fixed {n} line', 'Fixed {n} lines') + (r.skipped.length ? ', ' + T('skipped {n}', { n: r.skipped.length }) : '') + '. ' + T('Undo: “Undo the last file change”.'), r.skipped.length ? 'bad' : 'ok', 8000);
      S.files = null; await refreshHeader(); done?.();
    } catch (e) { fail(e); return false; }
  };
  const body = h('div', null, head, listBox);
  paint();
  dialog({ title: T('Fix'), body, buttons: [{ label: T('Cancel') }, { node: h('span', { style: { display: 'contents' } }, info, go) }] });
}

