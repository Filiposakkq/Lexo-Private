<script setup>
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { post } from '../api.js';
import { t, tn } from '../i18n.js';

/* The guide to prepare a game: look at the game, then (as a job that runs on its own) make a working copy, unpack the archives, read the scripts (unrpyc) when needed, and let the game write
   its translation. The game is only read. When it is done the translation folder is put on Home and can be opened. */
const props = defineProps({ path: { type: String, default: '' } });
const emit = defineEmits(['back', 'done']);

const path = ref(props.path);
const language = ref('czech');
const info = ref(null);
const error = ref('');
const phase = ref('form');           // form | running | done | failed
const status = ref({ current: '', steps: [] });
const result = ref(null);
const unrpycPath = ref('');
const where = ref('beside');         // beside (a working copy beside the game) | folder (a copy in a folder of your choice) | inplace (no copy: in the game itself)
const folder = ref('');
const plan = ref(null);
const sdkPath = ref('');
const askSdk = ref(null);           // the offer the user is asked to confirm: {family, version, human}
const download = ref(null);         // a download that runs: {received, total}
let timer = null;
let dlTimer = null;

const mb = 1024 * 1024;
const size = (n) => (n >= 1024 * mb ? (n / (1024 * mb)).toFixed(1) + ' GB' : n >= mb ? Math.round(n / mb) + ' MB' : Math.max(1, Math.round(n / 1024)) + ' kB');
const mode = computed(() => (where.value === 'inplace' ? 'inplace' : 'copy'));
const workspace = computed(() => (where.value === 'folder' ? folder.value.trim() : ''));
async function replan() {
  plan.value = null;
  if (!info.value || !info.value.ready) return;
  if (where.value === 'folder' && !folder.value.trim()) return;
  try { plan.value = await post('prepare/plan', { path: path.value.trim(), mode: mode.value, workspace: workspace.value }); error.value = ''; } catch (e) { error.value = e.message; }
}
watch([where, folder, info], replan);

const checks = computed(() => {
  const i = info.value;
  if (!i) return [];
  return [
    { ok: i.engine, text: !i.engine ? t('The game has no engine of its own, so a Ren\'Py SDK is needed to prepare it: choose the folder of one, or let Aside download it.') : i.engine_source === 'game' ? t('The game has its own engine, so nothing else is needed to write the translation.') : t('The Ren\'Py SDK is used to write the translation: {path}', { path: i.sdk }) },
    { ok: true, text: t('Scripts: {a} as text, {b} only as bytecode; archives: {c}.', { a: i.scripts, b: i.bytecode_only, c: i.archives }) },
    ...(i.needs_unrpyc && i.unrpyc_source === 'bundled' ? [{ ok: true, text: t('The scripts are read with unrpyc, which comes with Aside (version {v}).', { v: i.unrpyc_version === 'v1' ? '1.3.2' : '2.0.4' }) }] : []),
    ...(i.translations.length ? [{ ok: true, text: t('Translations that already exist: {names}.', { names: i.translations.join(', ') }) }] : []),
  ];
});
const needsSdk = computed(() => info.value && !info.value.engine);
async function saveSdk() {
  error.value = '';
  try { await post('prepare/sdk', { path: sdkPath.value }); await look(); } catch (e) { error.value = e.message; }
}
async function startDownload() {
  const offer = askSdk.value; askSdk.value = null; error.value = '';
  try {
    const r = await post('prepare/sdk_download', { family: offer.family });
    download.value = { received: 0, total: offer.size };
    dlTimer = setInterval(() => pollDownload(r.job), 700);
  } catch (e) { error.value = e.message; }
}
async function pollDownload(job) {
  try {
    const s = await post('prepare/status', { job });
    if (!s.done) { download.value = { received: (s.result && s.result.received) || 0, total: (s.result && s.result.total) || download.value.total, step: s.current }; return; }
    clearInterval(dlTimer); download.value = null;
    if (s.success) await look(); else error.value = s.error;
  } catch (e) { clearInterval(dlTimer); download.value = null; error.value = e.message; }
}
const percent = computed(() => (download.value && download.value.total ? Math.min(100, Math.round((download.value.received / download.value.total) * 100)) : 0));
const needsUnrpyc = computed(() => info.value && info.value.needs_unrpyc && !info.value.unrpyc);
const canStart = computed(() => info.value && info.value.ready && plan.value && plan.value.enough && (phase.value === 'form' || phase.value === 'failed'));

