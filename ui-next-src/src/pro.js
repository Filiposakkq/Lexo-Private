import { reactive } from 'vue';
import { post } from './api.js';
import { t } from './i18n.js';

/* Pro features: the interface asks the application what is available and shows a clear message instead of a failing call. The decision is the application's
   (the Pro pack); nothing here switches a feature on. */
export const pro = reactive({ locked: null });
let cache = null;

export function featureTitle(id) {
  return ({
    'mt.batch': () => t('Batch machine translation'), 'fixes.safe': () => t('Safe bulk fixes with preview'), 'replace.regex': () => t('Regex find & replace'),
    'bulk.categorize': () => t('Bulk categorisation suggestions'), 'publish.web': () => t('Publishing progress to a website'),
    'carry.over': () => t('Carry a translation over to a new version'), 'memory.fuzzy': () => t('Fill from similar translations'),
    'mt.profiles': () => t('Profiles of characters'), 'style.check': () => t('Style check'), 'release.build': () => t('Release builder'),
    'spell.hunspell': () => t('Spelling with a dictionary'), 'team.manage': () => t('Team management'),
  }[id] || (() => id))();
}
/* {tier, features: {id: {available, limit}}, problems}; without an answer everything counts as available so that the real call can report the truth. */
export async function features(fresh = false) {
  if (cache && !fresh) return cache;
  try { cache = await post('features', {}); } catch { cache = { features: {}, problems: [] }; }
  return cache;
}
export async function featureOf(id) {
  return ((await features()).features || {})[id] || { available: true, limit: null };
}
/* True when the feature can be used; otherwise the dialog that explains why is shown and false is returned. */
export async function requireFeature(id) {
  const f = await featureOf(id);
  if (f.available) return true;
  pro.locked = { id, problems: (cache && cache.problems) || [] };
  return false;
}
