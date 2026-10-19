import { t as T, tn } from './05-i18n.js';
import { h } from './00-core.js';
import { post } from './10-api.js';
import { dialog, fail, toast } from './13-feedback.js';
import { requireFeature } from './38-pro.js';
/* The release builder (a Pro feature): the translation as a zip the players unpack into their game, with the files you attach, a README, a changelog and the number of the release.
   The result is a file; you put it where you like. The checks tell what stands in the way. */
export async function releaseDialog() {
  if (!await requireFeature('release.build')) return;
  let st;
  try { st = await post('release/state', {}); } catch (e) { return fail(e); }
  const s = st.settings;
  // the number after the last release (the last number plus one), or 1.0
  const suggest = (last) => { const p = (last || '').split(/[-+]/)[0].split('.').map(Number); return p.length && p.every(Number.isFinite) && last ? p.slice(0, -1).concat(p[p.length - 1] + 1).join('.') : '1.0'; };
  const version = h('input', { type: 'text', value: suggest(st.last_version), style: { width: '9em' }, placeholder: '1.0' });
  const notes = h('textarea', { rows: '3', style: { width: '100%' }, placeholder: T('What is new, in your words (goes to the changelog and the release notes)') });
  const box = (key, label) => { const c = h('input', { type: 'checkbox', checked: !!s[key] }); return { c, node: h('label', { class: 'row', style: { alignItems: 'center', gap: '6px' } }, c, label) }; };
  const oReadme = box('readme', T('README for the players (how to install, credits)'));
  const oNotes = box('notes_md', T('Release notes (a Markdown file for a page: GitHub, F95, Patreon)'));
  const oScript = box('language_script', T('A script that makes the language the one the game starts in'));
  const credits = h('textarea', { rows: '2', style: { width: '100%' }, placeholder: T('Credits: who translated and proofread') }, s.credits || '');
  const extra = h('textarea', { rows: '2', style: { width: '100%' }, placeholder: T('Anything else for the README (a link, a note)') }, s.readme_extra || '');
  const outDir = h('input', { type: 'text', value: s.out_dir || '', style: { width: '100%' }, placeholder: T('Folder for the files (empty: the local data of the project)') });
  const extras = (s.extras || []).map((e) => ({ ...e }));
  const extrasBox = h('div'), checksBox = h('div'), resultBox = h('div'), info = h('div', { class: 'muted' });
  const build = h('button', { class: 'btn primary', disabled: true }, T('Build the release'));
  const check = h('button', { class: 'btn' }, T('Check'));
  const forceBox = h('input', { type: 'checkbox' });
  const forceRow = h('label', { class: 'row', style: { alignItems: 'center', gap: '6px', display: 'none' } }, forceBox, T('Build anyway (the errors of the check of the translation do not stop it)'));
  let plan = null;
  function paintExtras() {
    extrasBox.replaceChildren(h('b', null, T('Files to attach')),
      h('p', { class: 'muted' }, T('For example a font with the diacritics of your language, or a changed gui file. Each goes into the archive at the place you give (inside the game folder, like game/tl/czech/fonts/Font.ttf).')),
      ...extras.map((e, i) => h('div', { class: 'row', style: { gap: '6px', margin: '4px 0' } },
        h('input', { type: 'text', value: e.from, placeholder: T('File on this computer'), style: { flex: 1 }, oninput: (ev) => { e.from = ev.target.value; } }),
        h('input', { type: 'text', value: e.to, placeholder: 'game/tl/czech/fonts/Font.ttf', style: { flex: 1 }, oninput: (ev) => { e.to = ev.target.value; } }),
        h('button', { class: 'btn small ghost', onclick: () => { extras.splice(i, 1); paintExtras(); } }, T('Remove')))),
      h('button', { class: 'btn small', onclick: () => { extras.push({ from: '', to: 'game/tl/' + (st.language || 'language') + '/' }); paintExtras(); } }, T('+ Attach a file')));
  }
  const settings = () => ({ extras: extras.filter((e) => e.from.trim() || e.to.trim()), credits: credits.value, readme_extra: extra.value, out_dir: outDir.value, readme: oReadme.c.checked, notes_md: oNotes.c.checked, language_script: oScript.c.checked });
  async function doCheck(keepResult) {
    if (keepResult !== true) resultBox.replaceChildren();
    try {
      await post('release/settings', { settings: settings() });
      plan = await post('release/preview', { version: version.value.trim(), notes: notes.value });
    } catch (e) { return fail(e); }
    if (!version.value.trim()) version.value = plan.suggested_version;
    const mark = { error: '✗', warn: '⚠', ok: '✓' };
    checksBox.replaceChildren(h('b', null, T('Checks')), ...plan.checks.map((c) => h('div', { class: c.level === 'error' ? 'notice bad' : c.level === 'warn' ? 'notice warn' : 'muted', style: { margin: '4px 0' } }, `${mark[c.level]} ${c.message}`)));
    const forceable = !plan.can_build && plan.can_force;
    forceRow.style.display = forceable ? '' : 'none';
    if (!forceable) forceBox.checked = false;
    build.disabled = !(plan.can_build || forceable);
    info.textContent = T('Files of the translation: {files}, attached files: {extras}; {pct} % translated.', { files: plan.files, extras: plan.extras, pct: plan.stats.total ? Math.round(100 * plan.stats.translated / plan.stats.total) : 0 })
      + (plan.previous ? ' ' + T('The last release: {v} ({date}).', { v: plan.previous.version, date: plan.previous.date }) : ' ' + T('This is the first release.'));
  }
  check.onclick = () => doCheck();
  build.onclick = async () => {
    build.disabled = true;
    try {
      await post('release/settings', { settings: settings() });
      const r = await post('release/build', { version: version.value.trim(), notes: notes.value, force: forceBox.checked });
      toast(T('The release {v} is built.', { v: r.version }), 'ok', 6000);
      resultBox.replaceChildren(h('div', { class: 'notice ok' },
        h('div', null, h('b', null, T('The file is ready'))),
        h('div', { class: 'mono', style: { wordBreak: 'break-all', margin: '4px 0' } }, r.zip),
        h('div', { class: 'muted' }, T('{size} kB; SHA-256 {sha}', { size: Math.round(r.size / 1024).toLocaleString(), sha: r.sha256.slice(0, 16) + '…' })),
        h('div', { class: 'muted' }, r.notes_file ? T('Release notes: {file}', { file: r.notes_file }) : ''),
        h('button', { class: 'btn small', style: { marginTop: '6px' }, onclick: () => navigator.clipboard?.writeText(r.zip).then(() => toast(T('The path is copied.'), 'ok', 1500)) }, T('Copy the path'))));
      st = await post('release/state', {});
      version.value = suggest(r.version); // the next release is the next number
      notes.value = '';
    } catch (e) { fail(e); }
    await doCheck(true);
  };
  paintExtras();
  dialog({
    title: T('Build a release'),
    body: h('div', null,
      h('p', { class: 'muted' }, T('The translation as a zip the players unpack into their game folder. The result is a file: you put it where you like. Nothing is uploaded.')),
      h('div', { class: 'muted' }, T('Game: {name} {v}; language: {lang}.', { name: st.game.name || '–', v: st.game.version ? 'v' + st.game.version : '', lang: st.language }) + (st.last_version ? ' ' + tn(st.releases.length, '{n} release so far, the last {v}.', '{n} releases so far, the last {v}.', { v: st.last_version }) : '')),
      h('div', { class: 'row', style: { alignItems: 'center', gap: '8px', margin: '8px 0' } }, h('label', null, T('Number of the release')), version, check),
      notes, h('div', { style: { margin: '8px 0' } }, oReadme.node, oNotes.node, oScript.node), credits, extra, h('div', { style: { margin: '6px 0' } }, outDir), extrasBox,
      info, checksBox, forceRow, resultBox),
    buttons: [{ label: T('Close') }, { node: h('span', { style: { display: 'contents' } }, build) }],
  });
  doCheck();
}