const STEPS = [
  { key: 'workspace', label: () => t('Make a working copy'), note: () => t('Links to the files of the game; it takes almost no space.') },
  { key: 'extract', label: () => t('Unpack the archives'), note: () => t('The scripts inside .rpa files.') },
  { key: 'decompile', label: () => t('Read the scripts'), note: () => t('unrpyc turns bytecode into scripts; only when it is needed.') },
  { key: 'generate', label: () => t('Write the translation'), note: () => t('The game itself creates the files of the translation.') },
];
const state = (key) => {
  if (where.value === 'inplace' && key === 'workspace') return 'skipped';
  if (phase.value === 'done') return key === 'decompile' && !(status.value.steps || []).includes('decompile') ? 'skipped' : 'done';
  if ((status.value.steps || []).includes(key)) return 'done';
  if (status.value.current === key) return phase.value === 'failed' ? 'failed' : 'active';
  return 'waiting';
};

async function look() {
  error.value = ''; info.value = null;
  if (!path.value.trim()) return;
  try { info.value = await post('prepare/inspect', { path: path.value.trim() }); unrpycPath.value = info.value.unrpyc || ''; } catch (e) { error.value = e.message; }
}
async function choose(target) {
  if (target === 'folder') {
    try { const r = await post('carry/pick', {}); if (!r.cancelled) folder.value = r.path; } catch (e) { error.value = e.message; }
    return;
  }
  try {
    const r = await post('carry/pick', {});
    if (r.cancelled) return;
    if (target === 'game') { path.value = r.path; await look(); } else if (target === 'sdk') sdkPath.value = r.path; else unrpycPath.value = r.path;
  } catch (e) { error.value = e.message; }
}
async function saveUnrpyc() {
  error.value = '';
  try { await post('prepare/unrpyc', { path: unrpycPath.value }); await look(); } catch (e) { error.value = e.message; }
}
async function start() {
  error.value = '';
  try {
    const r = await post('prepare/start', { path: path.value.trim(), language: language.value.trim(), mode: mode.value, workspace: workspace.value });
    phase.value = 'running'; status.value = { current: 'workspace', steps: [] };
    timer = setInterval(() => poll(r.job), 700);
  } catch (e) { error.value = e.message; }
}
async function poll(job) {
  try {
    const s = await post('prepare/status', { job });
    status.value = s;
    if (s.done) {
      clearInterval(timer);
      if (s.success) { result.value = s.result; phase.value = 'done'; await post('library/add', { path: s.result.folder }).catch(() => {}); emit('done'); } else { error.value = s.error; phase.value = 'failed'; }
    }
  } catch (e) { clearInterval(timer); error.value = e.message; phase.value = 'failed'; }
}
async function open() {
  try { await post('project/set', { path: result.value.folder }); location.href = '/'; } catch (e) { error.value = e.message; }
}
onMounted(look);
onBeforeUnmount(() => { clearInterval(timer); clearInterval(dlTimer); });
</script>

