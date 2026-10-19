import { t as T, tx } from './05-i18n.js';
import { $, dateCz, fmtN, fmtPct, h } from './00-core.js';
import { get, post } from './10-api.js';
import { dialog, fail, toast } from './13-feedback.js';
import { STATUS_LABEL, chip, groupName, loading, sectionName } from './14-renderers.js';
import { go } from './16-tabs.js';
import { requireFeature } from './38-pro.js';
/* ================================================================ PUBLISH */
export async function viewPublish() {
  const settings = await get('settings');
  const root = h('div');
  const out = h('div');
  root.append(h('div', { class: 'notice' }, T('Nothing is sent to GitHub or to the website until you yourself confirm “Publish” in the confirmation window. '),
    T('Generating the data and publishing are two separate steps.')),
    h('div', { class: 'panel', style: { marginBottom: '12px' } }, h('h3', null, T('1. Generate and check the data for the website')),
      h('div', { class: 'panel-body' }, h('p', { class: 'muted' }, tx('Creates {file} in the local project data and checks it (episode totals, percentage range, no file names). It is the same file the website draws from.', { file: h('span', { class: 'mono' }, 'progress.json') })),
        h('button', { class: 'btn primary', onclick: preview }, T('Generate a preview')), out)),
    h('div', { class: 'panel' }, h('h3', null, T('2. Publish to the website')),
      h('div', { class: 'panel-body' },
        settings.web.repo ? h('div', null, h('dl', { class: 'kv' }, h('dt', null, T('Repository')), h('dd', null, settings.web.repo), h('dt', null, T('File in the repository')), h('dd', null, settings.web.path)),
          h('p', { class: 'muted' }, T('A preview is shown: the repository, the branch, what changes, the exact git commands and warnings (for example that a push to the main branch deploys the website live).')),
          h('button', { class: 'btn', onclick: publishPlan }, T('Prepare publishing…')))
          : h('div', null, h('p', null, T('The connection to the website is not set up yet.')), h('button', { class: 'btn', onclick: () => go('settings') }, T('Open Settings'))))));
  async function preview() {
    out.replaceChildren(loading(T('Generating…')));
    try {
      const r = await get('export/preview');
      const d = r.data;
      out.replaceChildren(h('div', { style: { marginTop: '12px' } },
        r.problems.length ? h('div', { class: 'notice bad' }, h('b', null, T('The check failed: ')), r.problems.join(' | ')) : h('div', { class: 'notice', style: { background: 'var(--good-bg)' } }, T('✓ The data passed the check.')),
        h('dl', { class: 'kv' }, h('dt', null, T('Total')), h('dd', null, `${fmtPct(d.translation.percent)} (${fmtN(d.translation.done)} / ${fmtN(d.translation.total)})`),
          h('dt', null, T('Proofreading')), h('dd', null, fmtPct(d.proofreading.percent)), h('dt', null, T('Files')), h('dd', null, d.files.total),
          h('dt', null, T('Finish estimate')), h('dd', null, d.estimate.status === 'ok' ? dateCz(d.estimate.eta) : T('not possible yet (little data)')),
          h('dt', null, T('Local file')), h('dd', { class: 'mono' }, r.path)),
        h('div', { class: 'tablewrap', style: { marginTop: '10px' } }, h('table', { class: 'data' }, h('colgroup', null, [160, 130, 160, 160, 110].map((w) => h('col', { style: { width: w + 'px' } }))),
          h('thead', null, h('tr', null, [T('Season / episode'), T('State'), T('Lines'), T('Percentages'), T('Files')].map((t) => h('th', null, t)))),
          h('tbody', null, d.seasons.map((s) => s.status === 'planned'
            ? h('tr', null, h('td', null, sectionName(s.n)), h('td', null, chip('planned', T('Planned'))), h('td', { colspan: 3, class: 'muted' }, T('no numbers, outside the calculations')))
            : [h('tr', null, h('td', null, h('b', null, sectionName(s.n))), h('td'), h('td', { class: 'num' }, `${fmtN(s.translation.done)} / ${fmtN(s.translation.total)}`), h('td', { class: 'num' }, fmtPct(s.translation.percent)), h('td', { class: 'num' }, s.files)),
              ...d.episodes.filter((e) => e.season === s.n).map((e) => h('tr', null, h('td', null, '   ' + groupName(e.n)), h('td', null, chip(e.status, STATUS_LABEL[e.status])), h('td', { class: 'num' }, `${fmtN(e.translation.done)} / ${fmtN(e.translation.total)}`), h('td', { class: 'num' }, fmtPct(e.translation.percent)), h('td', { class: 'num' }, e.files_total)))]))))));
    } catch (e) { out.replaceChildren(h('div', { class: 'notice bad' }, e.message)); }
  }
  async function publishPlan() {
    if (!await requireFeature('publish.web')) return;
    let info;
    try { info = await post('publish/plan'); } catch (e) { return fail(e); }
    const ack = h('input', { type: 'checkbox' });
    const go_ = h('button', { class: 'btn primary', disabled: true, onclick: async () => {
      go_.disabled = true; go_.textContent = T('Publishing…');
      try {
        const r = await post('publish/confirm', { token: info.token, branch: info.will_clone ? '*clone*' : info.branch });
        toast(r.message, 'ok', 9000); closeFn();
      } catch (e) { fail(e); go_.disabled = false; go_.textContent = T('Publish'); }
      return false;
    } }, T('Publish'));
    ack.addEventListener('change', () => { go_.disabled = !ack.checked || info.blockers.length > 0; });
    const closeFn = dialog({ title: T('Publishing preview'), body: h('div', null,
      info.blockers.map((b) => h('div', { class: 'notice bad' }, b)), info.warnings.map((w) => h('div', { class: 'notice warn' }, w)),
      h('dl', { class: 'kv' }, h('dt', null, T('Repository')), h('dd', null, info.repo_path), h('dt', null, T('Remote')), h('dd', null, info.remote || info.url || '—'),
        h('dt', null, T('Branch')), h('dd', null, info.branch ? `${info.branch} → ${info.upstream || T('(no upstream)')}` : T('(default after cloning)')),
        h('dt', null, T('File')), h('dd', { class: 'mono' }, info.target),
        h('dt', null, T('On the website so far')), h('dd', null, info.current ? T('translation {pct}, updated {when}', { pct: fmtPct(info.current.translation), when: info.current.updated || '?' }) : '—'),
        h('dt', null, T('After publishing')), h('dd', null, info.new ? T('translation {a}, proofreading {b}', { a: fmtPct(info.new.translation), b: fmtPct(info.new.proofreading) }) : '—'),
        h('dt', null, T('Changes')), h('dd', null, info.changed ? T('yes') : T('no (only the time)'))),
      h('h4', { style: { marginTop: '10px' } }, T('These commands will run (with this one file only)')),
      h('div', { class: 'textbox mono' }, info.commands.join('\n')),
      info.dirty.length ? h('div', { class: 'muted' }, T('Other uncommitted changes in the repository (they will not be touched): {files}', { files: info.dirty.slice(0, 5).join(', ') + (info.dirty.length > 5 ? '…' : '') })) : null,
      h('label', { class: 'check', style: { marginTop: '10px' } }, ack, info.live ? T('I understand that the data is sent to GitHub and the website is updated live.') : T('I understand that the data is sent to GitHub.'))),
      buttons: [{ label: T('Cancel') }, { node: go_ }] });
  }
  return root;
}

