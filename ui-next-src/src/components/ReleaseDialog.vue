<script setup>
import { computed, onMounted, ref } from 'vue';
import { post } from '../api.js';
import { t, tn } from '../i18n.js';
import Dialog from './Dialog.vue';

/* The release builder (Pro): the translation as a zip the players unpack into their game, with the files you attach, a README, a changelog and the number of the release.
   The result is a file; you put it where you like. The checks tell what stands in the way. */
const emit = defineEmits(['close']);
const st = ref(null);
const plan = ref(null);
const error = ref('');
const result = ref(null);
const busy = ref(false);
const version = ref('');
const notes = ref('');
const credits = ref('');
const extra = ref('');
const outDir = ref('');
const readme = ref(false);
const notesMd = ref(false);
const languageScript = ref(false);
const extras = ref([]);
const mode = ref('partial');
const include = ref([]);
const exclude = ref([]);
const force = ref(false);

// the number after the last release (the last number plus one), or 1.0
const suggest = (last) => { const p = (last || '').split(/[-+]/)[0].split('.').map(Number); return p.length && p.every(Number.isFinite) && last ? p.slice(0, -1).concat(p[p.length - 1] + 1).join('.') : '1.0'; };
const forceable = computed(() => plan.value && !plan.value.can_build && plan.value.can_force);
const STATE = () => ({ complete: t('translated'), partial: t('partly'), none: t('not translated'), other: t('no lines') });
const MARK = { error: '✗', warn: '⚠', ok: '✓' };

onMounted(async () => {
  try {
    st.value = await post('release/state', {});
    const s = st.value.settings;
    version.value = suggest(st.value.last_version);
    credits.value = s.credits || ''; extra.value = s.readme_extra || ''; outDir.value = s.out_dir || '';
    readme.value = !!s.readme; notesMd.value = !!s.notes_md; languageScript.value = !!s.language_script;
    extras.value = (s.extras || []).map((e) => ({ ...e }));
    mode.value = (s.files && s.files.mode) || 'partial'; include.value = [...((s.files && s.files.include) || [])]; exclude.value = [...((s.files && s.files.exclude) || [])];
    await check();
  } catch (e) { error.value = e.message; }
});
const settings = () => ({ extras: extras.value.filter((e) => e.from.trim() || e.to.trim()), files: { mode: mode.value, include: include.value, exclude: exclude.value }, credits: credits.value, readme_extra: extra.value, out_dir: outDir.value, readme: readme.value, notes_md: notesMd.value, language_script: languageScript.value });
async function check(keepResult) {
  if (keepResult !== true) result.value = null;
  error.value = '';
  try {
    await post('release/settings', { settings: settings() });
    plan.value = await post('release/preview', { version: version.value.trim(), notes: notes.value });
    if (!version.value.trim()) version.value = plan.value.suggested_version;
  } catch (e) { error.value = e.message; }
}
function setMode(m) { mode.value = m; include.value = []; exclude.value = []; check(); }
function toggleFile(f, on) {
  include.value = include.value.filter((x) => x !== f.file); exclude.value = exclude.value.filter((x) => x !== f.file);
  if (on !== f.by_mode) (on ? include : exclude).value.push(f.file);     // only what differs from the preset is remembered
  check();
}
async function build() {
  busy.value = true; error.value = '';
  try {
    await post('release/settings', { settings: settings() });
    result.value = await post('release/build', { version: version.value.trim(), notes: notes.value, force: force.value });
    st.value = await post('release/state', {});
    version.value = suggest(result.value.version); notes.value = '';
  } catch (e) { error.value = e.message; }
  busy.value = false;
  await check(true);
}
const copy = async (s) => { try { await navigator.clipboard.writeText(s); } catch { error.value = t('Copying failed.'); } };
</script>

