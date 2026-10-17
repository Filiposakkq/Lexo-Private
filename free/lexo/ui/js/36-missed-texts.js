import { tn, t as T } from './05-i18n.js';
import { h } from './00-core.js';
import { get, post } from './10-api.js';
import { dialog, fail, toast } from './13-feedback.js';
import { chip, empty, fmtText, fname, loading } from './14-renderers.js';
/* Texts that Ren'Py's generator leaves out (show text, input prompts, titles in variables, dialogue without a block): found in the game's scripts,
   added to the translation as a strings file. A heuristic: the list is shown first and nothing is written until the user chooses.
   The project remembers its own text functions (notify, popup...) and the texts the translator ignored (missed.json). */
export async function missedTextsDialog(done) {
  let data = null, level = 'likely';
  const sel = new Set();
  const results = h('div'), info = h('span', { class: 'grow' });
  const foot = h('button', { class: 'btn primary', disabled: true });
  const ignoreBtn = h('button', { class: 'btn', disabled: true });
  let intoFiles = false;
  const body = h('div', null, results);
  const KIND_LABEL = { text: T('text'), dialogue: T('dialogue without a block') };
  const fnInput = h('input', { type: 'text', class: 'search', style: { width: '260px' }, placeholder: 'notify, show_message' });
  const suggestBox = h('div', { class: 'toolbar' });
  async function load() {
    results.replaceChildren(loading());
    try { data = await get('missed', { level }); } catch (e) { return results.replaceChildren(h('div', { class: 'notice bad' }, e.message)); }
    sel.clear(); data.items.forEach((it, i) => { if (it.level === 'likely') sel.add(i); });
    fnInput.value = data.settings.functions.join(', ');
    paint();
  }
  const names = () => fnInput.value.split(/[,;\s]+/).filter(Boolean);
  async function saveFunctions() {
    try { await post('missed/settings', { functions: names() }); toast(T('Saved.'), 'ok'); await load(); } catch (e) { fail(e); }
  }
  async function findFunctions() {
    suggestBox.replaceChildren(h('span', { class: 'muted' }, T('Searching…')));
    try {
      const r = await get('missed/functions');
      suggestBox.replaceChildren(r.suggested.length
        ? h('span', { class: 'muted' }, T('Functions called again and again with a text (click to add):'))
        : h('span', { class: 'muted' }, T('No function called with a text again and again was found.')),
      ...r.suggested.map((f) => h('button', { class: 'btn small', title: T('{n} calls, for example: {text}', { n: f.calls, text: f.example }), onclick: () => { if (!names().includes(f.name)) fnInput.value = names().concat(f.name).join(', '); } }, `${f.name} ×${f.calls}`)));
    } catch (e) { suggestBox.replaceChildren(); fail(e); }
  }
  function paint() {
    if (data.message) { results.replaceChildren(empty(T('Nothing to search'), data.message)); foot.disabled = true; ignoreBtn.disabled = true; info.textContent = ''; return; }
    const shown = data.items.slice(0, 500);
    const rows = shown.map((it, i) => h('tr', null,
      h('td', { class: 'center' }, h('input', { type: 'checkbox', checked: sel.has(i), onchange: (e) => { e.target.checked ? sel.add(i) : sel.delete(i); paint2(); } })),
      h('td', null, chip(it.level === 'likely' ? 'done' : 'none', it.level === 'likely' ? T('likely') : T('maybe')), h('div', { class: 'muted', style: { fontSize: '11.5px' } }, KIND_LABEL[it.kind] || it.kind)),
      h('td', null, fname(it.file), h('span', { class: 'mono muted' }, T('line {n}', { n: it.line })), it.count > 1 ? h('div', { class: 'muted' }, tn(it.count, '{n} place', '{n} places')) : null),
      h('td', { class: 'txt' }, fmtText(it.text)), h('td', { class: 'mono muted' }, it.context)));
    const t = h('table', { class: 'data' }, h('colgroup', null, [40, 130, 230, 360, 300].map((w) => h('col', { style: { width: w + 'px' } }))),
      h('thead', null, h('tr', null, ['', T('Kind'), T('Where'), T('Text'), T('Code')].map((x) => h('th', null, x)))), h('tbody', null, rows));
    t.style.width = '1060px';
    const more = h('label', { class: 'check' },
      h('input', { type: 'checkbox', checked: level === 'all', onchange: (e) => { level = e.target.checked ? 'all' : 'likely'; load(); } }), ' ',
      T('Also show weaker guesses ({n})', { n: data.counts.maybe }));
    const forget = data.settings.ignored.length
      ? h('button', { class: 'btn small ghost', onclick: async () => { try { await post('missed/ignore', { clear: true }); await load(); } catch (e) { fail(e); } } }, T('Forget the {n} ignored texts', { n: data.settings.ignored.length })) : null;
    results.replaceChildren(
      h('div', { class: 'notice warn', style: { fontSize: '14px', padding: '12px 16px', borderWidth: '2px', margin: '0 0 12px' } },
        h('b', { style: { display: 'block', fontSize: '16px', letterSpacing: '.06em', textTransform: 'uppercase', marginBottom: '4px' } }, T('Experimental feature')),
        T("The list is a guess from the game's scripts and can be wrong in both directions (texts missing from it, texts that do not belong there). It still saves a lot of searching, but always review it before adding.")),
      h('p', { class: 'muted' }, T('Ren\'Py\'s generator takes only dialogue, menu choices, screens and strings marked _("…"). Texts shown in other ways, such as show text, an input prompt or a title kept in a variable, are not in the translation files. This list is a guess from the game\'s scripts – tick what is really shown to the player. The chosen texts are added to extra_strings.rpy and appear as untranslated lines.')),
      h('div', { class: 'toolbar' }, more, forget),
      h('label', { class: 'check', style: { display: 'block', margin: '4px 0' } }, h('input', { type: 'checkbox', checked: intoFiles, onchange: (e) => { intoFiles = e.target.checked; } }), ' ',
        T('Put each text into the translation file of its script'), ' ', chip('warn', T('Experimental')),
        h('div', { class: 'muted' }, T('Experimental, may not work correctly everywhere, but it mostly does and saves work. Without it all the texts go to extra_strings.rpy. The game does not care where they are; it only keeps the file and group statistics tidy.'))),
      h('div', { class: 'toolbar' }, h('b', null, T('Functions of this game that show text:')), fnInput, h('button', { class: 'btn small', onclick: saveFunctions }, T('Save')),
        h('button', { class: 'btn small', onclick: findFunctions }, T('Find functions…'))),
      suggestBox,
      data.truncated || data.items.length > 500 ? h('p', { class: 'muted' }, T('Showing the first 500 of {total}; only the shown ones can be added.', { total: data.items.length })) : null,
      data.items.length ? h('div', { class: 'toolbar', style: { marginTop: '10px' } }, h('b', null, T('Selection:')),
        h('button', { class: 'btn small', onclick: () => { shown.forEach((_, i) => sel.add(i)); paint(); } }, T('Select all')),
        h('button', { class: 'btn small', onclick: () => { sel.clear(); paint(); } }, T('Select none'))) : null,
      data.items.length ? h('div', { class: 'tablewrap', style: { maxHeight: '38vh', marginTop: '8px' } }, t) : empty(T('Nothing found'), T('The scripts hold no text that the translation lacks.')));
    paint2();
  }
  function paint2() {
    const n = sel.size;
    foot.disabled = !n; ignoreBtn.disabled = !n;
    foot.textContent = T('Add selected ({n})', { n });
    ignoreBtn.textContent = T('Ignore selected ({n})', { n });
    ignoreBtn.title = T('The project will not show these texts again');
    info.textContent = T('Found {total} · selected {n}', { total: data.counts.likely + (level === 'all' ? data.counts.maybe : 0), n });
    const chosen = () => [...sel].filter((i) => i < 500).map((i) => data.items[i]);
    foot.onclick = async () => {
      try {
        const res = await post('missed/add', { items: chosen(), into_files: intoFiles });
        toast(res.message + (res.skipped ? ' ' + T('skipped {n}', { n: res.skipped }) : ''), 'ok');
        close(); done?.();
      } catch (e) { fail(e); }
      return false;
    };
    ignoreBtn.onclick = async () => {
      try { await post('missed/ignore', { texts: chosen().map((i) => i.text) }); toast(T('The texts will not be shown again.'), 'ok'); await load(); } catch (e) { fail(e); }
      return false;
    };
  }
  const close = dialog({ title: h('span', null, T('Texts the generator missed'), ' ', chip('warn', T('Experimental'))), body, buttons: [{ label: T('Cancel') }, { node: h('span', { style: { display: 'contents' } }, info, ignoreBtn, foot) }] });
  load();
}
