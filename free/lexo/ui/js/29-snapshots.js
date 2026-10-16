import { $, h, plural } from './00-core.js';
import { get, post } from './10-api.js';
import { S } from './11-state.js';
import { confirmDialog, dialog, fail, toast } from './13-feedback.js';
import { buildTable, chip, empty, fname, loading } from './14-renderers.js';
import { go } from './16-tabs.js';
import { refreshHeader } from './17-project.js';
/* Zip backups of the translation folder (automatic, before bulk changes, manual). Restoring first saves the current state. */
export const fmtSize = (b) => (b >= 1048576 ? (b / 1048576).toFixed(1).replace('.', ',') + ' MB' : Math.max(1, Math.round(b / 1024)) + ' kB');
const SNAP_REASON = { auto: 'automatická', bulk: 'před hromadnou změnou', manual: 'ručně vytvořená', restore: 'před obnovením' };
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
    if (!r.items.length) return list.replaceChildren(empty('Zatím žádná záloha', 'První se vytvoří sama při dalším spuštění (nebo klikni na „Vytvořit zálohu teď“).'));
    const t = buildTable({ key: 'snapshots', cols: [{ k: 'time', w: 170, label: 'Vytvořeno' }, { k: 'why', w: 200, label: 'Druh' }, { k: 'n', w: 90, label: 'Souborů' }, { k: 'size', w: 100, label: 'Velikost' }, { k: 'act', w: 220, label: '', resize: false }] });
    for (const m of r.items) {
      t.tbody.append(h('tr', null, h('td', null, (m.time || '').replace('T', ' ')), h('td', null, SNAP_REASON[m.reason] || m.reason), h('td', { class: 'num' }, m.files), h('td', { class: 'num' }, fmtSize(m.size)),
        h('td', null, h('button', { class: 'btn small', onclick: () => snapshotRestoreDialog(m.name, paint) }, 'Obnovit…'), ' ',
          h('button', { class: 'btn small ghost', onclick: async () => { if (!await confirmDialog('Smazat zálohu?', `Záloha z ${(m.time || '').replace('T', ' ')} se nenávratně smaže.`, 'Smazat')) return; try { await post('snapshots/delete', { name: m.name }); paint(); } catch (e) { fail(e); } } }, 'Smazat'))));
    }
    list.replaceChildren(h('div', { class: 'tablewrap', style: { maxHeight: '260px' } }, t.table));
  }
  paint();
  return h('div', null,
    h('p', { class: 'muted' }, 'Zip všech souborů .rpy ve složce překladu ve složce ', h('span', { class: 'mono' }, '_progress_data\\snapshots'), '. Vytváří se automaticky při spuštění (když je poslední starší než nastavený počet hodin) a před hromadnými změnami (nahrazení, opravy, sjednocení). Obnovení nejdřív uloží současný stav jako další zálohu.'),
    h('div', { class: 'row', style: { marginBottom: '8px' } }, h('label', { class: 'check' }, on, ' Zálohovat automaticky'), h('span', null, 'každých'), hours, h('span', null, 'hodin, uchovat'), keep, h('span', null, 'záloh'),
      h('span', { class: 'spacer' }), h('button', { class: 'btn small primary', onclick: async () => { try { const r = await post('snapshots/create'); toast(`Záloha vytvořena (${r.snapshot.files} souborů, ${fmtSize(r.snapshot.bytes)}).`); paint(); } catch (e) { fail(e); } } }, 'Vytvořit zálohu teď')),
    list);
}
async function snapshotRestoreDialog(name, done) {
  let data;
  try { data = await get('snapshots/files', { name }); } catch (e) { return fail(e); }
  const STATE = { changed: ['liší se od zálohy', 'warning'], missing: ['ve složce chybí', 'error'], same: ['stejný', 'none'] };
  const sel = new Set(data.files.filter((f) => f.state !== 'same').map((f) => f.file));
  let armed = false;
  const info = h('span', { class: 'grow' }), go = h('button', { class: 'btn primary danger' });
  const rows = data.files.map((f) => h('tr', null, h('td', { class: 'center' }, h('input', { type: 'checkbox', checked: sel.has(f.file), onchange: (e) => { e.target.checked ? sel.add(f.file) : sel.delete(f.file); armed = false; paint2(); } })),
    h('td', null, fname(f.file)), h('td', null, chip(STATE[f.state][1], STATE[f.state][0]))));
  const t = h('table', { class: 'data' }, h('colgroup', null, [44, 520, 180].map((w) => h('col', { style: { width: w + 'px' } }))), h('thead', null, h('tr', null, ['', 'Soubor', 'Stav'].map((x) => h('th', null, x)))), h('tbody', null, rows));
  t.style.width = '744px';
  function paint2() {
    const n = sel.size;
    go.disabled = !n;
    go.textContent = armed ? `Potvrdit: přepsat ${n} ${plural(n, 'soubor', 'soubory', 'souborů')}` : `Obnovit ${n} ${plural(n, 'soubor', 'soubory', 'souborů')}`;
    info.textContent = `Vybráno ${n} z ${data.files.length}`;
  }
  go.onclick = async () => {
    if (!armed) { armed = true; paint2(); setTimeout(() => { armed = false; paint2(); }, 8000); return false; }
    try {
      const r = await post('snapshots/restore', { name, files: [...sel] });
      toast(`Obnoveno ${r.restored.length} ${plural(r.restored.length, 'soubor', 'soubory', 'souborů')}. Předchozí stav je uložený jako záloha „před obnovením“.`, 'ok', 9000);
      S.files = null; await refreshHeader(); done?.();
    } catch (e) { fail(e); return false; }
  };
  S.dlgDirty = true;
  dialog({ title: 'Obnovit ze zálohy', body: h('div', null, h('p', null, 'Vybrané soubory se přepíšou verzí ze zálohy. Co je v nich nového od zálohy, se ztratí z těchto souborů, ale nejdřív se uloží další záloha (druh „před obnovením“), takže jde vrátit i obnovení. Soubory, které v záloze nejsou, zůstanou beze změny.'),
    h('div', { class: 'tablewrap', style: { maxHeight: '46vh' } }, t)), buttons: [{ label: 'Zrušit' }, { node: h('span', { style: { display: 'contents' } }, info, go) }] });
  paint2();
}

