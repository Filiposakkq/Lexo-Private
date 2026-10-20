<script setup>
import { computed, onMounted, ref } from 'vue';
import { t, tn } from '../i18n.js';
import GroupsDialog from '../components/GroupsDialog.vue';
import { baseName, dateLong, fileState, fmtN, fmtPct, groupName, loadFiles, loadOverview, openCheck, openLines, pillOf, post, project, reloadAll } from '../project.js';

/* Overview: where you are in the translation and one click to carry on, then the numbers (translation, what is left, proofreading, the check),
   the groups (episodes, chapters…), the files by state and the recent activity. */
const emit = defineEmits(['go', 'quick']);
const error = ref('');
const groups = ref(false);
const o = computed(() => project.overview);
const files = computed(() => project.files || []);

onMounted(async () => {
  try { await Promise.all([loadOverview(), loadFiles()]); } catch (e) { error.value = e.message; }
});

const totals = computed(() => o.value.totals);
const est = computed(() => o.value.estimate || {});
const nearest = computed(() => files.value.filter((f) => f.todo > 0 && f.done > 0).sort((a, b) => a.todo - b.todo)[0]);
const last = computed(() => { try { return JSON.parse(localStorage.getItem('aside.last') || 'null'); } catch { return null; } });
const heading = computed(() => (totals.value.remaining
  ? tn(totals.value.remaining, '{n} line left to translate', '{n} lines left to translate', { n: fmtN(totals.value.remaining) })
  : t('Everything is translated')));
const sub = computed(() => [
  est.value.pace_per_day ? t('pace ≈ {n} lines per day', { n: fmtN(Math.round(est.value.pace_per_day)) }) : '',
  est.value.status === 'ok' && est.value.eta ? t('estimated finish {date}', { date: dateLong(est.value.eta) }) : '',
  t('{n} left to proofread ({pct} done)', { n: fmtN(Math.max(0, totals.value.total - totals.value.proofread)), pct: fmtPct(totals.value.proofread_percent) }),
].filter(Boolean).join(' · '));

/* Two rings: the outer one translation, the inner one proofreading. */
const ring = (r, pct) => { const c = 2 * Math.PI * r; return `${(c * Math.max(0, Math.min(100, pct))) / 100} ${c}`; };

function lines(filter) { openLines(filter); emit('go', 'lines'); }
function check(filter) { openCheck(filter); emit('go', 'check'); }
function resume() { if (last.value && last.value.file) lines({ file: last.value.file, status: 'all', line: last.value.line }); else lines({ status: 'todo' }); }
function files_(k) { return files.value.filter((f) => f.status === k).length; }
async function setEpisode(e, state) {
  try { await post('episode', { ep: e.n, state }); await loadOverview(); } catch (err) { error.value = err.message; }
}
const busy = ref(false);
async function undo() {
  busy.value = true; error.value = '';
  try { await post('undo'); await Promise.all([loadOverview(), loadFiles()]); } catch (e) { error.value = e.message; } finally { busy.value = false; }
}
const EPISODE_STATES = ['none', 'wip', 'tr', 'proof', 'done'];
</script>

