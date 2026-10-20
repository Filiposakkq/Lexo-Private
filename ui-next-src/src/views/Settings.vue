<script setup>
import { computed, onMounted, ref } from 'vue';
import { get, post } from '../api.js';
import SnapshotRestore from '../components/SnapshotRestore.vue';
import { availableLanguages, t, tn, uiLanguage } from '../i18n.js';
import { app } from '../state.js';
import { reloadAll } from '../project.js';

/* Settings of the application and of the project. Everything is one form with one Save button; a change takes effect after saving (the language at once, the page reloads). */
const emit = defineEmits(['go']);
const s = ref(null);
const langs = ref([{ code: 'en', name: 'English' }]);
const lang = ref(uiLanguage());
const skip = ref('');
const folder = ref('');
const mtClear = ref(false);
const mtKey = ref('');
const error = ref('');
const message = ref('');
const saving = ref(false);
const section = ref(stored());
const snaps = ref(null);
const restoring = ref('');
const deleting = ref('');

function stored() { try { return localStorage.getItem('aside.settingsSection') || 'general'; } catch { return 'general'; } }
function pick(k) { section.value = k; try { localStorage.setItem('aside.settingsSection', k); } catch { /* only for this page */ } }
const SECS = computed(() => [['general', t('General')], ['check', t('Check')], ['mt', t('Machine translation')], ['backup', t('Backups and lint')], ['keys', t('Shortcuts')], ['web', t('Website')], ['licence', t('Licence')], ['about', t('About')]]);
const EDITORS = computed(() => [['auto', t('Automatic (VS Code, otherwise Notepad++)')], ['vscode', 'Visual Studio Code'], ['notepadpp', 'Notepad++'], ['custom', t('Custom command')], ['system', t('Default program (no jump to the line)')]]);
const SERVICES = computed(() => [['', t('Off')], ['deepl', 'DeepL'], ['openai', t('OpenAI-compatible (also local)')], ['libretranslate', 'LibreTranslate']]);
const REASON = computed(() => ({ auto: t('automatic'), bulk: t('before a bulk change'), manual: t('created by hand'), restore: t('before a restore') }));
const fmtSize = (b) => (b >= 1048576 ? (b / 1048576).toFixed(1) + ' MB' : Math.max(1, Math.round(b / 1024)) + ' kB');

async function load() {
  error.value = '';
  try {
    const r = await get('settings');
    r.snapshots ||= { enabled: true, interval_hours: 12, keep: 20 }; r.lint ||= { sdk: '' }; r.goals ||= { translate_per_week: 0, proofread_per_week: 0 };
    r.mt ||= {}; r.web ||= { repo: '', path: '', title: '' };
    s.value = r; skip.value = (r.skip_files || []).join(', '); folder.value = r.project_dir || '';
  } catch (e) { error.value = e.message; }
}
async function loadSnaps() { try { snaps.value = await get('snapshots'); } catch (e) { snaps.value = { items: [], error: e.message }; } }
onMounted(async () => { langs.value = await availableLanguages(); load(); loadSnaps(); });