<template>
  <Dialog :title="t('Build a release')" width="860px" @close="emit('close')">
    <div v-if="error" class="notice danger" role="alert">{{ error }}</div>
    <div v-if="!st && !error" class="skeleton" style="height: 120px"></div>
    <template v-if="st">
      <p class="muted">{{ t('The translation as a zip the players unpack into their game folder. The result is a file: you put it where you like. Nothing is uploaded.') }}</p>
      <div class="muted">{{ t('Game: {name} {v}; language: {lang}.', { name: st.game.name || '–', v: st.game.version ? 'v' + st.game.version : '', lang: st.language }) }}<template v-if="st.last_version"> {{ tn(st.releases.length, '{n} release so far, the last {v}.', '{n} releases so far, the last {v}.', { v: st.last_version }) }}</template></div>
      <div class="line"><label for="rel-v">{{ t('Number of the release') }}</label><label class="field v"><input id="rel-v" v-model="version" placeholder="1.0"></label><button class="btn" @click="check()">{{ t('Check') }}</button></div>
      <textarea v-model="notes" class="edit" rows="3" :placeholder="t('What is new, in your words (goes to the changelog and the release notes)')" :aria-label="t('What is new, in your words (goes to the changelog and the release notes)')"></textarea>

      <template v-if="plan">
        <b>{{ t('Files') }}</b>
        <div class="line"><label for="rel-m">{{ t('Take') }}</label>
          <select id="rel-m" class="sel" :value="mode" @change="setMode($event.target.value)"><option value="partial">{{ t('Files translated at least partly') }}</option><option value="complete">{{ t('Only files translated completely') }}</option><option value="all">{{ t('All files') }}</option></select></div>
        <div class="wrap"><table>
          <thead><tr><th></th><th>{{ t('File') }}</th><th>{{ t('Translated') }}</th><th>{{ t('State') }}</th></tr></thead>
          <tbody><tr v-for="f in plan.file_list" :key="f.file"><td><input type="checkbox" :checked="f.selected" :aria-label="f.file" @change="toggleFile(f, $event.target.checked)"></td>
            <td class="mono">{{ f.file }}</td><td class="num">{{ f.total ? f.done + ' / ' + f.total : '–' }}</td><td>{{ STATE()[f.state] }}</td></tr></tbody></table></div>
      </template>

      <div class="opts">
        <label class="check"><input type="checkbox" v-model="readme"> {{ t('README for the players (how to install, credits)') }}</label>
        <label class="check"><input type="checkbox" v-model="notesMd"> {{ t('Release notes (a Markdown file for a page: GitHub, F95, Patreon)') }}</label>
        <label class="check"><input type="checkbox" v-model="languageScript"> {{ t('A script that makes the language the one the game starts in') }}</label>
      </div>
      <textarea v-model="credits" class="edit" rows="2" :placeholder="t('Credits: who translated and proofread')" :aria-label="t('Credits: who translated and proofread')"></textarea>
      <textarea v-model="extra" class="edit" rows="2" :placeholder="t('Anything else for the README (a link, a note)')" :aria-label="t('Anything else for the README (a link, a note)')"></textarea>
      <label class="field"><input v-model="outDir" :placeholder="t('Folder for the files (empty: the local data of the project)')" :aria-label="t('Folder for the files (empty: the local data of the project)')"></label>

      <b>{{ t('Files to attach') }}</b>
      <p class="muted">{{ t('For example a font with the diacritics of your language, or a changed gui file. Each goes into the archive at the place you give (inside the game folder, like game/tl/czech/fonts/Font.ttf).') }}</p>
      <div v-for="(e, i) in extras" :key="i" class="line"><label class="field grow"><input v-model="e.from" :placeholder="t('File on this computer')" :aria-label="t('File on this computer')"></label><label class="field grow"><input v-model="e.to" placeholder="game/tl/czech/fonts/Font.ttf"></label><button class="btn small ghost" @click="extras.splice(i, 1)">{{ t('Remove') }}</button></div>
      <div><button class="btn small" @click="extras.push({ from: '', to: 'game/tl/' + (st.language || 'language') + '/' })">{{ t('+ Attach a file') }}</button></div>

      <template v-if="plan">
        <div class="muted">{{ t('Files in the release: {files}, attached files: {extras}; {pct} % translated.', { files: plan.files, extras: plan.extras, pct: plan.stats.total ? Math.round(100 * plan.stats.translated / plan.stats.total) : 0 }) }}
          {{ plan.previous ? t('The last release: {v} ({date}).', { v: plan.previous.version, date: plan.previous.date }) : t('This is the first release.') }}</div>
        <b>{{ t('Checks') }}</b>
        <div v-for="(c, i) in plan.checks" :key="i" class="notice" :class="{ danger: c.level === 'error', warn: c.level === 'warn' }">{{ MARK[c.level] }} {{ c.message }}</div>
        <label v-if="forceable" class="check"><input type="checkbox" v-model="force"> {{ t('Build anyway (the errors of the check of the translation do not stop it)') }}</label>
      </template>
      <div v-if="result" class="notice"><div><b>{{ t('The file is ready') }}</b>
        <div class="mono path">{{ result.zip }}</div>
        <div class="muted">{{ t('{size} kB; SHA-256 {sha}', { size: Math.round(result.size / 1024).toLocaleString(), sha: result.sha256.slice(0, 16) + '…' }) }}</div>
        <div v-if="result.notes_file" class="muted">{{ t('Release notes: {file}', { file: result.notes_file }) }}</div>
        <button class="btn small" @click="copy(result.zip)">{{ t('Copy the path') }}</button></div></div>
    </template>
    <template #footer>
      <button class="btn" @click="emit('close')">{{ t('Close') }}</button>
      <button class="btn primary" :disabled="busy || !plan || !(plan.can_build || forceable)" @click="build">{{ t('Build the release') }}</button>
    </template>
  </Dialog>
</template>

<style scoped>
.line { display: flex; flex-wrap: wrap; gap: var(--s-2); align-items: center; margin: var(--s-2) 0; }
.field.v { width: 9em; }
.grow { flex: 1; }
.sel { height: var(--h-control-sm); background: var(--bg-input); color: var(--text); border: 1px solid var(--line); border-radius: var(--r-md); padding: 0 var(--s-2); }
.edit { width: 100%; box-sizing: border-box; margin: var(--s-2) 0; padding: var(--s-2) var(--s-3); background: var(--bg-input); color: var(--text); border: 1px solid var(--line); border-radius: var(--r-md); font: inherit; resize: vertical; }
.opts { margin: var(--s-3) 0; display: flex; flex-direction: column; gap: 2px; }
.check { display: flex; gap: var(--s-2); align-items: center; font-size: var(--fs-body); }
.wrap { max-height: 26vh; overflow: auto; border: 1px solid var(--line); border-radius: var(--r-md); margin: var(--s-2) 0; }
table { width: 100%; border-collapse: collapse; font-size: var(--fs-body); }
th { position: sticky; top: 0; background: var(--bg-bar); text-align: left; font-size: var(--fs-caption); font-weight: 500; color: var(--text-muted); padding: var(--s-2) var(--s-3); }
td { padding: 4px var(--s-3); border-top: 1px solid var(--line-soft); }
.num { text-align: right; }
.path { word-break: break-all; margin: 4px 0; }
.notice { margin: 4px 0; }
</style>
