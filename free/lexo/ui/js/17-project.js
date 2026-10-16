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
    h('h1', null, 'Vyber složku s překladem'),
    h('p', { class: 'muted' }, 'Složka s přeloženými soubory .rpy, typicky ', h('span', { class: 'mono' }, 'game/tl/czech'),
      '. Aplikace si ji pamatuje a příště ji otevře sama.'),
    ...notes,
    h('div', { class: 'row' }, input,
      h('button', { class: 'btn', onclick: pickProject }, 'Procházet…'),
      h('button', { class: 'btn primary', onclick: () => openProject(input.value) }, 'Otevřít')),
    h('p', { class: 'muted', style: { marginTop: '12px' } }, 'Cestu můžeš také vložit ručně. Data aplikace se ukládají do podsložky ', h('span', { class: 'mono' }, '_progress_data'), '.')));
  input.addEventListener('keydown', (e) => { if (e.key === 'Enter') openProject(input.value); });
}
/* Another running copy of the app owns the project: offer to take it over (the other copy then shows a warning). */
async function offerTakeover(e) {
  if (!e.data?.locked) return false;
  const ok = await confirmDialog('Projekt používá jiná kopie aplikace', `${e.message} Převzít projekt jen když víš, že druhá kopie už neběží nebo ji zavřeš.`, 'Převzít projekt');
  if (ok) await openProject(e.data.path, true);
  return true;
}
export async function openProject(path, force = false) {
  if (!path.trim()) return toast('Zadej cestu ke složce.', 'bad');
  try { status('Otevírám projekt…'); await post('project/set', { path, force }); await afterProjectChange(); } catch (e) { if (!await offerTakeover(e)) fail(e); }
}
export async function pickProject() {
  try {
    const r = await post('project/pick', {});
    if (r.cancelled) return status('Výběr složky zrušen.');
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
  if (S.ready) toast('Projekt otevřen.');
}
export async function refreshHeader() {
  $('#projectPath').textContent = S.ready ? S.projectDir : 'žádný projekt';
  $('#undoBtn').disabled = true;
  $('#refreshBtn').disabled = !S.ready;
  if (S.ready) {
    try { S.overview = await get('overview'); $('#undoBtn').disabled = !S.overview.undo.length; $('#undoBtn').title = S.overview.undo.length ? `Vrátit: ${S.overview.undo[0]} (Ctrl+Z)` : 'Není co vracet'; } catch { /* shown by the view */ }
  }
  renderTabs(); renderStatusRight();
}
export function renderStatusRight() {
  const o = S.overview;
  $('#statusRight').textContent = o ? `${o.project.files} souborů · ${fmtN(o.totals.done)} / ${fmtN(o.totals.total)} řádků · ${fmtPct(o.totals.percent)}` +
    (S.lastRefresh ? ` · obnoveno ${S.lastRefresh}` : '') : '';
}

