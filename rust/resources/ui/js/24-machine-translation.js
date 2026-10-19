import { locale, tn, t as T, tx } from './05-i18n.js';
import { $, h, plural } from './00-core.js';
import { get, post } from './10-api.js';
import { speakerLabel } from './11-state.js';
import { dialog, fail, toast } from './13-feedback.js';
import { chip, fmtText, fname } from './14-renderers.js';
import { go } from './16-tabs.js';
import { requireFeature } from './38-pro.js';
/* Machine translation of several lines. Suggestions come in small batches (progress, Stop), are shown in a table you can edit and
   tick, and only the ticked ones are written, as one undoable step. Approved lines are never overwritten. */
export async function mtDialog({ picked = [], anchor = null, file = '', scope = '' }, done) {
  if (!await requireFeature('mt.batch')) return;
  let cfg;
  try { cfg = (await get('settings')).mt || {}; } catch (e) { return fail(e); }
  if (!cfg.provider || !cfg.consent || !cfg.has_key) {
    return dialog({ title: T('Machine translation is not set up'), narrow: true, buttons: [{ label: T('Close') }, { label: T('Open Settings'), kind: 'primary', onclick: () => go('settings') }],
      body: h('p', null, tx('Set the service, the API key and the consent to send text in {where}. The key stays on your computer only and text is sent only after your consent.', { where: h('b', null, T('Settings → Machine translation')) })) });
  }
  const SCENE_R = 15, LIMIT = 500;
  const opts = [];
  if (picked.length) opts.push(['picked', T('Selected lines ({n})', { n: picked.length })]);
  if (anchor) opts.push(['scene', T('Scene around the open line (±{n})', { n: SCENE_R })]);
  if (file) opts.push(['file', T('File in the filter: {name}', { name: file.split(/[\\/]/).pop() })]);
  opts.push(['all', T('All files')]);
  let sc = scope && opts.some(([k]) => k === scope) ? scope : opts[0][0];
  let onlyTodo = true, results = [], target = { items: [], chars: 0 }, running = false, stop = false;
  const scopeSel = h('select', { onchange: (e) => { sc = e.target.value; plan(); } }, opts.map(([k, l]) => h('option', { value: k, selected: k === sc }, l)));
  const todoBox = h('input', { type: 'checkbox', checked: true, onchange: (e) => { onlyTodo = e.target.checked; plan(); } });
  const info = h('div', { class: 'muted' }), prog = h('div', { class: 'muted' }), list = h('div');
  const run = h('button', { class: 'btn primary', disabled: true }), stopBtn = h('button', { class: 'btn', style: { display: 'none' }, onclick: () => { stop = true; } }, T('Stop'));
  const write = h('button', { class: 'btn primary', disabled: true }), sel = new Set();
  const rk = (r) => r.file + '\t' + r.line;
  const flagged = (r) => (r.issues || []).some((i) => i.severity === 'error') || (r.warnings || []).length > 0;
  async function plan() {
    run.disabled = true; info.textContent = T('Counting…');
    try {
      let lines = null;
      if (sc === 'picked') lines = picked;
      else if (sc === 'scene') { const r = await get('entry/scene', { file: anchor.file, line: anchor.line, radius: SCENE_R }); lines = r.items.map((x) => ({ file: anchor.file, line: x.line })); }
      target = await post('mt/targets', { file: sc === 'file' ? file : '', lines, only_todo: onlyTodo });
    } catch (e) { info.textContent = e.message; return; }
    const n = target.total, over = n > LIMIT;
    info.replaceChildren(T('{lines}, about {chars} characters will be sent to {provider}.', { lines: tn(n, '{n} line', '{n} lines'), chars: target.chars.toLocaleString(locale()), provider: cfg.provider }),
      over ? h('div', { class: 'notice bad' }, T('At most {n} lines at once – the first {n} will be translated, run again for the rest.', { n: LIMIT })) : null,
      h('div', null, T('The service charges by the text sent – check the price with your provider.')));
    run.disabled = !n || running;
    run.textContent = n ? tn(Math.min(n, LIMIT), 'Translate {n} line', 'Translate {n} lines') : T('Nothing to translate');
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
        h('button', { class: 'btn small', onclick: () => { ok.forEach((r) => sel.add(rk(r))); paintList(); } }, T('Select all')),
        h('button', { class: 'btn small', title: T('Unticks the lines where the validator found something'), onclick: () => { ok.forEach((r) => { if (flagged(r)) sel.delete(rk(r)); else sel.add(rk(r)); }); paintList(); } }, T('Only without findings')),
        h('button', { class: 'btn small', onclick: () => { sel.clear(); paintList(); } }, T('None'))) : null,
      ok.length ? h('div', { class: 'tablewrap', style: { maxHeight: '44vh' } }, h('table', { class: 'data' }, h('colgroup', null, [44, 140, 330, 400].map((w) => h('col', { style: { width: w + 'px' } }))),
        h('thead', null, h('tr', null, ['', T('Where'), T('Original'), T('Suggestion (editable)')].map((x) => h('th', null, x)))), h('tbody', null, rows))) : null,
      errs.length ? h('p', { class: 'muted' }, `${tn(errs.length, '{n} line failed', '{n} lines failed')}: ${errs[0].error}`) : null);
    paintBtn();
  }
  function paintBtn() { const n = sel.size; write.disabled = !n || running; write.textContent = tn(n, 'Write {n} line', 'Write {n} lines'); }
  run.onclick = async () => {
    const todo = target.items.slice(0, LIMIT);
    running = true; stop = false; results = []; sel.clear(); run.disabled = true; stopBtn.style.display = ''; let stopped = '';
    for (let i = 0; i < todo.length && !stop; i += 10) {
      prog.textContent = T('Translating… {done} / {total}', { done: Math.min(i + 10, todo.length), total: todo.length });
      let r;
      try { r = await post('mt/batch', { items: todo.slice(i, i + 10) }); } catch (e) { stopped = e.message; break; }
      for (const x of r.results) { results.push(x); if (!x.error && !flagged(x)) sel.add(rk(x)); }
      paintList();
      if (r.stopped) { stopped = r.stopped; break; }
    }
    prog.textContent = stopped || (stop ? T('Stopped.') : T('Done – review the suggestions and write the selected ones.'));
    running = false; stopBtn.style.display = 'none'; paintList(); plan();
  };
  write.onclick = async () => {
    const selections = results.filter((r) => !r.error && sel.has(rk(r))).map((r) => ({ file: r.file, line: r.line, text: r.suggestion }));
    try {
      const r = await post('mt/apply', { selections });
      toast(tn(r.changed, 'Written {n} line', 'Written {n} lines') + (r.skipped ? ', ' + T('skipped {n}', { n: r.skipped }) : '') + '. ' + T('Undo: Ctrl+Z.'), 'ok', 8000);
      await done?.();
    } catch (e) { fail(e); return false; }
  };
  const body = h('div', null,
    h('p', { class: 'muted' }, T('The source text is sent to the configured service; nothing is written until you tick and confirm. Approved lines are never overwritten.')),
    h('div', { class: 'toolbar' }, h('label', { class: 'check' }, T('What: '), scopeSel), h('label', { class: 'check' }, todoBox, ' ' + T('only untranslated'))),
    info, h('div', { class: 'toolbar', style: { margin: '8px 0' } }, run, stopBtn, prog), list);
  dialog({ title: T('Translate with a machine'), body, buttons: [{ label: T('Close') }, { node: h('span', { style: { display: 'contents' } }, write) }] });
  plan();
}

