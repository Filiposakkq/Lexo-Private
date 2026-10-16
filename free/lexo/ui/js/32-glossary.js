"use strict";
/* ================================================================ GLOSSARY */
async function viewGlossary() {
  let data = await get('glossary');
  S.gl = data.names;
  const f = pref('glossary', {});
  const root = h('div');
  const body = h('div');
  const counter = h('span', { class: 'muted' });
  const search = h('input', { type: 'search', class: 'search', id: 'search', placeholder: 'Hledat kód nebo jméno… (Ctrl+F)', value: f.q || '',
    oninput: (ev) => { setPref('glossary.q', ev.target.value); draw(); } });
  const seg = h('div', { class: 'seg' }, [['all', 'Vše'], ['unnamed', 'Bez jména'], ['speaker', 'Mluvčí'], ['term', 'Termíny']].map(([k, l]) =>
    h('button', { class: (f.filter || 'all') === k ? 'on' : '', onclick: (ev) => { setPref('glossary.filter', k); seg.querySelectorAll('button').forEach((b) => b.classList.toggle('on', b === ev.currentTarget)); draw(); } }, l)));
  root.append(
    h('p', { class: 'muted' }, 'Slovník jmen. Mluvčí v souborech jsou jen zkratky (mc, de, ji…); zde jim přiřadíš celé jméno. ',
      'Zobrazení v aplikaci (Řádky, detail, okolí řádku) přepínáš vpravo: kód / celé jméno / obojí. Jméno se zapisuje automaticky po opuštění pole. Zkratky se hledají v souborech samy – nic se nevymýšlí.'),
    h('div', { class: 'toolbar filterbar' }, filterGroup('Typ', seg), search, speakerModeSelect(() => toast('Zobrazení mluvčích změněno.', 'ok', 1500)),
      h('button', { class: 'btn', onclick: () => addDialog() }, '+ Přidat položku (termín / jméno)'),
      h('button', { class: 'btn', title: 'Načte jména postav z characters.rpy hry', onclick: () => importDialog() }, 'Importovat z characters.rpy…'), h('span', { class: 'spacer' }), counter), body);

  async function save(item, quiet) {
    try { data = await post('glossary/set', { items: [item] }); S.gl = data.names; if (!quiet) toast('Uloženo ✓', 'ok', 1500); counter.textContent = stats(); }
    catch (e) { fail(e); }
  }
  const stats = () => `${data.items.length} položek · bez jména: ${data.unnamed}`;
  function draw() {
    const q = (pref('glossary.q', '') || '').toLowerCase(), flt = pref('glossary.filter', 'all');
    const rows = data.items.filter((i) => (!q || i.code.toLowerCase().includes(q) || i.name.toLowerCase().includes(q) || i.note.toLowerCase().includes(q)) &&
      (flt === 'all' || (flt === 'unnamed' && i.type === 'speaker' && i.in_project && !i.name) || (flt === 'speaker' && i.type === 'speaker') || (flt === 'term' && i.type === 'term')));
    counter.textContent = stats();
    if (!rows.length) return body.replaceChildren(empty('Nic k zobrazení', flt === 'all' ? 'V projektu nebyli nalezeni žádní mluvčí.' : 'Změň filtr nebo hledaný text.'));
    const t = buildTable({ key: 'glossary', cols: [{ k: 'type', w: 100, label: 'Typ' }, { k: 'code', w: 150, label: 'Kód / termín' }, { k: 'name', w: 280, label: 'Celé jméno / překlad' },
      { k: 'note', w: 260, label: 'Poznámka' }, { k: 'count', w: 80, label: 'Řádků' }, { k: 'ex', w: 340, label: 'Příklad' }, { k: 'act', w: 190, label: '', resize: false }] });
    for (const it of rows) {
      const name = h('input', { type: 'text', value: it.name, placeholder: it.type === 'speaker' ? 'např. celé jméno postavy' : 'překlad / jméno', style: { width: '100%' },
        onchange: (ev) => { it.name = ev.target.value.trim(); save({ code: it.code, name: it.name, type: it.type, note: it.note }); } });
      const note = h('input', { type: 'text', value: it.note, style: { width: '100%' },
        onchange: (ev) => { it.note = ev.target.value.trim(); save({ code: it.code, name: it.name, type: it.type, note: it.note }); } });
      t.tbody.append(h('tr', null, h('td', null, chip(it.type === 'speaker' ? 'info' : 'none', it.type === 'speaker' ? 'mluvčí' : 'termín')),
        h('td', { class: 'mono' }, it.code), h('td', null, name), h('td', null, note), h('td', { class: 'num' }, it.count || '–'),
        h('td', { class: 'txt muted' }, it.example), h('td', null,
          it.type === 'term' ? h('button', { class: 'btn small', title: 'Kde se termín v originále vyskytuje a jestli je přeložený podle slovníku', onclick: () => occurrencesDialog(it.code) }, 'Výskyty') : null, ' ',
          it.count ? h('button', { class: 'btn small', title: 'Zobrazit řádky tohoto mluvčího', onclick: () => go('entries', { file: '', q: it.code, status: 'all' }) }, 'Řádky') : null, ' ',
          (it.name || it.note) ? h('button', { class: 'btn small ghost', title: 'Smazat záznam ze slovníku', onclick: async () => {
            try { data = await post('glossary/delete', { codes: [it.code] }); S.gl = data.names; draw(); toast('Záznam smazán. Zpět: Ctrl+Z'); } catch (e) { fail(e); } } }, 'Smazat') : null)));
    }
    body.replaceChildren(h('div', { class: 'tablewrap' }, t.table));
  }
  async function importDialog() {
    let pv;
    try { pv = await post('glossary/import-preview', { path: '' }); } catch (e) { return fail(e); }
    const pathIn = h('input', { type: 'text', value: pv.path, placeholder: 'složka hry nebo soubor characters.rpy', style: { flex: 1 } });
    const holder = h('div');
    let rows = [];
    const apply = h('button', { class: 'btn primary', disabled: true }, 'Zapsat do slovníku');
    const upd = () => { const n = rows.filter((r) => r.on).length; apply.disabled = !n; apply.textContent = n ? `Zapsat do slovníku (${n})` : 'Zapsat do slovníku'; };
    const STATE = { new: ['Nové', 'tr'], same: ['Shodné', 'none'], conflict: ['Jiné než ve slovníku', 'error'], dynamic: ['Zadává hráč', 'none'] };
    function paint(data) {
      rows = data.items.map((i) => ({ ...i, on: i.state === 'new' }));
      if (!rows.length) { holder.replaceChildren(empty('Nic k importu', data.message || 'V souboru nejsou žádné řádky define … = Character("…").')); return upd(); }
      const t = buildTable({ key: 'glimport', cols: [{ k: 'on', w: 44, label: '', resize: false }, { k: 'code', w: 110, label: 'Kód' }, { k: 'name', w: 240, label: 'Jméno v characters.rpy' },
        { k: 'cur', w: 200, label: 'Dosud ve slovníku' }, { k: 'src', w: 230, label: 'Soubor' }, { k: 'st', w: 190, label: 'Stav' }] });
      for (const r of rows) {
        t.tbody.append(h('tr', null, h('td', { class: 'center' }, h('input', { type: 'checkbox', checked: r.on, disabled: r.state === 'dynamic' || r.state === 'same',
          title: r.state === 'conflict' ? 'Přepíše jméno, které máš ve slovníku' : '', onchange: (e) => { r.on = e.target.checked; upd(); } })),
          h('td', { class: 'mono' }, r.code), h('td', null, r.name || h('span', { class: 'muted' }, r.note)), h('td', { class: 'muted' }, r.current || '–'), h('td', { class: 'mono muted', style: { fontSize: '12px' } }, r.source),
          h('td', null, chip(STATE[r.state][1], STATE[r.state][0]), r.in_project ? null : h('span', { class: 'muted' }, '  (není v překladu)'))));
      }
      holder.replaceChildren(
        h('div', { class: 'muted', style: { marginBottom: '6px' } }, `Načtené soubory (${data.files.length}): `, h('span', { class: 'mono' }, data.files.map((f) => f.split(/[\\/]/).slice(-3).join('/')).join('  ·  '))),
        data.duplicates?.length ? h('div', { class: 'notice warn' }, 'Víckrát definované kódy (Ren\'Py použije poslední definici): ', h('span', { class: 'mono' }, data.duplicates.map((d) => `${d.code}: ${d.names.join(' → ')}`).join(';  '))) : null,
        data.missing?.length ? h('div', { class: 'notice warn' }, `V překladu je ${data.missing.length} kódů mluvčích, které soubory characters*.rpy nedefinují (mohou být definované jinde, nebo jde o překlep): `, h('span', { class: 'mono' }, data.missing.slice(0, 25).join(', ') + (data.missing.length > 25 ? ' …' : ''))) : null,
        h('div', { class: 'tablewrap', style: { maxHeight: '48vh' } }, t.table));
      upd();
    }
    const load = async () => { try { paint(await post('glossary/import-preview', { path: pathIn.value })); } catch (e) { fail(e); } };
    paint(pv);
    apply.onclick = async () => {
      try {
        data = await post('glossary/import-apply', { items: rows.filter((r) => r.on).map((r) => ({ code: r.code, name: r.name })) });
        S.gl = data.names; draw(); toast('Jména importována ✓  (Ctrl+Z vrátí)');
        $('#dlg').close();
      } catch (e) { fail(e); }
    };
    dialog({ title: 'Import jmen z characters.rpy', body: h('div', null,
      h('p', { class: 'muted' }, 'Soubory hry (characters.rpy i season2/scripts/characters_season2.rpy…) se jen čtou. Zaškrtnuté řádky se zapíšou do slovníku; jména, která už ve slovníku máš a liší se, se přepíšou jen když je sám zaškrtneš.'),
      h('div', { class: 'row nowrap' }, pathIn, h('button', { class: 'btn', onclick: load }, 'Načíst')), holder),
      buttons: [{ label: 'Zavřít' }, { node: apply }] });
  }
  function addDialog() {
    const code = h('input', { type: 'text', placeholder: 'kód nebo termín (např. de, Delta Iota Kappa)' });
    const name = h('input', { type: 'text', placeholder: 'celé jméno / překlad' });
    const type = h('select', null, h('option', { value: 'speaker' }, 'Mluvčí (zkratka postavy)'), h('option', { value: 'term' }, 'Termín / jméno / místo'));
    const note = h('input', { type: 'text', placeholder: 'poznámka (volitelné)' });
    dialog({ title: 'Nová položka slovníku', narrow: true, body: h('div', { class: 'form', style: { gridTemplateColumns: '120px 1fr' } },
      h('label', null, 'Typ'), type, h('label', null, 'Kód / termín'), code, h('label', null, 'Jméno'), name, h('label', null, 'Poznámka'), note),
      buttons: [{ label: 'Zrušit' }, { label: 'Přidat', kind: 'primary', onclick: async () => {
        if (!code.value.trim()) { toast('Vyplň kód nebo termín.', 'bad'); return false; }
        await save({ code: code.value, name: name.value, type: type.value, note: note.value }); draw();
      } }] });
  }
  root.afterMount = draw;
  return root;
}

