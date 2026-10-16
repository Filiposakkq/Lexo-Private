"use strict";
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
  renderTabs();
  try {
    const st = await get('state');
    S.ready = st.ready; S.version = st.version; S.prefs = st.prefs || {};
    applyTheme(); applyDensity(); $('#brandText').textContent = `Lexo ${S.version || ''}`.trim(); S.notices = st.notices || []; S.projectDir = st.project_dir || '';
    if (st.config_error) S.notices.push(st.config_error);
    S.view = pref('view', 'overview');
    if (SECONDARY) {
      document.title += COMPANION ? ' – detail' : ' – okno 2';
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
    if (S.ready) S.lastRefresh = new Date().toLocaleTimeString('cs-CZ');
  } catch (e) { $('#view').replaceChildren(h('div', { class: 'notice bad' }, e.message)); }
  let fails = 0;
  const pingQ = () => ({ w: WIN_ID, ...(COMPANION ? { k: 1 } : {}) });
  const seenWin = (r) => { if (COMPANION) return; const on = r.companions > 0; if (on !== !!S.companionOpen) { S.companionOpen = on; S.ent?.api?.redraw?.(); } };
  get('ping', pingQ()).then(seenWin).catch(() => {});
  setInterval(() => get('ping', pingQ()).then((r) => { fails = 0; seenWin(r); }).catch(() => {
    if (++fails >= 3) document.body.replaceChildren(h('div', { style: { padding: '3rem', textAlign: 'center' } }, 'Aplikace byla ukončena. Toto okno můžeš zavřít.'));
  }), 4000);
  addEventListener('pagehide', () => { try { navigator.sendBeacon('/api/bye?t=' + TOKEN + '&w=' + WIN_ID); } catch { /* ignore */ } });
})();