const num = (v) => Number(String(v).replace(',', '.'));
async function save() {
  saving.value = true; error.value = ''; message.value = '';
  const v = s.value, ln = v.validation.length, tb = v.validation.textbox;
  try {
    await post('settings', {
      skip_files: skip.value.split(',').map((x) => x.trim()).filter(Boolean),
      editor: { preferred: v.editor.preferred, custom: v.editor.custom },
      validation: { enabled: v.validation.enabled, spelling: v.validation.spelling,
        textbox: { enabled: tb.enabled, chars_per_line: num(tb.chars_per_line), max_lines: num(tb.max_lines) },
        length: { enabled: ln.enabled, max_ratio: num(ln.max_ratio), min_ratio: num(ln.min_ratio), min_source_chars: num(ln.min_source_chars), min_extra_chars: num(ln.min_extra_chars), min_missing_chars: num(ln.min_missing_chars) } },
      goals: { translate_per_week: num(v.goals.translate_per_week), proofread_per_week: num(v.goals.proofread_per_week) },
      snapshots: { enabled: v.snapshots.enabled, interval_hours: num(v.snapshots.interval_hours), keep: num(v.snapshots.keep) },
      lint: { sdk: v.lint.sdk },
      mt: { provider: v.mt.provider || '', api_key: mtKey.value, clear_key: mtClear.value, base_url: v.mt.base_url || '', model: v.mt.model || '', source_lang: v.mt.source_lang || 'EN', target_lang: v.mt.target_lang || '', consent: !!v.mt.consent },
      web: { repo: v.web.repo, path: v.web.path, title: v.web.title },
    });
    message.value = t('Settings saved ✓'); mtKey.value = ''; mtClear.value = false;
    reloadAll().catch(() => {}); load();
  } catch (e) { error.value = e.message; }
  saving.value = false;
}
function changeLang() {
  try { localStorage.setItem('aside.lang', lang.value); } catch { /* the choice lasts only until the page is closed */ }
  location.reload();
}
async function browse() {
  error.value = '';
  try { const r = await post('project/pick', {}); if (!r.cancelled) { await reloadAll(); emit('go', 'overview'); } } catch (e) { error.value = e.message; }
}
async function openFolder() {
  error.value = '';
  try { await post('project/set', { path: folder.value }); await reloadAll(); emit('go', 'overview'); } catch (e) { error.value = e.message; }
}
async function createSnap() {
  try { const r = await post('snapshots/create'); message.value = t('Backup created ({files} files, {size}).', { files: r.snapshot.files, size: fmtSize(r.snapshot.bytes) }); loadSnaps(); } catch (e) { error.value = e.message; }
}
async function deleteSnap(m) {
  if (deleting.value !== m.name) { deleting.value = m.name; setTimeout(() => { if (deleting.value === m.name) deleting.value = ''; }, 6000); return; }
  deleting.value = '';
  try { await post('snapshots/delete', { name: m.name }); loadSnaps(); } catch (e) { error.value = e.message; }
}
function restored(n) {
  restoring.value = '';
  message.value = t('{files}. The previous state is saved as a backup “before a restore”.', { files: tn(n, 'Restored {n} file', 'Restored {n} files') });
  loadSnaps(); reloadAll().catch(() => {});
}
</script>