<template>
  <main class="prep">
    <div class="head"><button class="btn ghost" @click="emit('back')">← {{ t('Back to Home') }}</button><h2 class="title">{{ t('Prepare a game') }}</h2><span class="muted" v-if="info">{{ info.name }}</span></div>
    <div class="layout">
      <ol class="steps" :aria-label="t('Steps')">
        <li v-for="s in STEPS" :key="s.key" :class="state(s.key)">
          <span class="dot" aria-hidden="true">{{ state(s.key) === 'done' ? '✓' : state(s.key) === 'failed' ? '!' : state(s.key) === 'active' ? '…' : '' }}</span>
          <div><div class="label">{{ s.label() }}<span v-if="state(s.key) === 'skipped'" class="faint"> · {{ t('not needed') }}</span></div><div class="faint small">{{ s.note() }}</div></div>
        </li>
      </ol>
      <section class="panel">
        <template v-if="phase === 'form' || phase === 'failed'">
          <div class="row-f"><label class="field grow"><input v-model="path" :placeholder="t('Folder of the game')" @keydown.enter="look" @change="look"></label><button class="btn" @click="choose('game')">{{ t('Browse…') }}</button></div>
          <div class="row-f"><label class="label" for="lang">{{ t('Language of the translation') }}</label><label class="field"><input id="lang" v-model="language" maxlength="40"></label></div>
          <ul class="checks" v-if="checks.length"><li v-for="(c, i) in checks" :key="i" :class="c.ok ? 'ok' : 'bad'"><span aria-hidden="true">{{ c.ok ? '✓' : '✗' }}</span> {{ c.text }}</li></ul>
          <div v-if="needsSdk" class="notice warn unrpyc">
            <div>{{ t('A Ren\'Py SDK is the program that writes the translation. This game does not carry one, so choose the folder of an SDK you already have, or let Aside download one from renpy.org.') }}
              <div class="row-f"><label class="field grow"><input v-model="sdkPath" :placeholder="t('Folder of the Ren\'Py SDK')"></label><button class="btn" @click="choose('sdk')">{{ t('Browse…') }}</button><button class="btn" @click="saveSdk">{{ t('Use it') }}</button></div>
              <div v-if="download" class="dl" role="status"><div class="bar"><div class="fill" :style="{ width: percent + '%' }"></div></div><span class="small muted">{{ download.step === 'verify' ? t('Checking the download…') : download.step === 'unpack' ? t('Unpacking…') : t('Downloading… {percent} %', { percent }) }}</span></div>
              <div v-else-if="askSdk" class="ask" role="alertdialog" :aria-label="t('Download the SDK')">
                <p>{{ t('Aside will download Ren\'Py {version} (about {size}) from renpy.org into its settings folder and check its checksum. Continue?', { version: askSdk.version, size: askSdk.human }) }}</p>
                <div class="row-f"><button class="btn primary" @click="startDownload">{{ t('Download') }}</button><button class="btn" @click="askSdk = null">{{ t('Cancel') }}</button></div>
              </div>
              <div v-else class="row-f"><button v-for="o in info.sdk_offers" :key="o.family" class="btn" @click="askSdk = o">{{ t('Download Ren\'Py {version} ({size})', { version: o.version, size: o.human }) }}</button></div>
            </div>
          </div>
          <div v-if="needsUnrpyc" class="notice warn unrpyc">
            <div>{{ t('unrpyc is needed to read the scripts of this game. Choose the file unrpyc.py (a free helper).') }}
              <div class="row-f"><label class="field grow"><input v-model="unrpycPath" placeholder="unrpyc.py"></label><button class="btn" @click="choose('unrpyc')">{{ t('Browse…') }}</button><button class="btn" @click="saveUnrpyc">{{ t('Use it') }}</button></div></div>
          </div>
          <fieldset class="where" v-if="info && info.ready">
            <legend class="label">{{ t('Where to work') }}</legend>
            <label><input type="radio" value="beside" v-model="where"> {{ t('A working copy beside the game (recommended)') }}</label>
            <label><input type="radio" value="folder" v-model="where"> {{ t('A working copy in a folder of my choice') }}</label>
            <div v-if="where === 'folder'" class="row-f"><label class="field grow"><input v-model="folder" :placeholder="t('Folder for the working copy')"></label><button class="btn" @click="choose('folder')">{{ t('Browse…') }}</button></div>
            <label><input type="radio" value="inplace" v-model="where"> {{ t('In the game itself, without a copy') }}</label>
            <p v-if="where === 'inplace'" class="muted small">{{ t('The scripts that are read and the translation are written into the game folder. Keep a backup of the game.') }}</p>
            <p v-else class="muted small">{{ t('The game stays untouched. The working copy links to the files of the game when it is on the same disk, so it takes almost no room.') }}</p>
            <p v-if="plan && mode === 'copy'" class="small" :class="plan.enough ? 'muted' : 'bad'">
              {{ plan.links ? t('The copy will be linked: about {size} to write.', { size: size(plan.need) }) : t('The copy is on another disk, so the game is copied: about {size}.', { size: size(plan.need) }) }}
              <template v-if="plan.free !== null"> {{ t('Free on that disk: {size}.', { size: size(plan.free) }) }}</template>
              <template v-if="!plan.enough"> {{ t('That is not enough: choose a folder on a disk with more room, or work in the game itself.') }}</template>
            </p>
            <p v-if="plan && mode === 'copy'" class="faint small mono">{{ plan.workspace }}</p>
          </fieldset>
        </template>
        <template v-else-if="phase === 'running'"><p>{{ t('Working… this can take a few minutes.') }}</p><div class="skeleton" style="height: 8px"></div></template>
        <template v-else>
          <div class="notice"><span>{{ result.reused ? t('The translation was already prepared.') : tn(result.files || 0, 'The translation is ready: {n} file.', 'The translation is ready: {n} files.') }}</span></div>
          <p class="muted small mono">{{ result.folder }}</p>
        </template>
        <div v-if="error" class="notice danger" role="alert">{{ error }}</div>
        <footer class="actions">
          <button class="btn" @click="emit('back')">{{ phase === 'done' ? t('Back to Home') : t('Cancel') }}</button>
          <button v-if="phase === 'done'" class="btn primary" @click="open">{{ t('Open the project') }}</button>
          <button v-else class="btn primary" :disabled="!canStart" @click="start">{{ phase === 'failed' ? t('Try again') : t('Prepare the game') }}</button>
        </footer>
      </section>
    </div>
  </main>
