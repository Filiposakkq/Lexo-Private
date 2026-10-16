import { labels, locale, tn, t as T, tx } from './05-i18n.js';
import { $, h, plural } from './00-core.js';
import { get, post } from './10-api.js';
import { S } from './11-state.js';
import { confirmDialog, dialog, fail, toast } from './13-feedback.js';
import { buildTable, chip, empty, fname, loading } from './14-renderers.js';
import { go } from './16-tabs.js';
import { refreshHeader } from './17-project.js';
/* Zip backups of the translation folder (automatic, before bulk changes, manual). Restoring first saves the current state. */
export const fmtSize = (b) => (b >= 1048576 ? new Intl.NumberFormat(locale(), { minimumFractionDigits: 1, maximumFractionDigits: 1 }).format(b / 1048576) + ' MB' : Math.max(1, Math.round(b / 1024)) + ' kB');
const SNAP_REASON = labels({ auto: 'automatic', bulk: 'before a bulk change', manual: 'created by hand', restore: 'before a restore' });
export function snapshotsPanel(s) {
  const sn = s.snapshots || { enabled: true, interval_hours: 12, keep: 20 };
  const list = h('div');
  const on = h('input', { type: 'checkbox', id: 'set_snap_on', checked: sn.enabled });
  const hours = h('input', { type: 'text', id: 'set_snap_h', value: String(sn.interval_hours), style: { width: '60px' } });
  const keep = h('input', { type: 'text', id: 'set_snap_keep', value: String(sn.keep), style: { width: '60px' } });
  async function paint() {
    list.replaceChildren(loading());
    let r;
    try { r = await get('snapshots'); } catch (e) { return list.replaceChildren(h('div', { class: 'notice bad' }, e.message)); }
    if (!r.items.length) return list.replaceChildren(empty(T('No backup yet'), T('The first one is created automatically on the next start (or click “Create a backup now”).')));
    const t = buildTable({ key: 'snapshots', cols: [{ k: 'time', w: 170, label: T('Created') }, { k: 'why', w: 200, label: T('Kind') }, { k: 'n', w: 90, label: T('File count') }, { k: 'size', w: 100, label: T('Size') }, { k: 'act', w: 220, label: '', resize: false }] });
    for (const m of r.items) {
      t.tbody.append(h('tr', null, h('td', null, (m.time || '').replace('T', ' ')), h('td', null, SNAP_REASON[m.reason] || m.reason), h('td', { class: 'num' }, m.files), h('td', { class: 'num' }, fmtSize(m.size)),
        h('td', null, h('button', { class: 'btn small', onclick: () => snapshotRestoreDialog(m.name, paint) }, T('Restore…')), ' ',
          h('button', { class: 'btn small ghost', onclick: async () => { if (!await confirmDialog(T('Delete the backup?'), T('The backup from {time} will be deleted for good.', { time: (m.time || '').replace('T', ' ') }), T('Delete'))) return; try { await post('snapshots/delete', { name: m.name }); paint(); } catch (e) { fail(e); } } }, T('Delete')))));
    }
    list.replaceChildren(h('div', { class: 'tablewrap', style: { maxHeight: '260px' } }, t.table));
  }
  paint();
  return h('div', null,
    h('p', { class: 'muted' }, tx('A zip of all .rpy files of the translation folder, saved in the subfolder {folder} of the local project data. It is created automatically at start-up (when the last one is older than the set number of hours) and before bulk changes (replacing, fixes, unifying). A restore first saves the current state as another backup.', { folder: h('span', { class: 'mono' }, 'snapshots') })),
    h('div', { class: 'row', style: { marginBottom: '8px' } }, h('label', { class: 'check' }, on, ' ' + T('Back up automatically')), h('span', null, T('every')), hours, h('span', null, T('hours, keep')), keep, h('span', null, T('backups')),
      h('span', { class: 'spacer' }), h('button', { class: 'btn small primary', onclick: async () => { try { const r = await post('snapshots/create'); toast(T('Backup created ({files} files, {size}).', { files: r.snapshot.files, size: fmtSize(r.snapshot.bytes) })); paint(); } catch (e) { fail(e); } } }, T('Create a backup now'))),
    list);
}
async function snapshotRestoreDialog(name, done) {
  let data;
  try { data = await get('snapshots/files', { name }); } catch (e) { return fail(e); }
  const STATE = { changed: [T('differs from the backup'), 'warning'], missing: [T('missing in the folder'), 'error'], same: [T('same'), 'none'] };
  const sel = new Set(data.files.filter((f) => f.state !== 'same').map((f) => f.file));
  let armed = false;
  const info = h('span', { class: 'grow' }), go = h('button', { class: 'btn primary danger' });
  const rows = data.files.map((f) => h('tr', null, h('td', { class: 'center' }, h('input', { type: 'checkbox', checked: sel.has(f.file), onchange: (e) => { e.target.checked ? sel.add(f.file) : sel.delete(f.file); armed = false; paint2(); } })),
    h('td', null, fname(f.file)), h('td', null, chip(STATE[f.state][1], STATE[f.state][0]))));
  const t = h('table', { class: 'data' }, h('colgroup', null, [44, 520, 180].map((w) => h('col', { style: { width: w + 'px' } }))), h('thead', null, h('tr', null, ['', T('File'), T('State')].map((x) => h('th', null, x)))), h('tbody', null, rows));
  t.style.width = '744px';
  function paint2() {
    const n = sel.size;
    go.disabled = !n;
    go.textContent = armed ? tn(n, 'Confirm: overwrite {n} file', 'Confirm: overwrite {n} files') : tn(n, 'Restore {n} file', 'Restore {n} files');
    info.textContent = T('Selected {n} of {total}', { n, total: data.files.length });
  }
  go.onclick = async () => {
    if (!armed) { armed = true; paint2(); setTimeout(() => { armed = false; paint2(); }, 8000); return false; }
    try {
      const r = await post('snapshots/restore', { name, files: [...sel] });
      toast(T('{files}. The previous state is saved as a backup “before a restore”.', { files: tn(r.restored.length, 'Restored {n} file', 'Restored {n} files') }), 'ok', 9000);
      S.files = null; await refreshHeader(); done?.();
    } catch (e) { fail(e); return false; }
  };
  S.dlgDirty = true;
  dialog({ title: T('Restore from a backup'), body: h('div', null, h('p', null, T('The selected files are overwritten with the version from the backup. What is new in them since the backup is lost from these files, but another backup (kind “before a restore”) is saved first, so even a restore can be undone. Files that are not in the backup stay unchanged.')),
    h('div', { class: 'tablewrap', style: { maxHeight: '46vh' } }, t)), buttons: [{ label: T('Cancel') }, { node: h('span', { style: { display: 'contents' } }, info, go) }] });
  paint2();
}

