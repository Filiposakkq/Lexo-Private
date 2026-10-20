<script setup>
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { get, post } from '../api.js';
import FixDialog from '../components/FixDialog.vue';
import IgnoreDialog from '../components/IgnoreDialog.vue';
import UnifyDialog from '../components/UnifyDialog.vue';
import { t, tn } from '../i18n.js';
import { requireFeature } from '../pro.js';
import { baseName, fmtN, loadFiles, loadOverview, openLines, project } from '../project.js';

/* Check: what the translation check found, the same English line translated differently, the findings you chose to ignore, and the lint of the game.
   Nothing here rewrites a file by itself: a fix or a unification is shown first and written only after you confirm (with a backup, and it can be undone). */
const emit = defineEmits(['go']);
const mode = ref('findings');            // findings | cons | ign | lint
const f = ref({ file: project.check.file || '', severity: '', category: '', q: '', word: false });
const items = ref([]);
const total = ref(0);
const findings = ref(0);
const sev = ref(null);
const loading = ref(false);
const error = ref('');
const showInfo = ref(false);
const files = computed(() => project.files || []);
const last = ref(null);
const fix = ref(null);                   // { items?, file?, kinds? } of the open fix dialog
const ignore = ref(null);                // the findings of the open ignore dialog
const unify = ref(null);                 // { group, variant }
const message = ref('');
let timer = null, lintTimer = null;

if (project.check.ignored) mode.value = 'ign';
if (project.check.mode) mode.value = project.check.mode;
project.check = { file: project.check.file || '', ignored: false, mode: '' };

const SEV = computed(() => [['', t('Errors + warnings')], ['error', t('Errors')], ['warning', t('Warnings')], ['info', t('Minor things')]]);
const CAT = computed(() => [['', t('All kinds')], ['syntax', t('Tags and variables')], ['structure', t('Structure (speaker, blocks)')], ['length', t('Text length')], ['text', t('Text (numbers, English, typos)')]]);
const sevLabel = (s) => (s === 'error' ? t('Error') : s === 'info' ? t('Info') : t('Warning'));
const sevPill = (s) => (s === 'error' ? 'danger' : s === 'info' ? 'none' : 'todo');
const counts = computed(() => (sev.value ? { '': sev.value.error + sev.value.warning + (showInfo.value ? sev.value.info : 0), error: sev.value.error, warning: sev.value.warning, info: sev.value.info } : null));

/* ---- findings */
async function load(reset = true) {
  if (loading.value) return;
  loading.value = true; error.value = '';
  try {
    if (reset) items.value = [];
    const r = await get('validation', { severity: f.value.severity, category: f.value.category, file: f.value.file, q: f.value.q, word: f.value.word ? '1' : '', offset: reset ? 0 : items.value.length, limit: 200, group: 'line' });
    items.value.push(...r.items);
    total.value = r.total; findings.value = r.findings ?? r.total; sev.value = r.sev_counts;
    if (reset) loadLast();
  } catch (e) { error.value = e.message; } finally { loading.value = false; }
}
async function loadLast() { try { last.value = (await get('fix/last')).last || null; } catch { last.value = null; } }
async function loadSettings() { try { showInfo.value = !!(await get('settings')).validation.show_info; } catch { /* the switch stays off */ } }
async function toggleInfo() {
  try { await post('settings', { validation: { show_info: showInfo.value } }); load(true); } catch (e) { error.value = e.message; }
}
const ignorable = (g) => g.issues.filter((x) => g.line && x.code !== 'duplicate-id');
const filtered = computed(() => !!(f.value.q || f.value.file || f.value.category || f.value.severity));
function resetFilters() { f.value = { file: '', severity: '', category: '', q: '', word: false }; }
watch(f, () => { clearTimeout(timer); timer = setTimeout(() => { if (mode.value === 'findings') load(true); }, 250); }, { deep: true });
async function undoLast() {
  if (!window.confirm(t('The files changed by the last change (a speaker fix or a text edit) are restored to the state before it, from the backup. If they have changed since, the restore is refused.'))) return;
  try { const r = await post('fix/undo'); message.value = t('Undone: {what} ({n} lines).', { what: r.label || t('last change'), n: r.lines }); await changed(); } catch (e) { error.value = e.message; }
}
async function changed() { await Promise.all([loadFiles(), loadOverview()].map((p) => p.catch(() => {}))); refresh(); }
function show(g) { openLines({ file: g.file, line: g.line, status: 'all' }); emit('go', 'lines'); }
async function openEditor(g) { try { await post('open-editor', { file: g.file, line: g.line }); } catch (e) { error.value = e.message; } }
async function fixAll() { if (await requireFeature('fixes.safe')) fix.value = { file: f.value.file }; }
function fixOne(g, x) { fix.value = { items: [{ file: g.file, line: g.line }], kinds: [x.extra.kind || 'speaker'] }; }
function ignoreSome(g, list) { ignore.value = list.map((x) => ({ file: g.file, line: g.line, code: x.code, severity: x.severity, message: x.message })); }