<template>
  <main class="screen" v-if="o">
    <div v-if="error" class="notice danger" role="alert">{{ error }}</div>
    <div v-if="o.unsaved && o.unsaved.length" class="notice warn">{{ t('Could not save: {files} (the file may be open in Excel). Close it and click Refresh files.', { files: o.unsaved.join(', ') }) }}</div>
    <div v-if="o.last_save_error" class="notice warn">{{ o.last_save_error }}</div>
    <div v-if="o.lock_lost" class="notice warn">{{ t('The project was taken over by another copy of the application in the meantime. Do not save more changes here: close this copy, or open the project again (Change…).') }}</div>

    <section v-if="totals.total" class="hero card">
      <div class="ring">
        <svg viewBox="0 0 100 100" width="132" height="132" role="img" :aria-label="t('Translation {a}, proofreading {b}', { a: fmtPct(totals.percent), b: fmtPct(totals.proofread_percent) })">
          <circle cx="50" cy="50" r="42" class="track" /><circle cx="50" cy="50" r="42" class="arc" :stroke-dasharray="ring(42, totals.percent)" />
          <circle cx="50" cy="50" r="30" class="track" /><circle cx="50" cy="50" r="30" class="arc inner" :stroke-dasharray="ring(30, totals.proofread_percent)" />
        </svg>
        <div class="pct"><b>{{ fmtPct(totals.percent) }}</b><span class="muted">{{ t('translated') }}</span></div>
      </div>
      <div class="body">
        <h2 class="hero-title">{{ heading }}</h2>
        <div class="muted">{{ sub }}</div>
        <div class="actions">
          <button class="btn primary" @click="resume">{{ last && last.file ? t('Continue: {where}', { where: baseName(last.file) + ':' + last.line }) : t('Start translating') }}</button>
          <button v-if="totals.total - totals.proofread > 0" class="btn" @click="lines({ status: 'review' })">{{ t('Continue proofreading') }}</button>
          <button class="btn" :title="t('Scene and editor over the whole screen, untranslated lines only')" @click="emit('quick', 'tr')">{{ t('Translate quickly') }}</button>
          <button v-if="totals.total - totals.proofread > 0" class="btn" :title="t('Scene and editor over the whole screen, only lines to check')" @click="emit('quick', 'proof')">{{ t('Proofread quickly') }}</button>
          <button v-if="nearest" class="btn ghost" :title="t('File with the least left: {name}', { name: nearest.name })" @click="lines({ file: nearest.name, status: 'todo' })">{{ t('Closest to done: {name} ({n} left)', { name: baseName(nearest.name), n: fmtN(nearest.todo) }) }}</button>
        </div>
      </div>
    </section>

    <section class="stats">
      <div class="card stat">
        <div class="label">{{ t('Translation') }}</div>
        <div class="value">{{ fmtPct(totals.percent) }}</div>
        <div class="bar"><i :style="{ width: totals.percent + '%' }"></i></div>
        <div class="sub muted">{{ t('{done} / {total} lines · of which {approved} approved by hand', { done: fmtN(totals.done), total: fmtN(totals.total), approved: fmtN(totals.approved) }) }}</div>
      </div>
      <div class="card stat">
        <div class="label">{{ t('Left to translate') }}</div>
        <div class="value">{{ fmtN(totals.remaining) }} <small>{{ t('lines') }}</small></div>
        <div class="sub muted">{{ t('{n} of {total} files unfinished', { n: o.file_counts.none + o.file_counts.wip, total: o.project.files }) }}</div>
      </div>
      <div class="card stat">
        <div class="label">{{ t('Proofreading') }}</div>
        <div class="value">{{ fmtPct(totals.proofread_percent) }}</div>
        <div class="bar"><i :style="{ width: totals.proofread_percent + '%' }"></i></div>
        <div class="sub muted">{{ t('Proofread: {done} / {total} lines', { done: fmtN(totals.proofread), total: fmtN(totals.total) }) }}</div>
        <div class="sub" v-if="totals.proof_fix"><a href="#" @click.prevent="lines({ status: 'fix' })">{{ t('to fix: {n}', { n: fmtN(totals.proof_fix) }) }}</a></div>
        <div class="sub" v-if="totals.proof_stale"><a href="#" @click.prevent="lines({ status: 'stale' })">{{ t('↻ changed after proofreading: {n}', { n: fmtN(totals.proof_stale) }) }}</a></div>
      </div>
      <div class="card stat">
        <div class="label">{{ t('Translation check') }}</div>
        <div class="value">{{ fmtN(o.validation.error) }} <small>{{ tn(o.validation.error, 'error', 'errors') }}</small></div>
        <div class="sub muted">{{ tn(o.validation.warning, '{n} warning ({length} about text length)', '{n} warnings ({length} about text length)', { n: fmtN(o.validation.warning), length: fmtN(o.validation.length) }) }}</div>
        <div class="sub"><button class="btn small" @click="check({})">{{ t('Open the check') }}</button></div>
      </div>
    </section>

    <div class="two">
      <div class="col">
        <section class="card">
          <h3 class="card-title">{{ t('Progress by group') }}</h3>
          <div v-if="!o.groups_defined" class="empty-groups">
            <b>{{ t('No groups yet') }}</b>
            <p class="muted">{{ t('Episodes, chapters or parts give you statistics for each of them. Aside can guess them from the file names.') }}</p>
            <button class="btn" @click="groups = true">{{ t('Set up groups…') }}</button>
          </div>
          <div v-for="e in o.episodes" :key="e.n" class="ep">
            <span class="nm">{{ groupName(e.n) }}</span>
            <div v-if="e.translation.total" class="bars"><div class="bar"><i :style="{ width: e.translation.percent + '%' }"></i></div><div class="bar thin two"><i :style="{ width: e.proofreading.percent + '%' }"></i></div></div>
            <span v-else class="muted">{{ t('no assigned files') }}</span>
            <span class="pcts num">{{ fmtPct(e.translation.percent) }} <span class="faint">/ {{ fmtPct(e.proofreading.percent) }}</span></span>
            <select class="sel" :value="e.status" :aria-label="t('Status of episode {n}', { n: e.n })" @change="setEpisode(e, $event.target.value)">
              <option v-for="k in EPISODE_STATES" :key="k" :value="k">{{ fileState(k) }}</option>
            </select>
          </div>
          <p class="muted small totals" v-if="o.general.translation.total || o.unassigned.translation.total">{{ t('General (the whole game)') }}: {{ t('{done} / {total} lines', { done: fmtN(o.general.translation.done), total: fmtN(o.general.translation.total) }) }} · {{ t('Unassigned files') }}: {{ t('{done} / {total} lines', { done: fmtN(o.unassigned.translation.done), total: fmtN(o.unassigned.translation.total) }) }}</p>
        </section>
      </div>
      <div class="col">
        <section class="card">
          <h3 class="card-title">{{ t('Files by status') }}</h3>
          <ul class="list"><li v-for="k in ['done', 'rev', 'tr', 'wip', 'none']" :key="k"><span class="pill" :class="pillOf(k)">{{ fileState(k) }}</span><b class="num">{{ o.file_counts[k] }}</b></li></ul>
        </section>
        <section class="card" v-if="o.worst_files && o.worst_files.length">
          <h3 class="card-title">{{ t('Most problematic files') }}</h3>
          <ul class="list"><li v-for="w in o.worst_files" :key="w.name"><span :title="t('{errors} errors, {warnings} warnings in {lines} lines', { errors: w.errors, warnings: w.warnings, lines: fmtN(w.lines) })">{{ baseName(w.name) }}</span><span class="num muted">{{ t('{n} warn · {score} / 100 lines', { n: w.warnings, score: w.score }) }}</span></li></ul>
        </section>
        <section class="card">
          <h3 class="card-title">{{ t('Recent activity') }}</h3>
          <ul class="list" v-if="o.recent.length"><li v-for="r in o.recent" :key="r.at"><span class="muted">{{ r.at }}</span><span class="num">{{ t('{n} translated', { n: (r.translated >= 0 ? '+' : '') + fmtN(r.translated) }) }}<template v-if="r.proofread"> · {{ t('{n} proofread', { n: (r.proofread >= 0 ? '+' : '') + fmtN(r.proofread) }) }}</template></span></li></ul>
          <p v-else class="muted">{{ t('No activity yet') }}</p>
        </section>
        <section class="card">
          <div class="head-row">
            <h3 class="card-title grow-t">{{ t('Recent changes (can be undone)') }}</h3>
            <button v-if="o.undo && o.undo.length" class="btn small" :disabled="busy" :title="t('Undo: {what} (Ctrl+Z)', { what: o.undo[0] })" @click="undo">{{ t('↶ Undo the last file change') }}</button>
          </div>
          <ul class="list" v-if="o.undo && o.undo.length"><li v-for="(u, i) in o.undo" :key="i"><span>{{ u }}</span><span v-if="i === 0" class="muted">Ctrl+Z</span></li></ul>
          <p v-else class="muted">{{ t('Nothing to revert') }}</p>
        </section>
      </div>
    </div>
    <GroupsDialog v-if="groups" @close="groups = false" @done="groups = false; reloadAll()" />
  </main>
  <main class="screen" v-else><div v-if="error" class="notice danger" role="alert">{{ error }}</div><div v-else class="skeleton" style="height: 180px"></div></main>
