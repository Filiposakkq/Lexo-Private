<script setup>
import { computed, ref } from 'vue';
import { get, post } from '../api.js';
import { t } from '../i18n.js';
import { requireFeature } from '../pro.js';
import { dateLong, fileState, fmtN, fmtPct, groupName, pillOf, project } from '../project.js';
import Dialog from '../components/Dialog.vue';

/* Publishing the progress to the website. Two separate steps: generate and check the data (a preview, nothing leaves the computer), then publish it, which shows the exact git
   commands first and needs your confirmation. */
const emit = defineEmits(['go']);
const web = ref(null);
const preview = ref(null);
const busy = ref(false);
const error = ref('');
const plan = ref(null);
const ack = ref(false);
const publishing = ref(false);
const message = ref('');
const sectionName = (n) => { const word = project.overview?.labels?.section || 'Season'; return word === 'Season' ? t('Season {n}', { n }) : `${t(word)} ${n}`; };

get('settings').then((s) => { web.value = s.web || {}; }).catch((e) => { error.value = e.message; });

async function generate() {
  busy.value = true; error.value = ''; preview.value = null;
  try { preview.value = await get('export/preview'); } catch (e) { error.value = e.message; }
  busy.value = false;
}
async function prepare() {
  error.value = '';
  if (!await requireFeature('publish.web')) return;
  try { plan.value = await post('publish/plan'); ack.value = false; } catch (e) { error.value = e.message; }
}
async function publish() {
  publishing.value = true;
  try {
    const r = await post('publish/confirm', { token: plan.value.token, branch: plan.value.will_clone ? '*clone*' : plan.value.branch });
    message.value = r.message; plan.value = null;
  } catch (e) { error.value = e.message; } finally { publishing.value = false; }
}
const canPublish = computed(() => ack.value && plan.value && plan.value.blockers.length === 0 && !publishing.value);
</script>