/* ---- the same original translated differently */
const cons = ref(null);
const consLoose = ref(false), consBySpeaker = ref(false), consHidden = ref(new Set()), consShowHidden = ref(false);
try { consHidden.value = new Set(JSON.parse(localStorage.getItem('aside.consHidden') || '[]')); } catch { /* none hidden */ }
const groups = computed(() => (cons.value ? cons.value.groups.filter((g) => consShowHidden.value || !consHidden.value.has(g.key)) : []));
async function loadCons() {
  loading.value = true; error.value = '';
  try { cons.value = await get('consistency', { loose: consLoose.value ? '1' : '', by_speaker: consBySpeaker.value ? '1' : '' }); } catch (e) { error.value = e.message; }
  loading.value = false;
}
function hideGroup(g) {
  const s = new Set(consHidden.value); if (s.has(g.key)) s.delete(g.key); else s.add(g.key); consHidden.value = s;
  try { localStorage.setItem('aside.consHidden', JSON.stringify([...s])); } catch { /* only for this page */ }
}

/* ---- ignored */
const ign = ref(null);
const IGN = computed(() => ({
  active: [t('Ignored'), 'none', t('The finding is hidden.')],
  expired: [t('The text changed'), 'todo', t('The translation changed since it was ignored, so the finding is shown again (the ignore no longer applies).')],
  resolved: [t('No longer happens'), 'ok', t('The text is the same, but the check no longer reports this finding (for example after a settings change).')],
  gone: [t('The line is gone'), 'none', t('The line is no longer in the file.')],
}));
async function loadIgn() {
  loading.value = true; error.value = '';
  try { ign.value = await get('ignore/list'); } catch (e) { error.value = e.message; }
  loading.value = false;
}
async function unignore(it) {
  try { const r = await post('ignore/set', { items: [{ file: it.file, line: it.line || undefined, key: it.key, code: it.code }], on: false }); message.value = t('The finding is active again ({n}).', { n: r.changed }); await changed(); } catch (e) { error.value = e.message; }
}

/* ---- lint */
const lint = ref(null);
const sdk = ref('');
async function loadLint() {
  clearTimeout(lintTimer);
  try {
    const [st, cfg] = await Promise.all([get('lint/status'), get('settings')]);
    lint.value = st; if (!sdk.value) sdk.value = cfg.lint?.sdk || '';
    if (st.state === 'running') lintTimer = setTimeout(() => { if (mode.value === 'lint') loadLint(); }, 1500);
  } catch (e) { error.value = e.message; }
}
async function runLint() {
  try { await post('settings', { lint: { sdk: sdk.value } }); await post('lint/start'); loadLint(); } catch (e) { error.value = e.message; }
}

function refresh() {
  error.value = '';
  if (mode.value === 'findings') load(true);
  else if (mode.value === 'cons') loadCons();
  else if (mode.value === 'ign') loadIgn();
  else loadLint();
}
function setMode(m) { mode.value = m; message.value = ''; refresh(); }
/* the keyboard (or the palette) asks for another view while Check is open */
watch(() => project.check.mode, (m) => { if (m) { setMode(m); project.check = { ...project.check, mode: '' }; } });
watch([consLoose, consBySpeaker], () => { if (mode.value === 'cons') loadCons(); });
onMounted(async () => { loadSettings(); try { if (!project.files) await loadFiles(); } catch { /* the file filter stays empty */ } refresh(); });
onBeforeUnmount(() => { clearTimeout(timer); clearTimeout(lintTimer); });
const nIgnored = computed(() => project.overview?.validation?.ignored || 0);
const MODES = computed(() => [['findings', t('Findings'), t('Findings of the translation check')], ['cons', t('Different translations'), t('The same English line translated differently')],
  ['ign', t('Ignored findings ({n})', { n: nIgnored.value }), t('Findings you let ignore on some lines')], ['lint', t('Game lint'), t("Runs the Ren'Py lint of the whole game (needs the Ren'Py SDK)")]]);
