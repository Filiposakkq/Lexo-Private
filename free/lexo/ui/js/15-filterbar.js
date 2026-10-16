"use strict";
/* ---------------------------------------------------------------- filter bar pieces (shared by all tabs) */
const filterGroup = (label, ...controls) => h('label', { class: 'fgroup' }, h('span', { class: 'flabel' }, label), ...controls);
/* A row of "pills" (one per option, with a count) – the main filter of a list. */
function pillRow(options, current, counts, onPick) {
  return options.map(([k, label]) => {
    const n = counts ? counts[k] : undefined;
    return h('button', { class: 'pill' + (current === k ? ' on' : '') + (n === 0 && current !== k ? ' zero' : ''), 'aria-pressed': String(current === k), onclick: () => onPick(k) },
      label, n !== undefined && n !== null ? h('span', { class: 'pc' }, fmtN(n)) : null);
  });
}
/* "Active filters" row: one removable chip per narrowing filter plus a reset. items: [[text, clearFn]] */
function activeChips(items, resetAll) {
  if (!items.length) return [];
  return [h('span', { class: 'flabel' }, 'Aktivní filtry:'),
    ...items.map(([text, clear]) => h('span', { class: 'fchip' }, text, h('button', { title: 'Zrušit tento filtr', 'aria-label': 'Zrušit filtr: ' + text, onclick: clear }, '✕'))),
    h('button', { class: 'btn small ghost', onclick: resetAll }, 'Zrušit všechny filtry')];
}

