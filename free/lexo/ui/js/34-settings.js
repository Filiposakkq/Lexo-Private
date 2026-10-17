import { t as T, uiLanguage } from './05-i18n.js';
import { $, h } from './00-core.js';
import { get, post } from './10-api.js';
import { S } from './11-state.js';
import { pref, setPref } from './12-chrome.js';
import { fail, toast } from './13-feedback.js';
import { openProject, pickProject, refreshHeader } from './17-project.js';
import { snapshotsPanel } from './29-snapshots.js';
import { keysPanel } from './35-keyboard.js';
/* ================================================================ SETTINGS */
export async function viewSettings() {
  const s = await get('settings');
  const root = h('div', { style: { maxWidth: '1100px' } });
  const val = (id) => $('#' + id);
  const field = (label, node, hint) => [h('label', { for: node.id }, label), node, hint ? h('div', { class: 'hint' }, hint) : null];
  const ln = s.validation.length;
  const tb = s.validation.textbox;
  const projectInput = h('input', { type: 'text', id: 'set_project', value: S.projectDir });
  // the interface language: saved at once, then the page is reloaded so that every text is drawn in it
  const langSelect = h('select', { id: 'set_lang', onchange: async (e) => { setPref('ui.lang', e.target.value); try { await post('prefs', S.prefs); } catch { /* ignore */ } location.reload(); } },
    ...(S.langs || [{ code: 'en', name: 'English' }]).map((l) => h('option', { value: l.code, selected: l.code === uiLanguage() }, l.name)));
  root.append(
    h('fieldset', null, h('legend', { 'data-k': 'language' }, T('Language')), h('div', { class: 'form' },
      ...field(T('Interface language'), langSelect, T('The page reloads when you change it.')))),
    h('fieldset', null, h('legend', { 'data-k': 'project' }, T('Project')), h('div', { class: 'form' },
      ...field(T('Translation folder'), h('div', { class: 'row nowrap' }, projectInput, h('button', { class: 'btn', onclick: pickProject }, T('Browse…')), h('button', { class: 'btn', onclick: () => openProject(projectInput.value) }, T('Open'))),
        T('Remembered between runs. The shared project data (CSV files) is in the subfolder .lexo of the translation folder; backups, snapshots and progress.json are in the local project data of the application.')),
      ...field(T('Skipped files'), h('input', { type: 'text', id: 'set_skip', value: s.skip_files.join(', ') }), T('File names separated by commas (in all subfolders). A change takes effect after “Refresh files”.')))),
    h('fieldset', null, h('legend', { 'data-k': 'editor' }, T('Editor (opens a file at a line)')), h('div', { class: 'form' },
      ...field(T('Editor'), h('select', { id: 'set_editor', value: s.editor.preferred }, [['auto', T('Automatic (VS Code, otherwise Notepad++)')], ['vscode', 'Visual Studio Code'], ['notepadpp', 'Notepad++'], ['custom', T('Custom command')], ['system', T('Default program (no jump to the line)')]].map(([k, l]) => h('option', { value: k }, l)))),
      ...field(T('Custom command'), h('input', { type: 'text', id: 'set_custom', value: s.editor.custom, placeholder: '"C:\\Tools\\editor.exe" --line {line} {file}' }), T('Placeholders {file} and {line}. It runs without a shell, so paths with spaces and special characters are safe.')))),
    h('fieldset', null, h('legend', { 'data-k': 'check' }, T('Translation check')), h('div', { class: 'form' },
      ...field(T('Check enabled'), h('input', { type: 'checkbox', id: 'set_val', checked: s.validation.enabled })),
      ...field(T('Text length warnings'), h('input', { type: 'checkbox', id: 'set_len', checked: ln.enabled }), T('Only an aid for the check, never an error.')),
      ...field(T('Longer than (× original)'), h('input', { type: 'text', id: 'set_max', value: String(ln.max_ratio) }), T('The translation is usually longer than the English, default 2.2×.')),
      ...field(T('Shorter than (× original)'), h('input', { type: 'text', id: 'set_min', value: String(ln.min_ratio) }), T('Default 0.45×.')),
      ...field(T('Smallest original (characters)'), h('input', { type: 'text', id: 'set_minsrc', value: String(ln.min_source_chars) }), T('Shorter texts are not checked for length.')),
      ...field(T('Min. difference – longer (characters)'), h('input', { type: 'text', id: 'set_extra', value: String(ln.min_extra_chars) })),
      ...field(T('Min. difference – shorter (characters)'), h('input', { type: 'text', id: 'set_missing', value: String(ln.min_missing_chars) })),
      ...field(T('Watch for typos'), h('input', { type: 'checkbox', id: 'set_spell', checked: s.validation.spelling }), T('Missing diacritics and swapped letters according to the dictionary of your own translation. Only a hint.')),
      ...field(T('Dialogue box check'), h('input', { type: 'checkbox', id: 'set_tb', checked: tb.enabled }), T('Warns when the translation does not fit the dialogue box and the original did.')),
      ...field(T('Characters per line'), h('input', { type: 'text', id: 'set_tbcpl', value: String(tb.chars_per_line) }), T("The default 65 comes from the game's gui.rpy (text width 1116 px, font 33 px).")),
      ...field(T('Most lines in the box'), h('input', { type: 'text', id: 'set_tblines', value: String(tb.max_lines) }), T('Default 4.')))),
    h('fieldset', null, h('legend', { 'data-k': 'mt' }, T('Machine translation (optional, experimental)')), h('div', { class: 'form' },
      ...field(T('Service'), h('select', { id: 'set_mt_provider' }, [['', T('Off')], ['deepl', 'DeepL'], ['openai', T('OpenAI-compatible (also local)')], ['libretranslate', 'LibreTranslate']].map(([k, l]) => h('option', { value: k, selected: (s.mt?.provider || '') === k }, l))), T('Gives only a SUGGESTION for one line after you click “Machine suggestion” in the editor; nothing is ever written by itself.')),
      ...field(T('API key'), h('input', { type: 'password', id: 'set_mt_key', value: '', placeholder: s.mt?.has_key ? T('(saved – leave empty if you do not want to change it)') : '', autocomplete: 'off' }), T('Saved as plain text in config.json in %APPDATA%\\Lexo. It is not sent back to the app.')),
      ...(s.mt?.has_key ? [h('span'), h('label', { class: 'check' }, h('input', { type: 'checkbox', id: 'set_mt_clear' }), T(' Delete the saved key'))] : []),
      ...field(T('Service address'), h('input', { type: 'text', id: 'set_mt_url', value: s.mt?.base_url || '', placeholder: T('only for your own server, e.g. http://localhost:1234/v1') })),
      ...field('Model', h('input', { type: 'text', id: 'set_mt_model', value: s.mt?.model || '', placeholder: T('only for OpenAI-compatible, e.g. gpt-4o-mini') })),
      ...field(T('Languages'), h('span', { class: 'row' }, h('input', { type: 'text', id: 'set_mt_src', value: s.mt?.source_lang || 'EN', style: { width: '70px' } }), ' → ', h('input', { type: 'text', id: 'set_mt_dst', value: s.mt?.target_lang || 'CS', style: { width: '70px' } }))),
      ...field(T('Consent'), h('label', { class: 'check' }, h('input', { type: 'checkbox', id: 'set_mt_consent', checked: !!s.mt?.consent }), T(' I agree that the source text of the line I click is sent to the selected external service')), T('Nothing is sent without consent. Only the text of one line is sent (and for an OpenAI-compatible service the 2 previous lines as context).')))),
    h('fieldset', null, h('legend', { 'data-k': 'keys' }, T('Keyboard shortcuts')), keysPanel()),
    h('fieldset', null, h('legend', { 'data-k': 'backup' }, T('Backups of the translation files')), snapshotsPanel(s)),
    h('fieldset', null, h('legend', { 'data-k': 'lint' }, T("Game check (Ren'Py lint)")), h('div', { class: 'form' },
      ...field(T("Ren'Py SDK folder"), h('input', { type: 'text', id: 'set_sdk', value: s.lint?.sdk || '', placeholder: 'C:\\renpy-8.1.1-sdk' }), T('The folder with renpy.py (the unpacked SDK from renpy.org). Lint runs in Check → Game lint; it needs the source files of the game above the translation folder.')))),
    h('fieldset', null, h('legend', { 'data-k': 'goals' }, T('Weekly goals')), h('div', { class: 'form' },
      ...field(T('Translation (lines per week)'), h('input', { type: 'text', id: 'set_goal_t', value: String(s.goals?.translate_per_week ?? 0) }), T('0 = automatic from your pace of the last weeks (10 % more).')),
      ...field(T('Proofreading (lines per week)'), h('input', { type: 'text', id: 'set_goal_p', value: String(s.goals?.proofread_per_week ?? 0) }), T('0 = automatic.')))),
    h('fieldset', null, h('legend', { 'data-k': 'web' }, T('Website (publishing)')), h('div', { class: 'form' },
      ...field(T('Repository'), h('input', { type: 'text', id: 'set_repo', value: s.web.repo, placeholder: 'owner/repository' }), T('A folder with a clone, an address or user/repository. Your git (Credential Manager) handles the sign-in – no tokens are stored here.')),
      ...field(T('File in the repository'), h('input', { type: 'text', id: 'set_path', value: s.web.path }), T('Default web/progress.json.')),
      ...field(T('Name of the game on the website'), h('input', { type: 'text', id: 'set_title', value: s.web.title })),
      s.web.migrated_from ? h('div', { class: 'hint' }, T('Taken over from {from}. The app no longer needs a separate web.json.', { from: s.web.migrated_from })) : null)),
    h('fieldset', null, h('legend', { 'data-k': 'about' }, T('About the app')), h('p', null, h('b', null, `Lexo ${S.version || ''}`),
      T(" – a translation manager for Ren'Py games: edit, proofread, check and follow the progress of a translation. Licence: see the file "), h('span', { class: 'mono' }, 'LICENSE'), '.'),
      h('p', { class: 'notice' }, h('b', null, T('State of the app: ')), T("it is under development and has been tried on a number of Ren'Py games of different kinds. "),
        T('The interface is English with Czech as a language pack; more languages are planned.'))),
    h('div', { class: 'row' }, h('button', { class: 'btn primary', onclick: save }, T('Save settings')),
      h('span', { class: 'muted' }, T('Settings: '), h('span', { class: 'mono' }, 'config.json'), T(' in %APPDATA%\\Lexo'))));
  /* Group the fieldsets into sections with a side navigation. Everything stays in the DOM (only hidden), so Save sees every field. */
  const fsets = [...root.querySelectorAll(':scope > fieldset')];
  const findSet = (k) => fsets.find((f) => f.querySelector('legend').dataset.k === k);
  const SECS = [['general', T('General'), ['language', 'project', 'editor', 'goals']], ['check', T('Check'), ['check']], ['mt', T('Machine translation'), ['mt']],
    ['backup', T('Backups and lint'), ['backup', 'lint']], ['keys', T('Shortcuts'), ['keys']], ['web', T('Website'), ['web']], ['about', T('About'), ['about']]];
  const saveRow = root.lastElementChild;
  const content = h('div', { class: 'setcontent' });
  const secs = SECS.map(([id, title, legends]) => { const sec = h('section', { class: 'setsec' }, ...legends.map(findSet).filter(Boolean)); content.append(sec); return [id, title, sec]; });
  const nav = h('nav', { class: 'setnav', 'aria-label': T('Settings sections') });
  function showSec(id) {
    setPref('ui.settingsSection', id);
    secs.forEach(([k, , sec]) => { sec.hidden = k !== id; });
    nav.replaceChildren(...secs.map(([k, t]) => h('button', { class: 'setnavbtn' + (k === id ? ' on' : ''), 'aria-current': k === id ? 'page' : 'false', onclick: () => showSec(k) }, t)));
  }
  root.replaceChildren(h('div', { class: 'settings' }, nav, content), h('div', { class: 'savebar' }, saveRow));
  const want = pref('ui.settingsSection', 'general');
  showSec(secs.some(([k]) => k === want) ? want : 'general');
  async function save() {
    try {
      await post('settings', {
        skip_files: val('set_skip').value.split(',').map((x) => x.trim()).filter(Boolean),
        editor: { preferred: val('set_editor').value, custom: val('set_custom').value },
        validation: { enabled: val('set_val').checked, spelling: val('set_spell').checked,
          textbox: { enabled: val('set_tb').checked, chars_per_line: num(val('set_tbcpl').value), max_lines: num(val('set_tblines').value) }, length: { enabled: val('set_len').checked, max_ratio: num(val('set_max').value), min_ratio: num(val('set_min').value),
          min_source_chars: num(val('set_minsrc').value), min_extra_chars: num(val('set_extra').value), min_missing_chars: num(val('set_missing').value) } },
        goals: { translate_per_week: num(val('set_goal_t').value), proofread_per_week: num(val('set_goal_p').value) },
        snapshots: { enabled: val('set_snap_on').checked, interval_hours: num(val('set_snap_h').value), keep: num(val('set_snap_keep').value) },
        lint: { sdk: val('set_sdk').value },
        mt: { provider: val('set_mt_provider').value, api_key: val('set_mt_key').value, clear_key: !!val('set_mt_clear')?.checked, base_url: val('set_mt_url').value, model: val('set_mt_model').value,
          source_lang: val('set_mt_src').value, target_lang: val('set_mt_dst').value, consent: val('set_mt_consent').checked },
        web: { repo: val('set_repo').value, path: val('set_path').value, title: val('set_title').value },
      });
      S.mtOn = undefined; toast(T('Settings saved ✓')); refreshHeader();
    } catch (e) { fail(e); }
  }
  const num = (v) => Number(String(v).replace(',', '.'));
  return root;
}

