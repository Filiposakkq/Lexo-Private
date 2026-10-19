import { locale, tn, t } from './05-i18n.js';
import { $, h } from './00-core.js';
import { post } from './10-api.js';
import { S } from './11-state.js';
import { confirmDialog, dialog, fail, status, toast } from './13-feedback.js';
import { render } from './16-tabs.js';
import { refreshHeader } from './17-project.js';
/* ---------------------------------------------------------------- refresh / undo */
const isDirty = () => !!document.querySelector('.ed[data-dirty="1"]') || !!S.dlgDirty;
export async function doRefresh(force = false) {
  if (!S.ready) return;
  const btn = $('#refreshBtn');
  btn.disabled = true; btn.replaceChildren(h('span', { class: 'spin' }), ' ' + t('Refreshing…'));
  status(t('Refreshing files…'));
  try {
    let r = await post('refresh', { force, dirty: isDirty() });
    if (r.needs_confirm) {
      if (!(await confirmDialog(t('Unsaved changes'), r.message, t('Refresh anyway')))) { status(t('Refresh cancelled.')); return; }
      r = await post('refresh', { force, discard: true });
    }
    S.lastRefresh = new Date().toLocaleTimeString(locale());
    const msg = r.changed ? t('Refreshed – {summary}', { summary: r.summary }) : t('Refreshed – no changes.');
    toast(msg + (r.moved_metadata ? ' ' + tn(r.moved_metadata, 'The data of {n} renamed file was carried over.', 'The data of {n} renamed files was carried over.') : ''), r.errors.length ? 'bad' : 'ok');
    if (r.changed || r.errors.length) showRefreshReport(r);
    S.files = null; S.ent.items = [];
    await refreshHeader(); render();
  } catch (e) { fail(e); }
  finally { btn.disabled = false; btn.replaceChildren(t('⟳ Refresh files')); }
}
function showRefreshReport(r) {
  const sec = (title, items) => items.length ? h('div', null, h('h4', null, `${title} (${items.length})`), h('ul', null, items.map((x) => h('li', { class: 'fname' }, typeof x === 'string' ? x : `${x.from}  →  ${x.to}`)))) : null;
  dialog({ title: t('Refresh result'), body: h('div', null,
    sec(t('Added files'), r.added), sec(t('Changed files'), r.modified), sec(t('Renamed files'), r.renamed),
    sec(t('Removed files (their data stays saved)'), r.removed),
    r.errors.length ? h('div', { class: 'notice bad' }, h('b', null, t('Problems: ')), r.errors.join(' | ')) : null,
    r.save_problems?.length ? h('div', { class: 'notice warn' }, r.save_problems.join(' | ')) : null),
    buttons: [{ label: t('Close'), kind: 'primary' }] });
}
export async function doUndo() {
  if (!S.ready) return;
  try {
    const r = await post('undo');
    toast(t('Undone: {what}', { what: r.undone }));
    S.files = null; S.ent.items = [];
    await refreshHeader(); render();
  } catch (e) { fail(e); }
}

