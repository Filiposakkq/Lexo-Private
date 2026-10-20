import { reactive } from 'vue';
import { get, post, TOKEN } from './api.js';

/* Several windows of the application on one project. The first (main) window is the one you work in; a second one (the detail window: the scene, the checks and the proofreading next to the
   editor, on another monitor) follows the line that is open in the main window. Every window tells the server it is alive (the application quits when the last one is closed), and the main
   window learns from the same answer that a second window is open. The line travels through the server (so also between two programs) and, instantly, through a BroadcastChannel when
   the windows share a browser. */
export const WIN_ID = Math.random().toString(36).slice(2, 10);
export const Q = new URLSearchParams(location.search);
export const SECONDARY = Q.has('win');
export const COMPANION = SECONDARY && Q.get('companion') === '1';       // the detail-only window that follows the main one

function stored(k, def) { try { const v = localStorage.getItem('aside.win.' + k); return v === null ? def : v === '1'; } catch { return def; } }
/* what the second window and the main window do together: follow, the editor in the second window, the scene over the whole second window, the main window slimmed down while a second one is open */
export const win = reactive({ companionOpen: false, follow: true, editor: false, full: false, slim: stored('slim', true), lost: false });
export function setSlim(on) { win.slim = on; try { localStorage.setItem('aside.win.slim', on ? '1' : '0'); } catch { /* only for this page */ } }

const channel = typeof BroadcastChannel !== 'undefined' ? new BroadcastChannel('lexo-focus') : null;

/* The main window: which line is open (at once through the channel, to the server after a pause: a run of lines that are passed is one announcement). */
let focusT = null;
export function announceFocus(file, line) {
  channel?.postMessage({ file, line, text: null });
  clearTimeout(focusT);
  focusT = setTimeout(() => { post('focus', { file, line }).catch(() => {}); }, 150);
}
/* What is typed in the main editor goes to the preview of the second window. */
let draftT = null;
export function announceDraft(file, line, text) {
  if (!win.companionOpen) return;
  channel?.postMessage({ file, line, text });
  clearTimeout(draftT);
  draftT = setTimeout(() => { post('focus', { file, line, text }).catch(() => {}); }, 150);
}

/* The second window: calls `apply({ file, line, text })` for every change of the line (or of the text typed in the main window). Returns a function that stops it. */
export function startFollowing(apply) {
  let stopped = false, seen = -1;
  if (channel) channel.onmessage = (e) => { if (!stopped) apply(e.data); };
  (async function loop() {
    while (!stopped) {
      try { const f = await get('focus', { after: seen, wait: 1 }); if (f.seq !== seen) { seen = f.seq; apply(f); } } catch { await new Promise((r) => setTimeout(r, 1500)); }
    }
  })();
  return () => { stopped = true; if (channel) channel.onmessage = null; };
}

/* Every window pings every four seconds; three misses in a row mean the application was closed. */
export function startPings() {
  let fails = 0;
  const q = () => ({ w: WIN_ID, ...(COMPANION ? { k: 1 } : {}) });
  const seen = (r) => { if (!COMPANION) win.companionOpen = r.companions > 0; };
  get('ping', q()).then(seen).catch(() => {});
  const timer = setInterval(() => get('ping', q()).then((r) => { fails = 0; win.lost = false; seen(r); }).catch(() => { if (++fails >= 3) win.lost = true; }), 4000);
  const bye = () => { try { navigator.sendBeacon('/api/bye?t=' + TOKEN + '&w=' + WIN_ID); } catch { /* the server notices that the pings stopped */ } };
  addEventListener('pagehide', bye);
  return () => { clearInterval(timer); removeEventListener('pagehide', bye); };
}

/* Open the detail window: in the application the shell opens a window; in a browser the server says so and a browser window is opened. */
export async function openDetailWindow(cur) {
  const q = { view: 'entries', companion: '1', tab: 'scene', status: 'all', ui: 'next', ...(cur ? { file: cur.file, line: cur.line } : {}) };
  try {
    const r = await post('window/open', q);
    if (!r.ok) throw new Error(r.error || 'window');
    win.companionOpen = true;
  } catch (e) {
    const u = new URLSearchParams({ win: '2', companion: '1', tab: 'scene', view: 'entries', status: 'all', ...(cur ? { file: cur.file, line: String(cur.line) } : {}) });
    if (!window.open(`${location.origin}/next/?${u}`, '_blank')) throw e;
  }
}
