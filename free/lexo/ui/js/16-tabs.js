"use strict";
/* ---------------------------------------------------------------- tabs */
const TABS = [['overview', 'Přehled'], ['files', 'Soubory'], ['entries', 'Řádky'], ['validation', 'Kontrola'], ['glossary', 'Slovník'], ['publish', 'Publikace'], ['settings', 'Nastavení']];
/* Small stroke icons for the tabs (24x24, drawn here so no icon font or file is needed). */
const TAB_ICONS = {
  overview: '<rect x="3" y="3" width="7" height="9" rx="1.5"/><rect x="14" y="3" width="7" height="5" rx="1.5"/><rect x="14" y="12" width="7" height="9" rx="1.5"/><rect x="3" y="16" width="7" height="5" rx="1.5"/>',
  files: '<path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/>',
  entries: '<path d="M8 6h13M8 12h13M8 18h13"/><circle cx="3.5" cy="6" r="1"/><circle cx="3.5" cy="12" r="1"/><circle cx="3.5" cy="18" r="1"/>',
  validation: '<path d="M12 3l8 3v6c0 4.5-3.2 8-8 9-4.8-1-8-4.5-8-9V6z"/><path d="m8.5 12 2.5 2.5 4.5-5"/>',
  glossary: '<path d="M5 4h11a3 3 0 0 1 3 3v13H8a3 3 0 0 1-3-3z"/><path d="M5 17a3 3 0 0 1 3-3h11"/>',
  publish: '<path d="M12 16V4"/><path d="m7 9 5-5 5 5"/><path d="M4 16v3a1 1 0 0 0 1 1h14a1 1 0 0 0 1-1v-3"/>',
  settings: '<circle cx="12" cy="12" r="3"/><path d="M12 2v3M12 19v3M2 12h3M19 12h3M4.9 4.9 7 7M17 17l2.1 2.1M4.9 19.1 7 17M17 7l2.1-2.1"/>',
};
function tabIcon(k) {
  const el = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
  el.setAttribute('viewBox', '0 0 24 24'); el.setAttribute('fill', 'none'); el.setAttribute('stroke', 'currentColor');
  el.setAttribute('stroke-width', '1.8'); el.setAttribute('stroke-linecap', 'round'); el.setAttribute('stroke-linejoin', 'round'); el.setAttribute('aria-hidden', 'true');
  el.innerHTML = TAB_ICONS[k] || '';
  return el;
}
function renderTabs() {
  const nav = $('#tabs');
  nav.replaceChildren(...TABS.map(([k, label], i) => {
    const bad = k === 'validation' && S.overview ? S.overview.validation.error : 0;
    return h('button', { class: 'tab', role: 'tab', 'aria-selected': String(S.view === k), title: `Alt+${i + 1}`, onclick: () => go(k) },
      tabIcon(k), label, bad ? h('span', { class: 'badge' }, fmtN(bad)) : null);
  }));
}

/* Look: automatic (follows the system) / dark / light. Stored as a preference, applied through the data-theme attribute. */
const DENSITY_LABEL = { comfortable: 'pohodlná', compact: 'kompaktní' };
function applyDensity() {
  const dn = pref('ui.density', 'comfortable') === 'compact' ? 'compact' : 'comfortable';
  document.documentElement.dataset.density = dn;
  const b = $('#densityBtn');
  if (b) { setIcon(b, dn === 'compact' ? 'densityC' : 'density'); b.title = `Hustota tabulek: ${DENSITY_LABEL[dn]} (klik přepne)`; }
}
function toggleDensity() {
  setPref('ui.density', pref('ui.density', 'comfortable') === 'compact' ? 'comfortable' : 'compact');
  applyDensity();
  toast(`Hustota tabulek: ${DENSITY_LABEL[pref('ui.density', 'comfortable')]}`, 'ok', 1400);
}
const THEME_ICON = { auto: '◐', dark: '☾', light: '☀' };
const THEME_LABEL = { auto: 'automaticky podle systému', dark: 'tmavý', light: 'světlý' };
function applyTheme() {
  const t = ['auto', 'dark', 'light'].includes(pref('ui.theme', 'auto')) ? pref('ui.theme', 'auto') : 'auto';
  const light = t === 'light' || (t === 'auto' && window.matchMedia?.('(prefers-color-scheme: light)').matches);
  document.documentElement.dataset.theme = light ? 'light' : 'dark';
  const b = $('#themeBtn');
  if (b) { setIcon(b, { auto: 'theme', dark: 'moon', light: 'sun' }[t]); b.title = `Vzhled: ${THEME_LABEL[t]} (klik přepne)`; }
}
function cycleTheme() {
  const order = ['auto', 'dark', 'light'];
  setPref('ui.theme', order[(order.indexOf(pref('ui.theme', 'auto')) + 1) % 3]);
  applyTheme();
  toast(`Vzhled: ${THEME_LABEL[pref('ui.theme', 'auto')]}`, 'ok', 1600);
}
window.matchMedia?.('(prefers-color-scheme: light)').addEventListener?.('change', () => applyTheme());
function go(view, opts) {
  S.view = view; setPref('view', view);
  S.goOpts = opts || null;
  renderTabs(); render();
}

async function render() {
  const view = $('#view');
  view.className = ['entries', 'files'].includes(S.view) && S.ready ? 'flush' : '';
  if (!S.ready) return renderWelcome();
  const token = (S.renderToken = (S.renderToken || 0) + 1);
  view.replaceChildren(loading());
  try {
    const fn = { overview: viewOverview, files: viewFiles, entries: viewEntries, validation: viewValidation, glossary: viewGlossary, publish: viewPublish, settings: viewSettings }[S.view];
    const node = await fn();
    if (token !== S.renderToken) return;
    view.replaceChildren(node);
    node.afterMount?.();
  } catch (e) {
    view.replaceChildren(h('div', { class: 'notice bad' }, e.message));
  }
}