<template>
  <main class="screen">
    <div v-if="error" class="notice danger" role="alert">{{ error }}</div>
    <div v-if="message" class="notice" role="status">{{ message }}<span class="act" role="button" tabindex="0" @click="message = ''" @keydown.enter="message = ''">{{ t('Close') }}</span></div>
    <div v-if="!s && !error" class="skeleton" style="height: 160px"></div>
    <div v-else-if="s" class="settings">
      <nav class="setnav" :aria-label="t('Settings sections')"><button v-for="[k, l] in SECS" :key="k" class="setnavbtn" :class="{ on: section === k }" :aria-current="section === k ? 'page' : 'false'" @click="pick(k)">{{ l }}</button></nav>
      <div class="setcontent">

        <template v-if="section === 'general'">
          <fieldset><legend>{{ t('Language') }}</legend>
            <label class="row"><span>{{ t('Interface language') }}</span><select v-model="lang" class="sel" @change="changeLang"><option v-for="l in langs" :key="l.code" :value="l.code">{{ l.name }}</option></select></label>
            <div class="hint">{{ t('The page reloads when you change it.') }}</div></fieldset>
          <fieldset><legend>{{ t('Project') }}</legend>
            <label class="row"><span>{{ t('Translation folder') }}</span><span class="inline"><input v-model="folder" class="text" type="text"><button class="btn" @click="browse">{{ t('Browse…') }}</button><button class="btn" @click="openFolder">{{ t('Open') }}</button></span></label>
            <div class="hint">{{ t('Remembered between runs. The shared project data (CSV files) is in the subfolder .aside of the translation folder; backups, snapshots and progress.json are in the local project data of the application.') }}</div>
            <label class="row"><span>{{ t('Skipped files') }}</span><input v-model="skip" class="text" type="text"></label>
            <div class="hint">{{ t('File names separated by commas (in all subfolders). A change takes effect after “Refresh files”.') }}</div></fieldset>
          <fieldset><legend>{{ t('Editor (opens a file at a line)') }}</legend>
            <label class="row"><span>{{ t('Editor') }}</span><select v-model="s.editor.preferred" class="sel"><option v-for="[k, l] in EDITORS" :key="k" :value="k">{{ l }}</option></select></label>
            <label class="row"><span>{{ t('Custom command') }}</span><input v-model="s.editor.custom" class="text" type="text" placeholder="&quot;C:\Tools\editor.exe&quot; --line {line} {file}"></label>
            <div class="hint">{{ t('Placeholders {file} and {line}. It runs without a shell, so paths with spaces and special characters are safe.') }}</div></fieldset>
          <fieldset><legend>{{ t('Weekly goals') }}</legend>
            <label class="row"><span>{{ t('Translation (lines per week)') }}</span><input v-model="s.goals.translate_per_week" class="text short" type="text"></label>
            <div class="hint">{{ t('0 = automatic from your pace of the last weeks (10 % more).') }}</div>
            <label class="row"><span>{{ t('Proofreading (lines per week)') }}</span><input v-model="s.goals.proofread_per_week" class="text short" type="text"></label>
            <div class="hint">{{ t('0 = automatic.') }}</div></fieldset>
        </template>

        <template v-else-if="section === 'check'">
          <fieldset><legend>{{ t('Translation check') }}</legend>
            <label class="row"><span>{{ t('Check enabled') }}</span><input v-model="s.validation.enabled" type="checkbox"></label>
            <label class="row"><span>{{ t('Text length warnings') }}</span><input v-model="s.validation.length.enabled" type="checkbox"></label><div class="hint">{{ t('Only an aid for the check, never an error.') }}</div>
            <label class="row"><span>{{ t('Longer than (× original)') }}</span><input v-model="s.validation.length.max_ratio" class="text short" type="text"></label><div class="hint">{{ t('The translation is usually longer than the English, default 2.2×.') }}</div>
            <label class="row"><span>{{ t('Shorter than (× original)') }}</span><input v-model="s.validation.length.min_ratio" class="text short" type="text"></label><div class="hint">{{ t('Default 0.45×.') }}</div>
            <label class="row"><span>{{ t('Smallest original (characters)') }}</span><input v-model="s.validation.length.min_source_chars" class="text short" type="text"></label><div class="hint">{{ t('Shorter texts are not checked for length.') }}</div>
            <label class="row"><span>{{ t('Min. difference – longer (characters)') }}</span><input v-model="s.validation.length.min_extra_chars" class="text short" type="text"></label>
            <label class="row"><span>{{ t('Min. difference – shorter (characters)') }}</span><input v-model="s.validation.length.min_missing_chars" class="text short" type="text"></label>
            <label class="row"><span>{{ t('Watch for typos') }}</span><input v-model="s.validation.spelling" type="checkbox"></label><div class="hint">{{ t('Missing diacritics and swapped letters according to the dictionary of your own translation. Only a hint.') }}</div>
            <label class="row"><span>{{ t('Dialogue box check') }}</span><input v-model="s.validation.textbox.enabled" type="checkbox"></label><div class="hint">{{ t('Warns when the translation does not fit the dialogue box and the original did.') }}</div>
            <label class="row"><span>{{ t('Characters per line') }}</span><input v-model="s.validation.textbox.chars_per_line" class="text short" type="text"></label><div class="hint">{{ t("The default 65 comes from the game's gui.rpy (text width 1116 px, font 33 px).") }}</div>
            <label class="row"><span>{{ t('Most lines in the box') }}</span><input v-model="s.validation.textbox.max_lines" class="text short" type="text"></label><div class="hint">{{ t('Default 4.') }}</div></fieldset>
        </template>

        <template v-else-if="section === 'mt'">
          <fieldset><legend>{{ t('Machine translation (optional, experimental)') }}</legend>
            <label class="row"><span>{{ t('Service') }}</span><select v-model="s.mt.provider" class="sel"><option v-for="[k, l] in SERVICES" :key="k" :value="k">{{ l }}</option></select></label>
            <div class="hint">{{ t('Gives only a SUGGESTION for one line after you click “Machine suggestion” in the editor; nothing is ever written by itself.') }}</div>
            <label class="row"><span>{{ t('API key') }}</span><input v-model="mtKey" class="text" type="password" autocomplete="off" :placeholder="s.mt.has_key ? t('(saved – leave empty if you do not want to change it)') : ''"></label>
            <div class="hint">{{ t('Saved as plain text in config.json in %APPDATA%\\Aside. It is not sent back to the app.') }}</div>
            <label v-if="s.mt.has_key" class="row"><span></span><span class="check"><input v-model="mtClear" type="checkbox"> {{ t(' Delete the saved key') }}</span></label>
            <label class="row"><span>{{ t('Service address') }}</span><input v-model="s.mt.base_url" class="text" type="text" :placeholder="t('only for your own server, e.g. http://localhost:1234/v1')"></label>
            <label class="row"><span>Model</span><input v-model="s.mt.model" class="text" type="text" :placeholder="t('only for OpenAI-compatible, e.g. gpt-4o-mini')"></label>
            <label class="row"><span>{{ t('Languages') }}</span><span class="inline"><input v-model="s.mt.source_lang" class="text tiny" type="text"> → <input v-model="s.mt.target_lang" class="text tiny" type="text" :placeholder="t('project')" :title="t('Empty = the language of the project')"></span></label>
            <label class="row"><span>{{ t('Consent') }}</span><span class="check"><input v-model="s.mt.consent" type="checkbox"> {{ t(' I agree that the source text of the line I click is sent to the selected external service') }}</span></label>
            <div class="hint">{{ t('Nothing is sent without consent. Only the text of one line is sent (and for an OpenAI-compatible service the 2 previous lines as context).') }}</div></fieldset>
        </template>

        <template v-else-if="section === 'backup'">
          <fieldset><legend>{{ t('Backups of the translation files') }}</legend>
            <p class="muted">{{ t('A zip of all .rpy files of the translation folder, saved in the subfolder {folder} of the local project data. It is created automatically at start-up (when the last one is older than the set number of hours) and before bulk changes (replacing, fixes, unifying). A restore first saves the current state as another backup.', { folder: 'snapshots' }) }}</p>
            <div class="inline wrap-row">
              <label class="check"><input v-model="s.snapshots.enabled" type="checkbox"> {{ t('Back up automatically') }}</label>
              <span>{{ t('every') }}</span><input v-model="s.snapshots.interval_hours" class="text tiny" type="text"><span>{{ t('hours, keep') }}</span><input v-model="s.snapshots.keep" class="text tiny" type="text"><span>{{ t('backups') }}</span>
              <span class="grow"></span><button class="btn small primary" @click="createSnap">{{ t('Create a backup now') }}</button></div>
            <div v-if="!snaps" class="skeleton" style="height: 60px"></div>
            <div v-else-if="snaps.error" class="notice danger">{{ snaps.error }}</div>
            <div v-else-if="!snaps.items.length" class="empty muted"><b>{{ t('No backup yet') }}</b> {{ t('The first one is created automatically on the next start (or click “Create a backup now”).') }}</div>
            <div v-else class="tablewrap"><table class="grid">
              <thead><tr><th>{{ t('Created') }}</th><th>{{ t('Kind') }}</th><th>{{ t('File count') }}</th><th>{{ t('Size') }}</th><th></th></tr></thead>
              <tbody><tr v-for="m in snaps.items" :key="m.name"><td>{{ (m.time || '').replace('T', ' ') }}</td><td>{{ REASON[m.reason] || m.reason }}</td><td class="num">{{ m.files }}</td><td class="num">{{ fmtSize(m.size) }}</td>
                <td class="nowrap"><button class="btn small" @click="restoring = m.name">{{ t('Restore…') }}</button> <button class="btn small ghost" :title="deleting === m.name ? t('The backup from {time} will be deleted for good.', { time: (m.time || '').replace('T', ' ') }) : ''" @click="deleteSnap(m)">{{ deleting === m.name ? t('Delete') + '?' : t('Delete') }}</button></td></tr></tbody></table></div></fieldset>
          <fieldset><legend>{{ t("Game check (Ren'Py lint)") }}</legend>
            <label class="row"><span>{{ t("Ren'Py SDK folder") }}</span><input v-model="s.lint.sdk" class="text" type="text" placeholder="C:\renpy-8.1.1-sdk"></label>
            <div class="hint">{{ t('The folder with renpy.py (the unpacked SDK from renpy.org). Lint runs in Check → Game lint; it needs the source files of the game above the translation folder.') }}</div></fieldset>
        </template>

        <template v-else-if="section === 'web'">
          <fieldset><legend>{{ t('Website (publishing)') }}</legend>
            <label class="row"><span>{{ t('Repository') }}</span><input v-model="s.web.repo" class="text" type="text" placeholder="owner/repository"></label>
            <div class="hint">{{ t('A folder with a clone, an address or user/repository. Your git (Credential Manager) handles the sign-in – no tokens are stored here.') }}</div>
            <label class="row"><span>{{ t('File in the repository') }}</span><input v-model="s.web.path" class="text" type="text"></label>
            <div class="hint">{{ t('Default web/progress.json.') }}</div>
            <label class="row"><span>{{ t('Name of the game on the website') }}</span><input v-model="s.web.title" class="text" type="text"></label></fieldset>
        </template>

        <template v-else-if="section === 'keys' || section === 'licence'">
          <fieldset><legend>{{ section === 'keys' ? t('Keyboard shortcuts') : t('Licence') }}</legend>
            <p class="muted">{{ t('This screen is in the classic interface for now') }}</p>
            <a class="btn" href="/">{{ section === 'keys' ? t('Keyboard shortcuts') : t('Licence') }}</a></fieldset>
        </template>

        <template v-else>
          <fieldset><legend>{{ t('About the app') }}</legend>
            <p><b>Aside {{ app.version }}</b>{{ t(" – a translation manager for Ren'Py games: edit, proofread, check and follow the progress of a translation. Licence: see the file ") }}<span class="mono">LICENSE</span>.</p>
            <p class="notice"><b>{{ t('State of the app: ') }}</b>{{ t("it is under development and has been tried on a number of Ren'Py games of different kinds. ") }}{{ t('The interface is English with Czech as a language pack; more languages are planned.') }}</p></fieldset>
        </template>

        <div class="savebar"><button class="btn primary" :disabled="saving" @click="save">{{ t('Save settings') }}</button><span class="muted">{{ t('Settings: ') }}<span class="mono">config.json</span>{{ t(' in %APPDATA%\\Aside') }}</span></div>
      </div>
    </div>
    <SnapshotRestore v-if="restoring" :name="restoring" @close="restoring = ''" @done="restored" />
  </main>
