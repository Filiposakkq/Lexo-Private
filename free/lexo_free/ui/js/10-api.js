import { fromWire, t } from './05-i18n.js';
import { TOKEN } from './00-core.js';
import { S } from './11-state.js';
/* ---------------------------------------------------------------- api */
export async function request(method, path, body) {
  const opts = { method, headers: {} };
  let url = '/api/' + path;
  if (method === 'POST') {
    opts.headers['X-Token'] = TOKEN;
    opts.headers['Content-Type'] = 'application/json';
    opts.body = JSON.stringify(body || {});
  } else if (body) {
    url += '?' + new URLSearchParams(Object.entries(body).filter(([, v]) => v !== '' && v !== undefined && v !== null));
  }
  let res;
  try { res = await fetch(url, opts); }
  catch { throw new Error(t('The connection to the application was lost – is the program still running?')); }
  let data;
  try { data = await res.json(); } catch { throw new Error(t('Invalid response from the application.')); }
  data = fromWire(data);
  if (!data.ok) { const err = new Error(data.error || t('The operation failed.')); err.data = data; throw err; }
  return data;
}
export const get = (p, q) => request('GET', p, q);
export const clearCtx = () => { S.ctxCache?.clear(); S.ctxDone?.clear(); S.scenesCache = {}; };       // the scene list itself (S.scenesDone) stays: it is refreshed in the background
const clearMenus = () => { S.menuCache?.clear(); };       // the menu of a line shows translations of other lines: a write makes it stale (cleared before and after, so a fetch in between is dropped too)
export const post = (p, b) => {
  const writes = p !== 'focus' && p !== 'prefs' && p !== 'entry/check' && p !== 'features';
  if (writes) { clearCtx(); clearMenus(); }       // a write may change the neighbouring lines
  const r = request('POST', p, b);
  if (writes) r.then(clearMenus, clearMenus);
  return r;
};

