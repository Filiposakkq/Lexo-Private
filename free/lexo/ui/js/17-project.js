import { tn, t, tx } from './05-i18n.js';
import { $, fmtN, fmtPct, h } from './00-core.js';
import { get, post } from './10-api.js';
import { S, loadGlossary } from './11-state.js';
import { confirmDialog, fail, status, toast } from './13-feedback.js';
import { render, renderTabs } from './16-tabs.js';
/* ---------------------------------------------------------------- welcome / project */
export function renderWelcome() {
  const view = $('#view');
  view.className = '';
  const input = h('input', { type: 'text', id: 'welPath', placeholder: 'D:\\Hra\\game\\tl\\czech', value: S.projectDir || '' });
  const notes = S.notices.map((n) => h('div', { class: 'notice warn' }, n));
  view.replaceChildren(h('div', { class: 'panel welcome' },
    h('h1', null, t('Choose the translation folder')),
    h('p', { class: 'muted' }, tx('The folder with the translated .rpy files, usually {path}. The app remembers it and opens it next time.',
      { path: h('span', { class: 'mono' }, 'game/tl/czech') })),
    ...notes,
    h('div', { class: 'row' }, input,
      h('button', { class: 'btn', onclick: pickProject }, t('Browse…')),
      h('button', { class: 'btn primary', onclick: () => openProject(input.value) }, t('Open'))),
    h('p', { class: 'muted', style: { marginTop: '12px' } }, tx('You can also paste the path by hand. The app data is saved in the subfolder {folder}.', { folder: h('span', { class: 'mono' }, '_progress_data') }))));
  input.addEventListener('keydown', (e) => { if (e.key === 'Enter') openProject(input.value); });
}
/* Another running copy of the app owns the project: offer to take it over (the other copy then shows a warning). */
async function offerTakeover(e) {
  if (!e.data?.locked) return false;
  const ok = await confirmDialog(t('The project is in use by another copy of the application'),
    `${e.message} ${t('Take over the project only if you know the other copy is no longer running or you will close it.')}`, t('Take over the project'));
  if (ok) await openProject(e.data.path, true);
  return true;
}
export async function openProject(path, force = false) {
  if (!path.trim()) return toast(t('Enter the path to the folder.'), 'bad');
  try { status(t('Opening the project…')); await post('project/set', { path, force }); await afterProjectChange(); } catch (e) { if (!await offerTakeover(e)) fail(e); }
}
export async function pickProject() {
  try {
    const r = await post('project/pick', {});
    if (r.cancelled) return status(t('Folder selection cancelled.'));
    await afterProjectChange();
  } catch (e) { if (!await offerTakeover(e)) fail(e); }
}
async function afterProjectChange() {
  const st = await get('state');
  S.ready = st.ready; S.notices = st.notices; S.projectDir = st.project_dir;
  S.files = null; S.overview = null; S.ent.items = []; S.ent.sel = -1; S.fl.picked.clear();
  await refreshHeader();
  if (S.ready) await loadGlossary();
  render();
  if (S.ready) toast(t('Project opened.'));
}
export async function refreshHeader() {
  $('#projectPath').textContent = S.ready ? S.projectDir : t('no project');
  $('#undoBtn').disabled = true;
  $('#refreshBtn').disabled = !S.ready;
  if (S.ready) {
    try { S.overview = await get('overview'); $('#undoBtn').disabled = !S.overview.undo.length; $('#undoBtn').title = S.overview.undo.length ? t('Undo: {what} (Ctrl+Z)', { what: S.overview.undo[0] }) : t('Nothing to undo'); } catch { /* shown by the view */ }
  }
  renderTabs(); renderStatusRight();
}
export function renderStatusRight() {
  const o = S.overview;
  $('#statusRight').textContent = o ? `${tn(o.project.files, '{n} file', '{n} files')} · ${t('{done} / {total} lines', { done: fmtN(o.totals.done), total: fmtN(o.totals.total) })} · ${fmtPct(o.totals.percent)}` +
    (S.lastRefresh ? ` · ${t('refreshed {time}', { time: S.lastRefresh })}` : '') : '';
}

