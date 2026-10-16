import { N_, tn, t, tx } from './05-i18n.js';
import { $, h } from './00-core.js';
import { post } from './10-api.js';
import { S } from './11-state.js';
import { dialog, fail, toast } from './13-feedback.js';
import { SEV_LABEL, chip } from './14-renderers.js';
import { refreshHeader } from './17-project.js';
/* Ignoring findings on single lines. An ignore is bound to the current translation text: when the text changes, the
   finding shows up again. Ignored findings stay listed (Kontrola -> Ignorované) and can be restored. */
export function ignoreDialog(items, done) {
  const note = h('input', { type: 'text', placeholder: t('Why is it ignored? (optional, you will see it in the list of ignored findings)'), style: { width: '100%' } });
  const all = items.length > 1 ? h('label', { class: 'check' }, h('input', { type: 'checkbox', checked: true, id: 'ignAll' }), ' ' + tn(items.length, 'Ignore all {n} finding on this line', 'Ignore all {n} findings on this line')) : null;
  const body = h('div', null,
    h('p', { class: 'muted' }, tx('The finding disappears from the lists, counts and line highlighting. It stays in the list {where}, where you can restore it with one click. If you change the translation of the line, the finding is shown again.',
      { where: h('b', null, t('Check → Ignored')) })),
    ...items.map((x) => h('div', { class: 'issue ' + (x.severity || 'warning') }, x.severity ? chip(x.severity, SEV_LABEL[x.severity]) : null, ' ', x.message || x.code)),
    all, h('div', { style: { marginTop: '10px' } }, note));
  dialog({ title: t('Ignore the finding'), narrow: true, body, buttons: [{ label: t('Cancel') }, { label: t('Ignore'), kind: 'primary', onclick: async () => {
    const chosen = all && !document.getElementById('ignAll').checked ? items.slice(0, 1) : items;
    try {
      const r = await post('ignore/set', { items: chosen.map((x) => ({ file: x.file, line: x.line, code: x.code })), on: true, note: note.value });
      toast(tn(r.changed, 'Ignored {n} finding. Undo: Ctrl+Z or Check → Ignored.', 'Ignored {n} findings. Undo: Ctrl+Z or Check → Ignored.'), 'ok');
      S.files = null; await refreshHeader(); done?.();
    } catch (e) { fail(e); }
  } }] });
  setTimeout(() => note.focus(), 0);
}
export async function unignore(items, done) {
  try {
    const r = await post('ignore/set', { items: items.map((x) => ({ file: x.file, line: x.line || undefined, key: x.key, code: x.code })), on: false });
    toast(t('The finding is active again ({n}).', { n: r.changed }), 'ok');
    S.files = null; await refreshHeader(); done?.();
  } catch (e) { fail(e); }
}
const IGN_RAW = { active: [N_('Ignored'), 'none', N_('The finding is hidden.')], expired: [N_('The text changed'), 'warning', N_('The translation changed since it was ignored, so the finding is shown again (the ignore no longer applies).')],
  resolved: [N_('No longer happens'), 'tr', N_('The text is the same, but the check no longer reports this finding (for example after a settings change).')], gone: [N_('The line is gone'), 'none', N_('The line is no longer in the file.')] };
// [label, chip class, explanation]; the texts are translated when read
export const IGN_STATE = new Proxy(IGN_RAW, { get: (o, k) => (o[k] ? [t(o[k][0]), o[k][1], t(o[k][2])] : o[k]) });

