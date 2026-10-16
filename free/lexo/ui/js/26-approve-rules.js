"use strict";
/* Approve lines by a simple rule (symbols only, interjections, names from the glossary). Goes through the normal approval. */
async function ruleApproveDialog(done, preselected = []) {
  const rules = { symbols: true, interjections: true, names: true };
  const allFiles = (S.files || []).map((x) => x.name);
  const chosen = new Set(preselected.filter((n) => allFiles.includes(n)));
  let scope = chosen.size ? 'chosen' : 'all';
  let data = null, armed = false;
  const sel = new Set();
  const scopeBox = h('div'), results = h('div'), body = h('div', null, scopeBox, results), info = h('span', { class: 'grow' });
  const foot = h('button', { class: 'btn primary', disabled: true });
  const RULE_LABEL = { symbols: 'jen znaky', interjections: 'citoslovce', names: 'jméno' };
  async function load() {
    if (scope === 'chosen' && !chosen.size) { data = null; results.replaceChildren(empty('Vyber soubory', 'Zaškrtni aspoň jeden soubor, nebo přepni na „Všechny soubory“.')); foot.disabled = true; info.textContent = ''; return; }
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
      h('td', null, chip('none', RULE_LABEL[it.rule])), h('td', null, fname(it.file), h('span', { class: 'mono muted' }, `řádek ${it.line}`)),
      h('td', { class: 'who' }, speakerLabel(it.who) || '—'), h('td', { class: 'txt' }, fmtText(it.text)), h('td', { class: 'muted' }, it.notes.join('; '))));
    const t = h('table', { class: 'data' }, h('colgroup', null, [40, 100, 250, 110, 280, 100].map((w) => h('col', { style: { width: w + 'px' } }))),
      h('thead', null, h('tr', null, ['', 'Pravidlo', 'Kde', 'Mluvčí', 'Text', ''].map((x) => h('th', null, x)))), h('tbody', null, rows));
    t.style.width = '880px';
    results.replaceChildren(
      h('p', { class: 'muted' }, 'Řádky, které se překládat nemusí, protože jsou v češtině stejné. Nic se nemění, dokud nepotvrdíš. Schválení lze vrátit přes Ctrl+Z. Jména se berou ze Slovníku – čím víc jmen tam máš, tím víc řádků pravidlo najde.'),
      h('div', null, checks),
      data.items.length > 500 ? h('p', { class: 'muted' }, `Zobrazeno prvních 500 z ${data.items.length}; schválí se jen zobrazené.`) : null,
      data.items.length ? h('div', { class: 'tablewrap', style: { maxHeight: '44vh', marginTop: '8px' } }, t) : empty('Nic nenalezeno', 'Žádný nepřeložený řádek nevyhovuje vybraným pravidlům.'));
    paint2();
  }
  function paint2() {
    const n = sel.size;
    foot.disabled = !n;
    foot.textContent = armed ? `Potvrdit: schválit ${n} řádků` : `Schválit vybrané (${n})`;
    foot.classList.toggle('danger', armed);
    info.textContent = `Nalezeno ${data.counts.total} · vybráno ${n}` + (n > 1 ? ' · hromadná změna se musí potvrdit dvakrát' : '');
    foot.onclick = async () => {
      if (n > 1 && !armed) { armed = true; paint2(); setTimeout(() => { armed = false; paint2(); }, 8000); return false; }
      try {
        const res = await post('approve/apply', { selections: [...sel].map((i) => ({ file: data.items[i].file, key: data.items[i].key })), on: true });
        toast(`Schváleno ${res.changed} řádků` + (res.skipped.length ? `, přeskočeno ${res.skipped.length}` : '') + '. Zpět: Ctrl+Z', res.skipped.length ? 'bad' : 'ok');
        close(); done?.();
      } catch (e) { fail(e); }
      return false;
    };
  }
  function paintScope() {
    const segBtn = (k, l) => h('button', { class: scope === k ? 'on' : '', onclick: () => { scope = k; paintScope(); load(); } }, l);
    const list = h('div', { class: 'filepick' });
    const q = h('input', { type: 'search', class: 'search', placeholder: 'Hledat soubor…', style: { maxWidth: '260px' } });
    const fill = () => {
      const t = q.value.toLowerCase();
      list.replaceChildren(...allFiles.filter((n) => !t || n.toLowerCase().includes(t)).map((n) => h('label', { class: 'check' },
        h('input', { type: 'checkbox', checked: chosen.has(n), onchange: (e) => { e.target.checked ? chosen.add(n) : chosen.delete(n); count.textContent = `vybráno ${chosen.size}`; clearTimeout(S.ruleTimer); S.ruleTimer = setTimeout(load, 350); } }), ' ', n)));
    };
    const count = h('span', { class: 'muted' }, `vybráno ${chosen.size}`);
    q.oninput = fill;
    const setAll = (on) => { allFiles.filter((n) => !q.value || n.toLowerCase().includes(q.value.toLowerCase())).forEach((n) => (on ? chosen.add(n) : chosen.delete(n))); fill(); count.textContent = `vybráno ${chosen.size}`; load(); };
    scopeBox.replaceChildren(h('div', { class: 'toolbar' }, h('b', null, 'Kde hledat:'), h('div', { class: 'seg' }, segBtn('all', `Všechny soubory (${allFiles.length})`), segBtn('chosen', 'Jen vybrané soubory')),
      scope === 'chosen' ? [count, h('button', { class: 'btn small', onclick: () => setAll(true) }, 'Vše zobrazené'), h('button', { class: 'btn small', onclick: () => setAll(false) }, 'Nic'), q] : null),
      scope === 'chosen' ? list : null);
    if (scope === 'chosen') fill();
  }
  paintScope();
  S.dlgDirty = true;
  const close = dialog({ title: 'Schválit podle pravidla', body, buttons: [{ label: 'Zrušit' }, { node: h('span', { style: { display: 'contents' } }, info, foot) }] });
  load();
}

