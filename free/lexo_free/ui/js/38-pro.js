import { t as T } from './05-i18n.js';
import { h } from './00-core.js';
import { post } from './10-api.js';
import { dialog } from './13-feedback.js';
/* Pro features that still live in this UI (variant A, see docs/RUST-PORT.md): the UI asks Core what is available and shows a clear message instead of a failing call.
   The decision is Core's (features.py, the Pro pack); nothing here switches a feature on. */
const TITLES = {
  'mt.batch': () => T('Batch machine translation'), 'fixes.safe': () => T('Safe bulk fixes with preview'), 'replace.regex': () => T('Regex find & replace'),
  'bulk.categorize': () => T('Bulk categorisation suggestions'), 'publish.web': () => T('Publishing progress to a website'),
};
let cache = null;
/* {tier, features: {id: {available, limit}}, problems}; without an answer everything counts as available so the real call can report the truth. */
export async function features(fresh = false) {
  if (cache && !fresh) return cache;
  try { cache = await post('features', {}); } catch { cache = { features: {}, problems: [] }; }
  return cache;
}
export const resetFeatures = () => { cache = null; };
export async function featureOf(id) {
  const f = ((await features()).features || {})[id];
  return f || { available: true, limit: null };
}
export function lockedDialog(id, problems = []) {
  dialog({ title: T('Pro feature'), narrow: true, buttons: [{ label: T('Close'), kind: 'primary' }],
    body: h('div', null,
      h('p', null, T('“{title}” is a Pro feature.', { title: (TITLES[id] || (() => id))() })),
      h('p', { class: 'muted' }, T('It is not available in this installation. The rest of the application works without it.')),
      problems.length ? h('div', { class: 'notice warn' }, problems.map(String).join(' ')) : null) });
}
/* True when the feature can be used; otherwise explains why and returns false. */
export async function requireFeature(id) {
  const f = await featureOf(id);
  if (f.available) return true;
  lockedDialog(id, (cache && cache.problems) || []);
  return false;
}
