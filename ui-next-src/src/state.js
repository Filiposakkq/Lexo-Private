import { reactive } from 'vue';
import { get, post } from './api.js';

/* What every screen shares: the version, whether Pro is on (the title says "Aside Pro"), the state of the licence, the library of games. */
export const app = reactive({ version: '', pro: false, licence: null, games: null, error: '' });

export async function loadGames() {
  try {
    const r = await post('library/list');
    app.games = r.games;
    app.pro = !!r.pro;
    app.error = '';
  } catch (e) {
    app.games = app.games || [];
    app.error = e.message;
  }
}

export async function loadApp() {
  try { app.version = (await get('state')).version || ''; } catch { /* the version is only for the title */ }
  try { app.licence = await get('licence'); } catch { app.licence = null; }
  await loadGames();
  document.title = app.pro ? 'Aside Pro' : 'Aside';
}

/* "2 hours ago" in the language of the interface; nothing when there is no time. */
export function ago(iso, locale) {
  if (!iso) return '';
  const secs = (new Date(iso).getTime() - Date.now()) / 1000;
  if (Number.isNaN(secs)) return '';
  const rtf = new Intl.RelativeTimeFormat(locale, { numeric: 'auto' });
  const units = [['year', 31536000], ['month', 2592000], ['day', 86400], ['hour', 3600], ['minute', 60]];
  for (const [unit, size] of units) if (Math.abs(secs) >= size) return rtf.format(Math.round(secs / size), unit);
  return rtf.format(0, 'second');
}
