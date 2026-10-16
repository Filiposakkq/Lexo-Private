"use strict";
/* Every line whose original contains a glossary term, and whether the Czech term is used there. */
async function occurrencesDialog(code) {
  let only = false;
  const body = h('div');
  async function load() {
    body.replaceChildren(loading());
    let r;
    try { r = await get('glossary/occurrences', { code, missing: only ? '1' : '' }); } catch (e) { return body.replaceChildren(h('div', { class: 'notice bad' }, e.message)); }
    const state = (it) => (it.used === true ? chip('proof', 'použito') : it.used === false ? chip('warning', 'chybí') : chip('none', it.translated ? '—' : 'nepřeloženo'));
    const rows = r.items.map((it) => h('tr', null, h('td', null, state(it)), h('td', null, fname(it.file), h('span', { class: 'mono muted' }, `řádek ${it.line}`)),
      h('td', { class: 'txt' }, fmtText(it.source)), h('td', { class: 'txt' }, fmtText(it.target)),
      h('td', null, h('button', { class: 'btn small', onclick: () => { close(); go('entries', { file: it.file, line: it.line, status: 'all' }); } }, 'Zobrazit'))));
    const t = h('table', { class: 'data' }, h('colgroup', null, [90, 230, 300, 300, 90].map((w) => h('col', { style: { width: w + 'px' } }))),
      h('thead', null, h('tr', null, ['Stav', 'Kde', 'Originál', 'Překlad', ''].map((x) => h('th', null, x)))), h('tbody', null, rows));
    t.style.width = '1010px';
    body.replaceChildren(
      h('p', null, h('b', null, code), ' → ', r.czech ? h('b', null, r.czech) : h('span', { class: 'muted' }, '(český překlad není ve Slovníku vyplněný)')),
      h('p', { class: 'muted' }, `Použito ${r.found}× · chybí ${r.missing}× · nepřeloženo ${r.untranslated}×. Porovnává se podle kmene slova, takže koncovky nevadí; krátké nebo hodně ohnuté tvary může hlídat špatně.`),
      h('label', { class: 'check' }, h('input', { type: 'checkbox', checked: only, onchange: (e) => { only = e.target.checked; load(); } }), ' Jen řádky, kde český termín chybí'),
      r.items.length ? h('div', { class: 'tablewrap', style: { maxHeight: '50vh', marginTop: '8px' } }, t) : empty('Žádné výskyty', only ? 'V přeložených řádcích termín nechybí.' : 'V originále se termín nevyskytuje.'),
      r.truncated ? h('p', { class: 'muted' }, 'Zobrazeno prvních 400 výskytů.') : null);
  }
  const close = dialog({ title: 'Výskyty termínu', body, buttons: [{ label: 'Zavřít', kind: 'primary' }] });
  load();
}

