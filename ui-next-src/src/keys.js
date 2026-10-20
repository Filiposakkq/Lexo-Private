import { reactive } from 'vue';
import { t } from './i18n.js';

/* The keyboard: one list of actions with their default keys, the keys the user changed (kept in this browser), and the handlers the screens register while they are open.
   A key press is looked up here; an action that no open screen handles does nothing. The same list feeds the help (?), the command palette (Ctrl+K) and the panel in Settings. */

/* id (no dots), label, default keys, typing (works while typing in a field), scope: 'lines' (only on the Lines screen) or '' (everywhere) */
export const ACTIONS = [
  { id: 'palette', get label() { return t('Command palette (search actions and files)'); }, keys: ['Ctrl+K'], typing: true },
  { id: 'search', get label() { return t('Search in the current section'); }, keys: ['Ctrl+F'], typing: true },
  { id: 'search_slash', get label() { return t('Search in the current section (slash)'); }, keys: ['/'] },
  { id: 'refresh', get label() { return t('Refresh files'); }, keys: ['F5', 'Ctrl+R'], typing: true },
  { id: 'undo', get label() { return t('Undo the last change (approval, proofreading, glossary)'); }, keys: ['Ctrl+Z'] },
  { id: 'fullscreen', get label() { return t('Full screen (on / off)'); }, keys: ['F11'], typing: true },
  { id: 'help', get label() { return t('Show shortcuts'); }, keys: ['?'] },
  { id: 'tab_overview', get label() { return t('Go to: {name}', { name: t('Overview') }); }, keys: ['Alt+1'], typing: true },
  { id: 'tab_files', get label() { return t('Go to: {name}', { name: t('Files') }); }, keys: ['Alt+2'], typing: true },
  { id: 'tab_lines', get label() { return t('Go to: {name}', { name: t('Lines') }); }, keys: ['Alt+3'], typing: true },
  { id: 'tab_check', get label() { return t('Go to: {name}', { name: t('Check') }); }, keys: ['Alt+4'], typing: true },
  { id: 'tab_glossary', get label() { return t('Go to: {name}', { name: t('Glossary') }); }, keys: ['Alt+5'], typing: true },
  { id: 'tab_tools', get label() { return t('Go to: {name}', { name: t('Tools') }); }, keys: ['Alt+6'], typing: true },
  { id: 'tab_team', get label() { return t('Go to: {name}', { name: t('Team') }); }, keys: ['Alt+7'], typing: true },
  { id: 'tab_publish', get label() { return t('Go to: {name}', { name: t('Publishing') }); }, keys: ['Alt+8'], typing: true },
  { id: 'tab_settings', get label() { return t('Go to: {name}', { name: t('Settings') }); }, keys: ['Alt+9'], typing: true },
  { id: 'next', get label() { return t('Lines: next line'); }, keys: ['ArrowDown', 'ArrowRight', 'J'], scope: 'lines' },
  { id: 'prev', get label() { return t('Lines: previous line'); }, keys: ['ArrowUp', 'ArrowLeft', 'K'], scope: 'lines' },
  { id: 'pagedown', get label() { return t('Lines: a page down'); }, keys: ['PageDown'], scope: 'lines' },
  { id: 'pageup', get label() { return t('Lines: a page up'); }, keys: ['PageUp'], scope: 'lines' },
  { id: 'first', get label() { return t('Lines: first line'); }, keys: ['Home'], scope: 'lines' },
  { id: 'last', get label() { return t('Lines: last line'); }, keys: ['End'], scope: 'lines' },
  { id: 'approve', get label() { return t('Lines: approve / unapprove the line'); }, keys: ['A'], scope: 'lines' },
  { id: 'approve_all', get label() { return t('Lines: approve all identical… (with a preview)'); }, keys: ['Shift+A'], scope: 'lines' },
  { id: 'proof', get label() { return t('Lines: proofreading – checked (and the next one)'); }, keys: ['P'], scope: 'lines' },
  { id: 'proof_upto', get label() { return t('Lines: mark everything up to this line as checked'); }, keys: ['Shift+P'], scope: 'lines' },
  { id: 'unproof_upto', get label() { return t('Lines: remove the checked mark up to this line'); }, keys: ['Shift+U'], scope: 'lines' },
  { id: 'flag', get label() { return t('Lines: flag for a fix + note'); }, keys: ['F'], scope: 'lines' },
  { id: 'open_editor', get label() { return t('Lines: open the line in an external editor'); }, keys: ['E', 'Enter'], scope: 'lines' },
  { id: 'consistency', get label() { return t('Check: different translations of the same original'); }, keys: [] },
  { id: 'ignored', get label() { return t('Check: ignored findings'); }, keys: [] },
  { id: 'lint', get label() { return t("Check: game lint (Ren'Py)"); }, keys: [] },
  { id: 'findings', get label() { return t('Check: findings'); }, keys: [] },
  { id: 'snapshot', get label() { return t('Create a backup of the translation files now'); }, keys: [] },
];

