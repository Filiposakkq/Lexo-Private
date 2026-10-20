import { fromWire, t } from './i18n.js';

/* Talking to the application (the same /api as the classic interface). The token of the run is in the page (__TOKEN__ of index.html is replaced by the server). */
export const TOKEN = document.querySelector('meta[name="token"]')?.content || '';

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
  try { res = await fetch(url, opts); } catch { throw new Error(t('The connection to the application was lost – is the program still running?')); }
  let data;
  try { data = await res.json(); } catch { throw new Error(t('Invalid response from the application.')); }
  data = fromWire(data);
  if (!data.ok) { const err = new Error(data.error || t('The operation failed.')); err.data = data; throw err; }
  return data;
}

export const get = (path, query) => request('GET', path, query);
export const post = (path, body) => request('POST', path, body);
