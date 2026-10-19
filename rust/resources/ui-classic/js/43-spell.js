import { tn, t as T } from './05-i18n.js';
import { h } from './00-core.js';
import { post } from './10-api.js';
import { S } from './11-state.js';
import { dialog, fail, toast } from './13-feedback.js';
import { requireFeature } from './38-pro.js';
/* Spelling with a dictionary (a Pro feature): the translated lines are checked against a Hunspell dictionary (.dic + .aff) that you add for the language of the project. The words it does
   not know are listed with how often they occur and with suggestions; a correction is planned first (the lines that would change) and written as one undoable step, a word you ignore is
   remembered in the project. Words of the original, of the glossary and tags or variables are never reported. */
export async function spellDialog({ file = '' } = {}, done) {
  if (!await requireFeature('spell.hunspell')) return;
  let state;
  try { state = await post('spell/state', {}); } catch (e) { return fail(e); }
  let changed = false;
  const finish = () => { if (changed) done?.(); };
  if (!state.installed) return addDialog(state, () => spellDialog({ file }, done), finish);
  let scope = file ? 'file' : 'all';
  const scopeSel = h('select', { onchange: (e) => { scope = e.target.value; } },
    [...(file ? [['file', T('File in the filter: {name}', { name: file.split(/[\\/]/).pop() })]] : []), ['all', T('All files')]].map(([k, l]) => h('option', { value: k, selected: k === scope }, l)));
  const info = h('div', { class: 'muted' }), list = h('div');
  const run = h('button', { class: 'btn primary' }, T('Check'));
  let items = [];
  function drawList() {
    if (!items.length) { list.replaceChildren(); return; }
    list.replaceChildren(h('div', { class: 'tablewrap', style: { maxHeight: '48vh' } }, h('table', { class: 'data' },
      h('thead', null, h('tr', null, [T('Word'), T('Times'), T('Where'), T('Replace with'), ''].map((x) => h('th', null, x)))),
      h('tbody', null, items.map((it) => rowFor(it))))));
  }
  function rowFor(it) {
    const preview = h('div');
    const to = h('input', { type: 'text', value: (it.suggestions || [])[0] || '', list: 'sp_' + it.word, style: { width: '170px' }, placeholder: T('the correct word') });
    const dl = h('datalist', { id: 'sp_' + it.word }, (it.suggestions || []).map((s) => h('option', { value: s })));
    const gone = () => { items = items.filter((x) => x !== it); info.textContent = items.length ? tn(items.length, '{n} unknown word.', '{n} unknown words.') : T('No unknown words.'); drawList(); };
    const plan = async () => {
      try {
        const r = await post('spell/replace', { word: it.word, to: to.value.trim() });
        if (!r.total) { preview.replaceChildren(h('div', { class: 'muted' }, T('No line contains this word on its own.'))); return; }
        const usable = r.items.filter((x) => !x.approved);
        preview.replaceChildren(
          ...r.items.slice(0, 4).map((x) => h('div', { class: 'muted', style: { fontSize: '12.5px' } }, h('s', null, x.before), ' → ', x.after, x.approved ? h('i', null, ' ' + T('(approved: stays)')) : null)),
          r.total > 4 ? h('div', { class: 'muted' }, tn(r.total - 4, '… and {n} more line.', '… and {n} more lines.')) : null,
          h('button', { class: 'btn small primary', disabled: !usable.length, onclick: async () => {
            try {
              const w = await post('mt/apply', { selections: usable.map((x) => ({ file: x.file, line: x.line, text: x.after })) });
              changed = true; S.files = null;
              toast(tn(w.changed ?? usable.length, 'Corrected {n} line.', 'Corrected {n} lines.'), 'ok');
              gone();
            } catch (e) { fail(e); }
          } }, tn(usable.length, 'Replace in {n} line', 'Replace in {n} lines')));
      } catch (e) { fail(e); }
    };
    return h('tr', null,
      h('td', null, h('b', null, it.word)), h('td', { class: 'num' }, it.count),
      h('td', { class: 'muted', style: { fontSize: '12px' } }, it.places.map((p) => `${p.file.split(/[\\/]/).pop()}:${p.line}`).join(', '), it.count > it.places.length ? ' …' : ''),
      h('td', null, to, dl, preview),
      h('td', { style: { whiteSpace: 'nowrap' } },
        h('button', { class: 'btn small', onclick: () => (to.value.trim() ? plan() : toast(T('Enter the word to replace it with.'), 'bad')) }, T('Replace…')), ' ',
        h('button', { class: 'btn small ghost', title: T('Do not report this word again in this project'), onclick: async () => {
          try { await post('spell/ignore', { words: [it.word] }); gone(); } catch (e) { fail(e); }
        } }, T('Ignore'))));
  }
  run.onclick = async () => {
    run.disabled = true; info.textContent = T('Checking…'); list.replaceChildren();
    try {
      const r = await post('spell/check', { file: scope === 'file' ? file : '', lines: null });
      items = r.items;
      info.textContent = items.length
        ? tn(items.length, '{n} unknown word.', '{n} unknown words.') + (r.truncated ? ' ' + T('Only the first ones are listed; correct or ignore them and check again.') : '')
        : T('No unknown words.');
      drawList();
    } catch (e) { info.textContent = ''; fail(e); } finally { run.disabled = false; }
  };
  dialog({ title: T('Spelling with a dictionary'),
    body: h('div', null,
      h('p', { class: 'muted' }, T('Dictionary: {name}. Words of the original and of the glossary, tags and variables are not reported. A correction is written as one step that can be undone; approved lines are never overwritten.', { name: state.name || '–' })),
      h('div', { class: 'toolbar' }, scopeSel, run, h('span', { class: 'spacer' }),
        h('button', { class: 'btn small ghost', onclick: async () => {
          try { await post('spell/remove', {}); toast(T('Dictionary removed.'), 'ok'); } catch (e) { fail(e); }
        } }, T('Remove the dictionary'))),
      info, list),
    buttons: [{ label: T('Close'), onclick: finish }], onclose: finish });
}

/* No dictionary for this language yet: choose its .dic file (or the folder that holds it). */
function addDialog(state, again, finish) {
  const path = h('input', { type: 'text', style: { width: '100%' }, placeholder: T('Folder or .dic file of the dictionary') });
  let close = () => {};
  const add = async () => {
    try { await post('spell/add', { path: path.value.trim() }); toast(T('Dictionary added.'), 'ok'); close(); again(); } catch (e) { fail(e); }
  };
  const browse = h('button', { class: 'btn', onclick: async () => {
    try { const r = await post('carry/pick', {}); if (!r.cancelled) path.value = r.path; } catch (e) { fail(e); }
  } }, T('Browse…'));
  close = dialog({ title: T('Spelling with a dictionary'),
    body: h('div', null,
      h('p', null, T('Add a Hunspell dictionary for the language of this project ({language}): the .dic file and the .aff file with the same name. It is copied to the settings of the application and checked at once.', { language: state.language || '–' })),
      h('div', { class: 'toolbar' }, path, browse)),
    buttons: [{ label: T('Cancel'), onclick: finish }, { label: T('Add the dictionary'), kind: 'primary', onclick: async () => { await add(); return false; } }] });
}
