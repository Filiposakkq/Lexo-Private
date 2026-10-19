import { t as T, tn } from './05-i18n.js';
import { h } from './00-core.js';
import { post } from './10-api.js';
import { dialog, fail, toast } from './13-feedback.js';
import { requireFeature } from './38-pro.js';
/* Team work, version 1 (no server): a team is the people who share the translation folder. Everybody can say who they are and see the team and their files; creating the team and giving roles and
   files is for the lead (a Pro feature). Each person writes only their own file in the shared folder, so sharing (git, Dropbox...) never makes a conflict. */
export const ROLE_LABEL = () => ({ lead: T('lead'), translator: T('translator'), proofreader: T('proofreader') });

/* The state of the team of the open project, or null when it cannot be read. Remembers whether there is a team (the Lines page shows its notes then). */
export let TEAM_ON = false;
export async function teamState() {
  try { const s = await post('team/state', {}); TEAM_ON = !!s.exists; return s; } catch { return null; }
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
  /* A button that asks "really?" first (one dialog at a time, so the question is in place). */
  function sure(label, question, onYes, cls = 'btn small ghost') {
    const box = h('span');
    const show = () => box.replaceChildren(h('button', { class: cls, onclick: ask }, label));
    const ask = () => box.replaceChildren(h('span', { class: 'muted' }, question + ' '), h('button', { class: 'btn small danger', onclick: onYes }, T('Yes')), ' ', h('button', { class: 'btn small', onclick: show }, T('No')));
    show();
    return box;
  }
  const act = (route, body, done) => async () => {
    try { await reload(await post(route, body)); if (done) toast(done, 'ok'); } catch (e) { fail(e); }
  };
  function paint() {
    const kids = [];
    const labels = ROLE_LABEL();
    if (st.me.removed) {
      kids.push(h('div', { class: 'notice bad' }, T('The lead removed you from this team. Ask the lead to take you back.')));
    } else if (!st.me.joined) {
      const name = h('input', { type: 'text', maxlength: '40', value: st.suggest_name || '', placeholder: T('Your name'), style: { width: '240px' } });
      kids.push(h('p', null, T('Say who you are in this project. Your name is shown to the others in the team; it is written in your own file in the shared folder.')),
        h('div', { class: 'toolbar' }, name, h('button', { class: 'btn primary', onclick: async () => {
          try { await reload(await post('team/join', { name: name.value })); } catch (e) { fail(e); }
        } }, T('Join'))));
    } else if (!st.exists) {
      kids.push(h('p', null, T('This project has no team yet. A team is made by the person who leads it (this needs a Pro licence); the others join by opening the same folder and entering their name here.')),
        h('div', { class: 'toolbar' }, h('button', { class: 'btn primary', onclick: async () => {
          if (!await requireFeature('team.manage')) return;
          try { await reload(await post('team/create', {})); } catch (e) { fail(e); }
        } }, T('Make this a team (I am the lead)')), sure(T('Forget my name here'), T('Really?'), act('team/leave', {}))));
    }
    if (st.exists) {
      const me = st.members.find((m) => m.me);
      if (me) {
        kids.push(h('div', { class: 'toolbar' }, h('span', null, T('You are'), ' ', h('b', null, me.name || me.author), ' · ', labels[me.role] || me.role),
          h('span', { class: 'spacer' }), me.role === 'lead' ? null : sure(T('Leave the team'), T('Leave this team?'), act('team/leave', {}, T('You left the team.')))));
      }
      if (st.me.offered) {
        kids.push(h('div', { class: 'notice warn' }, T('The lead offers you the lead of this team. Taking it needs a Pro licence on this computer; until you accept, the lead stays the lead.'),
          h('div', { class: 'toolbar' }, h('button', { class: 'btn primary', onclick: async () => {
            if (!await requireFeature('team.manage')) return;
            try { await reload(await post('team/accept', {})); toast(T('You are the lead now.'), 'ok'); } catch (e) { fail(e); }
          } }, T('Accept the lead')))));
      }
      if (st.can_manage && st.offer) {
        kids.push(h('div', { class: 'notice' }, T('You offered the lead to {name}; you stay the lead until they accept.', { name: st.offer.name || st.offer.author }), ' ', sure(T('Take the offer back'), T('Take the offer back?'), act('team/offer_cancel', {}))));
      }
      const roles = {};
      const rows = st.members.map((m) => {
        const mine = !!m.me;
        const sel = m.role === 'lead' || !st.can_manage
          ? h('span', null, labels[m.role] || m.role)
          : h('select', { onchange: (e) => { roles[m.author] = e.target.value; } }, ['translator', 'proofreader'].map((r) => h('option', { value: r, selected: m.role === r }, labels[r])));
        roles[m.author] = m.role;
        return h('tr', null, h('td', null, h('b', null, m.name || m.author), mine ? h('span', { class: 'muted' }, ' ' + T('(you)')) : null), h('td', null, sel), h('td', { class: 'num' }, m.files),
          st.can_manage ? h('td', { style: { whiteSpace: 'nowrap' } }, mine ? null : h('span', null,
            st.offer && st.offer.author === m.author ? h('span', { class: 'muted' }, T('offered the lead')) : sure(T('Offer the lead'), T('Offer the lead to this person?'), act('team/offer', { author: m.author }, T('The lead was offered.'))), ' ',
            sure(T('Remove'), T('Remove this person?'), act('team/remove', { author: m.author })))) : null);
      });
      kids.push(h('div', { class: 'tablewrap' }, h('table', { class: 'data' },
        h('thead', null, h('tr', null, [T('Name'), T('Role'), T('Files'), st.can_manage ? '' : null].filter((x) => x !== null).map((x) => h('th', null, x)))), h('tbody', null, rows))));
      if (st.can_manage) {
        kids.push(h('div', { class: 'toolbar' }, h('button', { class: 'btn primary', onclick: async () => {
          try { await reload(await post('team/set', { roles, assign: st.assign })); toast(T('Saved.'), 'ok'); } catch (e) { fail(e); }
        } }, T('Save roles'))));
        if (st.removed.length) {
          kids.push(h('p', { class: 'muted' }, T('Removed:'), ' ', ...st.removed.flatMap((r) => [h('b', null, r.name || r.author), ' ', sure(T('Take back'), T('Take this person back?'), act('team/restore', { author: r.author })), '  '])));
        }
      }
      kids.push(h('p', { class: 'muted' }, st.can_manage
        ? T('You are the lead: give roles here and files in the table of files (the column “Assigned to”). Everybody in the folder sees them.')
        : T('Roles and files are given by the lead. Your files are marked in the table of files.')));
      if (st.my_files.length) kids.push(h('p', null, h('b', null, T('Your files:')), ' ', st.my_files.map((f) => f.split(/[\\/]/).pop()).join(', ')));
      kids.push(newsBox());
      kids.push(gitBox());
      if (st.can_manage) {
        kids.push(statsBox());
        kids.push(h('details', null, h('summary', { class: 'muted' }, T('End the team')),
          h('p', { class: 'muted' }, T('Ending the team removes the file of the lead; the people, their notes and their logs stay in the folder. You can make a new team later.')),
          sure(T('End the team'), T('End this team?'), act('team/disband', {}, T('The team was ended.')), 'btn small danger')));
      }
    }
    if (!st.exists) kids.push(gitBox());
    body.replaceChildren(...kids);
  }
  /* What the others did since your last look (everybody). */
  function newsBox() {
    const box = h('div', { class: 'notice' }, T('Looking for news…'));
    post('team/changes', {}).then((r) => {
      if (!r.total) return box.replaceChildren(T('Nothing new since your last look.'));
      const lines = r.items.map((i) => {
        const k = i.what === 'edited' ? tn(i.n, 'changed {n} line', 'changed {n} lines') : i.what === 'approved' ? tn(i.n, 'approved {n} line', 'approved {n} lines')
          : i.what === 'marked' ? tn(i.n, 'marked {n} line', 'marked {n} lines') : tn(i.n, 'moved {n} file to a group', 'moved {n} files to groups');
        return h('div', null, h('b', null, i.name), ': ', k, ' · ', h('span', { class: 'mono' }, i.file.split(/[\\/]/).pop()));
      });
      box.replaceChildren(h('div', null, h('b', null, T('Since your last look:'))), ...lines,
        r.notes ? h('div', null, tn(r.notes, '{n} new note to the team.', '{n} new notes to the team.')) : null,
        h('div', { class: 'toolbar' }, h('button', { class: 'btn small', onclick: async () => { try { await post('team/seen', {}); box.replaceChildren(T('Nothing new since your last look.')); changed = true; } catch (e) { fail(e); } } }, T('Mark as seen'))));
    }).catch(() => box.replaceChildren());
    return box;
  }
  /* Sharing the folder with git: two buttons for people who do not use a command line (everybody, Free). */
  function gitBox() {
    const box = h('div', { class: 'notice' }, T('Looking at git…'));
    const info = h('div', { class: 'muted' });
    let busy = false;
    async function paintGit(s) {
      try { s = s || await post('git/state', {}); } catch (e) { return box.replaceChildren(h('div', { class: 'notice bad' }, e.message)); }
      if (!s.available) return box.replaceChildren(h('b', null, T('Sharing with git')), h('div', { class: 'muted' }, T('git is not installed on this computer. Install git to get and send changes with a button; any other way of sharing the folder (Dropbox, Syncthing) works without it.')));
      if (!s.repo) return box.replaceChildren(h('b', null, T('Sharing with git')), h('div', { class: 'muted' }, T('This folder is not a git repository, so the buttons are off. Clone the shared repository and open the translation folder inside it.')));
      const run = (route, done) => async () => {
        if (busy) return;
        busy = true; info.textContent = T('Working…');
        try { const r = await post(route, {}); changed = true; await paintGit(r); toast(done(r), 'ok'); } catch (e) { fail(e); await paintGit(); } finally { busy = false; }
      };
      const parts = [];
      parts.push(T('Branch {branch}', { branch: s.branch || '–' }));
      parts.push(s.changes ? tn(s.changes, '{n} changed file here, not sent yet', '{n} changed files here, not sent yet') : T('nothing waiting to be sent'));
      if (s.upstream && s.behind) parts.push(tn(s.behind, '{n} newer change from the others (get it)', '{n} newer changes from the others (get them)'));
      if (s.upstream && s.ahead) parts.push(tn(s.ahead, '{n} saved change not sent yet', '{n} saved changes not sent yet'));
      if (!s.remote) parts.push(T('no server (remote) is set'));
      box.replaceChildren(h('b', null, T('Sharing with git')), h('div', { class: 'muted' }, parts.join(' · ')),
        h('div', { class: 'toolbar' },
          h('button', { class: 'btn', title: T('Gets what the others sent (git pull) and reads the files again. A conflict is not solved for you: nothing changes and you see what git said.'), onclick: run('git/pull', () => T('Changes received.')) }, T('Get changes')),
          h('button', { class: 'btn primary', title: T('Saves your changes of the translation folder as one commit and sends them (git push).'), onclick: run('git/push', (r) => (r.committed ? T('Changes sent.') : T('Nothing new to send; the saved changes were sent.'))) }, T('Send changes')), info));
    }
    paintGit();
    return box;
  }
  /* Statistics per person (the lead, Pro). */
  function statsBox() {
    const out = h('div');
    let days = 30;
    const sel = h('select', { onchange: (e) => { days = +e.target.value; load(); } }, [[7, T('Last 7 days')], [30, T('Last 30 days')], [0, T('All time')]].map(([v, l]) => h('option', { value: v, selected: v === days }, l)));
    async function load() {
      try {
        const r = await post('team/stats', { days: days || null });
        const labels = ROLE_LABEL();
        out.replaceChildren(h('div', { class: 'tablewrap' }, h('table', { class: 'data' },
          h('thead', null, h('tr', null, [T('Name'), T('Role'), T('Files'), T('Translated'), T('Proofread'), T('Changed lines'), T('Approved'), T('Marked'), T('Last activity')].map((x) => h('th', null, x)))),
          h('tbody', null, r.members.map((m) => h('tr', null, h('td', null, h('b', null, m.name || m.author)), h('td', null, labels[m.role] || m.role), h('td', { class: 'num' }, m.files),
            h('td', { class: 'num' }, m.lines ? m.done_pct + ' %' : '–'), h('td', { class: 'num' }, m.lines ? m.proof_pct + ' %' : '–'),
            h('td', { class: 'num' }, m.edited), h('td', { class: 'num' }, m.approved), h('td', { class: 'num' }, m.marked),
            h('td', { class: 'muted' }, m.last ? String(m.last).slice(0, 16).replace('T', ' ') : '–')))))),
          r.unassigned.files ? h('p', { class: 'muted' }, T('Files nobody has: {files} ({pct} % translated).', { files: r.unassigned.files, pct: r.unassigned.done_pct })) : null,
          h('p', { class: 'muted' }, T('“Changed lines” counts texts written in the app since the team was made; approved and marked come from the shared log. The period applies to these numbers, not to the progress of the files.')));
      } catch (e) { out.replaceChildren(h('div', { class: 'notice bad' }, e.message)); }
    }
    load();
    return h('div', null, h('h4', null, T('Statistics')), h('div', { class: 'toolbar' }, sel), out);
  }
  paint();
  close = dialog({ title: T('Team'), body, buttons: [{ label: T('Close'), onclick: finish }], onclose: finish });
  return close;
}


