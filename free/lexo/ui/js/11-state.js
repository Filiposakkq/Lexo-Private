import { $ } from './00-core.js';
import { get } from './10-api.js';
import { loadChars, pref } from './12-chrome.js';
import { loading } from './14-renderers.js';
/* ---------------------------------------------------------------- state & prefs */
export const S = {
  ready: false, view: 'overview', prefs: {}, notices: [], projectDir: '',
  overview: null, files: null,
  fl: { sel: null, picked: new Set(), lastPick: null },
  ent: { items: [], total: 0, start: 0, sel: -1, loading: false, detail: null, pendingLine: 0 },
  val: { items: [], total: 0 },
  lastRefresh: null,
  gl: {},   // glossary: speaker code -> full name
};

/* Speaker display: the character code ("mc"), the glossary's full name, or both. Attributes
   ("mc happy" -> "happy") are kept. The mode is a persisted preference. */
export const speakerMode = () => pref('display.speaker', 'both');   // both = two columns (code, name)
export function speakerLabel(who) {
  if (!who) return '';
  const [code, ...attrs] = who.trim().split(/\s+/);
  const name = S.gl[code];
  const rest = attrs.length ? ' ' + attrs.join(' ') : '';
  const mode = speakerMode();
  if (!name || mode === 'code') return who;
  return mode === 'name' ? name + (attrs.length ? ` (${attrs.join(' ')})` : '') : `${name} · ${code}${rest}`;
}
/* "mc happy" -> the glossary name of "mc" */
export const speakerName = (who) => S.gl[(who || '').trim().split(/\s+/)[0]] || '';
/* detail pane: code and name always both, as "mc – Name" */
export function speakerFull(who) {
  const n = speakerName(who);
  return n ? `${who} – ${n}` : who;
}
export async function loadGlossary() {
  try { S.gl = (await get('glossary')).names || {}; } catch { S.gl = {}; }
  if (S.chars === undefined) await loadChars();
}