</template>

<style scoped>
.screen { padding: var(--s-5) var(--s-6) var(--s-8); display: flex; flex-direction: column; gap: var(--s-4); max-width: 1180px; width: 100%; margin: 0 auto; }
.card { padding: var(--s-5); }
.card-title { margin-bottom: var(--s-4); }
.empty-groups { display: flex; flex-direction: column; align-items: flex-start; gap: var(--s-2); }
.empty-groups p { margin: 0; line-height: var(--lh-normal); max-width: 60ch; }
.empty-groups .btn { margin-top: var(--s-2); }
.totals { margin: var(--s-4) 0 0; padding-top: var(--s-3); border-top: 1px solid var(--line-soft); line-height: var(--lh-normal); }
.hero { display: flex; gap: var(--s-6); align-items: center; padding: var(--s-5); border-radius: var(--r-xl); }
.ring { position: relative; flex: none; width: 132px; height: 132px; }
.ring circle { fill: none; stroke-width: 9; stroke-linecap: round; transform: rotate(-90deg); transform-origin: 50% 50%; }
.ring .track { stroke: var(--track); }
.ring .arc { stroke: var(--accent-hi); }
.ring circle:nth-of-type(n+3) { stroke-width: 7; }
.ring .arc.inner { stroke: var(--checked); }
.pct { position: absolute; inset: 0; display: flex; flex-direction: column; align-items: center; justify-content: center; font-size: var(--fs-small); }
.pct b { font-size: var(--fs-heading); font-weight: 500; }
.hero-title { margin: 0 0 var(--s-1); font-size: var(--fs-hero); font-weight: 500; }
.actions { display: flex; flex-wrap: wrap; gap: var(--s-2); margin-top: var(--s-4); }
.stats { display: grid; grid-template-columns: repeat(auto-fit, minmax(230px, 1fr)); gap: var(--s-3); }
.stat .label { color: var(--text-muted); font-size: var(--fs-small); margin-bottom: var(--s-1); }
.stat .value { font-size: var(--fs-hero); font-weight: 500; margin-bottom: var(--s-2); }
.stat .value small { font-size: var(--fs-small); color: var(--text-muted); font-weight: 400; }
.stat .sub { font-size: var(--fs-small); margin-top: var(--s-2); }
.two { display: grid; grid-template-columns: minmax(0, 1.4fr) minmax(0, 1fr); gap: var(--s-3); align-items: start; }
.col { display: flex; flex-direction: column; gap: var(--s-3); }
.list { list-style: none; margin: 0; padding: 0; }
.list li { display: flex; justify-content: space-between; gap: var(--s-3); padding: var(--s-2) 0; border-top: 1px solid var(--line-soft); font-size: var(--fs-body); }
.list li:first-child { border-top: 0; }
.ep { display: grid; grid-template-columns: 110px minmax(0, 1fr) 110px 120px; gap: var(--s-3); align-items: center; padding: var(--s-2) 0; border-top: 1px solid var(--line-soft); font-size: var(--fs-body); }
.ep .bars { display: flex; flex-direction: column; gap: 3px; }
.sel { height: var(--h-control-sm); background: var(--bg-input); color: var(--text); border: 1px solid var(--line); border-radius: var(--r-md); padding: 0 var(--s-2); }
.head-row { display: flex; align-items: center; gap: var(--s-3); }
.grow-t { flex: 1; }
.num { font-variant-numeric: tabular-nums; text-align: right; }
.small { font-size: var(--fs-small); }
@media (max-width: 900px) { .two { grid-template-columns: 1fr; } .hero { flex-direction: column; align-items: flex-start; } }
</style>
