import { $, dateCz, fmtN, fmtPct, h } from './00-core.js';
import { get, post } from './10-api.js';
import { dialog, fail, toast } from './13-feedback.js';
import { STATUS_LABEL, chip, loading } from './14-renderers.js';
import { go } from './16-tabs.js';
/* ================================================================ PUBLISH */
export async function viewPublish() {
  const settings = await get('settings');
  const root = h('div');
  const out = h('div');
  root.append(h('div', { class: 'notice' }, 'Nic se nepošle na GitHub ani na web, dokud v potvrzovacím okně sám nepotvrdíš „Publikovat“. ',
    'Generování dat a publikování jsou dva oddělené kroky.'),
    h('div', { class: 'panel', style: { marginBottom: '12px' } }, h('h3', null, '1. Vygenerovat a zkontrolovat data pro web'),
      h('div', { class: 'panel-body' }, h('p', { class: 'muted' }, 'Vytvoří ', h('span', { class: 'mono' }, 'progress.json'), ' ve složce ', h('span', { class: 'mono' }, '_progress_data'),
        ' a zkontroluje ho (součty epizod, rozsah procent, žádné názvy souborů). Je to stejný soubor, ze kterého čerpá web.'),
        h('button', { class: 'btn primary', onclick: preview }, 'Vygenerovat náhled'), out)),
    h('div', { class: 'panel' }, h('h3', null, '2. Publikovat na web'),
      h('div', { class: 'panel-body' },
        settings.web.repo ? h('div', null, h('dl', { class: 'kv' }, h('dt', null, 'Repozitář'), h('dd', null, settings.web.repo), h('dt', null, 'Soubor v repozitáři'), h('dd', null, settings.web.path)),
          h('p', { class: 'muted' }, 'Zobrazí se náhled: repozitář, větev, co se změní, přesné příkazy gitu a varování (např. že push do větve main nasadí web na ostro).'),
          h('button', { class: 'btn', onclick: publishPlan }, 'Připravit publikování…'))
          : h('div', null, h('p', null, 'Propojení s webem zatím není nastavené.'), h('button', { class: 'btn', onclick: () => go('settings') }, 'Otevřít Nastavení')))));
  async function preview() {
    out.replaceChildren(loading('Generuji…'));
    try {
      const r = await get('export/preview');
      const d = r.data;
      out.replaceChildren(h('div', { style: { marginTop: '12px' } },
        r.problems.length ? h('div', { class: 'notice bad' }, h('b', null, 'Kontrola neprošla: '), r.problems.join(' | ')) : h('div', { class: 'notice', style: { background: 'var(--good-bg)' } }, '✓ Data prošla kontrolou.'),
        h('dl', { class: 'kv' }, h('dt', null, 'Celkem'), h('dd', null, `${fmtPct(d.translation.percent)} (${fmtN(d.translation.done)} / ${fmtN(d.translation.total)})`),
          h('dt', null, 'Korektura'), h('dd', null, fmtPct(d.proofreading.percent)), h('dt', null, 'Soubory'), h('dd', null, d.files.total),
          h('dt', null, 'Odhad dokončení'), h('dd', null, d.estimate.status === 'ok' ? dateCz(d.estimate.eta) : 'zatím nelze (málo dat)'),
          h('dt', null, 'Místní soubor'), h('dd', { class: 'mono' }, r.path)),
        h('div', { class: 'tablewrap', style: { marginTop: '10px' } }, h('table', { class: 'data' }, h('colgroup', null, [160, 130, 160, 160, 110].map((w) => h('col', { style: { width: w + 'px' } }))),
          h('thead', null, h('tr', null, ['Sezóna / epizoda', 'Stav', 'Řádky', 'Procenta', 'Soubory'].map((t) => h('th', null, t)))),
          h('tbody', null, d.seasons.map((s) => s.status === 'planned'
            ? h('tr', null, h('td', null, `Sezóna ${s.n}`), h('td', null, chip('planned', 'Plánováno')), h('td', { colspan: 3, class: 'muted' }, 'bez čísel, mimo výpočty'))
            : [h('tr', null, h('td', null, h('b', null, `Sezóna ${s.n}`)), h('td'), h('td', { class: 'num' }, `${fmtN(s.translation.done)} / ${fmtN(s.translation.total)}`), h('td', { class: 'num' }, fmtPct(s.translation.percent)), h('td', { class: 'num' }, s.files)),
              ...d.episodes.filter((e) => e.season === s.n).map((e) => h('tr', null, h('td', null, `   Epizoda ${e.n}`), h('td', null, chip(e.status, STATUS_LABEL[e.status])), h('td', { class: 'num' }, `${fmtN(e.translation.done)} / ${fmtN(e.translation.total)}`), h('td', { class: 'num' }, fmtPct(e.translation.percent)), h('td', { class: 'num' }, e.files_total)))]))))));
    } catch (e) { out.replaceChildren(h('div', { class: 'notice bad' }, e.message)); }
  }
  async function publishPlan() {
    let info;
    try { info = await post('publish/plan'); } catch (e) { return fail(e); }
    const ack = h('input', { type: 'checkbox' });
    const go_ = h('button', { class: 'btn primary', disabled: true, onclick: async () => {
      go_.disabled = true; go_.textContent = 'Publikuji…';
      try {
        const r = await post('publish/confirm', { token: info.token, branch: info.will_clone ? '*clone*' : info.branch });
        toast(r.message, 'ok', 9000); closeFn();
      } catch (e) { fail(e); go_.disabled = false; go_.textContent = 'Publikovat'; }
      return false;
    } }, 'Publikovat');
    ack.addEventListener('change', () => { go_.disabled = !ack.checked || info.blockers.length > 0; });
    const closeFn = dialog({ title: 'Náhled publikování', body: h('div', null,
      info.blockers.map((b) => h('div', { class: 'notice bad' }, b)), info.warnings.map((w) => h('div', { class: 'notice warn' }, w)),
      h('dl', { class: 'kv' }, h('dt', null, 'Repozitář'), h('dd', null, info.repo_path), h('dt', null, 'Vzdálený'), h('dd', null, info.remote || info.url || '—'),
        h('dt', null, 'Větev'), h('dd', null, info.branch ? `${info.branch} → ${info.upstream || '(bez upstreamu)'}` : '(výchozí po naklonování)'),
        h('dt', null, 'Soubor'), h('dd', { class: 'mono' }, info.target),
        h('dt', null, 'Dosud na webu'), h('dd', null, info.current ? `překlad ${fmtPct(info.current.translation)}, aktualizováno ${info.current.updated || '?'}` : '—'),
        h('dt', null, 'Nově'), h('dd', null, info.new ? `překlad ${fmtPct(info.new.translation)}, korektura ${fmtPct(info.new.proofreading)}` : '—'),
        h('dt', null, 'Změní se'), h('dd', null, info.changed ? 'ano' : 'ne (jen čas)')),
      h('h4', { style: { marginTop: '10px' } }, 'Proběhnou tyto příkazy (jen s tímto jedním souborem)'),
      h('div', { class: 'textbox mono' }, info.commands.join('\n')),
      info.dirty.length ? h('div', { class: 'muted' }, `Jiné neuložené změny v repozitáři (nedotkne se jich): ${info.dirty.slice(0, 5).join(', ')}${info.dirty.length > 5 ? '…' : ''}`) : null,
      h('label', { class: 'check', style: { marginTop: '10px' } }, ack, info.warnings.some((w) => /main|master|ostro/.test(w)) ? 'Rozumím, že se data odešlou na GitHub a web se aktualizuje na ostro.' : 'Rozumím, že se data odešlou na GitHub.')),
      buttons: [{ label: 'Zrušit' }, { node: go_ }] });
  }
  return root;
}

