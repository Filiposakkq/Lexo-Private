import { locale, tn, t as T, tx } from './05-i18n.js';
import { $, dateCz, fmtN, fmtPct, h, plural } from './00-core.js';
import { get, post } from './10-api.js';
import { S } from './11-state.js';
import { enterProof, enterQuick, pref } from './12-chrome.js';
import { fail, status, toast } from './13-feedback.js';
import { STATUS_LABEL, bar, chip, empty, groupName, sectionName } from './14-renderers.js';
import { groupsDialog } from './37-groups.js';
import { go, render } from './16-tabs.js';
import { refreshHeader, renderStatusRight } from './17-project.js';
/* ================================================================ OVERVIEW */
export async function viewOverview() {
  const o = (S.overview = await get('overview'));
  renderStatusRight();
  const t = o.totals;
  const v = o.validation;
  const root = h('div');
  for (const n of S.notices) root.append(h('div', { class: 'notice' }, n));
  if (o.unsaved.length) root.append(h('div', { class: 'notice warn' }, T('Could not save: {files} (the file may be open in Excel). Close it and click Refresh files.', { files: o.unsaved.join(', ') })));
  if (o.last_save_error) root.append(h('div', { class: 'notice warn' }, o.last_save_error));
  if (o.lock_lost) root.append(h('div', { class: 'notice warn' }, T('The project was taken over by another copy of the application in the meantime. Do not save more changes here: close this copy, or open the project again (Change…).')));
  if (t.total) root.append(overviewHero(o, S.files || (S.files = (await get('files')).files)));
  root.append(h('div', { class: 'grid cards' },
    h('div', { class: 'panel stat' }, h('div', { class: 'label' }, T('Translation')),
      h('div', { class: 'value' }, fmtPct(t.percent)), bar(t.percent),
      h('div', { class: 'sub' }, T('{done} / {total} lines · of which {approved} approved by hand', { done: fmtN(t.done), total: fmtN(t.total), approved: fmtN(t.approved) }))),
    h('div', { class: 'panel stat' }, h('div', { class: 'label' }, T('Left to translate')),
      h('div', { class: 'value' }, fmtN(t.remaining), h('small', null, T('lines'))),
      h('div', { class: 'sub' }, T('{n} of {total} files unfinished', { n: o.file_counts.none + o.file_counts.wip, total: o.project.files }))),
    h('div', { class: 'panel stat' }, h('div', { class: 'label' }, T('Proofreading')),
      h('div', { class: 'value' }, fmtPct(t.proofread_percent)), bar(t.proofread_percent, 'alt'),
      h('div', { class: 'sub' }, T('Proofread: {done} / {total} lines', { done: fmtN(t.proofread), total: fmtN(t.total) })),
      h('div', { class: 'sub' }, T('marked line by line: {n}', { n: fmtN(t.proof_marked) }), t.proof_fix ? [' · ', h('a', { href: '#', onclick: (ev) => { ev.preventDefault(); go('entries', { file: '', status: 'fix' }); } }, T('to fix: {n}', { n: fmtN(t.proof_fix) }))] : null),
      t.proof_stale ? h('div', { class: 'sub' }, h('a', { href: '#', onclick: (ev) => { ev.preventDefault(); go('entries', { file: '', status: 'stale' }); } }, T('↻ changed after proofreading: {n}', { n: fmtN(t.proof_stale) })), T(' – the translation changed after it was marked')) : null,
      h('div', { class: 'row', style: { marginTop: '8px' } },
        h('button', { class: 'btn small primary', onclick: () => go('entries', { file: '', status: 'review', select: true }) }, T('Continue proofreading')))),
    h('div', { class: 'panel stat', style: { cursor: 'pointer' }, onclick: () => go('validation') }, h('div', { class: 'label' }, T('Translation check')),
      h('div', { class: 'value' }, fmtN(v.error), h('small', null, tn(v.error, 'error', 'errors'))),
      h('div', { class: 'sub' }, tn(v.warning, '{n} warning ({length} about text length)', '{n} warnings ({length} about text length)', { n: fmtN(v.warning), length: fmtN(v.length) })),
      v.ignored ? h('div', { class: 'sub' }, h('a', { href: '#', onclick: (ev) => { ev.preventDefault(); ev.stopPropagation(); go('validation', { ignored: 'only' }); } }, T('ignored {n}', { n: fmtN(v.ignored) }))) : null)));
  if (o.goals) root.append(goalsPanel(o.goals));
  root.append(estimatePanel(o.estimate));

  const left = h('div'), right = h('div');
  if (!o.groups_defined) left.append(h('div', { class: 'panel' }, h('div', { class: 'panel-body' }, h('b', null, T('No groups yet')), h('p', { class: 'muted' }, T('Episodes, chapters or parts give you statistics for each of them. Aside can guess them from the file names.')), h('button', { class: 'btn', onclick: () => groupsDialog(() => render()) }, T('Set up groups…')))));
  for (const s of o.seasons) left.append(seasonPanel(s, o.episodes));
  const ug = (label, g) => g.translation.total ? h('li', null, h('span', null, label, h('span', { class: 'muted' }, ' · ' + tn(g.files, '{n} file', '{n} files'))), h('span', { class: 'num' }, `${fmtN(g.translation.done)} / ${fmtN(g.translation.total)} (${fmtPct(g.translation.percent)})`)) : null;
  left.append(h('div', { class: 'panel' }, h('h3', null, T('Outside episodes')),
    h('ul', { class: 'list' }, ug(T('General (the whole game)'), o.general), ug(T('Unassigned files'), o.unassigned),
      !o.general.translation.total && !o.unassigned.translation.total ? h('li', { class: 'muted' }, T('All files are assigned to episodes.')) : null),
    h('div', { class: 'panel-body muted' }, T('The total translation above includes these files too. The percentages of episodes and seasons count only the files assigned to an episode.'))));

  if (o.worst_files?.length) right.append(h('div', { class: 'panel', style: { marginBottom: '12px' } }, h('h3', null, T('Most problematic files')),
    h('ul', { class: 'list' }, o.worst_files.map((w) => h('li', null,
      h('a', { href: '#', title: T('{errors} errors, {warnings} warnings in {lines} lines', { errors: w.errors, warnings: w.warnings, lines: fmtN(w.lines) }), onclick: (ev) => { ev.preventDefault(); go('validation', { file: w.name }); } }, w.name.split(/[\\/]/).pop()),
      h('span', { class: 'num' }, (w.errors ? T('{n} err · ', { n: w.errors }) : '') + T('{n} warn · {score} / 100 lines', { n: w.warnings, score: new Intl.NumberFormat(locale(), { maximumFractionDigits: 1 }).format(w.score) }))))),
    h('div', { class: 'panel-body muted' }, T('Sorted by the number of problems per 100 lines (an error counts three times). Click opens the check of the file.'))));
  right.append(h('div', { class: 'panel', style: { marginBottom: '12px' } }, h('h3', null, T('Files by status')),
    h('ul', { class: 'list' }, ['done', 'rev', 'tr', 'wip', 'none'].map((k) => h('li', null, chip(k, STATUS_LABEL[k]), h('b', { class: 'num' }, o.file_counts[k]))))));
  right.append(h('div', { class: 'panel', style: { marginBottom: '12px' } }, h('h3', null, T('Recent activity')),
    o.recent.length ? h('ul', { class: 'list' }, o.recent.map((r) => h('li', null, h('span', { class: 'muted' }, r.at),
      h('span', { class: 'num' }, T('{n} translated', { n: (r.translated >= 0 ? '+' : '') + fmtN(r.translated) }) + (r.proofread ? ' · ' + T('{n} proofread', { n: (r.proofread >= 0 ? '+' : '') + fmtN(r.proofread) }) : '')))))
      : empty(T('No activity yet'), T('Records appear as soon as the counts change.'))));
  right.append(h('div', { class: 'panel', style: { marginBottom: '12px' } }, h('h3', null, T('Recent changes (can be undone)')),
    o.undo.length ? h('ul', { class: 'list' }, o.undo.map((u, i) => h('li', null, u, i === 0 ? h('span', { class: 'muted' }, 'Ctrl+Z') : null))) : empty(T('Nothing to revert'), '')));
  if (o.last_refresh) {
    const r = o.last_refresh;
    right.append(h('div', { class: 'panel' }, h('h3', null, T('Last loading of files')),
      h('div', { class: 'panel-body' }, h('dl', { class: 'kv' },
        h('dt', null, T('File count')), h('dd', null, o.project.files), h('dt', null, T('Took')), h('dd', null, r.seconds + ' s'),
        h('dt', null, T('Skipped files')), h('dd', null, o.project.skipped.join(', ') || '—')))));
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
  svg.setAttribute('aria-label', T('Translation {a}, proofreading {b}', { a: fmtPct(translated), b: fmtPct(proofread) }));
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
    h('div', { class: 'hero-ring' }, ringChart(t.percent, t.proofread_percent), h('div', { class: 'hero-pct' }, h('b', null, fmtPct(t.percent)), h('span', { class: 'muted' }, T('translated')))),
    h('div', { class: 'hero-body' },
      h('h2', null, t.remaining ? tn(t.remaining, '{n} line left to translate', '{n} lines left to translate', { n: fmtN(t.remaining) }) : T('Everything is translated')),
      h('div', { class: 'muted' }, [
        e.pace_per_day ? T('pace ≈ {n} lines per day', { n: fmtN(Math.round(e.pace_per_day)) }) : '',
        e.status === 'ok' && e.eta ? T('estimated finish {date}', { date: dateCz(e.eta) }) : '',
        T('{n} left to proofread ({pct} done)', { n: fmtN(Math.max(0, t.total - t.proofread)), pct: fmtPct(t.proofread_percent) })].filter(Boolean).join(' · ')),
      h('div', { class: 'row', style: { marginTop: '12px', flexWrap: 'wrap' } },
        h('button', { class: 'btn primary', onclick: resume }, last && last.file ? T('Continue: {where}', { where: `${last.file.split(/[\\/]/).pop()}:${last.line}` }) : T('Start translating')),
        h('button', { class: 'btn', title: T('Scene and editor over the whole screen, untranslated lines only'), onclick: () => enterQuick() }, T('Translate quickly')),
        t.total - t.proofread > 0 ? h('button', { class: 'btn', title: T('Scene and editor over the whole screen, only lines to check (P marks one as checked and moves on)'), onclick: () => enterProof() }, T('Proofread quickly')) : null,
        near ? h('button', { class: 'btn ghost', title: T('File with the least left: {name}', { name: near.name }), onclick: () => go('entries', { file: near.name, status: 'todo', select: true }) }, T('Closest to done: {name} ({n} left)', { name: near.name.split(/[\\/]/).pop(), n: fmtN(near.todo) })) : null)));
}
function goalsPanel(g) {
  const row = (label, p, cls) => {
    if (!p.goal) return h('div', { class: 'goalrow' }, h('b', null, label), h('span', { class: 'muted' }, T('this week {lines} · the goal cannot be derived from the pace yet (little history) – set it in Settings', { lines: tn(p.done, '{n} line', '{n} lines', { n: fmtN(p.done) }) })));
    const msg = { met: [T('Goal met'), 'done'], on_track: [T('On track'), 'tr'], behind: [T('{left} left, that is about {perDay} per day', { left: fmtN(p.left), perDay: fmtN(p.needed_per_day) }), 'warning'] }[p.status];
    return h('div', { class: 'goalrow' }, h('b', null, label), bar(p.percent, cls),
      h('span', { class: 'num' }, `${fmtN(p.done)} / ${fmtN(p.goal)}`), chip(msg[1], msg[0]),
      h('span', { class: 'muted' }, p.auto ? T('goal automatically from your pace') + (p.pace_per_week ? ' ' + T('(≈ {n} / week)', { n: fmtN(p.pace_per_week) }) : '') : T('goal set by hand')));
  };
  return h('div', { class: 'panel', style: { marginBottom: '12px' } }, h('div', { class: 'panel-body' },
    h('div', { class: 'row', style: { justifyContent: 'space-between', marginBottom: '8px' } }, h('b', null, T('Goal for this week')), h('span', { class: 'muted' }, tx('from Monday {date} · goals can be changed in {where}', { date: dateCz(g.week_start), where: h('a', { href: '#', onclick: (ev) => { ev.preventDefault(); go('settings'); } }, T('Settings')) }))),
    row(T('Translation'), g.translate), row(T('Proofreading'), g.proofread, 'alt')));
}
function estimatePanel(e) {
  let body;
  if (e.status === 'ok') {
    const range = e.eta_early && e.eta_late ? ` ${T('(range {a} – {b})', { a: dateCz(e.eta_early), b: dateCz(e.eta_late) })}` : '';
    body = h('div', null, h('b', null, T('Estimated finish of the translation: '), dateCz(e.eta)), h('span', { class: 'muted' }, range),
      h('div', { class: 'muted' }, `${e.reason} ${T('Confidence: {level}. {n} lines left.', { level: ({ low: T('low'), medium: T('medium'), high: T('high') })[e.confidence] || '—', n: fmtN(e.remaining) })}`));
  } else if (e.status === 'complete') body = h('b', null, T('Everything is translated.'));
  else body = h('div', null, h('b', null, T('The finish estimate cannot be calculated yet. ')), h('span', { class: 'muted' }, e.reason));
  return h('div', { class: 'panel', style: { marginBottom: '12px' } }, h('div', { class: 'panel-body' }, body));
}
function seasonPanel(s, episodes) {
  if (s.status === 'planned') {
    return h('div', { class: 'panel season planned' }, h('div', { class: 'season-head' }, h('h3', null, sectionName(s.n)), chip('planned', T('Planned')), h('span', { class: 'grow' })),
      h('div', { class: 'panel-body muted' }, T('The season is planned for the future and is not being translated yet. It does not count in any percentage or in the finish estimate.')));
  }
  const eps = episodes.filter((e) => e.season === s.n);
  const single = (S.overview?.seasons || []).filter((x) => x.status !== 'planned').length === 1;     // one section: no "Season 1" heading
  const head = h('div', { class: 'season-head' }, h('h3', null, single ? T('Progress by group') : sectionName(s.n)), h('span', { class: 'muted' }, s.episodes.length > 1 ? `${groupName(s.episodes[0])} – ${groupName(s.episodes.at(-1))}` : s.episodes.length ? groupName(s.episodes[0]) : ''),
    h('span', { class: 'grow' }),
    h('div', { class: 'season-stats' },
      h('div', { class: 'st' }, h('span', { class: 'lbl' }, T('Translation')), h('b', null, fmtPct(s.translation.percent)), h('div', { class: 'muted num' }, `${fmtN(s.translation.done)} / ${fmtN(s.translation.total)}`)),
      h('div', { class: 'st pr' }, h('span', { class: 'lbl' }, T('Proofreading')), h('b', null, fmtPct(s.proofreading.percent)), h('div', { class: 'muted num' }, `${fmtN(s.proofreading.done)} / ${fmtN(s.proofreading.total)}`))));
  const panel = h('div', { class: 'panel season' }, head,
    h('div', { class: 'seasonbars' }, bar(s.translation.percent), bar(s.proofreading.percent, 'alt')));
  for (const e of eps) {
    const sel = h('select', { class: 'sel', value: e.status, 'aria-label': T('Status of episode {n}', { n: e.n }), onchange: async () => {
      try { await post('episode', { ep: e.n, state: sel.value }); toast(`${groupName(e.n)}: ${STATUS_LABEL[sel.value]}`); await refreshHeader(); } catch (err) { fail(err); }
    } }, ['none', 'wip', 'tr', 'proof', 'done'].map((k) => h('option', { value: k }, STATUS_LABEL[k])));
    sel.value = e.status;
    panel.append(h('div', { class: 'ep-row' }, h('span', { class: 'nm' }, groupName(e.n)),
      e.translation.total ? h('div', { class: 'bars' }, bar(e.translation.percent), bar(e.proofreading.percent, 'alt')) : h('span', { class: 'muted' }, T('no assigned files')),
      h('div', { class: 'pcts' }, h('span', null, T('Translation '), h('b', null, fmtPct(e.translation.percent))), h('span', { class: 'pr' }, T('Proofreading '), h('b', null, fmtPct(e.proofreading.percent)))),
      h('span', { class: 'meta' }, e.files ? T('{done} / {total} lines · {files} files ({finished} finished)', { done: fmtN(e.translation.done), total: fmtN(e.translation.total), files: e.files, finished: e.files_done }) : T('no files'),
        e.eta_days ? h('div', { title: T('At your current pace (lines per day)') }, T('at your pace ≈ {days}', { days: tn(e.eta_days, '{n} day', '{n} days', { n: fmtN(e.eta_days) }) })) : null),
      h('span', { class: 'sel' }, sel)));
  }
  return panel;
}

