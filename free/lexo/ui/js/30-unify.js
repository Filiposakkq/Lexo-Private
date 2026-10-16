"use strict";
/* Unify: set ONE chosen translation on the lines you tick. The same words can mean something else in another place, so every line
   of the group is listed (speaker, current translation) and only the ticked ones change. */
function unifyDialog(g, v, loose, bySp, done) {
  const lines = g.variants.flatMap((x) => x.lines.map((l) => ({ ...l, current: x.target })));
  const sel = new Set(lines.filter((l) => l.current !== v.target).map((l) => `${l.file}|${l.line}`));
  const key = (l) => `${l.file}|${l.line}`;
  const info = h('span', { class: 'grow' }), go1 = h('button', { class: 'btn primary', disabled: true });
  const speakers = new Set(v.lines.map((l) => (l.who || '').split(' ')[0]));
  const box = h('div');
  function paint() {
    const rows = lines.map((l) => { const same = l.current === v.target; const d = same ? null : diffSpans(l.current, v.target); return h('tr', null,
      h('td', { class: 'center' }, h('input', { type: 'checkbox', checked: sel.has(key(l)), disabled: same, onchange: (e) => { e.target.checked ? sel.add(key(l)) : sel.delete(key(l)); paint2(); } })),
      h('td', null, fname(l.file), h('span', { class: 'mono muted' }, `řádek ${l.line}`)), h('td', { class: 'who' }, speakerLabel(l.who) || '—'),
      h('td', { class: 'txt' }, same ? h('span', null, fmtText(l.current), ' ', chip('done', 'už je')) : d.before),
      h('td', { class: 'txt' }, same ? '' : d.after),
      h('td', null, h('button', { class: 'btn small ghost', onclick: () => { closeAllDialogs(); go('entries', { file: l.file, line: l.line, status: 'all' }); } }, 'Zobrazit'))); });
    const t = h('table', { class: 'data' }, h('colgroup', null, [44, 200, 110, 240, 240, 90].map((w) => h('col', { style: { width: w + 'px' } }))),
      h('thead', null, h('tr', null, ['', 'Kde', 'Mluvčí', 'Teď', 'Po sjednocení', ''].map((x) => h('th', null, x)))), h('tbody', null, rows));
    t.style.width = '924px';
    box.replaceChildren(h('p', null, 'Originál: ', h('b', null, fmtText(g.source)), ' → nový překlad: ', h('b', null, fmtText(v.target))),
      h('p', { class: 'muted' }, 'Stejný text může mít jiný význam (jiný mluvčí, rod, tón, kontext). Zaškrtni jen řádky, kde to opravdu sedí.'),
      h('div', { class: 'toolbar' },
        h('button', { class: 'btn small', onclick: () => { lines.forEach((l) => { if (l.current !== v.target) sel.add(key(l)); }); paint(); } }, 'Vybrat vše'),
        h('button', { class: 'btn small', onclick: () => { sel.clear(); paint(); } }, 'Nic'),
        h('button', { class: 'btn small', title: 'Jen řádky mluvčích, kteří tuhle variantu používají', onclick: () => { sel.clear(); lines.forEach((l) => { if (l.current !== v.target && speakers.has((l.who || '').split(' ')[0])) sel.add(key(l)); }); paint(); } }, 'Jen stejný mluvčí')),
      h('div', { class: 'tablewrap', style: { maxHeight: '46vh' } }, t));
    paint2();
  }
  function paint2() { const n = sel.size; go1.disabled = !n; go1.textContent = `Sjednotit ${n} ${plural(n, 'řádek', 'řádky', 'řádků')}`; info.textContent = `Vybráno ${n} z ${lines.filter((l) => l.current !== v.target).length} odlišných`; }
  go1.onclick = async () => {
    try {
      const r = await post('consistency/unify', { key: g.key, target: v.target, loose, by_speaker: bySp, lines: lines.filter((l) => sel.has(key(l))).map((l) => ({ file: l.file, line: l.line })) });
      toast(`Sjednoceno: ${r.changed} ${plural(r.changed, 'řádek', 'řádky', 'řádků')}. Zpět: „Vrátit poslední změnu souboru“.`);
      S.files = null; await refreshHeader(); done?.();
    } catch (e) { fail(e); return false; }
  };
  S.dlgDirty = true;
  dialog({ title: 'Sjednotit překlad', body: box, buttons: [{ label: 'Zrušit' }, { node: h('span', { style: { display: 'contents' } }, info, go1) }] });
  paint();
}

