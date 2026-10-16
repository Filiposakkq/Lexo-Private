import { $, h } from './00-core.js';
import { post } from './10-api.js';
import { S } from './11-state.js';
import { dialog, fail, toast } from './13-feedback.js';
import { SEV_LABEL, chip } from './14-renderers.js';
import { refreshHeader } from './17-project.js';
/* Ignoring findings on single lines. An ignore is bound to the current translation text: when the text changes, the
   finding shows up again. Ignored findings stay listed (Kontrola -> Ignorované) and can be restored. */
export function ignoreDialog(items, done) {
  const note = h('input', { type: 'text', placeholder: 'Proč se to ignoruje? (nepovinné, uvidíš to v seznamu ignorovaných)', style: { width: '100%' } });
  const all = items.length > 1 ? h('label', { class: 'check' }, h('input', { type: 'checkbox', checked: true, id: 'ignAll' }), ` Ignorovat všech ${items.length} nálezů u tohoto řádku`) : null;
  const body = h('div', null,
    h('p', { class: 'muted' }, 'Nález zmizí ze seznamů, počtů a ze zvýraznění řádků. Zůstane v seznamu ',
      h('b', null, 'Kontrola → Ignorované'), ', kde ho jedním klikem vrátíš. Když překlad řádku změníš, nález se znovu zobrazí.'),
    ...items.map((x) => h('div', { class: 'issue ' + (x.severity || 'warning') }, x.severity ? chip(x.severity, SEV_LABEL[x.severity]) : null, ' ', x.message || x.code)),
    all, h('div', { style: { marginTop: '10px' } }, note));
  dialog({ title: 'Ignorovat nález', narrow: true, body, buttons: [{ label: 'Zrušit' }, { label: 'Ignorovat', kind: 'primary', onclick: async () => {
    const chosen = all && !document.getElementById('ignAll').checked ? items.slice(0, 1) : items;
    try {
      const r = await post('ignore/set', { items: chosen.map((x) => ({ file: x.file, line: x.line, code: x.code })), on: true, note: note.value });
      toast(`Ignorováno ${r.changed} nálezů. Zpět: Ctrl+Z nebo Kontrola → Ignorované.`, 'ok');
      S.files = null; await refreshHeader(); done?.();
    } catch (e) { fail(e); }
  } }] });
  setTimeout(() => note.focus(), 0);
}
export async function unignore(items, done) {
  try {
    const r = await post('ignore/set', { items: items.map((x) => ({ file: x.file, line: x.line || undefined, key: x.key, code: x.code })), on: false });
    toast(`Nález je znovu aktivní (${r.changed}).`, 'ok');
    S.files = null; await refreshHeader(); done?.();
  } catch (e) { fail(e); }
}
export const IGN_STATE = { active: ['Ignorováno', 'none', 'Nález je schovaný.'], expired: ['Text se změnil', 'warning', 'Překlad se od ignorování změnil, takže se nález zobrazuje znovu (ignorování už neplatí).'],
  resolved: ['Už se neděje', 'tr', 'Text je stejný, ale kontrola tenhle nález už nehlásí (třeba po změně nastavení).'], gone: ['Řádek zmizel', 'none', 'Řádek už v souboru není.'] };