/* The notes of the team on one line (the detail of a line in Lines, when the project has a team): anybody can write one and mark it as dealt with. */
export function notesPane(d) {
  const box = h('div');
  const list = h('div');
  const ta = h('textarea', { rows: 2, style: { width: '100%' }, placeholder: T('A note to the team about this line…') });
  async function paint(r) {
    try { r = r || await post('team/notes', { file: d.file, line: d.line }); } catch (e) { return fail(e); }
    list.replaceChildren(...(r.notes.length ? r.notes.map((n) => h('div', { class: 'memitem' },
      h('div', { class: 'memhead' }, h('b', null, n.name), ' ', h('span', { class: 'muted' }, String(n.t).slice(0, 16).replace('T', ' ')), n.resolved ? h('span', { class: 'chip done' }, T('dealt with')) : null),
      h('div', { class: 'txt', style: n.resolved ? { opacity: '.6', textDecoration: 'line-through' } : {} }, n.text),
      n.resolved ? null : h('div', { class: 'actions' }, h('button', { class: 'btn small', onclick: async () => {
        try { await paint(await post('team/note_resolve', { id: n.id, file: d.file, line: d.line })); } catch (e) { fail(e); }
      } }, T('Mark as dealt with'))))) : [h('div', { class: 'muted' }, T('No note on this line yet.'))]));
  }
  paint();
  box.append(list, ta, h('div', { class: 'toolbar' }, h('button', { class: 'btn primary', onclick: async () => {
    try { const r = await post('team/note_add', { file: d.file, line: d.line, text: ta.value }); ta.value = ''; await paint(r); } catch (e) { fail(e); }
  } }, T('Add the note'))));
  return box;
}
