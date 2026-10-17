import { tn, t as T } from './05-i18n.js';
import { $, h } from './00-core.js';
import { get, post } from './10-api.js';
import { S } from './11-state.js';
import { pref, setPref, speakerModeSelect } from './12-chrome.js';
import { dialog, fail, status, toast } from './13-feedback.js';
import { buildTable, chip, empty } from './14-renderers.js';
import { filterGroup, wordToggle } from './15-filterbar.js';
import { go } from './16-tabs.js';
import { occurrencesDialog } from './27-glossary-terms.js';
/* ================================================================ GLOSSARY */
export async function viewGlossary() {
  let data = await get('glossary');
  S.gl = data.names;
  const f = pref('glossary', {});
  const root = h('div');
  const body = h('div');
  const counter = h('span', { class: 'muted' });
  const search = h('input', { type: 'search', class: 'search', id: 'search', placeholder: T('Search code or name… (Ctrl+F)'), value: f.q || '',
    oninput: (ev) => { setPref('glossary.q', ev.target.value); draw(); } });
  const seg = h('div', { class: 'seg' }, [['all', T('All')], ['unnamed', T('Without a name')], ['speaker', T('Speaker')], ['term', T('Terms')]].map(([k, l]) =>
    h('button', { class: (f.filter || 'all') === k ? 'on' : '', onclick: (ev) => { setPref('glossary.filter', k); seg.querySelectorAll('button').forEach((b) => b.classList.toggle('on', b === ev.currentTarget)); draw(); } }, l)));
  root.append(
    h('p', { class: 'muted' }, T('Glossary of names. Speakers in the files are only abbreviations (mc, de, ji…); here you give them the full name. '),
      T('You switch the display in the app (Lines, detail, surroundings of a line) on the right: code / full name / both. A name is saved automatically when you leave the field. The abbreviations are found in the files by themselves – nothing is made up.')),
    h('div', { class: 'toolbar filterbar' }, filterGroup(T('Type'), seg), search, wordToggle('glossary.word', pref, setPref, draw), speakerModeSelect(() => toast(T('Speaker display changed.'), 'ok', 1500)),
      h('button', { class: 'btn', onclick: () => addDialog() }, T('+ Add an item (term / name)')),
      h('button', { class: 'btn', title: T("Loads the character names from the game's characters.rpy"), onclick: () => importDialog() }, T('Import from characters.rpy…')), h('span', { class: 'spacer' }), counter), body);

  async function save(item, quiet) {
    try { data = await post('glossary/set', { items: [item] }); S.gl = data.names; if (!quiet) toast(T('Saved ✓'), 'ok', 1500); counter.textContent = stats(); }
    catch (e) { fail(e); }
  }
  const stats = () => T('{n} items · without a name: {unnamed}', { n: data.items.length, unnamed: data.unnamed });
  function draw() {
    const q = (pref('glossary.q', '') || '').toLowerCase(), flt = pref('glossary.filter', 'all');
    const word = pref('glossary.word', false) && q ? new RegExp('(?<![\\p{L}\\p{N}_])' + q.replace(/[.*+?^${}()|[\]\\]/g, '\\$&') + '(?![\\p{L}\\p{N}_])', 'iu') : null;
    const has = (text) => (word ? word.test(text) : text.toLowerCase().includes(q));
    const rows = data.items.filter((i) => (!q || has(i.code) || has(i.name) || has(i.note)) &&
      (flt === 'all' || (flt === 'unnamed' && i.type === 'speaker' && i.in_project && !i.name) || (flt === 'speaker' && i.type === 'speaker') || (flt === 'term' && i.type === 'term')));
    counter.textContent = stats();
    if (!rows.length) return body.replaceChildren(empty(T('Nothing to show'), flt === 'all' ? T('No speakers were found in the project.') : T('Change the filter or the search text.')));
    const t = buildTable({ key: 'glossary', cols: [{ k: 'type', w: 100, label: T('Type') }, { k: 'code', w: 150, label: T('Code / term') }, { k: 'name', w: 280, label: T('Full name / translation') },
      { k: 'note', w: 260, label: T('Note') }, { k: 'count', w: 80, label: T('Line count') }, { k: 'ex', w: 340, label: T('Example') }, { k: 'act', w: 190, label: '', resize: false }] });
    for (const it of rows) {
      const name = h('input', { type: 'text', value: it.name, placeholder: it.type === 'speaker' ? T('e.g. the full name of the character') : T('translation / name'), style: { width: '100%' },
        onchange: (ev) => { it.name = ev.target.value.trim(); save({ code: it.code, name: it.name, type: it.type, note: it.note }); } });
      const note = h('input', { type: 'text', value: it.note, style: { width: '100%' },
        onchange: (ev) => { it.note = ev.target.value.trim(); save({ code: it.code, name: it.name, type: it.type, note: it.note }); } });
      t.tbody.append(h('tr', null, h('td', null, chip(it.type === 'speaker' ? 'info' : 'none', it.type === 'speaker' ? T('speaker') : T('term'))),
        h('td', { class: 'mono' }, it.code), h('td', null, name), h('td', null, note), h('td', { class: 'num' }, it.count || '–'),
        h('td', { class: 'txt muted' }, it.example), h('td', null,
          it.type === 'term' ? h('button', { class: 'btn small', title: T('Where the term occurs in the original and whether it is translated according to the glossary'), onclick: () => occurrencesDialog(it.code) }, T('Occurrences')) : null, ' ',
          it.count ? h('button', { class: 'btn small', title: T('Show the lines of this speaker'), onclick: () => go('entries', { file: '', q: it.code, status: 'all' }) }, T('Lines')) : null, ' ',
          (it.name || it.note) ? h('button', { class: 'btn small ghost', title: T('Delete the entry from the glossary'), onclick: async () => {
            try { data = await post('glossary/delete', { codes: [it.code] }); S.gl = data.names; draw(); toast(T('Entry deleted. Undo: Ctrl+Z')); } catch (e) { fail(e); } } }, T('Delete')) : null)));
    }
    body.replaceChildren(h('div', { class: 'tablewrap' }, t.table));
  }
  async function importDialog() {
    let pv;
    try { pv = await post('glossary/import-preview', { path: '' }); } catch (e) { return fail(e); }
    const pathIn = h('input', { type: 'text', value: pv.path, placeholder: T('game folder or the characters.rpy file'), style: { flex: 1 } });
    const holder = h('div');
    let rows = [];
    const apply = h('button', { class: 'btn primary', disabled: true }, T('Write to the glossary'));
    const upd = () => { const n = rows.filter((r) => r.on).length; apply.disabled = !n; apply.textContent = n ? T('Write to the glossary ({n})', { n }) : T('Write to the glossary'); };
    const STATE = { new: [T('New'), 'tr'], same: [T('Matching'), 'none'], conflict: [T('Different from the glossary'), 'error'], dynamic: [T('Entered by the player'), 'none'] };
    function paint(data) {
      rows = data.items.map((i) => ({ ...i, on: i.state === 'new' }));
      if (!rows.length) { holder.replaceChildren(empty(T('Nothing to import'), data.message || T('There are no define … = Character("…") lines in the file.'))); return upd(); }
      const t = buildTable({ key: 'glimport', cols: [{ k: 'on', w: 44, label: '', resize: false }, { k: 'code', w: 110, label: T('Code') }, { k: 'name', w: 240, label: T('Name in characters.rpy') },
        { k: 'cur', w: 200, label: T('In the glossary so far') }, { k: 'src', w: 230, label: T('File') }, { k: 'st', w: 190, label: T('State') }] });
      for (const r of rows) {
        t.tbody.append(h('tr', null, h('td', { class: 'center' }, h('input', { type: 'checkbox', checked: r.on, disabled: r.state === 'dynamic' || r.state === 'same',
          title: r.state === 'conflict' ? T('Overwrites the name you have in the glossary') : '', onchange: (e) => { r.on = e.target.checked; upd(); } })),
          h('td', { class: 'mono' }, r.code), h('td', null, r.name || h('span', { class: 'muted' }, r.note)), h('td', { class: 'muted' }, r.current || '–'), h('td', { class: 'mono muted', style: { fontSize: '12px' } }, r.source),
          h('td', null, chip(STATE[r.state][1], STATE[r.state][0]), r.in_project ? null : h('span', { class: 'muted' }, T('  (not in the translation)')))));
      }
      holder.replaceChildren(
        h('div', { class: 'muted', style: { marginBottom: '6px' } }, T('Loaded files ({n}): ', { n: data.files.length }), h('span', { class: 'mono' }, data.files.map((f) => f.split(/[\\/]/).slice(-3).join('/')).join('  ·  '))),
        data.duplicates?.length ? h('div', { class: 'notice warn' }, T("Codes defined several times (Ren'Py uses the last definition): "), h('span', { class: 'mono' }, data.duplicates.map((d) => `${d.code}: ${d.names.join(' → ')}`).join(';  '))) : null,
        data.missing?.length ? h('div', { class: 'notice warn' }, tn(data.missing.length, "The translation has {n} speaker code that the characters*.rpy files do not define (it may be defined elsewhere, or it is a typo): ", "The translation has {n} speaker codes that the characters*.rpy files do not define (they may be defined elsewhere, or it is a typo): "), h('span', { class: 'mono' }, data.missing.slice(0, 25).join(', ') + (data.missing.length > 25 ? ' …' : ''))) : null,
        h('div', { class: 'tablewrap', style: { maxHeight: '48vh' } }, t.table));
      upd();
    }
    const load = async () => { try { paint(await post('glossary/import-preview', { path: pathIn.value })); } catch (e) { fail(e); } };
    paint(pv);
    apply.onclick = async () => {
      try {
        data = await post('glossary/import-apply', { items: rows.filter((r) => r.on).map((r) => ({ code: r.code, name: r.name })) });
        S.gl = data.names; draw(); toast(T('Names imported ✓  (Ctrl+Z undoes)'));
        $('#dlg').close();
      } catch (e) { fail(e); }
    };
    dialog({ title: T('Import of names from characters.rpy'), body: h('div', null,
      h('p', { class: 'muted' }, T('The game files (characters.rpy and season2/scripts/characters_season2.rpy…) are only read. The ticked rows are written to the glossary; names you already have in the glossary that differ are overwritten only if you tick them yourself.')),
      h('div', { class: 'row nowrap' }, pathIn, h('button', { class: 'btn', onclick: load }, T('Load'))), holder),
      buttons: [{ label: T('Close') }, { node: apply }] });
  }
  function addDialog() {
    const code = h('input', { type: 'text', placeholder: T('code or term (e.g. de, Delta Iota Kappa)') });
    const name = h('input', { type: 'text', placeholder: T('full name / translation') });
    const type = h('select', null, h('option', { value: 'speaker' }, T('Speaker (abbreviation of a character)')), h('option', { value: 'term' }, T('Term / name / place')));
    const note = h('input', { type: 'text', placeholder: T('note (optional)') });
    dialog({ title: T('New glossary item'), narrow: true, body: h('div', { class: 'form', style: { gridTemplateColumns: '120px 1fr' } },
      h('label', null, T('Type')), type, h('label', null, T('Code / term')), code, h('label', null, T('Name')), name, h('label', null, T('Note')), note),
      buttons: [{ label: T('Cancel') }, { label: T('Add'), kind: 'primary', onclick: async () => {
        if (!code.value.trim()) { toast(T('Fill in the code or the term.'), 'bad'); return false; }
        await save({ code: code.value, name: name.value, type: type.value, note: note.value }); draw();
      } }] });
  }
  root.afterMount = draw;
  return root;
}

