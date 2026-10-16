import { t as T } from './05-i18n.js';
import { $, h } from './00-core.js';
import { get } from './10-api.js';
import { dialog, status } from './13-feedback.js';
import { chip, empty, fmtText, fname, loading } from './14-renderers.js';
import { go } from './16-tabs.js';
/* Every line whose original contains a glossary term, and whether the Czech term is used there. */
export async function occurrencesDialog(code) {
  let only = false;
  const body = h('div');
  async function load() {
    body.replaceChildren(loading());
    let r;
    try { r = await get('glossary/occurrences', { code, missing: only ? '1' : '' }); } catch (e) { return body.replaceChildren(h('div', { class: 'notice bad' }, e.message)); }
    const state = (it) => (it.used === true ? chip('proof', T('used')) : it.used === false ? chip('warning', T('missing')) : chip('none', it.translated ? '—' : T('untranslated')));
    const rows = r.items.map((it) => h('tr', null, h('td', null, state(it)), h('td', null, fname(it.file), h('span', { class: 'mono muted' }, T('line {n}', { n: it.line }))),
      h('td', { class: 'txt' }, fmtText(it.source)), h('td', { class: 'txt' }, fmtText(it.target)),
      h('td', null, h('button', { class: 'btn small', onclick: () => { close(); go('entries', { file: it.file, line: it.line, status: 'all' }); } }, T('Show')))));
    const t = h('table', { class: 'data' }, h('colgroup', null, [90, 230, 300, 300, 90].map((w) => h('col', { style: { width: w + 'px' } }))),
      h('thead', null, h('tr', null, [T('State'), T('Where'), T('Original'), T('Translation'), ''].map((x) => h('th', null, x)))), h('tbody', null, rows));
    t.style.width = '1010px';
    body.replaceChildren(
      h('p', null, h('b', null, code), ' → ', r.czech ? h('b', null, r.czech) : h('span', { class: 'muted' }, T('(the translation is not filled in in the Glossary)'))),
      h('p', { class: 'muted' }, T('Used {found}× · missing {missing}× · untranslated {untranslated}×. Words are compared by stem, so endings do not matter; short or heavily inflected forms may be checked badly.', { found: r.found, missing: r.missing, untranslated: r.untranslated })),
      h('label', { class: 'check' }, h('input', { type: 'checkbox', checked: only, onchange: (e) => { only = e.target.checked; load(); } }), ' ' + T('Only lines where the translated term is missing')),
      r.items.length ? h('div', { class: 'tablewrap', style: { maxHeight: '50vh', marginTop: '8px' } }, t) : empty(T('No occurrences'), only ? T('The term is not missing in the translated lines.') : T('The term does not occur in the original.')),
      r.truncated ? h('p', { class: 'muted' }, T('Showing the first 400 occurrences.')) : null);
  }
  const close = dialog({ title: T('Occurrences of the term'), body, buttons: [{ label: T('Close'), kind: 'primary' }] });
  load();
}

