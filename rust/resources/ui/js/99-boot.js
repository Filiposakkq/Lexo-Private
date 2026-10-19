import { availableLanguages, chooseLanguage, fromWire, loadLanguage, locale, setLanguage, t } from './05-i18n.js';
import { $, TABS, TOKEN, h } from './00-core.js';
import { get, post } from './10-api.js';
import { S, loadGlossary } from './11-state.js';
import { COMPANION, SECONDARY, URL_Q, WIN_ID, enterProof, pref, readableOn, setFocus, setIcon, setPref, speakerBadge } from './12-chrome.js';
import { fail, status } from './13-feedback.js';
import { gameText } from './14-renderers.js';
import { applyDensity, applyTheme, cycleTheme, go, render, renderTabs, toggleDensity } from './16-tabs.js';
import { pickProject, refreshHeader } from './17-project.js';
import { doRefresh, doUndo } from './18-refresh.js';
import { openDetailWindow, startFollowing } from './21-files.js';
import { mtDialog } from './24-machine-translation.js';
import { updateBrand } from './38-pro.js';
import { ACTIONS, comboOf, findAction, helpDialog, keysOf, toggleFullscreen } from './35-keyboard.js';
/* ================================================================ boot */
$('#refreshBtn').addEventListener('click', () => doRefresh());
$('#undoBtn').addEventListener('click', doUndo);
$('#themeBtn').addEventListener('click', cycleTheme);
$('#newWinBtn').addEventListener('click', () => openDetailWindow());
$('#fullBtn').addEventListener('click', () => toggleFullscreen());
$('#focusBtn').addEventListener('click', () => setFocus(!S.focus));
for (const [id, n] of [['newWinBtn', 'win'], ['fullBtn', 'full'], ['focusBtn', 'focus']]) setIcon(id, n);
/* A change made in another window: when this one gets the focus back, refresh the counters and the open line. */
window.addEventListener('focus', () => {
  if (!S.ready || document.querySelector('#dlg[open]')) return;
  refreshHeader().catch(() => {});
  if (S.view === 'entries' && S.ent.api && S.ent.sel >= 0 && !document.querySelector('#editTarget[data-dirty="1"]')) S.ent.api.select(S.ent.sel);
});
$('#densityBtn').addEventListener('click', toggleDensity);
$('#changeProject').addEventListener('click', pickProject);
document.querySelectorAll('[data-link]').forEach((b) => b.addEventListener('click', () => post('open-url', { id: b.dataset.link }).catch(fail)));

if (new URLSearchParams(location.search).has('selftest')) {      // UI self-test (see tests/ui)
  const st = document.createElement('script'); st.src = '/selftest.js'; document.body.append(st);
}

(async function boot() {
  try {
    const st = await get('state');
    S.ready = st.ready; S.version = st.version; S.prefs = st.prefs || {};
    S.langs = await availableLanguages();
    try { setLanguage(await loadLanguage(chooseLanguage(pref('ui.lang', ''), S.langs))); } catch { setLanguage(null); }
    renderTabs();
    applyTheme(); applyDensity(); $('#brandText').textContent = `Aside ${S.version || ''}`.trim(); updateBrand().catch(() => {}); S.notices = st.notices || []; S.projectDir = st.project_dir || '';
    if (st.config_error) S.notices.push(st.config_error);
    S.view = pref('view', 'overview');
    if (SECONDARY) {
      document.title += ' – ' + (COMPANION ? t('detail') : t('window 2'));
      if (COMPANION) document.body.classList.add('companion');
      if (COMPANION) { S.prefs.ui = { ...(S.prefs.ui || {}), detailTab: URL_Q.get('tab') || 'scene' }; startFollowing(); }
      const v = URL_Q.get('view');
      if (v && TABS.some(([k]) => k === v)) S.view = v;
      if (S.view === 'entries') S.goOpts = { file: URL_Q.get('file') || '', line: +URL_Q.get('line') || 0, status: URL_Q.get('status') || 'all', select: true };
      if (S.view === 'validation' && URL_Q.get('file')) S.goOpts = { file: URL_Q.get('file') };
    }
    if (!TABS.some(([k]) => k === S.view)) S.view = 'overview';
    await refreshHeader();
    if (S.ready) await loadGlossary();
    render();
    if (S.ready) S.lastRefresh = new Date().toLocaleTimeString(locale());
  } catch (e) { $('#view').replaceChildren(h('div', { class: 'notice bad' }, e.message)); }
  let fails = 0;
  const pingQ = () => ({ w: WIN_ID, ...(COMPANION ? { k: 1 } : {}) });
  const seenWin = (r) => { if (COMPANION) return; const on = r.companions > 0; if (on !== !!S.companionOpen) { S.companionOpen = on; S.ent?.api?.redraw?.(); } };
  get('ping', pingQ()).then(seenWin).catch(() => {});
  setInterval(() => get('ping', pingQ()).then((r) => { fails = 0; seenWin(r); }).catch(() => {
    if (++fails >= 3) document.body.replaceChildren(h('div', { style: { padding: '3rem', textAlign: 'center' } }, t('The application was closed. You can close this window.')));
  }), 4000);
  addEventListener('pagehide', () => { try { navigator.sendBeacon('/api/bye?t=' + TOKEN + '&w=' + WIN_ID); } catch { /* ignore */ } });
})();


// the UI self-test (selftest.js) is a classic script and reads these names from the global scope
Object.assign(window, { $, ACTIONS, fromWire, S, SECONDARY, applyDensity, applyTheme, comboOf, cycleTheme, enterProof, findAction, gameText, get, go, helpDialog, keysOf, mtDialog, post, pref, readableOn, setFocus, setPref, speakerBadge, status, toggleDensity });
