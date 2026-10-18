import { t as T } from './05-i18n.js';
import { h } from './00-core.js';
import { get, post } from './10-api.js';
import { S } from './11-state.js';
import { dialog, fail, toast } from './13-feedback.js';
import { empty } from './14-renderers.js';
import { refreshHeader } from './17-project.js';
/* The groups of a project (episodes, chapters, parts, days...): what they are called and which numbers exist. Suggested from the file names. */
const WORD_LABEL = () => ({ Episode: T('Episode'), Chapter: T('Chapter'), Part: T('Part'), Day: T('Day'), Update: T('Update'), Act: T('Act'), Folder: T('Folder') });

/* "1-4, 6, 8-9" -> [1,2,3,4,6,8,9]; null when the text is not a list of numbers */
export function parseGroups(text) {
  const out = new Set();
  for (const part of String(text).split(/[,;\s]+/).filter(Boolean)) {
    const m = part.match(/^(\d{1,3})(?:\s*[-–]\s*(\d{1,3}))?$/);
    if (!m) return null;
    const a = +m[1], b = m[2] ? +m[2] : a;
    if (a < 1 || b < a || b - a > 500) return null;
    for (let i = a; i <= b; i++) out.add(i);
  }
  return [...out].sort((x, y) => x - y);
}
/* [1,2,3,4,6] -> "1-4, 6" */
export function formatGroups(list) {
  const out = [];
  for (let i = 0; i < list.length; i++) {
    let j = i;
    while (j + 1 < list.length && list[j + 1] === list[j] + 1) j++;
    out.push(j > i + 1 ? `${list[i]}-${list[j]}` : list.slice(i, j + 1).join(', '));
    i = j;
  }
  return out.join(', ');
}

export async function groupsDialog(done) {
  let data;
  try { data = await get('groups/suggest'); } catch (e) { return fail(e); }
  const body = h('div');
  let close = () => {};
  const words = WORD_LABEL();
  const start = data.current.groups.length ? data.current : data.proposal;
  const sel = h('select', null, data.words.map((w) => h('option', { value: w, selected: w === start.label }, words[w] || w)));
  const list = h('input', { type: 'text', value: formatGroups(start.groups), placeholder: '1-8', style: { width: '320px' } });
  const info = h('p', { class: 'muted' });
  const stem = data.current.series || data.proposal.series || '';
  const seriesBox = h('input', { type: 'checkbox', checked: data.current.groups.length ? !!data.current.series : !!data.proposal.series });
  const seriesRow = stem ? h('label', { class: 'check', style: { display: 'block', margin: '8px 0' } }, seriesBox, ' ',
    T('Files called {a}, {b}2, {b}3… are groups 1, 2, 3…', { a: stem, b: stem })) : null;
  const locked = data.current.sections > 1;
  const folders = data.current.folders.length ? data.current.folders : data.proposal.folders || [];
  const folderBox = h('input', { type: 'checkbox', checked: !!data.current.folders.length || (!data.proposal.groups.length && folders.length > 0) });
  const folderRow = folders.length ? h('label', { class: 'check', style: { display: 'block', margin: '8px 0' } }, folderBox, ' ',
    T('One group per folder ({n} folders): {list}', { n: folders.length, list: folders.slice(0, 6).join(', ') + (folders.length > 6 ? '…' : '') })) : null;
  const apply = h('button', { class: 'btn primary', onclick: async () => {
    const groups = folders.length && folderBox.checked ? [1] : parseGroups(list.value);
    if (!groups || !groups.length) return toast(T('Write the numbers like 1-8 or 1, 2, 5-7.'), 'bad');
    try {
      await post('groups/apply', { label: sel.value, groups, series: stem && seriesBox.checked ? stem : '', folders: folders.length && folderBox.checked ? folders : [] });
      toast(T('Groups saved: {list}', { list: formatGroups(groups) }), 'ok');
      S.files = null; S.overview = null; close(); await refreshHeader(); done?.();
    } catch (e) { fail(e); }
    return false;
  } }, T('Save the groups'));
  const p = data.proposal;
  info.textContent = locked ? T('This project has several sections (seasons) in aside.json; edit the file by hand to change them.')
    : p.groups.length ? T('Suggested from the file names: {label} {list} ({matched} of {total} files).', { label: words[p.label] || p.label, list: formatGroups(p.groups), matched: p.matched, total: p.total })
      : T('No numbered groups (ep1, chapter2, part3, day4, update5…) were found in the file names; write them yourself if the game has them.');
  if (locked) { apply.disabled = true; sel.disabled = true; list.disabled = true; seriesBox.disabled = true; folderBox.disabled = true; }
  body.replaceChildren(
    h('p', { class: 'muted' }, T('Groups are the numbered units you want statistics for, such as episodes or chapters. Files are assigned to them in the Files tab; the suggestion there reads the numbers from the file and folder names.')),
    info,
    h('div', { class: 'toolbar' }, h('b', null, T('They are called:')), sel, h('b', null, T('Numbers:')), list),
    seriesRow,
    folderRow,
    data.current.groups.length ? h('p', { class: 'muted' }, T('Now: {label} {list}', { label: words[data.current.label] || data.current.label, list: formatGroups(data.current.groups) })) : empty(T('No groups yet'), T('Until groups exist, files can only be “General” or unassigned.')));
  close = dialog({ title: T('Groups of the project'), body, buttons: [{ label: T('Cancel') }, { node: apply }] });
}