</template>

<style scoped>
.screen { padding: var(--s-4) var(--s-5) var(--s-6); display: flex; flex-direction: column; gap: var(--s-3); width: 100%; min-width: 0; box-sizing: border-box; }
.act { margin-left: auto; cursor: pointer; text-decoration: underline; }
.settings { display: grid; grid-template-columns: 180px minmax(0, 1fr); gap: var(--s-5); align-items: start; max-width: 1100px; }
.setnav { display: flex; flex-direction: column; gap: 2px; position: sticky; top: var(--s-3); }
.setnavbtn { all: unset; cursor: pointer; padding: var(--s-2) var(--s-3); border-radius: var(--r-md); color: var(--text-muted); font-size: var(--fs-body); }
.setnavbtn:hover { background: var(--bg-hover); color: var(--text); }
.setnavbtn.on { background: var(--accent-soft); color: var(--accent-text); }
.setnavbtn:focus-visible { box-shadow: var(--focus); }
.setcontent { display: flex; flex-direction: column; gap: var(--s-4); min-width: 0; }
fieldset { border: 1px solid var(--line); border-radius: var(--r-lg); background: var(--bg-card); padding: var(--s-4) var(--s-5); margin: 0; display: flex; flex-direction: column; gap: var(--s-2); }
legend { padding: 0 var(--s-2); font-size: var(--fs-text); font-weight: 500; }
.row { display: grid; grid-template-columns: 260px minmax(0, 1fr); gap: var(--s-3); align-items: center; font-size: var(--fs-body); }
.inline { display: flex; gap: var(--s-2); align-items: center; }
.wrap-row { flex-wrap: wrap; margin-bottom: var(--s-3); }
.hint { margin: 0 0 var(--s-2) calc(260px + var(--s-3)); font-size: var(--fs-small); color: var(--text-faint); }
.text { height: var(--h-control); padding: 0 var(--s-3); background: var(--bg-input); color: var(--text); border: 1px solid var(--line); border-radius: var(--r-md); font: inherit; min-width: 0; flex: 1; width: 100%; box-sizing: border-box; }
.text.short { max-width: 120px; flex: none; } .text.tiny { max-width: 70px; flex: none; }
.text:focus { outline: none; border-color: var(--accent-hi); }
.sel { height: var(--h-control); background: var(--bg-input); color: var(--text); border: 1px solid var(--line); border-radius: var(--r-md); padding: 0 var(--s-2); max-width: 360px; }
.check { display: flex; gap: var(--s-2); align-items: center; }
.grow { flex: 1; }
.tablewrap { overflow: auto; max-height: 280px; border: 1px solid var(--line); border-radius: var(--r-lg); }
.grid { width: 100%; border-collapse: collapse; font-size: var(--fs-body); }
.grid th { position: sticky; top: 0; background: var(--bg-bar); text-align: left; font-size: var(--fs-caption); font-weight: 500; color: var(--text-muted); padding: var(--s-2) var(--s-3); border-bottom: 1px solid var(--line); }
.grid td { padding: 6px var(--s-3); border-bottom: 1px solid var(--line-soft); }
.num { text-align: right; font-variant-numeric: tabular-nums; }
.nowrap { white-space: nowrap; }
.empty { padding: var(--s-5); text-align: center; }
.savebar { display: flex; gap: var(--s-3); align-items: center; padding-top: var(--s-2); border-top: 1px solid var(--line-soft); position: sticky; bottom: 0; background: var(--bg-app); padding-bottom: var(--s-3); }
@media (max-width: 900px) { .settings { grid-template-columns: 1fr; } .setnav { flex-direction: row; flex-wrap: wrap; position: static; } .row { grid-template-columns: 1fr; } .hint { margin-left: 0; } }
</style>
