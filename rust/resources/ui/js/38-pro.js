import { t as T, tx } from './05-i18n.js';
import { h } from './00-core.js';
import { get, post } from './10-api.js';
import { dialog, fail, toast } from './13-feedback.js';
/* Pro features that still live in this UI (variant A, see docs/technical/RUST-PORT.md): the UI asks Core what is available and shows a clear message instead of a failing call.
   The decision is Core's (features.py, the Pro pack); nothing here switches a feature on. */
const TITLES = {
  'mt.batch': () => T('Batch machine translation'), 'fixes.safe': () => T('Safe bulk fixes with preview'), 'replace.regex': () => T('Regex find & replace'),
  'bulk.categorize': () => T('Bulk categorisation suggestions'), 'publish.web': () => T('Publishing progress to a website'),
  'carry.over': () => T('Carry a translation over to a new version'),
  'memory.fuzzy': () => T('Fill from similar translations'),
  'mt.profiles': () => T('Profiles of characters'),
  'style.check': () => T('Style check'),
  'release.build': () => T('Release builder'),
  'spell.hunspell': () => T('Spelling with a dictionary'),
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
/* The one notice of every Pro dialog when the demo cut the work to its limit ({feature, limit, total} from the pack), or nothing. */
export function demoNotice(limited) {
  if (!limited) return null;
  const a = { limit: limited.limit, total: limited.total };
  return h('div', { class: 'notice warn' }, limited.feature === 'bulk.categorize'
    ? T('The demo handles only the first {limit} files ({total} found). A licence removes the limit.', a)
    : T('The demo handles only the first {limit} lines ({total} found). A licence removes the limit.', a));
}
/* True when the feature can be used; otherwise explains why and returns false. */
export async function requireFeature(id) {
  const f = await featureOf(id);
  if (f.available) return true;
  lockedDialog(id, (cache && cache.problems) || []);
  return false;
}
/* The reasons the pack gives for a licence file it cannot use (English texts of the pack); each is a literal t() so that the pack of texts can be checked. */
const reasonText = (r) => ({
  'the licence file is not readable': () => T('the licence file is not readable'),
  'the signature of the licence is wrong': () => T('the signature of the licence is wrong'),
  'the licence belongs to another computer': () => T('the licence belongs to another computer'),
  'the licence covers another major version of the application': () => T('the licence covers another major version of the application'),
  'this build cannot verify licences': () => T('this build cannot verify licences'),
  'the licence was signed with a key this version does not know': () => T('the licence was signed with a key this version does not know'),
  'the licence was signed with a key that is no longer trusted': () => T('the licence was signed with a key that is no longer trusted'),
  'the licence file comes from a newer version of the licence system': () => T('the licence file comes from a newer version of the licence system'),
  'this licence was cancelled': () => T('this licence was cancelled'),
  'this computer was removed from the licence': () => T('this computer was removed from the licence'),
}[r] || (() => r || ''))();
/* The computers that hold the licence: their names, the places, and freeing another computer (limited by the server). Filled in when the server answers; without a connection the box says so. */
function deviceBox(run) {
  const box = h('div', { style: { marginTop: '14px' } });
  (async () => {
    let d;
    try { d = await post('licence/devices', {}); } catch (e) { box.append(h('p', { class: 'muted' }, T('The list of computers needs a connection to the licence server.'))); return; }
    const date = (ts) => new Date(ts * 1000).toLocaleDateString();
    const rows = d.devices.map((x) => h('div', { class: 'row', style: { alignItems: 'center', borderTop: '0.5px solid var(--border)', padding: '6px 0' } },
      h('span', { class: 'grow' }, h('b', null, x.name || T('Unnamed computer')), ' ', x.current ? h('span', { class: 'chip' }, T('This computer')) : null,
        h('div', { class: 'muted' }, T('last seen {date}', { date: date(x.last_seen) }))),
      x.current
        ? h('button', { class: 'btn', onclick: () => { const n = prompt(T('Name of this computer:'), x.name || ''); if (n !== null) run('licence/rename', { name: n }, T('Computer renamed.')); } }, T('Rename'))
        : h('button', { class: 'btn', disabled: d.releases_left === 0,
          onclick: () => { if (confirm(T('Free the place of “{name}”? You can do this {left} more time(s).', { name: x.name || T('Unnamed computer'), left: d.releases_left }))) run('licence/release', { target: x.id }, T('Computer removed.')); } }, T('Remove'))));
    box.append(h('b', null, T('Your computers')), h('div', { class: 'muted' }, T('Places used: {n} of {max}.', { n: d.devices.length, max: d.max })), ...rows,
      d.releases_left === 0 ? h('p', { class: 'muted' }, T('You have used all releases. Write to the seller if you need to free a place.')) : null);
  })();
  return box;
}
/* A lost key: the server mails a code to the address of the purchase, with the code it replaces the key (once per licence) and this computer is activated with the new one. */
function lostKeyDialog(done) {
  const email = h('input', { type: 'text', placeholder: 'name@example.com', autocomplete: 'email', style: { minWidth: '280px' } });
  const code = h('input', { type: 'text', placeholder: 'XXXX-XXXX', autocomplete: 'off', spellcheck: false, style: { minWidth: '160px' } });
  const body = h('div');
  const step1 = () => body.replaceChildren(
    h('p', { class: 'muted' }, T('Enter the e-mail address you bought Aside Pro with. We will send a code to it; your old key keeps working until you enter the code.')),
    h('div', { class: 'row', style: { alignItems: 'center' } }, h('label', null, T('E-mail address')), email));
  const step2 = (minutes) => body.replaceChildren(
    h('p', null, T('If the address belongs to a licence, a code was sent to it (valid for {n} minutes). Enter it here.', { n: minutes })),
    h('div', { class: 'row', style: { alignItems: 'center' } }, h('label', null, T('Code')), code),
    h('p', { class: 'muted' }, T('You get a new key and the old one stops working. Computers where Aside is already activated keep working. This can be done once per licence.')));
  let sent = false;
  step1();
  dialog({ title: T('Lost licence key'), body, buttons: [
    { label: T('Cancel') },
    { label: T('Continue'), kind: 'primary', onclick: async () => {
      try {
        if (!sent) { const r = await post('licence/recover', { email: email.value }); sent = true; step2(r.minutes || 15); return false; }
        const r = await post('licence/recover-confirm', { email: email.value, code: code.value });
        resetFeatures();
        try { await post('licence/activate', { key: r.key }); } catch (e) { fail(e); }
        dialog({ title: T('Your new key'), narrow: true, buttons: [{ label: T('Close'), kind: 'primary' }],
          body: h('div', null, h('p', null, tx('Your new key is {key}. The old key no longer works. It was also sent to your e-mail.', { key: h('b', { class: 'mono' }, r.key) }))) });
        done?.();
      } catch (e) { fail(e); return false; }
    } },
  ] });
}
/* Settings, Licence: the state of the licence of the Pro pack, entering a key, the regular check and freeing this computer. The pack decides everything (it checks the signed
   licence file); this only shows its state and sends the key. Without the pack the state is "none". */
export function licencePanel() {
  const box = h('div');
  async function paint() {
    let st;
    try { st = await get('licence'); } catch (e) { box.replaceChildren(h('div', { class: 'notice bad' }, e.message)); return; }
    const run = async (path, body, done) => {
      try { await post(path, body); resetFeatures(); toast(done, 'ok'); } catch (e) { fail(e); }
      paint();
    };
    const key = h('input', { type: 'text', id: 'lic_key', style: { minWidth: '300px' }, placeholder: 'ASIDE-XXXXX-XXXXX-XXXXX-XXXXX', autocomplete: 'off', spellcheck: false });
    const nameBox = h('input', { type: 'text', id: 'lic_name', maxlength: '40', style: { minWidth: '220px' }, placeholder: T('e.g. Home PC'), autocomplete: 'off' });
    const enter = [h('div', { class: 'row', style: { alignItems: 'center' } }, h('label', { for: 'lic_key' }, T('Licence key')), key,
      h('button', { class: 'btn primary', onclick: () => run('licence/activate', { key: key.value, name: nameBox.value }, T('Licence activated.')) }, T('Activate')),
      h('button', { class: 'btn ghost', onclick: () => lostKeyDialog(paint) }, T('I lost my key'))),
    h('div', { class: 'row', style: { alignItems: 'center' } }, h('label', { for: 'lic_name' }, T('Name of this computer (optional)')), nameBox)];
    const manage = h('div', { class: 'row' },
      h('button', { class: 'btn', onclick: () => run('licence/refresh', {}, T('Licence checked.')) }, T('Check now')),
      h('button', { class: 'btn', onclick: () => run('licence/deactivate', {}, T('Licence removed from this computer.')) }, T('Remove from this computer')));
    const s = st.state;
    const kids = [];
    if (s === 'none') kids.push(h('p', { class: 'muted' }, T('The Pro pack is not installed in this copy of Aside, so there is nothing to license.')));
    else if (s === 'licensed') kids.push(h('p', null, h('b', null, T('Licensed.')), ' ', T('Licence {id}; the next check with the server is due in {n} days.', { id: st.id, n: st.days_to_check })), manage, deviceBox(run));
    else if (s === 'grace') kids.push(h('div', { class: 'notice warn' }, T('The check with the licence server is overdue. Pro keeps working for {n} more days; connect to the internet and press “Check now”.', { n: st.days_left })), manage, deviceBox(run));
    else if (s === 'expired') kids.push(h('div', { class: 'notice bad' }, T('The licence has expired because it could not be checked with the server for too long. Pro works with its limits. Press “Check now” when you are online, or enter your key again.')), manage, ...enter);
    else if (s === 'refused') kids.push(h('div', { class: 'notice bad' }, T('The licence file cannot be used: {reason}', { reason: reasonText(st.reason) })), manage, ...enter);
    else kids.push(h('p', { class: 'muted' }, T('No licence is entered: the Pro features that are free to try work with limits.')), ...enter);
    box.replaceChildren(...kids);
  }
  paint();
  return box;
}
