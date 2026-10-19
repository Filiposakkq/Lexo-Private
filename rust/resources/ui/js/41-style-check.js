import { locale, tn, t as T, tx } from './05-i18n.js';
import { h } from './00-core.js';
import { get, post } from './10-api.js';
import { speakerLabel } from './11-state.js';
import { dialog, fail, toast } from './13-feedback.js';
import { chip, fmtText, fname } from './14-renderers.js';
import { go } from './16-tabs.js';
import { requireFeature } from './38-pro.js';
/* The style check (a Pro feature): the translated lines are sent, with the original, the profile of the speaker and the lines before, to the language model set up for the machine
   translation; it says what is wrong (the form of address, the gender forms, the tone, the meaning) and gives a corrected text. Lines with a remark are shown with the correction
   you can edit and tick; only the ticked ones are written (the same step as the machine translation, undoable). Approved lines are never overwritten. */
const KIND = () => ({ address: T('form of address'), gender: T('gender'), tone: T('tone'), meaning: T('meaning'), other: T('other') });
export async function styleDialog({ picked = [], anchor = null, file = '', scope = '' }, done) {
  if (!await requireFeature('style.check')) return;
  let cfg;
  try { cfg = (await get('settings')).mt || {}; } catch (e) { return fail(e); }
  if (cfg.provider !== 'openai' || !cfg.consent || !cfg.has_key) {
    return dialog({ title: T('The style check needs a language model'), narrow: true, buttons: [{ label: T('Close') }, { label: T('Open Settings'), kind: 'primary', onclick: () => go('settings') }],
      body: h('p', null, tx('Choose the OpenAI-compatible service, enter the key and give the consent to send text in {where}. DeepL and LibreTranslate are not language models and cannot check the style.', { where: h('b', null, T('Settings → Machine translation')) })) });
  }
  const SCENE_R = 15, LIMIT = 200, BATCH = 5;
  const opts = [];
  if (picked.length) opts.push(['picked', T('Selected lines ({n})', { n: picked.length })]);
  if (anchor) opts.push(['scene', T('Scene around the open line (±{n})', { n: SCENE_R })]);
  if (file) opts.push(['file', T('File in the filter: {name}', { name: file.split(/[\\/]/).pop() })]);
  opts.push(['all', T('All files')]);
  let sc = scope && opts.some(([k]) => k === scope) ? scope : opts[0][0];
  let results = [], checked = 0, target = { items: [], chars: 0, total: 0 }, running = false, stop = false;
  const scopeSel = h('select', { onchange: (e) => { sc = e.target.value; plan(); } }, opts.map(([k, l]) => h('option', { value: k, selected: k === sc }, l)));
  const info = h('div', { class: 'muted' }), prog = h('div', { class: 'muted' }), list = h('div');
  const run = h('button', { class: 'btn primary', disabled: true }), stopBtn = h('button', { class: 'btn', style: { display: 'none' }, onclick: () => { stop = true; } }, T('Stop'));
  const write = h('button', { class: 'btn primary', disabled: true }), sel = new Set();
  const rk = (r) => r.file + '\t' + r.line;
  const flagged = (r) => (r.issues || []).some((i) => i.severity === 'error');
  async function plan() {
    run.disabled = true; info.textContent = T('Counting…');
    try {
      let lines = null;
      if (sc === 'picked') lines = picked;
      else if (sc === 'scene') { const r = await get('entry/scene', { file: anchor.file, line: anchor.line, radius: SCENE_R }); lines = r.items.map((x) => ({ file: anchor.file, line: x.line })); }
      target = await post('style/targets', { file: sc === 'file' ? file : '', lines });
    } catch (e) { info.textContent = e.message; return; }
    const n = target.items.length, over = n > LIMIT;
    info.replaceChildren(T('{lines}, about {chars} characters will be sent to {provider}.', { lines: tn(target.total, '{n} translated line', '{n} translated lines'), chars: (target.chars || 0).toLocaleString(locale()), provider: cfg.provider }),
      target.limited ? h('div', { class: 'notice warn' }, T('The demo checks only the first {limit} lines ({total} found). A licence removes the limit.', { limit: target.limited.limit, total: target.limited.total })) : null,
      over ? h('div', { class: 'notice bad' }, T('At most {n} lines at once – the first {n} will be checked, run again for the rest.', { n: LIMIT })) : null,
      h('div', null, T('The service charges by the text sent – check the price with your provider.')));
    run.disabled = !n || running;
    run.textContent = n ? tn(Math.min(n, LIMIT), 'Check {n} line', 'Check {n} lines') : T('Nothing to check');
  }
  function paintList() {
    const bad = results.filter((r) => !r.error && !r.ok);
    const rows = bad.map((r) => {
      const inp = h('input', { type: 'text', class: 'mtedit', value: r.fix || r.current, oninput: (e) => { r.fix = e.target.value; } });
      return h('tr', null,
        h('td', { class: 'center' }, h('input', { type: 'checkbox', checked: sel.has(rk(r)), disabled: !r.fix, onchange: (e) => { e.target.checked ? sel.add(rk(r)) : sel.delete(rk(r)); paintBtn(); } })),
        h('td', null, fname(r.file), h('span', { class: 'mono muted' }, ` ${r.line}`), r.who ? h('div', { class: 'mono muted' }, speakerLabel(r.who)) : null),
        h('td', { class: 'txt' }, h('div', null, fmtText(r.source ?? '')), h('div', { class: 'muted' }, fmtText(r.current ?? ''))),
        h('td', { class: 'txt' }, ...(r.remarks || []).map((m) => h('div', null, chip('warning', KIND()[m.kind] || m.kind), ' ', m.explanation))),
        h('td', { class: 'txt' }, inp, flagged(r) ? h('div', null, ...(r.issues || []).map((i) => chip(i.severity === 'error' ? 'error' : 'warning', i.message || i.code))) : null));
    });
    const errs = results.filter((r) => r.error);
    list.replaceChildren(
      results.length ? h('p', { class: 'muted' }, T('{checked} lines checked, {n} with a remark.', { checked, n: bad.length })) : null,
      bad.length ? h('div', { class: 'toolbar' },
        h('button', { class: 'btn small', onclick: () => { bad.forEach((r) => { if (r.fix) sel.add(rk(r)); }); paintList(); } }, T('Select all')),
        h('button', { class: 'btn small', title: T('Unticks the lines where the validator found something in the correction'), onclick: () => { bad.forEach((r) => { if (r.fix && !flagged(r)) sel.add(rk(r)); else sel.delete(rk(r)); }); paintList(); } }, T('Only safe ones')),
        h('button', { class: 'btn small', onclick: () => { sel.clear(); paintList(); } }, T('None'))) : null,
      bad.length ? h('div', { class: 'tablewrap', style: { maxHeight: '44vh' } }, h('table', { class: 'data' }, h('colgroup', null, [44, 120, 260, 260, 300].map((w) => h('col', { style: { width: w + 'px' } }))),
        h('thead', null, h('tr', null, ['', T('Where'), T('Original and translation'), T('Remark'), T('Correction (editable)')].map((x) => h('th', null, x)))), h('tbody', null, rows))) : null,
      errs.length ? h('p', { class: 'muted' }, `${tn(errs.length, '{n} line failed', '{n} lines failed')}: ${errs[0].error}`) : null);
    paintBtn();
  }
  function paintBtn() { const n = sel.size; write.disabled = !n || running; write.textContent = tn(n, 'Write {n} correction', 'Write {n} corrections'); }
  run.onclick = async () => {
    const todo = target.items.slice(0, LIMIT);
    running = true; stop = false; results = []; checked = 0; sel.clear(); run.disabled = true; stopBtn.style.display = ''; let stopped = '';
    for (let i = 0; i < todo.length && !stop; i += BATCH) {
      prog.textContent = T('Checking… {done} / {total}', { done: Math.min(i + BATCH, todo.length), total: todo.length });
      let r;
      try { r = await post('style/batch', { items: todo.slice(i, i + BATCH) }); } catch (e) { stopped = e.message; break; }
      for (const x of r.results) { results.push(x); if (!x.error) checked += 1; if (!x.error && !x.ok && x.fix && !flagged(x)) sel.add(rk(x)); }
      paintList();
      if (r.stopped) { stopped = r.stopped; break; }
    }
    prog.textContent = stopped || (stop ? T('Stopped.') : T('Done – review the remarks and write the corrections you agree with.'));
    running = false; stopBtn.style.display = 'none'; paintList(); plan();
  };
  write.onclick = async () => {
    const selections = results.filter((r) => !r.error && !r.ok && sel.has(rk(r))).map((r) => ({ file: r.file, line: r.line, text: r.fix }));
    try {
      const r = await post('mt/apply', { selections });
      toast(tn(r.changed, 'Written {n} line', 'Written {n} lines') + (r.skipped ? ', ' + T('skipped {n}', { n: r.skipped }) : '') + '. ' + T('Undo: Ctrl+Z.'), 'ok', 8000);
      await done?.();
    } catch (e) { fail(e); return false; }
  };
  const body = h('div', null,
    h('p', { class: 'muted' }, T('The original and the translation are sent to the configured language model, with the profile of the speaker if there is one; nothing is written until you tick and confirm. Approved lines are never overwritten.')),
    h('div', { class: 'toolbar' }, h('label', { class: 'check' }, T('What: '), scopeSel)),
    info, h('div', { class: 'toolbar', style: { margin: '8px 0' } }, run, stopBtn, prog), list);
  dialog({ title: T('Check the style'), body, buttons: [{ label: T('Close') }, { node: h('span', { style: { display: 'contents' } }, write) }] });
  plan();
}