<template>
  <main class="screen">
    <div class="notice">{{ t('Nothing is sent to GitHub or to the website until you yourself confirm “Publish” in the confirmation window. ') }}{{ t('Generating the data and publishing are two separate steps.') }}</div>
    <div v-if="message" class="notice" role="status">{{ message }}<span class="act" role="button" tabindex="0" @click="message = ''" @keydown.enter="message = ''">{{ t('Close') }}</span></div>
    <div v-if="error" class="notice danger" role="alert">{{ error }}</div>

    <section class="card">
      <h3 class="card-title">{{ t('1. Generate and check the data for the website') }}</h3>
      <p class="muted">{{ t('Creates {file} in the local project data and checks it (episode totals, percentage range, no file names). It is the same file the website draws from.', { file: 'progress.json' }) }}</p>
      <button class="btn primary" :disabled="busy" @click="generate">{{ t('Generate a preview') }}</button>
      <div v-if="busy" class="muted top">{{ t('Generating…') }}</div>
      <template v-if="preview">
        <div v-if="preview.problems.length" class="notice danger top"><b>{{ t('The check failed: ') }}</b> {{ preview.problems.join(' | ') }}</div>
        <div v-else class="notice top">{{ t('✓ The data passed the check.') }}</div>
        <dl class="kv">
          <dt>{{ t('Total') }}</dt><dd>{{ fmtPct(preview.data.translation.percent) }} ({{ fmtN(preview.data.translation.done) }} / {{ fmtN(preview.data.translation.total) }})</dd>
          <dt>{{ t('Proofreading') }}</dt><dd>{{ fmtPct(preview.data.proofreading.percent) }}</dd>
          <dt>{{ t('Files') }}</dt><dd>{{ preview.data.files.total }}</dd>
          <dt>{{ t('Finish estimate') }}</dt><dd>{{ preview.data.estimate.status === 'ok' ? dateLong(preview.data.estimate.eta) : t('not possible yet (little data)') }}</dd>
          <dt>{{ t('Local file') }}</dt><dd class="mono">{{ preview.path }}</dd>
        </dl>
        <div class="tablewrap"><table class="grid">
          <thead><tr><th>{{ t('Season / episode') }}</th><th>{{ t('State') }}</th><th>{{ t('Lines') }}</th><th>{{ t('Percentages') }}</th><th>{{ t('Files') }}</th></tr></thead>
          <tbody>
            <template v-for="s in preview.data.seasons" :key="s.n">
              <tr v-if="s.status === 'planned'"><td>{{ sectionName(s.n) }}</td><td><span class="pill none">{{ t('Planned') }}</span></td><td colspan="3" class="muted">{{ t('no numbers, outside the calculations') }}</td></tr>
              <template v-else>
                <tr class="season"><td><b>{{ sectionName(s.n) }}</b></td><td></td><td class="num">{{ fmtN(s.translation.done) }} / {{ fmtN(s.translation.total) }}</td><td class="num">{{ fmtPct(s.translation.percent) }}</td><td class="num">{{ s.files }}</td></tr>
                <tr v-for="e in preview.data.episodes.filter((x) => x.season === s.n)" :key="s.n + '-' + e.n"><td class="indent">{{ groupName(e.n) }}</td><td><span class="pill" :class="pillOf(e.status)">{{ fileState(e.status) }}</span></td><td class="num">{{ fmtN(e.translation.done) }} / {{ fmtN(e.translation.total) }}</td><td class="num">{{ fmtPct(e.translation.percent) }}</td><td class="num">{{ e.files_total }}</td></tr>
              </template>
            </template>
          </tbody></table></div>
      </template>
    </section>

    <section class="card">
      <h3 class="card-title">{{ t('2. Publish to the website') }}</h3>
      <template v-if="web && web.repo">
        <dl class="kv"><dt>{{ t('Repository') }}</dt><dd>{{ web.repo }}</dd><dt>{{ t('File in the repository') }}</dt><dd>{{ web.path }}</dd></dl>
        <p class="muted">{{ t('A preview is shown: the repository, the branch, what changes, the exact git commands and warnings (for example that a push to the main branch deploys the website live).') }}</p>
        <button class="btn" @click="prepare">{{ t('Prepare publishing…') }}</button>
      </template>
      <template v-else-if="web">
        <p>{{ t('The connection to the website is not set up yet.') }}</p>
        <button class="btn" @click="emit('go', 'settings')">{{ t('Open Settings') }}</button>
      </template>
    </section>

    <Dialog v-if="plan" :title="t('Publishing preview')" width="760px" @close="plan = null">
      <div v-for="(b, i) in plan.blockers" :key="'b' + i" class="notice danger">{{ b }}</div>
      <div v-for="(w, i) in plan.warnings" :key="'w' + i" class="notice">{{ w }}</div>
      <dl class="kv">
        <dt>{{ t('Repository') }}</dt><dd>{{ plan.repo_path }}</dd>
        <dt>{{ t('Remote') }}</dt><dd>{{ plan.remote || plan.url || '—' }}</dd>
        <dt>{{ t('Branch') }}</dt><dd>{{ plan.branch ? `${plan.branch} → ${plan.upstream || t('(no upstream)')}` : t('(default after cloning)') }}</dd>
        <dt>{{ t('File') }}</dt><dd class="mono">{{ plan.target }}</dd>
        <dt>{{ t('On the website so far') }}</dt><dd>{{ plan.current ? t('translation {pct}, updated {when}', { pct: fmtPct(plan.current.translation), when: plan.current.updated || '?' }) : '—' }}</dd>
        <dt>{{ t('After publishing') }}</dt><dd>{{ plan.new ? t('translation {a}, proofreading {b}', { a: fmtPct(plan.new.translation), b: fmtPct(plan.new.proofreading) }) : '—' }}</dd>
        <dt>{{ t('Changes') }}</dt><dd>{{ plan.changed ? t('yes') : t('no (only the time)') }}</dd>
      </dl>
      <h4>{{ t('These commands will run (with this one file only)') }}</h4>
      <pre class="code">{{ plan.commands.join('\n') }}</pre>
      <div v-if="plan.dirty.length" class="muted">{{ t('Other uncommitted changes in the repository (they will not be touched): {files}', { files: plan.dirty.slice(0, 5).join(', ') + (plan.dirty.length > 5 ? '…' : '') }) }}</div>
      <label class="check"><input v-model="ack" type="checkbox"> {{ plan.live ? t('I understand that the data is sent to GitHub and the website is updated live.') : t('I understand that the data is sent to GitHub.') }}</label>
      <template #footer>
        <button class="btn" @click="plan = null">{{ t('Cancel') }}</button>
        <button class="btn primary" :disabled="!canPublish" @click="publish">{{ publishing ? t('Publishing…') : t('Publish') }}</button>
      </template>
    </Dialog>
  </main>
</template>

<style scoped>
.screen { padding: var(--s-4) var(--s-5) var(--s-6); display: flex; flex-direction: column; gap: var(--s-3); width: 100%; min-width: 0; max-width: 1100px; box-sizing: border-box; }
.card { padding: var(--s-5); }
.card-title { margin-bottom: var(--s-3); }
.top { margin-top: var(--s-3); }
.act { margin-left: auto; cursor: pointer; text-decoration: underline; }
.kv { display: grid; grid-template-columns: 200px 1fr; gap: var(--s-1) var(--s-3); margin: var(--s-3) 0; font-size: var(--fs-body); }
.kv dt { color: var(--text-muted); } .kv dd { margin: 0; word-break: break-word; }
.tablewrap { overflow: auto; border: 1px solid var(--line); border-radius: var(--r-lg); background: var(--bg-card); margin-top: var(--s-3); }
.grid { width: 100%; border-collapse: collapse; font-size: var(--fs-body); }
.grid th { text-align: left; font-size: var(--fs-caption); font-weight: 500; color: var(--text-muted); padding: var(--s-2) var(--s-3); border-bottom: 1px solid var(--line); }
.grid td { padding: 6px var(--s-3); border-bottom: 1px solid var(--line-soft); }
.num { text-align: right; font-variant-numeric: tabular-nums; }
.indent { padding-left: var(--s-6) !important; }
.code { margin: var(--s-2) 0; padding: var(--s-3); background: var(--bg-code); border-radius: var(--r-md); font-family: var(--mono); font-size: var(--fs-small); overflow: auto; white-space: pre; }
.check { display: flex; gap: var(--s-2); align-items: center; margin-top: var(--s-3); font-size: var(--fs-body); }
h4 { margin: var(--s-3) 0 var(--s-1); font-size: var(--fs-body); font-weight: 500; color: var(--text-2); }
</style>
