import { tn, t as T } from './05-i18n.js';
import { $, h, plural } from './00-core.js';
import { post, request } from './10-api.js';
import { S } from './11-state.js';
import { dialog, fail, toast } from './13-feedback.js';
import { empty, fname, loading } from './14-renderers.js';
import { go } from './16-tabs.js';
import { refreshHeader } from './17-project.js';
import { diffSpans } from './23-fixes.js';
/* Find and replace in the translated texts of all or chosen files, with a preview you tick line by line. */
export async function replaceDialog(done, preselected = []) {
  const allFiles = (S.files || []).map((x) => x.name);
  const chosen = new Set(preselected.filter((n) => allFiles.includes(n)));
  const st = { mode: 'plain', scope: chosen.size ? 'chosen' : 'all', armed: false };
  let data = null, seq = 0, timer;
  const sel = new Set();
  const find = h('input', { type: 'text', placeholder: T('What to find'), style: { flex: 1 } });
  const repl = h('input', { type: 'text', placeholder: T('Replace with (may be empty)'), style: { flex: 1 } });
  const caseBox = h('input', { type: 'checkbox' }), protBox = h('input', { type: 'checkbox', checked: true });
  const MODES = [['plain', T('Text')], ['word', T('Whole word')], ['stem', T('By stem (experimental)')], ['regex', T('Regular expression')]];
  const modeSeg = h('div', { class: 'seg' }), scopeBox = h('div'), results = h('div'), info = h('span', { class: 'grow' });
  const go = h('button', { class: 'btn primary', disabled: true }, T('Replace'));
  const MODE_HELP = { plain: T('Finds text anywhere (also inside words).'), word: T('Finds whole words only.'), stem: T('Also finds other forms of the word by its stem (dikmince → dikminci) and keeps their ending. Works well only for words with the same declension – always check the preview.'), regex: T('A Python regular expression; in the replacement you can use \\1, \\2…') };
  const hint = h('div', { class: 'muted' });
  function paintMode() {
    modeSeg.replaceChildren(...MODES.map(([k, l]) => h('button', { class: st.mode === k ? 'on' : '', onclick: () => { st.mode = k; paintMode(); schedule(); } }, l)));
    hint.textContent = MODE_HELP[st.mode];
  }
  function paintScope() {
    const segBtn = (k, l) => h('button', { class: st.scope === k ? 'on' : '', onclick: () => { st.scope = k; paintScope(); schedule(); } }, l);
    const list = h('div', { class: 'filepick' });
    const q = h('input', { type: 'search', class: 'search', placeholder: T('Search files…'), style: { maxWidth: '240px' } });
    const count = h('span', { class: 'muted' }, T('selected {n}', { n: chosen.size }));
    const fill = () => { const t = q.value.toLowerCase(); list.replaceChildren(...allFiles.filter((n) => !t || n.toLowerCase().includes(t)).map((n) => h('label', { class: 'check' }, h('input', { type: 'checkbox', checked: chosen.has(n), onchange: (e) => { e.target.checked ? chosen.add(n) : chosen.delete(n); count.textContent = T('selected {n}', { n: chosen.size }); schedule(); } }), ' ', n))); };
    q.oninput = fill;
    scopeBox.replaceChildren(h('div', { class: 'toolbar' }, h('b', null, T('Where:')), h('div', { class: 'seg' }, segBtn('all', T('All files ({n})', { n: allFiles.length })), segBtn('chosen', T('Only selected files'))),
      st.scope === 'chosen' ? [count, h('button', { class: 'btn small', onclick: () => { allFiles.forEach((n) => chosen.add(n)); fill(); count.textContent = T('selected {n}', { n: chosen.size }); schedule(); } }, T('All')), h('button', { class: 'btn small', onclick: () => { chosen.clear(); fill(); count.textContent = T('selected {n}', { n: 0 }); schedule(); } }, T('None')), q] : null), st.scope === 'chosen' ? list : null);
    if (st.scope === 'chosen') fill();
  }
  const request = () => ({ find: find.value, replace: repl.value, mode: st.mode, case: caseBox.checked, protect: protBox.checked, files: st.scope === 'chosen' ? [...chosen] : [] });
  function schedule() { clearTimeout(timer); timer = setTimeout(preview, 350); }
  async function preview() {
    const my = ++seq;
    if (!find.value) { data = null; sel.clear(); results.replaceChildren(empty(T('Enter the text to find'), T('The preview shows at once; nothing is written until you confirm.'))); paint2(); return; }
    if (st.scope === 'chosen' && !chosen.size) { data = null; results.replaceChildren(empty(T('Choose files'), T('Tick at least one file, or switch to “All files”.'))); paint2(); return; }
    results.replaceChildren(loading());
    let r;
    try { r = await post('replace/preview', request()); } catch (e) { if (my === seq) { data = null; sel.clear(); results.replaceChildren(h('div', { class: 'notice bad' }, e.message)); paint2(); } return; }
    if (my !== seq) return;
    data = r; sel.clear(); r.items.forEach((it, i) => { if (!it.skip) sel.add(i); }); st.armed = false; paintResults();
  }
  function paintResults() {
    if (!data.items.length) { results.replaceChildren(empty(T('Nothing found'), T('The text is not in the translated lines (untranslated lines are not searched).'))); return paint2(); }
    const shown = data.items.slice(0, 500);
    const rows = shown.map((it, i) => { const d = diffSpans(it.before, it.after); return h('tr', null,
      h('td', { class: 'center' }, h('input', { type: 'checkbox', checked: sel.has(i), disabled: !!it.skip, onchange: (e) => { e.target.checked ? sel.add(i) : sel.delete(i); st.armed = false; paint2(); } })),
      h('td', null, fname(it.file), h('span', { class: 'mono muted' }, T('line {n}', { n: it.line }))), h('td', { class: 'txt' }, d.before),
      h('td', { class: 'txt' }, it.skip ? h('span', { class: 'muted' }, it.skip) : d.after), h('td', { class: 'num' }, it.count)); });
    const t = h('table', { class: 'data' }, h('colgroup', null, [44, 200, 330, 330, 50].map((w) => h('col', { style: { width: w + 'px' } }))),
      h('thead', null, h('tr', null, ['', T('Where'), T('Now'), T('After the replacement'), '×'].map((x) => h('th', null, x)))), h('tbody', null, rows));
    t.style.width = '954px';
    results.replaceChildren(h('div', { class: 'toolbar' }, h('span', { class: 'muted' }, T('{lines}, {matches}', { lines: tn(data.total, '{n} line', '{n} lines'), matches: tn(data.matches, '{n} match', '{n} matches') }) + (data.truncated ? ' ' + T('(only the first 2000 shown)') : '')), h('span', { class: 'spacer' }),
      h('button', { class: 'btn small', onclick: () => { shown.forEach((it, i) => { if (!it.skip) sel.add(i); }); paintResults(); } }, T('Select all')), h('button', { class: 'btn small', onclick: () => { sel.clear(); paintResults(); } }, T('None'))),
      data.items.length > 500 ? h('p', { class: 'muted' }, T('Showing the first 500; only the shown ones are replaced.')) : null, h('div', { class: 'tablewrap', style: { maxHeight: '40vh' } }, t));
    paint2();
  }
  function paint2() {
    const n = sel.size;
    go.disabled = !n;
    go.classList.toggle('danger', st.armed);
    go.textContent = st.armed ? tn(n, 'Confirm: replace in {n} line', 'Confirm: replace in {n} lines') : T('Replace ({n})', { n });
    info.textContent = data ? T('Selected {n} of {total}', { n, total: data.applicable }) + (n > 1 ? ' · ' + T('a bulk change must be confirmed twice') : '') : '';
  }
  go.onclick = async () => {
    if (sel.size > 1 && !st.armed) { st.armed = true; paint2(); setTimeout(() => { st.armed = false; paint2(); }, 8000); return false; }
    try {
      const shown = data.items.slice(0, 500);
      const r = await post('replace/apply', { ...request(), selections: [...sel].map((i) => ({ file: shown[i].file, line: shown[i].line })) });
      toast(T('{lines} ({files}). Undo: “Undo the last file change”.', { lines: tn(r.changed, 'Replaced in {n} line', 'Replaced in {n} lines'), files: tn(r.files, '{n} file', '{n} files') }), 'ok', 8000);
      S.files = null; await refreshHeader(); done?.();
    } catch (e) { fail(e); return false; }
  };
  [find, repl].forEach((i) => i.addEventListener('input', schedule));
  [caseBox, protBox].forEach((i) => i.addEventListener('change', schedule));
  paintMode(); paintScope();
  const body = h('div', null,
    h('p', { class: 'muted' }, T('Searches and replaces in already translated text. Tags {i}, variables [name] and \\n sequences are never changed by default. A backup is made before writing; the change can be undone in one step.')),
    h('div', { class: 'row', style: { marginBottom: '8px' } }, find, repl),
    h('div', { class: 'toolbar' }, modeSeg, h('label', { class: 'check' }, caseBox, ' ' + T('Match case')), h('label', { class: 'check' }, protBox, ' ' + T('Leave tags and variables alone'))), hint, scopeBox, results);
  S.dlgDirty = true;
  dialog({ title: T('Find and replace'), body, buttons: [{ label: T('Close') }, { node: h('span', { style: { display: 'contents' } }, info, go) }] });
  setTimeout(() => find.focus(), 0);
  preview();
}

