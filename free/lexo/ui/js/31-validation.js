"use strict";
/* ================================================================ VALIDATION */
async function viewValidation() {
  const o = S.goOpts; S.goOpts = null;
  if (o?.file !== undefined) setPref('validation.file', o.file);
  if (o?.ignored !== undefined) setPref('validation.ignored', o.ignored);
  const settings = await get('settings');
  const root = h('div');
  const list = h('div'), counter = h('span', { class: 'muted' });
  const files = S.files || (S.files = (await get('files')).files);
  const v = pref('validation', {});
  const SEV_OPTS = [['', 'Chyby + varování'], ['error', 'Chyby'], ['warning', 'Varování'], ['info', 'Drobnosti']];
  const CAT_OPTS = [['', 'Všechny druhy'], ['syntax', 'Značky a proměnné'], ['structure', 'Struktura (mluvčí, bloky)'], ['length', 'Délka textu'], ['text', 'Text (čísla, angličtina, překlepy)']];
  const cat = h('select', { title: 'Druh nálezu', onchange: (ev) => { setPref('validation.category', ev.target.value); load(true); } }, CAT_OPTS.map(([k, l]) => h('option', { value: k }, l)));
  cat.value = v.category || '';
  const fsel = h('select', { title: 'Soubor', style: { maxWidth: '320px' }, onchange: (ev) => { setPref('validation.file', ev.target.value); load(true); } }, h('option', { value: '' }, 'Všechny soubory'), files.map((f) => h('option', { value: f.name }, f.name)));
  fsel.value = v.file || '';
  const search = h('input', { type: 'search', class: 'search', id: 'search', placeholder: 'Hledat ve zprávách a překladu… (Ctrl+F)', value: v.q || '', oninput: (ev) => { setPref('validation.q', ev.target.value); clearTimeout(S.valTimer); S.valTimer = setTimeout(() => load(true), 250); } });
  const info = h('label', { class: 'check', title: 'Například jiný počet pauz {w} / {p}' }, h('input', { type: 'checkbox', checked: !!settings.validation.show_info, onchange: async (ev) => { try { await post('settings', { validation: { show_info: ev.target.checked } }); load(true); } catch (e) { fail(e); } } }), 'Zobrazit i drobnosti');
  const findBtn = h('button', { title: 'Nálezy kontroly překladu' }, 'Nálezy');
  const consBtn = h('button', { title: 'Stejný anglický řádek přeložený různě' }, 'Rozdílné překlady');
  const ignBtn = h('button', { title: 'Nálezy, které jsi u některých řádků nechal ignorovat' }, 'Ignorované');
  const lintBtn = h('button', { title: 'Spustí Ren\'Py lint celé hry (potřebuje Ren\'Py SDK)' }, 'Lint hry');
  const consOn = () => !!pref('validation.consistency', false);
  const lintOn = () => !!pref('validation.lint', false);
  const ignOn = () => pref('validation.ignored', 'hide') === 'only';
  const special = () => consOn() || lintOn() || ignOn();
  const paintIgnBtn = () => { const n = S.overview?.validation?.ignored || 0; ignBtn.textContent = `Ignorované (${n})`; ignBtn.classList.toggle('on', ignOn()); };
  const paintConsBtn = () => consBtn.classList.toggle('on', consOn());
  const paintLintBtn = () => { lintBtn.classList.toggle('on', lintOn()); findBtn.classList.toggle('on', !special()); };
  const setMode = (m) => { setPref('validation.consistency', m === 'cons'); setPref('validation.lint', m === 'lint'); setPref('validation.ignored', m === 'ign' ? 'only' : 'hide'); paintIgnBtn(); paintConsBtn(); paintLintBtn(); load(true); };
  findBtn.onclick = () => setMode(''); consBtn.onclick = () => setMode('cons'); ignBtn.onclick = () => setMode('ign'); lintBtn.onclick = () => setMode('lint');
  const modeSeg = h('div', { class: 'seg', role: 'group', 'aria-label': 'Co zobrazit' }, findBtn, consBtn, ignBtn, lintBtn);
  const undoFix = h('button', { class: 'btn small', style: { display: 'none' } });
  async function loadLast() {
    try {
      const r = await get('fix/last');
      undoFix.style.display = r.last ? '' : 'none';
      if (r.last) { undoFix.textContent = `↶ Vrátit: ${r.last.label || 'poslední změnu'}`; undoFix.title = `Provedeno ${r.last.time.replace('T', ' ')}. Soubory se vrátí ze zálohy, ale jen když se od opravy neměnily.`; }
    } catch { undoFix.style.display = 'none'; }
  }
  undoFix.onclick = async () => {
    if (!await confirmDialog('Vrátit poslední změnu souboru?', 'Soubory změněné poslední změnou (oprava mluvčích nebo úprava textu) se vrátí do stavu před ní, ze zálohy. Pokud se od té doby změnily, vrácení se odmítne.', 'Vrátit změnu')) return;
    try { const r = await post('fix/undo'); toast(`Vráceno: ${r.label || 'poslední změna'} (${r.lines} řádků).`); S.files = null; await refreshHeader(); load(true); } catch (e) { fail(e); }
  };
  const fixAll = h('button', { class: 'btn small', title: 'Bezpečné opravy: mluvčí / atributy podle originálu, dvojité mezery, překlepy. Ukáže náhled, kde si každý řádek zaškrtneš.', onclick: () => fixDialog({ file: pref('validation.file', '') }, () => load(true)) }, 'Opravit…');
  const pillsBox = h('div', { class: 'frow pills', role: 'group', 'aria-label': 'Závažnost' }), chipsBox = h('div', { class: 'frow fchips' });
  function resetFilters() {
    for (const k of ['file', 'category', 'q', 'severity']) setPref('validation.' + k, '');
    search.value = ''; fsel.value = ''; cat.value = '';
    load(true);
  }
  function paintFilters() {
    const ignoredMode = special();
    const sc = S.val.sevCounts;
    const showInfo = !!settings.validation.show_info;
    const counts = sc ? { '': sc.error + sc.warning + (showInfo ? sc.info : 0), error: sc.error, warning: sc.warning, info: sc.info } : null;
    pillsBox.replaceChildren(...(ignoredMode ? [] : pillRow(SEV_OPTS, pref('validation.severity', ''), counts, (k) => { setPref('validation.severity', k); load(true); })));
    const active = [];
    const lab = (list, k) => (list.find(([x]) => x === k) || [k, k])[1];
    if (pref('validation.file', '')) active.push(['Soubor: ' + pref('validation.file', '').split(/[\\/]/).pop(), () => { setPref('validation.file', ''); fsel.value = ''; load(true); }]);
    if (pref('validation.category', '')) active.push(['Druh: ' + lab(CAT_OPTS, pref('validation.category', '')), () => { setPref('validation.category', ''); cat.value = ''; load(true); }]);
    if (pref('validation.severity', '')) active.push(['Závažnost: ' + lab(SEV_OPTS, pref('validation.severity', '')), () => { setPref('validation.severity', ''); load(true); }]);
    if (pref('validation.q', '')) active.push([`Hledání: „${pref('validation.q', '')}“`, () => { setPref('validation.q', ''); search.value = ''; load(true); }]);
    chipsBox.replaceChildren(...activeChips(active, resetFilters));
  }
  root.append(h('div', { class: 'filterbar' },
      h('div', { class: 'frow' }, filterGroup('Zobrazit', modeSeg), h('span', { class: 'spacer' }), undoFix, fixAll),
      h('div', { class: 'frow' }, search, filterGroup('Soubor', fsel), filterGroup('Druh', cat), info),
      h('div', { class: 'frow' }, pillsBox, h('span', { class: 'spacer' }), counter), chipsBox),
    h('p', { class: 'muted' }, 'Jeden řádek překladu = jedna položka se všemi nálezy pod sebou. Kontrola nic nepřepisuje sama; jen tlačítko ', h('b', null, 'Opravit'), ' u chyby mluvčího / atributů po tvém potvrzení zapíše opravu podle originálu do souboru (se zálohou). ', h('b', null, 'Chyba'), ' = Ren\'Py by pravděpodobně spadl nebo je rozbitá struktura skriptu. ', h('b', null, 'Varování'), ' = podezřelé, ale může být záměr (např. češtině chybí značka, text je výrazně delší). Porovnává se vždy s originálem, ne s pevnými pravidly.'), list);
  S.val.items = [];
  async function loadLint() {
    let st, cfg;
    try { [st, cfg] = await Promise.all([get('lint/status'), get('settings')]); } catch (e) { return list.replaceChildren(h('div', { class: 'notice bad' }, e.message)); }
    paintFilters();
    const sdk = h('input', { type: 'text', value: cfg.lint?.sdk || '', placeholder: 'Složka Ren\'Py SDK, např. C:\\renpy-8.1.1-sdk', style: { flex: 1, minWidth: '320px' } });
    const run = h('button', { class: 'btn primary', disabled: st.state === 'running', onclick: async () => { try { await post('settings', { lint: { sdk: sdk.value } }); await post('lint/start'); loadLint(); } catch (e) { fail(e); } } }, st.state === 'running' ? 'Běží…' : 'Uložit cestu a spustit lint');
    const head = h('div', null, h('div', { class: 'notice' }, 'Ren\'Py lint zkontroluje celou hru (chybějící obrázky, neznámá návěští, rozbité překlady) a najde věci, které naše kontroly nevidí. Potřebuje složku Ren\'Py SDK a zdrojové soubory hry (game/…) nad složkou překladu. Může trvat minutu i déle; aplikace se mezitím dá používat dál.'),
      h('div', { class: 'row', style: { margin: '8px 0 12px' } }, sdk, run));
    let body;
    if (st.state === 'running') { counter.textContent = ''; body = loading(`Lint běží… ${Math.round(st.elapsed || 0)} s`); S.lintTimer = setTimeout(() => { if (lintOn() && S.view === 'validation') loadLint(); }, 1500); }
    else if (st.state === 'error') { counter.textContent = ''; body = h('div', { class: 'notice bad' }, st.error); }
    else if (st.state === 'done') {
      counter.textContent = `${fmtN(st.issues.length)} ${plural(st.issues.length, 'nález', 'nálezy', 'nálezů')} · ${st.seconds} s`;
      const rows = st.issues.map((i) => h('tr', null, h('td', null, fname(i.file), h('span', { class: 'mono muted' }, `řádek ${i.line}`)), h('td', null, i.message),
        h('td', null, i.rel ? h('button', { class: 'btn small', onclick: () => go('entries', { file: i.rel, line: i.line, status: 'all' }) }, 'Zobrazit') : null)));
      const t = h('table', { class: 'data' }, h('colgroup', null, [320, 700, 110].map((w) => h('col', { style: { width: w + 'px' } }))), h('thead', null, h('tr', null, ['Soubor : řádek', 'Hlášení', ''].map((x) => h('th', null, x)))), h('tbody', null, rows));
      body = h('div', null, h('div', { class: 'notice ' + (st.ok ? '' : 'warn') }, st.ok ? 'Lint doběhl bez chyby.' : `Lint skončil s kódem ${st.exit_code} – podívej se na výstup níže.`),
        rows.length ? h('div', { class: 'tablewrap' }, t) : h('div', { class: 'muted' }, 'Lint nehlásí žádný řádek ve tvaru soubor:řádek.'),
        h('details', { style: { marginTop: '10px' } }, h('summary', null, 'Celý výstup lintu'), h('pre', { class: 'ctxcode', style: { maxHeight: '320px' } }, st.raw || '')));
    } else { counter.textContent = ''; body = h('div', { class: 'muted' }, 'Lint ještě nebyl spuštěn.'); }
    list.replaceChildren(head, body);
  }
  async function loadConsistency() {
    list.replaceChildren(loading());
    try {
      const loose = !!pref('consistency.loose', false), bySp = !!pref('consistency.bySpeaker', false);
      const r = await get('consistency', { loose: loose ? '1' : '', by_speaker: bySp ? '1' : '' });
      const hidden = new Set(pref('consistency.hidden', []));
      const showHidden = !!pref('consistency.showHidden', false);
      const groups = r.groups.filter((g) => showHidden || !hidden.has(g.key));
      counter.textContent = `${fmtN(groups.length)} ${plural(groups.length, 'originál', 'originály', 'originálů')} s víc překlady (${fmtN(r.lines)} řádků celkem)`;
      paintFilters();
      const opt = (key, def, label, title) => h('label', { class: 'check', title }, h('input', { type: 'checkbox', checked: !!pref(key, def), onchange: (e) => { setPref(key, e.target.checked); loadConsistency(); } }), ' ' + label);
      const note = h('div', { class: 'notice' },
        h('div', null, 'Stejný anglický řádek přeložený různě. ', h('b', null, 'Porovnává se přesný text originálu'), ' – „Yeah.“, „Yeah!“ a „YEAH!!!“ jsou různé řádky a správně se překládají různě, takže se nikdy nespojí. Stejný text může mít i jiný význam nebo mluvčího, proto se nic nesjednocuje samo: u varianty klikni ', h('b', null, 'Sjednotit…'), ', vyber řádky, kterých se to týká, a potvrď (jde vrátit). ', h('b', null, 'Nechat jak je'), ' skupinu schová.'),
        h('div', { class: 'row', style: { marginTop: '6px', gap: '16px' } },
          opt('consistency.bySpeaker', false, 'Rozlišovat mluvčího', 'Řádky různých postav se nikdy nespojí do jedné skupiny'),
          opt('consistency.loose', false, 'Volnější srovnání (jen slova)', 'Ignoruje velikost písmen a interpunkci – najde i „Yeah.“ vs „YEAH!!!“, jen jako nápovědu, sjednocovat je většinou nechceš'),
          opt('consistency.showHidden', false, `Zobrazit i skryté (${hidden.size})`, '')));
      if (!groups.length) return list.replaceChildren(note, empty('Vše je jednotné', 'Žádný opakovaný originál nemá víc různých překladů.'));
      const t = buildTable({ key: 'consistency', cols: [{ k: 'src', w: 300, label: 'Originál' }, { k: 'var', w: 900, label: 'Varianty překladu' }, { k: 'act', w: 140, label: '', resize: false }] });
      for (const g of groups) {
        const variants = g.variants.map((v) => h('div', { class: 'variant' },
          h('div', { class: 'row' }, h('span', { class: 'chip none' }, `×${v.count}`), h('span', { class: 'txt grow' }, fmtText(v.target)),
            h('button', { class: 'btn small', title: 'Vybereš řádky a změníš je na tuhle variantu', onclick: () => unifyDialog(g, v, loose, bySp, () => loadConsistency()) }, 'Sjednotit…')),
          h('div', { class: 'vlines' }, v.lines.slice(0, 6).map((l) => h('a', { href: '#', class: 'mono', title: l.who ? `mluvčí: ${l.who}` : '', onclick: (ev) => { ev.preventDefault(); go('entries', { file: l.file, line: l.line, status: 'all' }); } }, `${l.file.split(/[\\/]/).pop()}:${l.line}${l.who ? ' (' + l.who.split(' ')[0] + ')' : ''}`)), v.lines.length > 6 ? ` … +${v.lines.length - 6}` : '')));
        t.tbody.append(h('tr', null,
          h('td', { class: 'txt' }, (g.sources && g.sources.length > 1) ? g.sources.map((x) => h('div', null, fmtText(x))) : fmtText(g.source), h('div', { class: 'muted' }, `${g.total}× celkem`)),
          h('td', null, ...variants),
          h('td', null, h('button', { class: 'btn small ghost', title: 'Tahle skupina je v pořádku, schovat ji ze seznamu', onclick: () => { const hs = new Set(pref('consistency.hidden', [])); hs.has(g.key) ? hs.delete(g.key) : hs.add(g.key); setPref('consistency.hidden', [...hs]); loadConsistency(); } }, hidden.has(g.key) ? 'Zase hlídat' : 'Nechat jak je'))));
      }
      list.replaceChildren(note, h('div', { class: 'tablewrap' }, t.table));
    } catch (e) { list.replaceChildren(h('div', { class: 'notice bad' }, e.message)); }
  }
  async function loadIgnored() {
    list.replaceChildren(loading());
    try {
      const r = await get('ignore/list');
      counter.textContent = `${fmtN(r.items.length)} ${plural(r.items.length, 'ignorovaný nález', 'ignorované nálezy', 'ignorovaných nálezů')}`;
      paintFilters();
      if (!r.items.length) return list.replaceChildren(empty('Nic neignoruješ', 'U nálezů v seznamu Kontrola je tlačítko Ignorovat. Ignorované se objeví tady.'));
      const t = buildTable({ key: 'ignored', cols: [{ k: 'st', w: 130, label: 'Stav' }, { k: 'loc', w: 280, label: 'Soubor : řádek' }, { k: 'msg', w: 380, label: 'Nález' }, { k: 'note', w: 260, label: 'Poznámka' }, { k: 'tgt', w: 320, label: 'Překlad' }, { k: 'act', w: 170, label: '', resize: false }] });
      for (const it of r.items) {
        const [lab, cls, tip] = IGN_STATE[it.state];
        t.tbody.append(h('tr', null, h('td', { title: tip }, chip(cls, lab)), h('td', null, fname(it.file), h('span', { class: 'mono muted' }, it.line ? `řádek ${it.line}` : '')),
          h('td', null, it.message || h('span', { class: 'muted' }, it.code)), h('td', null, it.note || h('span', { class: 'muted' }, '—'), it.date ? h('div', { class: 'muted', style: { fontSize: '11.5px' } }, it.date) : null),
          h('td', { class: 'txt' }, fmtText(it.target)), h('td', null,
            it.line ? h('button', { class: 'btn small', onclick: () => go('entries', { file: it.file, line: it.line, status: 'all' }) }, 'Zobrazit') : null, ' ',
            h('button', { class: 'btn small', title: 'Nález se znovu začne hlásit', onclick: () => unignore([it], () => { loadIgnored(); }) }, 'Přestat ignorovat'))));
      }
      list.replaceChildren(h('div', { class: 'tablewrap' }, t.table));
    } catch (e) { list.replaceChildren(h('div', { class: 'notice bad' }, e.message)); }
  }
  async function load(reset) {
    paintIgnBtn(); paintConsBtn(); paintLintBtn();
    clearTimeout(S.lintTimer);
    if (lintOn()) return loadLint();
    if (consOn()) return loadConsistency();
    if (pref('validation.ignored', 'hide') === 'only') return loadIgnored();
    if (reset) { S.val.items = []; list.replaceChildren(loading()); loadLast(); }
    try {
      const q = { severity: pref('validation.severity', ''), category: pref('validation.category', ''), file: pref('validation.file', ''), q: pref('validation.q', ''), offset: S.val.items.length, limit: 200, group: 'line' };
      const r = await get('validation', q);
      S.val.items.push(...r.items); S.val.total = r.total; S.val.sevCounts = r.sev_counts;
      const nf = r.findings ?? r.total;
      counter.textContent = `${fmtN(r.total)} ${plural(r.total, 'řádek', 'řádky', 'řádků')} s nálezem · ${fmtN(nf)} ${plural(nf, 'nález', 'nálezy', 'nálezů')}` + (S.val.items.length < r.total ? ` (zobrazeno ${fmtN(S.val.items.length)})` : '');
      paintFilters();
      paintList();
    } catch (e) { list.replaceChildren(h('div', { class: 'notice bad' }, e.message)); }
  }
  function findingRow(g, x) {
    return h('div', { class: 'finding ' + x.severity },
      chip(x.severity, SEV_LABEL[x.severity]), h('span', { class: 'fmsg' }, x.message),
      h('span', { class: 'fact' },
        x.extra?.fixable ? h('button', { class: 'btn small primary', title: x.extra.kind === 'speaker' ? `Zkopíruje mluvčího a atributy z originálu („${g.src_who}“)` : 'Ukáže náhled opravy', onclick: () => fixDialog({ items: [{ file: g.file, line: g.line }], kinds: [x.extra.kind || 'speaker'] }, () => load(true)) }, 'Opravit') : null,
        g.line && x.code !== 'duplicate-id' ? h('button', { class: 'btn small ghost', title: 'Schová tenhle nález u tohoto řádku (jde vrátit)', onclick: () => ignoreDialog([{ file: g.file, line: g.line, code: x.code, severity: x.severity, message: x.message }], () => load(true)) }, 'Ignorovat…') : null));
  }
  function paintList() {
    if (!S.val.items.length) return list.replaceChildren(empty('Žádné nálezy', pref('validation.q', '') || pref('validation.file', '') || pref('validation.category', '') || pref('validation.severity', '') ? 'Pro tyhle filtry nebyl nalezen žádný problém. Zkus filtry zrušit.' : 'Překlad nemá žádný problém.'));
    const t = buildTable({ key: 'validation2', cols: [{ k: 'sev', w: 110, label: 'Závažnost' }, { k: 'loc', w: 220, label: 'Soubor : řádek' }, { k: 'msg', w: 560, label: 'Nálezy na řádku' }, { k: 'src', w: 300, label: 'Originál' }, { k: 'tgt', w: 300, label: 'Překlad' }, { k: 'act', w: 150, label: '', resize: false }] });
    for (const g of S.val.items) {
      const ignorable = g.issues.filter((x) => g.line && x.code !== 'duplicate-id');
      t.tbody.append(h('tr', { class: g.severity === 'error' ? 'has-error' : '' },
        h('td', null, chip(g.severity, SEV_LABEL[g.severity]), g.issues.length > 1 ? h('div', { class: 'muted', style: { fontSize: '11.5px', marginTop: '3px' } }, `${g.issues.length} nálezů`) : null),
        h('td', null, fname(g.file), h('span', { class: 'mono muted' }, `řádek ${g.line}`)),
        h('td', null, ...g.issues.map((x) => findingRow(g, x)),
          ignorable.length > 1 ? h('div', { class: 'fall' }, h('button', { class: 'btn small ghost', title: 'Schová všechny nálezy tohoto řádku najednou', onclick: () => ignoreDialog(ignorable.map((x) => ({ file: g.file, line: g.line, code: x.code, severity: x.severity, message: x.message })), () => load(true)) }, `Ignorovat všech ${ignorable.length}…`)) : null),
        h('td', { class: 'txt' }, fmtText(g.source)), h('td', { class: 'txt' }, fmtText(g.target)),
        h('td', null, h('button', { class: 'btn small', title: 'Zobrazit v seznamu řádků (tam jde text rovnou upravit)', onclick: () => go('entries', { file: g.file, line: g.line, status: 'all' }) }, 'Zobrazit'), ' ',
          h('button', { class: 'btn small ghost', title: 'Otevřít v externím editoru', onclick: () => openEditor(g.file, g.line) }, 'Editor'))));
    }
    list.replaceChildren(h('div', { class: 'tablewrap' }, t.table), S.val.items.length < S.val.total ? h('div', { class: 'more' }, h('button', { class: 'btn', onclick: () => load(false) }, `Načíst další (${fmtN(S.val.items.length)} z ${fmtN(S.val.total)} řádků)`)) : null);
  }
  root.afterMount = () => load(true);
  return root;
}

