import { $, dateCz, fmtN, fmtPct, h, plural } from './00-core.js';
import { get, post } from './10-api.js';
import { S } from './11-state.js';
import { enterProof, enterQuick, pref } from './12-chrome.js';
import { fail, status, toast } from './13-feedback.js';
import { STATUS_LABEL, bar, chip, empty } from './14-renderers.js';
import { go } from './16-tabs.js';
import { refreshHeader, renderStatusRight } from './17-project.js';
/* ================================================================ OVERVIEW */
export async function viewOverview() {
  const o = (S.overview = await get('overview'));
  renderStatusRight();
  const t = o.totals;
  const v = o.validation;
  const root = h('div');
  for (const n of S.notices) root.append(h('div', { class: 'notice' }, n));
  if (o.unsaved.length) root.append(h('div', { class: 'notice warn' }, `Nepodařilo se uložit: ${o.unsaved.join(', ')} (soubor je možná otevřený v Excelu). Zavři ho a klikni na Obnovit soubory.`));
  if (o.last_save_error) root.append(h('div', { class: 'notice warn' }, o.last_save_error));
  if (o.lock_lost) root.append(h('div', { class: 'notice warn' }, 'Projekt mezitím převzala jiná kopie aplikace. Neukládej tady další změny: zavři tuto kopii, nebo projekt otevři znovu (Změnit složku).'));
  if (t.total) root.append(overviewHero(o, S.files || (S.files = (await get('files')).files)));
  root.append(h('div', { class: 'grid cards' },
    h('div', { class: 'panel stat' }, h('div', { class: 'label' }, 'Překlad'),
      h('div', { class: 'value' }, fmtPct(t.percent)), bar(t.percent),
      h('div', { class: 'sub' }, `${fmtN(t.done)} / ${fmtN(t.total)} řádků · z toho ručně schváleno ${fmtN(t.approved)}`)),
    h('div', { class: 'panel stat' }, h('div', { class: 'label' }, 'Zbývá přeložit'),
      h('div', { class: 'value' }, fmtN(t.remaining), h('small', null, 'řádků')),
      h('div', { class: 'sub' }, `${o.file_counts.none + o.file_counts.wip} z ${o.project.files} souborů nedokončeno`)),
    h('div', { class: 'panel stat' }, h('div', { class: 'label' }, 'Korektura'),
      h('div', { class: 'value' }, fmtPct(t.proofread_percent)), bar(t.proofread_percent, 'alt'),
      h('div', { class: 'sub' }, `Zkontrolováno: ${fmtN(t.proofread)} / ${fmtN(t.total)} řádků`),
      h('div', { class: 'sub' }, `řádek po řádku označeno ${fmtN(t.proof_marked)}`, t.proof_fix ? [' · ', h('a', { href: '#', onclick: (ev) => { ev.preventDefault(); go('entries', { file: '', status: 'fix' }); } }, `k opravě ${fmtN(t.proof_fix)}`)] : null),
      t.proof_stale ? h('div', { class: 'sub' }, h('a', { href: '#', onclick: (ev) => { ev.preventDefault(); go('entries', { file: '', status: 'stale' }); } }, `↻ změněno po korektuře ${fmtN(t.proof_stale)}`), ' – překlad se po označení změnil') : null,
      h('div', { class: 'row', style: { marginTop: '8px' } },
        h('button', { class: 'btn small primary', onclick: () => go('entries', { file: '', status: 'review', select: true }) }, 'Pokračovat v korektuře'))),
    h('div', { class: 'panel stat', style: { cursor: 'pointer' }, onclick: () => go('validation') }, h('div', { class: 'label' }, 'Kontrola překladu'),
      h('div', { class: 'value' }, fmtN(v.error), h('small', null, v.error === 1 ? 'chyba' : 'chyb')),
      h('div', { class: 'sub' }, `${fmtN(v.warning)} varování (z toho ${fmtN(v.length)} k délce textu)`),
      v.ignored ? h('div', { class: 'sub' }, h('a', { href: '#', onclick: (ev) => { ev.preventDefault(); ev.stopPropagation(); go('validation', { ignored: 'only' }); } }, `ignorováno ${fmtN(v.ignored)}`)) : null)));
  if (o.goals) root.append(goalsPanel(o.goals));
  root.append(estimatePanel(o.estimate));

  const left = h('div'), right = h('div');
  for (const s of o.seasons) left.append(seasonPanel(s, o.episodes));
  const ug = (label, g) => g.translation.total ? h('li', null, h('span', null, label, h('span', { class: 'muted' }, ` · ${g.files} souborů`)), h('span', { class: 'num' }, `${fmtN(g.translation.done)} / ${fmtN(g.translation.total)} (${fmtPct(g.translation.percent)})`)) : null;
  left.append(h('div', { class: 'panel' }, h('h3', null, 'Mimo epizody'),
    h('ul', { class: 'list' }, ug('Obecné (celá hra)', o.general), ug('Nezařazené soubory', o.unassigned),
      !o.general.translation.total && !o.unassigned.translation.total ? h('li', { class: 'muted' }, 'Všechny soubory jsou zařazené k epizodám.') : null),
    h('div', { class: 'panel-body muted' }, 'Celkový překlad výše zahrnuje i tyto soubory. Procenta epizod a sezón počítají jen soubory zařazené k epizodě.')));

  if (o.worst_files?.length) right.append(h('div', { class: 'panel', style: { marginBottom: '12px' } }, h('h3', null, 'Nejproblémovější soubory'),
    h('ul', { class: 'list' }, o.worst_files.map((w) => h('li', null,
      h('a', { href: '#', title: `${w.errors} chyb, ${w.warnings} varování na ${fmtN(w.lines)} řádků`, onclick: (ev) => { ev.preventDefault(); go('validation', { file: w.name }); } }, w.name.split(/[\\/]/).pop()),
      h('span', { class: 'num' }, `${w.errors ? w.errors + ' ch · ' : ''}${w.warnings} var · ${String(w.score).replace('.', ',')} / 100 řádků`)))),
    h('div', { class: 'panel-body muted' }, 'Řazeno podle počtu problémů na 100 řádků (chyba se počítá třikrát). Klik otevře Kontrolu souboru.')));
  right.append(h('div', { class: 'panel', style: { marginBottom: '12px' } }, h('h3', null, 'Soubory podle stavu'),
    h('ul', { class: 'list' }, ['done', 'rev', 'tr', 'wip', 'none'].map((k) => h('li', null, chip(k, STATUS_LABEL[k]), h('b', { class: 'num' }, o.file_counts[k]))))));
  right.append(h('div', { class: 'panel', style: { marginBottom: '12px' } }, h('h3', null, 'Poslední aktivita'),
    o.recent.length ? h('ul', { class: 'list' }, o.recent.map((r) => h('li', null, h('span', { class: 'muted' }, r.at),
      h('span', { class: 'num' }, `${r.translated >= 0 ? '+' : ''}${fmtN(r.translated)} přeloženo` + (r.proofread ? ` · ${r.proofread >= 0 ? '+' : ''}${fmtN(r.proofread)} korektura` : '')))))
      : empty('Zatím žádná aktivita', 'Záznamy se objeví, jakmile se počty změní.')));
  right.append(h('div', { class: 'panel', style: { marginBottom: '12px' } }, h('h3', null, 'Poslední změny (lze vrátit)'),
    o.undo.length ? h('ul', { class: 'list' }, o.undo.map((u, i) => h('li', null, u, i === 0 ? h('span', { class: 'muted' }, 'Ctrl+Z') : null))) : empty('Nic k vrácení', '')));
  if (o.last_refresh) {
    const r = o.last_refresh;
    right.append(h('div', { class: 'panel' }, h('h3', null, 'Poslední načtení souborů'),
      h('div', { class: 'panel-body' }, h('dl', { class: 'kv' },
        h('dt', null, 'Souborů'), h('dd', null, o.project.files), h('dt', null, 'Trvalo'), h('dd', null, r.seconds + ' s'),
        h('dt', null, 'Přeskočené soubory'), h('dd', null, o.project.skipped.join(', ') || '—')))));
  }
  root.append(h('div', { class: 'two-col' }, left, right));
  return root;
}
/* This week's goal for translating and proofreading. Automatic goals come from the recent weekly pace (+10 %); a goal set in Nastavení wins. */
/* Two concentric rings: the outer one translation, the inner one proofreading. */
function ringChart(translated, proofread, size = 132) {
  const NS = 'http://www.w3.org/2000/svg';
  const svg = document.createElementNS(NS, 'svg');
  svg.setAttribute('viewBox', '0 0 100 100'); svg.setAttribute('width', size); svg.setAttribute('height', size); svg.setAttribute('role', 'img');
  svg.setAttribute('aria-label', `Překlad ${fmtPct(translated)}, korektura ${fmtPct(proofread)}`);
  const ring = (r, pct, color, w) => {
    const c = 2 * Math.PI * r;
    for (const [stroke, len] of [['var(--raise)', c], [color, c * Math.max(0, Math.min(100, pct)) / 100]]) {
      const el = document.createElementNS(NS, 'circle');
      for (const [k, v] of Object.entries({ cx: 50, cy: 50, r, fill: 'none', stroke, 'stroke-width': w, 'stroke-linecap': 'round', 'stroke-dasharray': `${len} ${c}`, transform: 'rotate(-90 50 50)' })) el.setAttribute(k, v);
      svg.append(el);
    }
  };
  ring(42, translated, 'var(--accent)', 9); ring(30, proofread, 'var(--proof)', 7);
  return svg;
}
/* The top of the overview: where you are, and one click to carry on. */
function overviewHero(o, files) {
  const t = o.totals, e = o.estimate || {};
  const last = pref('ui.last', null);
  const near = (files || []).filter((f) => f.todo > 0 && f.done > 0).sort((a, b) => a.todo - b.todo)[0];
  const resume = () => (last && last.file ? go('entries', { file: last.file, line: last.line, status: 'all', select: true }) : go('entries', { file: '', status: 'todo', select: true }));
  return h('div', { class: 'panel hero' },
    h('div', { class: 'hero-ring' }, ringChart(t.percent, t.proofread_percent), h('div', { class: 'hero-pct' }, h('b', null, fmtPct(t.percent)), h('span', { class: 'muted' }, 'přeloženo'))),
    h('div', { class: 'hero-body' },
      h('h2', null, t.remaining ? `Zbývá přeložit ${fmtN(t.remaining)} ${plural(t.remaining, 'řádek', 'řádky', 'řádků')}` : 'Vše je přeloženo'),
      h('div', { class: 'muted' }, [
        e.pace_per_day ? `tempo ≈ ${fmtN(Math.round(e.pace_per_day))} řádků za den` : '',
        e.status === 'ok' && e.eta ? `odhad dokončení ${dateCz(e.eta)}` : '',
        `zbývá zkontrolovat ${fmtN(Math.max(0, t.total - t.proofread))} (${fmtPct(t.proofread_percent)} hotovo)`].filter(Boolean).join(' · ')),
      h('div', { class: 'row', style: { marginTop: '12px', flexWrap: 'wrap' } },
        h('button', { class: 'btn primary', onclick: resume }, last && last.file ? `Pokračovat: ${last.file.split(/[\\/]/).pop()}:${last.line}` : 'Začít překládat'),
        h('button', { class: 'btn', title: 'Scéna a editor přes celou obrazovku, jen nepřeložené řádky', onclick: () => enterQuick() }, 'Rychle překládat'),
        t.total - t.proofread > 0 ? h('button', { class: 'btn', title: 'Scéna a editor přes celou obrazovku, jen řádky ke kontrole (P zkontroluje a skočí dál)', onclick: () => enterProof() }, 'Rychle zkontrolovat') : null,
        near ? h('button', { class: 'btn ghost', title: `Soubor s nejmenším zbytkem: ${near.name}`, onclick: () => go('entries', { file: near.name, status: 'todo', select: true }) }, `Nejblíž dokončení: ${near.name.split(/[\\/]/).pop()} (zbývá ${fmtN(near.todo)})`) : null)));
}
function goalsPanel(g) {
  const row = (label, p, cls) => {
    if (!p.goal) return h('div', { class: 'goalrow' }, h('b', null, label), h('span', { class: 'muted' }, `tento týden ${fmtN(p.done)} ${plural(p.done, 'řádek', 'řádky', 'řádků')} · cíl zatím nejde odvodit z tempa (málo historie) – nastav ho v Nastavení`));
    const msg = { met: ['Splněno', 'done'], on_track: ['Jdeš podle plánu', 'tr'], behind: [`Zbývá ${fmtN(p.left)}, to je asi ${fmtN(p.needed_per_day)} za den`, 'warning'] }[p.status];
    return h('div', { class: 'goalrow' }, h('b', null, label), bar(p.percent, cls),
      h('span', { class: 'num' }, `${fmtN(p.done)} / ${fmtN(p.goal)}`), chip(msg[1], msg[0]),
      h('span', { class: 'muted' }, p.auto ? `cíl automaticky z tvého tempa${p.pace_per_week ? ` (≈ ${fmtN(p.pace_per_week)} / týden)` : ''}` : 'cíl nastavený ručně'));
  };
  return h('div', { class: 'panel', style: { marginBottom: '12px' } }, h('div', { class: 'panel-body' },
    h('div', { class: 'row', style: { justifyContent: 'space-between', marginBottom: '8px' } }, h('b', null, 'Cíl na tento týden'), h('span', { class: 'muted' }, `od pondělí ${dateCz(g.week_start)} · cíle jde změnit v `, h('a', { href: '#', onclick: (ev) => { ev.preventDefault(); go('settings'); } }, 'Nastavení'))),
    row('Překlad', g.translate), row('Korektura', g.proofread, 'alt')));
}
function estimatePanel(e) {
  let body;
  if (e.status === 'ok') {
    const range = e.eta_early && e.eta_late ? ` (rozmezí ${dateCz(e.eta_early)} – ${dateCz(e.eta_late)})` : '';
    body = h('div', null, h('b', null, 'Odhad dokončení překladu: ', dateCz(e.eta)), h('span', { class: 'muted' }, range),
      h('div', { class: 'muted' }, `${e.reason} Spolehlivost: ${{ low: 'nízká', medium: 'střední', high: 'vysoká' }[e.confidence] || '—'}. Zbývá ${fmtN(e.remaining)} řádků.`));
  } else if (e.status === 'complete') body = h('b', null, 'Vše je přeloženo.');
  else body = h('div', null, h('b', null, 'Odhad dokončení zatím nelze spočítat. '), h('span', { class: 'muted' }, e.reason));
  return h('div', { class: 'panel', style: { marginBottom: '12px' } }, h('div', { class: 'panel-body' }, body));
}
function seasonPanel(s, episodes) {
  if (s.status === 'planned') {
    return h('div', { class: 'panel season planned' }, h('div', { class: 'season-head' }, h('h3', null, `Sezóna ${s.n}`), chip('planned', 'Plánováno'), h('span', { class: 'grow' })),
      h('div', { class: 'panel-body muted' }, 'Sezóna je plánovaná do budoucna a zatím se nepřekládá. Nezapočítává se do žádných procent ani do odhadu dokončení.'));
  }
  const eps = episodes.filter((e) => e.season === s.n);
  const head = h('div', { class: 'season-head' }, h('h3', null, `Sezóna ${s.n}`), h('span', { class: 'muted' }, `epizody ${s.episodes[0]}–${s.episodes.at(-1)}`),
    h('span', { class: 'grow' }),
    h('div', { class: 'season-stats' },
      h('div', { class: 'st' }, h('span', { class: 'lbl' }, 'Překlad'), h('b', null, fmtPct(s.translation.percent)), h('div', { class: 'muted num' }, `${fmtN(s.translation.done)} / ${fmtN(s.translation.total)}`)),
      h('div', { class: 'st pr' }, h('span', { class: 'lbl' }, 'Korektura'), h('b', null, fmtPct(s.proofreading.percent)), h('div', { class: 'muted num' }, `${fmtN(s.proofreading.done)} / ${fmtN(s.proofreading.total)}`))));
  const panel = h('div', { class: 'panel season' }, head,
    h('div', { class: 'seasonbars' }, bar(s.translation.percent), bar(s.proofreading.percent, 'alt')));
  for (const e of eps) {
    const sel = h('select', { class: 'sel', value: e.status, 'aria-label': `Stav epizody ${e.n}`, onchange: async () => {
      try { await post('episode', { ep: e.n, state: sel.value }); toast(`Epizoda ${e.n}: ${STATUS_LABEL[sel.value]}`); await refreshHeader(); } catch (err) { fail(err); }
    } }, ['none', 'wip', 'tr', 'proof', 'done'].map((k) => h('option', { value: k }, STATUS_LABEL[k])));
    sel.value = e.status;
    panel.append(h('div', { class: 'ep-row' }, h('span', { class: 'nm' }, `Epizoda ${e.n}`),
      e.translation.total ? h('div', { class: 'bars' }, bar(e.translation.percent), bar(e.proofreading.percent, 'alt')) : h('span', { class: 'muted' }, 'žádné zařazené soubory'),
      h('div', { class: 'pcts' }, h('span', null, 'Překlad ', h('b', null, fmtPct(e.translation.percent))), h('span', { class: 'pr' }, 'Korektura ', h('b', null, fmtPct(e.proofreading.percent)))),
      h('span', { class: 'meta' }, e.files ? `${fmtN(e.translation.done)} / ${fmtN(e.translation.total)} řádků · ${e.files} souborů (${e.files_done} hotových)` : 'bez souborů',
        e.eta_days ? h('div', { title: 'Podle tvého současného tempa (řádků za den)' }, `při tvém tempu ≈ ${fmtN(e.eta_days)} ${plural(e.eta_days, 'den', 'dny', 'dní')}`) : null),
      h('span', { class: 'sel' }, sel)));
  }
  return panel;
}

