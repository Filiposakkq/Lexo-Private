import { tn, t as T } from './05-i18n.js';
import { $, h } from './00-core.js';
import { post } from './10-api.js';
import { S, speakerLabel } from './11-state.js';
import { dialog, fail, toast } from './13-feedback.js';
import { chip, empty, fmtText, fname, loading } from './14-renderers.js';
/* Approve lines by a simple rule (symbols only, interjections, names from the glossary). Goes through the normal approval. */
export async function ruleApproveDialog(done, preselected = []) {
  const rules = { symbols: true, interjections: true, names: true };
  const allFiles = (S.files || []).map((x) => x.name);
  const chosen = new Set(preselected.filter((n) => allFiles.includes(n)));
  let scope = chosen.size ? 'chosen' : 'all';
  let data = null, armed = false;
  const sel = new Set();
  const scopeBox = h('div'), results = h('div'), body = h('div', null, scopeBox, results), info = h('span', { class: 'grow' });
  const foot = h('button', { class: 'btn primary', disabled: true });
  const RULE_LABEL = { symbols: T('symbols only'), interjections: T('interjections'), names: T('name') };
  async function load() {
    if (scope === 'chosen' && !chosen.size) { data = null; results.replaceChildren(empty(T('Choose files'), T('Tick at least one file, or switch to “All files”.'))); foot.disabled = true; info.textContent = ''; return; }
    results.replaceChildren(loading());
    try { data = await post('approve/rules', { rules: Object.keys(rules).filter((k) => rules[k]), files: scope === 'chosen' ? [...chosen] : [] }); }
    catch (e) { return results.replaceChildren(h('div', { class: 'notice bad' }, e.message)); }
    sel.clear(); data.items.forEach((it, i) => { if (it.selected) sel.add(i); });
    armed = false; paint();
  }
  function paint() {
    const checks = Object.entries(data.rules).map(([k, label]) => h('label', { class: 'check', style: { display: 'block' } },
      h('input', { type: 'checkbox', checked: rules[k], onchange: (e) => { rules[k] = e.target.checked; load(); } }), ' ', label, h('span', { class: 'muted' }, `  (${data.counts.by_rule[k] || 0})`)));
    const rows = data.items.slice(0, 500).map((it, i) => h('tr', null,
      h('td', { class: 'center' }, h('input', { type: 'checkbox', checked: sel.has(i), disabled: !it.selectable, onchange: (e) => { e.target.checked ? sel.add(i) : sel.delete(i); armed = false; paint2(); } })),
      h('td', null, chip('none', RULE_LABEL[it.rule])), h('td', null, fname(it.file), h('span', { class: 'mono muted' }, T('line {n}', { n: it.line }))),
      h('td', { class: 'who' }, speakerLabel(it.who) || '—'), h('td', { class: 'txt' }, fmtText(it.text)), h('td', { class: 'muted' }, it.notes.join('; '))));
    const t = h('table', { class: 'data' }, h('colgroup', null, [40, 100, 250, 110, 280, 100].map((w) => h('col', { style: { width: w + 'px' } }))),
      h('thead', null, h('tr', null, ['', T('Rule'), T('Where'), T('Speaker'), T('Text'), ''].map((x) => h('th', null, x)))), h('tbody', null, rows));
    t.style.width = '880px';
    results.replaceChildren(
      h('p', { class: 'muted' }, T('Lines that need no translation because they are the same in the target language. Nothing changes until you confirm. An approval can be undone with Ctrl+Z. Names are taken from the Glossary – the more names you have there, the more lines the rule finds.')),
      h('div', null, checks),
      data.items.length > 500 ? h('p', { class: 'muted' }, T('Showing the first 500 of {total}; only the shown ones are approved.', { total: data.items.length })) : null,
      data.items.length ? h('div', { class: 'tablewrap', style: { maxHeight: '44vh', marginTop: '8px' } }, t) : empty(T('Nothing found'), T('No untranslated line matches the selected rules.')));
    paint2();
  }
  function paint2() {
    const n = sel.size;
    foot.disabled = !n;
    foot.textContent = armed ? tn(n, 'Confirm: approve {n} line', 'Confirm: approve {n} lines') : T('Approve selected ({n})', { n });
    foot.classList.toggle('danger', armed);
    info.textContent = T('Found {total} · selected {n}', { total: data.counts.total, n }) + (n > 1 ? ' · ' + T('a bulk change must be confirmed twice') : '');
    foot.onclick = async () => {
      if (n > 1 && !armed) { armed = true; paint2(); setTimeout(() => { armed = false; paint2(); }, 8000); return false; }
      try {
        const res = await post('approve/apply', { selections: [...sel].map((i) => ({ file: data.items[i].file, key: data.items[i].key })), on: true });
        toast(tn(res.changed, 'Approved {n} line', 'Approved {n} lines') + (res.skipped.length ? ', ' + T('skipped {n}', { n: res.skipped.length }) : '') + '. ' + T('Undo: Ctrl+Z'), res.skipped.length ? 'bad' : 'ok');
        close(); done?.();
      } catch (e) { fail(e); }
      return false;
    };
  }
  function paintScope() {
    const segBtn = (k, l) => h('button', { class: scope === k ? 'on' : '', onclick: () => { scope = k; paintScope(); load(); } }, l);
    const list = h('div', { class: 'filepick' });
    const q = h('input', { type: 'search', class: 'search', placeholder: T('Search files…'), style: { maxWidth: '260px' } });
    const fill = () => {
      const t = q.value.toLowerCase();
      list.replaceChildren(...allFiles.filter((n) => !t || n.toLowerCase().includes(t)).map((n) => h('label', { class: 'check' },
        h('input', { type: 'checkbox', checked: chosen.has(n), onchange: (e) => { e.target.checked ? chosen.add(n) : chosen.delete(n); count.textContent = T('selected {n}', { n: chosen.size }); clearTimeout(S.ruleTimer); S.ruleTimer = setTimeout(load, 350); } }), ' ', n)));
    };
    const count = h('span', { class: 'muted' }, T('selected {n}', { n: chosen.size }));
    q.oninput = fill;
    const setAll = (on) => { allFiles.filter((n) => !q.value || n.toLowerCase().includes(q.value.toLowerCase())).forEach((n) => (on ? chosen.add(n) : chosen.delete(n))); fill(); count.textContent = T('selected {n}', { n: chosen.size }); load(); };
    scopeBox.replaceChildren(h('div', { class: 'toolbar' }, h('b', null, T('Where to search:')), h('div', { class: 'seg' }, segBtn('all', T('All files ({n})', { n: allFiles.length })), segBtn('chosen', T('Only selected files'))),
      scope === 'chosen' ? [count, h('button', { class: 'btn small', onclick: () => setAll(true) }, T('All shown')), h('button', { class: 'btn small', onclick: () => setAll(false) }, T('None')), q] : null),
      scope === 'chosen' ? list : null);
    if (scope === 'chosen') fill();
  }
  paintScope();
  S.dlgDirty = true;
  const close = dialog({ title: T('Approve by rule'), body, buttons: [{ label: T('Cancel') }, { node: h('span', { style: { display: 'contents' } }, info, foot) }] });
  load();
}