/* the keys the user changed: { id: ['Ctrl+X', …] }; an action that is not here has its default keys */
function readUser() { try { return JSON.parse(localStorage.getItem('aside.keys') || '{}') || {}; } catch { return {}; } }
export const userKeys = reactive(readUser());
export function saveKeys(map) {
  for (const k of Object.keys(userKeys)) delete userKeys[k];
  Object.assign(userKeys, map);
  try { localStorage.setItem('aside.keys', JSON.stringify(map)); } catch { /* only for this page */ }
}
export const keysOf = (a) => (Array.isArray(userKeys[a.id]) ? userKeys[a.id] : a.keys);
export const isChanged = (a) => Array.isArray(userKeys[a.id]);
export function currentMap() { const m = {}; ACTIONS.forEach((a) => { m[a.id] = [...keysOf(a)]; }); return m; }

/* "Ctrl+Shift+F", "Alt+3", "ArrowDown", "A": the key of an event as it is written in the list (a letter alone is upper case, Shift is named for letters and named keys) */
export function comboOf(e) {
  const k = e.key;
  if (!k || ['Control', 'Shift', 'Alt', 'Meta', 'AltGraph', 'CapsLock', 'Dead'].includes(k)) return '';
  const letter = k.length === 1 && /\p{L}/u.test(k);
  const parts = [];
  if (e.ctrlKey || e.metaKey) parts.push('Ctrl');
  if (e.altKey) parts.push('Alt');
  if (e.shiftKey && (letter || k.length > 1)) parts.push('Shift');
  parts.push(k === ' ' ? 'Space' : k.length === 1 ? k.toUpperCase() : k);
  return parts.join('+');
}

/* what the open screens do for an action: { id: () => … }; a screen adds its handlers when it opens and takes them away when it closes */
export const handlers = {};
export function register(map) {
  Object.assign(handlers, map);
  return () => { for (const [k, fn] of Object.entries(map)) if (handlers[k] === fn) delete handlers[k]; };
}
export function findAction(combo, typing) {
  return ACTIONS.find((a) => keysOf(a).includes(combo) && (!typing || a.typing) && handlers[a.id]);
}
const isTyping = (el) => !!el && (el.isContentEditable || ['INPUT', 'TEXTAREA', 'SELECT'].includes(el.tagName));
/* The one listener of the application. Nothing runs under an open dialog; a button or a link keeps Enter and Space for itself. */
export function onKeydown(e) {
  if (document.querySelector('[role="dialog"]')) return;
  const combo = comboOf(e);
  if (!combo) return;
  const typing = isTyping(e.target);
  if (typing && e.key === 'Escape') { e.target.blur(); return; }
  if ((combo === 'Enter' || combo === 'Space') && e.target.closest && e.target.closest('button, a, [role="button"]')) return;
  const act = findAction(combo, typing);
  if (!act) return;
  e.preventDefault();
  handlers[act.id](e);
}
export const lineActions = () => ACTIONS.filter((a) => a.scope === 'lines');
