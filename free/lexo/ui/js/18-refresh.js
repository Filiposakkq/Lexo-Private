"use strict";
/* ---------------------------------------------------------------- refresh / undo */
const isDirty = () => !!document.querySelector('.ed[data-dirty="1"]') || !!S.dlgDirty;
async function doRefresh(force = false) {
  if (!S.ready) return;
  const btn = $('#refreshBtn');
  btn.disabled = true; btn.replaceChildren(h('span', { class: 'spin' }), ' Obnovuji…');
  status('Obnovuji soubory…');
  try {
    let r = await post('refresh', { force, dirty: isDirty() });
    if (r.needs_confirm) {
      if (!(await confirmDialog('Neuložené změny', r.message, 'Obnovit přesto'))) { status('Obnovení zrušeno.'); return; }
      r = await post('refresh', { force, discard: true });
    }
    S.lastRefresh = new Date().toLocaleTimeString('cs-CZ');
    const msg = r.changed ? `Obnoveno – ${r.summary}` : 'Obnoveno – beze změn.';
    toast(msg + (r.moved_metadata ? ` Údaje ${r.moved_metadata} přejmenovaných souborů byly přeneseny.` : ''), r.errors.length ? 'bad' : 'ok');
    if (r.changed || r.errors.length) showRefreshReport(r);
    S.files = null; S.ent.items = [];
    await refreshHeader(); render();
  } catch (e) { fail(e); }
  finally { btn.disabled = false; btn.replaceChildren('⟳ Obnovit soubory'); }
}
function showRefreshReport(r) {
  const sec = (title, items) => items.length ? h('div', null, h('h4', null, `${title} (${items.length})`), h('ul', null, items.map((x) => h('li', { class: 'fname' }, typeof x === 'string' ? x : `${x.from}  →  ${x.to}`)))) : null;
  dialog({ title: 'Výsledek obnovení', body: h('div', null,
    sec('Přidané soubory', r.added), sec('Změněné soubory', r.modified), sec('Přejmenované soubory', r.renamed),
    sec('Odebrané soubory (údaje o nich zůstávají uložené)', r.removed),
    r.errors.length ? h('div', { class: 'notice bad' }, h('b', null, 'Problémy: '), r.errors.join(' | ')) : null,
    r.save_problems?.length ? h('div', { class: 'notice warn' }, r.save_problems.join(' | ')) : null),
    buttons: [{ label: 'Zavřít', kind: 'primary' }] });
}
async function doUndo() {
  if (!S.ready) return;
  try {
    const r = await post('undo');
    toast(`Vráceno: ${r.undone}`);
    S.files = null; S.ent.items = [];
    await refreshHeader(); render();
  } catch (e) { fail(e); }
}

