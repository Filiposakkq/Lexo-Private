"use strict";
/* Machine translation of several lines. Suggestions come in small batches (progress, Stop), are shown in a table you can edit and
   tick, and only the ticked ones are written, as one undoable step. Approved lines are never overwritten. */
async function mtDialog({ picked = [], anchor = null, file = '', scope = '' }, done) {
  let cfg;
  try { cfg = (await get('settings')).mt || {}; } catch (e) { return fail(e); }
  if (!cfg.provider || !cfg.consent || !cfg.has_key) {
    return dialog({ title: 'Strojový překlad není nastavený', narrow: true, buttons: [{ label: 'Zavřít' }, { label: 'Otevřít Nastavení', kind: 'primary', onclick: () => go('settings') }],
      body: h('p', null, 'Nastav službu, klíč k API a souhlas s odesíláním textu v ', h('b', null, 'Nastavení → Strojový překlad'), '. Klíč zůstane jen v tvém počítači a text se odešle až po tvém souhlasu.') });
  }
  const SCENE_R = 15, LIMIT = 500;
  const opts = [];
  if (picked.length) opts.push(['picked', `Vybrané řádky (${picked.length})`]);
  if (anchor) opts.push(['scene', `Scéna kolem otevřeného řádku (±${SCENE_R})`]);
  if (file) opts.push(['file', 'Soubor ve filtru: ' + file.split(/[\\/]/).pop()]);
  opts.push(['all', 'Všechny soubory']);
  let sc = scope && opts.some(([k]) => k === scope) ? scope : opts[0][0];
  let onlyTodo = true, results = [], target = { items: [], chars: 0 }, running = false, stop = false;
  const scopeSel = h('select', { onchange: (e) => { sc = e.target.value; plan(); } }, opts.map(([k, l]) => h('option', { value: k, selected: k === sc }, l)));
  const todoBox = h('input', { type: 'checkbox', checked: true, onchange: (e) => { onlyTodo = e.target.checked; plan(); } });
  const info = h('div', { class: 'muted' }), prog = h('div', { class: 'muted' }), list = h('div');
  const run = h('button', { class: 'btn primary', disabled: true }), stopBtn = h('button', { class: 'btn', style: { display: 'none' }, onclick: () => { stop = true; } }, 'Zastavit');
  const write = h('button', { class: 'btn primary', disabled: true }), sel = new Set();
  const rk = (r) => r.file + '\t' + r.line;
  const flagged = (r) => (r.issues || []).some((i) => i.severity === 'error') || (r.warnings || []).length > 0;
  async function plan() {
    run.disabled = true; info.textContent = 'Počítám…';
    try {
      let lines = null;
      if (sc === 'picked') lines = picked;
      else if (sc === 'scene') { const r = await get('entry/scene', { file: anchor.file, line: anchor.line, radius: SCENE_R }); lines = r.items.map((x) => ({ file: anchor.file, line: x.line })); }
      target = await post('mt/targets', { file: sc === 'file' ? file : '', lines, only_todo: onlyTodo });
    } catch (e) { info.textContent = e.message; return; }
    const n = target.total, over = n > LIMIT;
    info.replaceChildren(`${n} ${plural(n, 'řádek', 'řádky', 'řádků')}, asi ${target.chars.toLocaleString('cs')} znaků se odešle službě ${cfg.provider}.`,
      over ? h('div', { class: 'notice bad' }, `Najednou jde nejvýš ${LIMIT} řádků – přeloží se prvních ${LIMIT}, zbytek spusť znovu.`) : null,
      h('div', null, 'Služba si účtuje podle odeslaného textu – cenu si zkontroluj u svého poskytovatele.'));
    run.disabled = !n || running;
    run.textContent = n ? `Přeložit ${Math.min(n, LIMIT)} ${plural(Math.min(n, LIMIT), 'řádek', 'řádky', 'řádků')}` : 'Není co překládat';
  }
  function paintList() {
    const ok = results.filter((r) => !r.error);
    const rows = ok.map((r) => {
      const inp = h('input', { type: 'text', class: 'mtedit', value: r.suggestion, oninput: (e) => { r.suggestion = e.target.value; } });
      return h('tr', null,
        h('td', { class: 'center' }, h('input', { type: 'checkbox', checked: sel.has(rk(r)), onchange: (e) => { e.target.checked ? sel.add(rk(r)) : sel.delete(rk(r)); paintBtn(); } })),
        h('td', null, fname(r.file), h('span', { class: 'mono muted' }, ` ${r.line}`), r.who ? h('div', { class: 'mono muted' }, speakerLabel(r.who)) : null),
        h('td', { class: 'txt' }, fmtText(r.source ?? '')), h('td', { class: 'txt' }, inp,
          flagged(r) ? h('div', null, ...(r.warnings || []).map((w) => chip('warning', w)), ...(r.issues || []).map((i) => chip(i.severity === 'error' ? 'error' : 'warning', i.message || i.code))) : null));
    });
    const errs = results.filter((r) => r.error);
    list.replaceChildren(
      ok.length ? h('div', { class: 'toolbar' },
        h('button', { class: 'btn small', onclick: () => { ok.forEach((r) => sel.add(rk(r))); paintList(); } }, 'Vybrat vše'),
        h('button', { class: 'btn small', title: 'Odškrtne řádky, u kterých validátor něco našel', onclick: () => { ok.forEach((r) => { if (flagged(r)) sel.delete(rk(r)); else sel.add(rk(r)); }); paintList(); } }, 'Jen bez nálezů'),
        h('button', { class: 'btn small', onclick: () => { sel.clear(); paintList(); } }, 'Nic')) : null,
      ok.length ? h('div', { class: 'tablewrap', style: { maxHeight: '44vh' } }, h('table', { class: 'data' }, h('colgroup', null, [44, 140, 330, 400].map((w) => h('col', { style: { width: w + 'px' } }))),
        h('thead', null, h('tr', null, ['', 'Kde', 'Originál', 'Návrh (jde upravit)'].map((x) => h('th', null, x)))), h('tbody', null, rows))) : null,
      errs.length ? h('p', { class: 'muted' }, `${errs.length} ${plural(errs.length, 'řádek se nepodařil', 'řádky se nepodařily', 'řádků se nepodařilo')}: ${errs[0].error}`) : null);
    paintBtn();
  }
  function paintBtn() { const n = sel.size; write.disabled = !n || running; write.textContent = `Zapsat ${n} ${plural(n, 'řádek', 'řádky', 'řádků')}`; }
  run.onclick = async () => {
    const todo = target.items.slice(0, LIMIT);
    running = true; stop = false; results = []; sel.clear(); run.disabled = true; stopBtn.style.display = ''; let stopped = '';
    for (let i = 0; i < todo.length && !stop; i += 10) {
      prog.textContent = `Překládám… ${Math.min(i + 10, todo.length)} / ${todo.length}`;
      let r;
      try { r = await post('mt/batch', { items: todo.slice(i, i + 10) }); } catch (e) { stopped = e.message; break; }
      for (const x of r.results) { results.push(x); if (!x.error && !flagged(x)) sel.add(rk(x)); }
      paintList();
      if (r.stopped) { stopped = r.stopped; break; }
    }
    prog.textContent = stopped || (stop ? 'Zastaveno.' : 'Hotovo – projdi návrhy a zapiš vybrané.');
    running = false; stopBtn.style.display = 'none'; paintList(); plan();
  };
  write.onclick = async () => {
    const selections = results.filter((r) => !r.error && sel.has(rk(r))).map((r) => ({ file: r.file, line: r.line, text: r.suggestion }));
    try {
      const r = await post('mt/apply', { selections });
      toast(`Zapsáno ${r.changed} ${plural(r.changed, 'řádek', 'řádky', 'řádků')}` + (r.skipped ? `, přeskočeno ${r.skipped}` : '') + '. Zpět: Ctrl+Z.', 'ok', 8000);
      await done?.();
    } catch (e) { fail(e); return false; }
  };
  const body = h('div', null,
    h('p', { class: 'muted' }, 'Anglický text se pošle nastavené službě; nic se nezapíše, dokud nezaškrtneš a nepotvrdíš. Schválené řádky se nikdy nepřepíšou.'),
    h('div', { class: 'toolbar' }, h('label', { class: 'check' }, 'Co: ', scopeSel), h('label', { class: 'check' }, todoBox, ' jen nepřeložené')),
    info, h('div', { class: 'toolbar', style: { margin: '8px 0' } }, run, stopBtn, prog), list);
  dialog({ title: 'Přeložit strojem', body, buttons: [{ label: 'Zavřít' }, { node: h('span', { style: { display: 'contents' } }, write) }] });
  plan();
}