</template>

<style scoped>
.prep { padding: var(--s-5) var(--s-6) var(--s-8); max-width: 980px; margin: 0 auto; width: 100%; }
.head { display: flex; align-items: center; gap: var(--s-3); margin-bottom: var(--s-4); }
.layout { display: grid; grid-template-columns: 250px minmax(0, 1fr); gap: var(--s-5); align-items: start; }
.steps { list-style: none; margin: 0; padding: var(--s-4); display: flex; flex-direction: column; gap: var(--s-4); background: var(--bg-card); border: 1px solid var(--line); border-radius: var(--r-lg); }
.steps li { display: flex; gap: var(--s-3); align-items: flex-start; }
.dot { flex: none; width: 22px; height: 22px; border-radius: 50%; border: 1px solid var(--line-accent); display: flex; align-items: center; justify-content: center; font-size: var(--fs-caption); color: var(--accent-text); }
.steps .done .dot { background: var(--accent); border-color: var(--accent); color: var(--on-accent); }
.steps .active .dot { border-color: var(--accent-hi); }
.steps .failed .dot { border-color: var(--danger); color: var(--danger); }
.steps .skipped, .steps .waiting { color: var(--text-muted); }
.label { font-size: var(--fs-body); }
.small { font-size: var(--fs-small); }
.panel { display: flex; flex-direction: column; gap: var(--s-3); background: var(--bg-card); border: 1px solid var(--line); border-radius: var(--r-lg); padding: var(--s-5); }
.row-f { display: flex; gap: var(--s-2); align-items: center; margin-top: var(--s-2); flex-wrap: wrap; }
.grow { flex: 1; min-width: 160px; }
.checks { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: var(--s-1); font-size: var(--fs-small); }
.checks .ok span { color: var(--accent-hi); } .checks .bad span { color: var(--danger); }
.actions { display: flex; gap: var(--s-2); justify-content: flex-end; margin-top: var(--s-2); }
.unrpyc { flex-direction: column; align-items: stretch; }
.shield { margin: 0; }
.dl { display: flex; align-items: center; gap: var(--s-3); margin-top: var(--s-3); }
.bar { flex: 1; height: 8px; border-radius: 4px; background: var(--bg-input, var(--line)); overflow: hidden; }
.fill { height: 100%; background: var(--accent); transition: width .3s; }
.ask p { margin: var(--s-3) 0 0; }
.where { border: 0; padding: 0; margin: 0; display: flex; flex-direction: column; gap: var(--s-2); }
.where label { display: flex; gap: var(--s-2); align-items: center; font-size: var(--fs-body); cursor: pointer; }
.where .field { flex: 1; }
.where p { margin: 0; }
.bad { color: var(--danger); }
@media (max-width: 760px) { .layout { grid-template-columns: 1fr; } }
</style>
