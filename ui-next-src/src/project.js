import { reactive } from 'vue';
import { get, post } from './api.js';
import { locale, t } from './i18n.js';

/* What the screens of an open game share: the overview, the list of files, the glossary names and the filter of the Lines screen
   (the other screens set it before they open Lines: "the untranslated lines of this file"). */
export const project = reactive({
  overview: null, files: null, names: {}, team: null, error: '',
  lines: { file: '', status: 'todo', q: '', line: 0 },
  check: { file: '', ignored: false, mode: '' },          // what the Check screen opens with (the other screens set it before they open Check)
});

export async function loadOverview() {
  project.overview = await get('overview');
  return project.overview;
}
export async function loadFiles() {
  project.files = (await get('files')).files;
  return project.files;
}
/* The team of the project ({exists, me, members, assign, can_manage…}), or null when it cannot be read. */
export async function loadTeam() {
  try { project.team = await post('team/state', {}); } catch { project.team = null; }
  return project.team;
}
export const roleLabel = (r) => ({ lead: t('lead'), translator: t('translator'), proofreader: t('proofreader') }[r] || r);
export async function loadNames() {
  try { project.names = (await get('glossary')).names || {}; } catch { project.names = {}; }
}
/* After anything was written: the numbers are fetched again. */
export async function reloadAll() {
  await Promise.all([loadOverview(), loadFiles()]);
}
export function openLines(filter) {
  project.lines = { file: '', status: 'all', q: '', line: 0, ...filter };
}

export function openCheck(filter) {
  project.check = { file: '', ignored: false, mode: '', ...filter };
}

export const fmtN = (n) => new Intl.NumberFormat(locale()).format(n ?? 0);
export const fmtPct = (p) => (p === null || p === undefined ? '–'
  : new Intl.NumberFormat(locale(), { minimumFractionDigits: 1, maximumFractionDigits: 1 }).format(p) + ' %');
export const pctOf = (a, b) => (b ? Math.max(0, Math.min(100, (a / b) * 100)) : 0);
export const baseName = (p) => String(p || '').split(/[\\/]/).pop();
export const dateLong = (iso) => (iso ? new Date(iso + 'T12:00:00').toLocaleDateString(locale(), { day: 'numeric', month: 'long', year: 'numeric' }) : '');

/* The state of a file or of an episode, and of a line. */
export function fileState(k) {
  return { done: t('Done'), rev: t('Proofreading'), proof: t('In proofreading'), tr: t('Translated'), wip: t('In progress'), none: t('Not started'), planned: t('Planned') }[k] || k;
}
export function lineState(k) {
  return { translated: t('Translated'), approved: t('Approved'), todo: t('Untranslated') }[k] || k;
}
export function lineKind(k) {
  return { empty: t('empty'), english: t('in English'), same: t('identical'), symbols: t('symbols only') }[k] || '';
}
/* The look of the pill for a state. */
export const pillOf = (k) => ({ done: 'checked', rev: 'checked', proof: 'checked', tr: 'ok', translated: 'ok', approved: 'approved', wip: 'todo', todo: 'todo', none: 'none', planned: 'none' }[k] || 'none');

export function groupName(n) {
  const named = project.overview?.group_names?.[String(n)];
  if (named) return named;
  const word = project.overview?.labels?.group || 'Episode';
  const f = { Episode: () => t('Episode {n}', { n }), Chapter: () => t('Chapter {n}', { n }), Part: () => t('Part {n}', { n }),
    Day: () => t('Day {n}', { n }), Update: () => t('Update {n}', { n }), Act: () => t('Act {n}', { n }) }[word];
  return f ? f() : `${word} ${n}`;
}
export const groupNumbers = () => (project.overview?.episodes || []).map((e) => e.n);
export function assignLabel(a) {
  return a === '' || a === undefined || a === null ? t('Unassigned') : a === 'gen' ? t('General') : groupName(+String(a).slice(3));
}

export { post, get };
