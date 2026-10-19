import { t as T } from './05-i18n.js';
import { h } from './00-core.js';
import { post } from './10-api.js';
import { dialog, fail, toast } from './13-feedback.js';
import { requireFeature } from './38-pro.js';
/* Team work, version 1 (no server): a team is the people who share the translation folder. Everybody can say who they are and see the team and their files; creating the team and giving roles and
   files is for the lead (a Pro feature). Each person writes only their own file in the shared folder, so sharing (git, Dropbox...) never makes a conflict. */
export const ROLE_LABEL = () => ({ lead: T('lead'), translator: T('translator'), proofreader: T('proofreader') });

/* The state of the team of the open project, or null when it cannot be read. */
export async function teamState() {
  try { return await post('team/state', {}); } catch { return null; }
}

/* Roles and assignments as the lead's save wants them, from the state of the team. */
export const rolesOf = (st) => Object.fromEntries(st.members.filter((m) => m.role !== 'lead').map((m) => [m.author, m.role]));

export async function teamDialog(done) {
  let st = await teamState();
  if (!st) return fail(new Error(T('The team could not be read.')));
  let changed = false;
  const finish = () => { if (changed) done?.(); };
  const body = h('div');
  let close = () => {};
  const reload = async (fresh) => { st = fresh || await teamState(); changed = true; paint(); };
  function paint() {
    const kids = [];
    if (!st.me.joined) {
      const name = h('input', { type: 'text', maxlength: '40', placeholder: T('Your name'), style: { width: '240px' } });
      kids.push(h('p', null, T('Say who you are in this project. Your name is shown to the others in the team; it is written in your own file in the shared folder.')),
        h('div', { class: 'toolbar' }, name, h('button', { class: 'btn primary', onclick: async () => {
          try { await reload(await post('team/join', { name: name.value })); } catch (e) { fail(e); }
        } }, T('Join'))));
    } else if (!st.exists) {
      kids.push(h('p', null, T('This project has no team yet. A team is made by the person who leads it (this needs a Pro licence); the others join by opening the same folder and entering their name here.')),
        h('div', { class: 'toolbar' }, h('button', { class: 'btn primary', onclick: async () => {
          if (!await requireFeature('team.manage')) return;
          try { await reload(await post('team/create', {})); } catch (e) { fail(e); }
        } }, T('Make this a team (I am the lead)'))));
    }
    if (st.exists) {
      const labels = ROLE_LABEL();
      const roles = {};
      const rows = st.members.map((m) => {
        const sel = m.role === 'lead' || !st.can_manage
          ? h('span', null, labels[m.role] || m.role)
          : h('select', { onchange: (e) => { roles[m.author] = e.target.value; } }, ['translator', 'proofreader'].map((r) => h('option', { value: r, selected: m.role === r }, labels[r])));
        roles[m.author] = m.role;
        return h('tr', null, h('td', null, h('b', null, m.name || m.author), m.me ? h('span', { class: 'muted' }, ' ' + T('(you)')) : null), h('td', null, sel),
          h('td', { class: 'num' }, m.files));
      });
      kids.push(h('div', { class: 'tablewrap' }, h('table', { class: 'data' },
        h('thead', null, h('tr', null, [T('Name'), T('Role'), T('Files')].map((x) => h('th', null, x)))), h('tbody', null, rows))));
      kids.push(h('p', { class: 'muted' }, st.can_manage
        ? T('You are the lead: give roles here and files in the table of files (the column “Assigned to”). Everybody in the folder sees them.')
        : T('Roles and files are given by the lead. Your files are marked in the table of files.')));
      if (st.my_files.length) kids.push(h('p', null, h('b', null, T('Your files:')), ' ', st.my_files.map((f) => f.split(/[\\/]/).pop()).join(', ')));
      if (st.can_manage) {
        kids.push(h('div', { class: 'toolbar' }, h('button', { class: 'btn primary', onclick: async () => {
          try { await reload(await post('team/set', { roles, assign: st.assign })); toast(T('Saved.'), 'ok'); } catch (e) { fail(e); }
        } }, T('Save roles'))));
      }
    }
    body.replaceChildren(...kids);
  }
  paint();
  close = dialog({ title: T('Team'), body, buttons: [{ label: T('Close'), onclick: finish }], onclose: finish });
  return close;
}