const diffNote = (s) => String(s ?? '');
</script>

<template>
  <main class="screen">
    <div class="toolbar">
      <span class="label-inline">{{ t('Show') }}</span>
      <div class="seg" role="group" :aria-label="t('What to show')"><button v-for="[k, label, title] in MODES" :key="k" class="btn small" :class="{ primary: mode === k }" :aria-pressed="mode === k" :title="title" @click="setMode(k)">{{ label }}</button></div>
      <span class="grow"></span>
      <button v-if="last" class="btn small" :title="t('Done {time}. The files are restored from the backup, but only if they have not changed since the fix.', { time: String(last.time || '').replace('T', ' ') })" @click="undoLast">{{ t('↶ Undo: {what}', { what: last.label || t('the last change') }) }}</button>
      <button class="btn small" :title="t('Safe fixes: speaker / attributes from the original, double spaces, typos. Shows a preview where you tick each line.')" @click="fixAll">{{ t('Fix…') }}</button>
    </div>
    <div v-if="message" class="notice" role="status">{{ message }}<span class="act" role="button" tabindex="0" @click="message = ''" @keydown.enter="message = ''">{{ t('Close') }}</span></div>
    <div v-if="error" class="notice danger" role="alert">{{ error }}</div>

    <!-- findings -->
    <template v-if="mode === 'findings'">
      <div class="toolbar">
        <label class="field search"><input v-model="f.q" type="search" :placeholder="t('Search in the messages and the translation… (Ctrl+F)')" :aria-label="t('Search')"></label>
        <label class="check"><input v-model="f.word" type="checkbox"> {{ t('Whole word') }}</label>
        <select v-model="f.file" class="sel wide" :title="t('File')" :aria-label="t('File')"><option value="">{{ t('All files') }}</option><option v-for="x in files" :key="x.name" :value="x.name">{{ x.name }}</option></select>
        <select v-model="f.category" class="sel" :title="t('Kind of finding')" :aria-label="t('Kind of finding')"><option v-for="[k, l] in CAT" :key="k" :value="k">{{ l }}</option></select>
        <label class="check" :title="t('For example a different number of pauses {w} / {p}')"><input v-model="showInfo" type="checkbox" @change="toggleInfo"> {{ t('Show minor things too') }}</label>
      </div>
      <div class="toolbar">
        <div class="pills" role="group" :aria-label="t('Severity')"><button v-for="[k, l] in SEV" :key="k" class="chip" :class="{ on: f.severity === k }" @click="f.severity = k">{{ l }}<span v-if="counts" class="faint"> {{ fmtN(counts[k]) }}</span></button></div>
        <span class="grow"></span>
        <span class="muted small">{{ t('{lines} with a finding · {findings}', { lines: tn(total, '{n} line', '{n} lines', { n: fmtN(total) }), findings: tn(findings, '{n} finding', '{n} findings', { n: fmtN(findings) }) }) }}<template v-if="items.length < total"> {{ t('(shown {n})', { n: fmtN(items.length) }) }}</template></span>
      </div>
      <div v-if="filtered" class="toolbar"><button class="btn small ghost" @click="resetFilters">{{ t('Clear all filters') }}</button></div>
      <p class="muted small">{{ t("One line of the translation = one item with all its findings below each other. The check never rewrites anything by itself; only the button {fix} next to a speaker / attributes error writes the fix from the original into the file after you confirm (with a backup). {error} = Ren'Py would probably crash or the structure of the script is broken. {warning} = suspicious, but it may be intended (for example the translation lacks a tag, the text is much longer). It is always compared with the original, not with fixed rules.", { fix: t('Fix'), error: t('Error'), warning: t('Warning') }) }}</p>
      <div v-if="loading && !items.length" class="skeleton" style="height: 120px"></div>
      <div v-else-if="!items.length" class="empty muted"><b>{{ t('No findings') }}</b> {{ filtered ? t('No problem was found for these filters. Try clearing the filters.') : t('The translation has no problem.') }}</div>
      <div v-else class="tablewrap"><table class="grid">
        <thead><tr><th>{{ t('Severity') }}</th><th>{{ t('File : line') }}</th><th>{{ t('Findings on the line') }}</th><th>{{ t('Original') }}</th><th>{{ t('Translation') }}</th><th></th></tr></thead>
        <tbody><tr v-for="g in items" :key="g.file + ':' + g.line">
          <td><span class="pill" :class="sevPill(g.severity)">{{ sevLabel(g.severity) }}</span><div v-if="g.issues.length > 1" class="muted small">{{ tn(g.issues.length, '{n} finding', '{n} findings') }}</div></td>
          <td class="nowrap">{{ baseName(g.file) }} <span class="mono muted">{{ t('line {n}', { n: g.line }) }}</span></td>
          <td>
            <div v-for="x in g.issues" :key="x.code" class="finding">
              <span class="pill" :class="sevPill(x.severity)">{{ sevLabel(x.severity) }}</span> <span>{{ x.message }}</span>
              <span class="fact">
                <button v-if="x.extra && x.extra.fixable" class="btn small primary" :title="x.extra.kind === 'speaker' ? t('Copies the speaker and attributes from the original (“{who}”)', { who: g.src_who }) : t('Shows a preview of the fix')" @click="fixOne(g, x)">{{ t('Fix') }}</button>
                <button v-if="g.line && x.code !== 'duplicate-id'" class="btn small ghost" :title="t('Hides this finding on this line (can be undone)')" @click="ignoreSome(g, [x])">{{ t('Ignore…') }}</button>
              </span>
            </div>
            <button v-if="ignorable(g).length > 1" class="btn small ghost" :title="t('Hides all findings of this line at once')" @click="ignoreSome(g, ignorable(g))">{{ t('Ignore all {n}…', { n: ignorable(g).length }) }}</button>
          </td>
          <td class="txt">{{ diffNote(g.source) }}</td><td class="txt">{{ diffNote(g.target) }}</td>
          <td class="nowrap"><button class="btn small" :title="t('Show in the list of lines (where the text can be edited at once)')" @click="show(g)">{{ t('Show') }}</button> <button class="btn small ghost" :title="t('Open in an external editor')" @click="openEditor(g)">{{ t('Editor') }}</button></td>
        </tr></tbody></table></div>
      <div v-if="items.length < total" class="more-row"><button class="btn" :disabled="loading" @click="load(false)">{{ t('Load more ({n} of {total} lines)', { n: fmtN(items.length), total: fmtN(total) }) }}</button></div>
    </template>

    <!-- the same original translated differently -->
    <template v-else-if="mode === 'cons'">
      <div class="notice stack">
        <div>{{ t('The same English line translated differently. {exact} – “Yeah.”, “Yeah!” and “YEAH!!!” are different lines and rightly translated differently, so they are never merged. The same text can also mean something else or have another speaker, so nothing is unified by itself: click {unify} at a variant, choose the lines concerned, and confirm (it can be undone). {keep} hides the group.', { exact: t('The exact text of the original is compared'), unify: t('Unify…'), keep: t('Leave as it is') }) }}</div>
        <div class="toolbar">
          <label class="check" :title="t('Lines of different characters are never merged into one group')"><input v-model="consBySpeaker" type="checkbox"> {{ t('Distinguish the speaker') }}</label>
          <label class="check" :title="t('Ignores case and punctuation – also finds “Yeah.” vs “YEAH!!!”, only as a hint, you usually do not want to unify them')"><input v-model="consLoose" type="checkbox"> {{ t('Looser comparison (words only)') }}</label>
          <label class="check"><input v-model="consShowHidden" type="checkbox"> {{ t('Show hidden ones too ({n})', { n: consHidden.size }) }}</label>
        </div>
      </div>
      <div v-if="loading && !cons" class="skeleton" style="height: 120px"></div>
      <template v-else-if="cons">
        <div class="muted small">{{ t('{originals} with several translations ({n} lines in total)', { originals: tn(groups.length, '{n} original', '{n} originals', { n: fmtN(groups.length) }), n: fmtN(cons.lines) }) }}</div>
        <div v-if="!groups.length" class="empty muted"><b>{{ t('Everything is consistent') }}</b> {{ t('No repeated original has several different translations.') }}</div>
        <div v-else class="tablewrap"><table class="grid">
          <thead><tr><th>{{ t('Original') }}</th><th>{{ t('Translation variants') }}</th><th></th></tr></thead>
          <tbody><tr v-for="g in groups" :key="g.key">
            <td class="txt"><div v-if="g.sources && g.sources.length > 1"><div v-for="(s, i) in g.sources" :key="i">{{ s }}</div></div><div v-else>{{ g.source }}</div><div class="muted small">{{ t('{n}× in total', { n: g.total }) }}</div></td>
            <td>
              <div v-for="(v, i) in g.variants" :key="i" class="variant">
                <div class="vrow"><span class="pill none">×{{ v.count }}</span><span class="vtxt">{{ v.target }}</span><button class="btn small" :title="t('You choose lines and change them to this variant')" @click="unify = { group: g, variant: v }">{{ t('Unify…') }}</button></div>
                <div class="vlines"><a v-for="l in v.lines.slice(0, 6)" :key="l.file + l.line" href="#" class="mono" :title="l.who ? t('speaker: {who}', { who: l.who }) : ''" @click.prevent="show(l)">{{ baseName(l.file) }}:{{ l.line }}<template v-if="l.who"> ({{ l.who.split(' ')[0] }})</template></a><span v-if="v.lines.length > 6"> … +{{ v.lines.length - 6 }}</span></div>
              </div>
            </td>
            <td><button class="btn small ghost" :title="t('This group is fine, hide it from the list')" @click="hideGroup(g)">{{ consHidden.has(g.key) ? t('Watch again') : t('Leave as it is') }}</button></td>
          </tr></tbody></table></div>
      </template>
    </template>

    <!-- ignored -->
    <template v-else-if="mode === 'ign'">
      <div v-if="loading && !ign" class="skeleton" style="height: 120px"></div>
      <template v-else-if="ign">
        <div class="muted small">{{ tn(ign.items.length, '{n} ignored finding', '{n} ignored findings', { n: fmtN(ign.items.length) }) }}</div>
        <div v-if="!ign.items.length" class="empty muted"><b>{{ t('You are not ignoring anything') }}</b> {{ t('Findings in the Check list have an Ignore button. Ignored ones appear here.') }}</div>
        <div v-else class="tablewrap"><table class="grid">
          <thead><tr><th>{{ t('State') }}</th><th>{{ t('File : line') }}</th><th>{{ t('Finding') }}</th><th>{{ t('Note') }}</th><th>{{ t('Translation') }}</th><th></th></tr></thead>
          <tbody><tr v-for="(it, i) in ign.items" :key="i">
            <td :title="IGN[it.state] && IGN[it.state][2]"><span class="pill" :class="IGN[it.state] ? IGN[it.state][1] : 'none'">{{ IGN[it.state] ? IGN[it.state][0] : it.state }}</span></td>
            <td class="nowrap">{{ baseName(it.file) }} <span class="mono muted">{{ it.line ? t('line {n}', { n: it.line }) : '' }}</span></td>
            <td>{{ it.message || it.code }}</td>
            <td>{{ it.note || '—' }}<div v-if="it.date" class="muted small">{{ it.date }}</div></td>
            <td class="txt">{{ diffNote(it.target) }}</td>
            <td class="nowrap"><button v-if="it.line" class="btn small" @click="show(it)">{{ t('Show') }}</button> <button class="btn small" :title="t('The finding is reported again')" @click="unignore(it)">{{ t('Stop ignoring') }}</button></td>
          </tr></tbody></table></div>
      </template>
    </template>

    <!-- lint -->
    <template v-else>
      <div class="notice">{{ t("Ren'Py lint checks the whole game (missing images, unknown labels, broken translations) and finds things our checks cannot see. It needs the Ren'Py SDK folder and the source files of the game (game/…) above the translation folder. It can take a minute or longer; you can keep using the app meanwhile.") }}</div>
      <div class="toolbar">
        <label class="field grow"><input v-model="sdk" type="text" :placeholder="t('Ren\'Py SDK folder, e.g. C:\\renpy-8.1.1-sdk')" :aria-label="t('Ren\'Py SDK folder, e.g. C:\\renpy-8.1.1-sdk')"></label>
        <button class="btn primary" :disabled="lint && lint.state === 'running'" @click="runLint">{{ lint && lint.state === 'running' ? t('Running…') : t('Save the path and run lint') }}</button>
      </div>
      <div v-if="!lint" class="skeleton" style="height: 60px"></div>
      <div v-else-if="lint.state === 'running'" class="muted">{{ t('Lint is running… {n} s', { n: Math.round(lint.elapsed || 0) }) }}</div>
      <div v-else-if="lint.state === 'error'" class="notice danger" role="alert">{{ lint.error }}</div>
      <template v-else-if="lint.state === 'done'">
        <div class="muted small">{{ tn(lint.issues.length, '{n} finding', '{n} findings', { n: fmtN(lint.issues.length) }) }} · {{ lint.seconds }} s</div>
        <div class="notice" :class="{ warn: !lint.ok }">{{ lint.ok ? t('Lint finished without an error.') : t('Lint ended with code {code} – see the output below.', { code: lint.exit_code }) }}</div>
        <div v-if="lint.issues.length" class="tablewrap"><table class="grid">
          <thead><tr><th>{{ t('File : line') }}</th><th>{{ t('Message') }}</th><th></th></tr></thead>
          <tbody><tr v-for="(i, k) in lint.issues" :key="k"><td class="nowrap">{{ baseName(i.file) }} <span class="mono muted">{{ t('line {n}', { n: i.line }) }}</span></td><td>{{ i.message }}</td><td><button v-if="i.rel" class="btn small" @click="show({ file: i.rel, line: i.line })">{{ t('Show') }}</button></td></tr></tbody></table></div>
        <div v-else class="muted">{{ t('Lint reports no line of the form file:line.') }}</div>
        <details><summary>{{ t('The whole lint output') }}</summary><pre class="code">{{ lint.raw || '' }}</pre></details>
      </template>
      <div v-else class="muted">{{ t('Lint has not been run yet.') }}</div>
    </template>

    <FixDialog v-if="fix" :items="fix.items || []" :file="fix.file || ''" :kinds="fix.kinds || ['speaker', 'spaces', 'typo']" @close="fix = null" @done="fix = null; changed()" />
    <IgnoreDialog v-if="ignore" :items="ignore" @close="ignore = null" @done="(n) => { ignore = null; message = tn(n, 'Ignored {n} finding. Undo: Ctrl+Z or Check → Ignored.', 'Ignored {n} findings. Undo: Ctrl+Z or Check → Ignored.'); changed(); }" />
    <UnifyDialog v-if="unify" :group="unify.group" :variant="unify.variant" :loose="consLoose" :by-speaker="consBySpeaker" @close="unify = null" @done="(n) => { unify = null; message = t('{lines}. Undo: “Undo the last file change”.', { lines: tn(n, 'Unified: {n} line', 'Unified: {n} lines') }); changed(); }" />
  </main>
