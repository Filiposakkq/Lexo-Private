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
  catch { throw new Error('Spojení s aplikací se přerušilo – běží ještě program?'); }
  let data;
  try { data = await res.json(); } catch { throw new Error('Neplatná odpověď aplikace.'); }
  if (!data.ok) { const err = new Error(data.error || 'Operace selhala.'); err.data = data; throw err; }
  return data;
}
export const get = (p, q) => request('GET', p, q);
export const clearCtx = () => { S.ctxCache?.clear(); S.ctxDone?.clear(); S.scenesCache = {}; };       // the scene list itself (S.scenesDone) stays: it is refreshed in the background
export const post = (p, b) => { if (p !== 'focus' && p !== 'prefs' && p !== 'entry/check') clearCtx(); return request('POST', p, b); };   // a write may change the neighbouring lines