</template>

<style scoped>
.screen { padding: var(--s-4) var(--s-5) var(--s-6); display: flex; flex-direction: column; gap: var(--s-3); width: 100%; min-width: 0; box-sizing: border-box; }
.toolbar { display: flex; flex-wrap: wrap; gap: var(--s-2); align-items: center; }
.label-inline { font-size: var(--fs-small); color: var(--text-muted); }
.seg, .pills { display: flex; flex-wrap: wrap; gap: var(--s-1); }
.grow { flex: 1; }
.search { width: 300px; }
.check { display: flex; gap: var(--s-1); align-items: center; font-size: var(--fs-small); color: var(--text-muted); white-space: nowrap; }
.sel { height: var(--h-control-sm); background: var(--bg-input); color: var(--text); border: 1px solid var(--line); border-radius: var(--r-md); padding: 0 var(--s-2); max-width: 220px; }
.sel.wide { max-width: 300px; }
.small { font-size: var(--fs-small); }
.notice.stack { flex-direction: column; align-items: stretch; }
.act { margin-left: auto; cursor: pointer; text-decoration: underline; }
.tablewrap { overflow: auto; border: 1px solid var(--line); border-radius: var(--r-lg); background: var(--bg-card); }
.grid { width: 100%; border-collapse: collapse; font-size: var(--fs-body); }
.grid th { position: sticky; top: 0; background: var(--bg-bar); text-align: left; font-size: var(--fs-caption); font-weight: 500; color: var(--text-muted); padding: var(--s-2) var(--s-3); border-bottom: 1px solid var(--line); }
.grid td { padding: var(--s-2) var(--s-3); border-bottom: 1px solid var(--line-soft); vertical-align: top; }
.nowrap { white-space: nowrap; }
.txt { min-width: 180px; max-width: 320px; word-break: break-word; }
.finding { display: flex; flex-wrap: wrap; gap: var(--s-2); align-items: center; padding: var(--s-1) 0; }
.fact { display: inline-flex; gap: var(--s-1); margin-left: auto; }
.variant { margin-bottom: var(--s-3); }
.vrow { display: flex; gap: var(--s-2); align-items: center; }
.vtxt { flex: 1; }
.vlines { display: flex; flex-wrap: wrap; gap: var(--s-2); font-size: var(--fs-small); margin-top: var(--s-1); }
.more-row, .empty { padding: var(--s-5); text-align: center; }
.code { margin: var(--s-2) 0 0; padding: var(--s-3); background: var(--bg-code); border-radius: var(--r-md); font-family: var(--mono); font-size: var(--fs-small); overflow: auto; max-height: 320px; }
</style>
