<script setup>
import { computed, onMounted, ref } from 'vue';
import { post } from '../api.js';
import SureButton from '../components/SureButton.vue';
import { t, tn } from '../i18n.js';
import { baseName, loadFiles, loadTeam, project, roleLabel } from '../project.js';
import { requireFeature } from '../pro.js';

/* Team work, version 1 (no server): a team is the people who share the translation folder. Everybody can say who they are and see the team and their files; creating the team and
   giving roles and files is for the lead (Pro). Each person writes only their own file in the shared folder, so sharing (git, Dropbox...) never makes a conflict. */
const error = ref('');
const notice = ref('');
const name = ref('');
const roles = ref({});
const news = ref(null);
const git = ref(null);
const gitBusy = ref(false);
const stats = ref(null);
const days = ref(30);
const busy = ref(false);

const st = computed(() => project.team);
const me = computed(() => (st.value ? st.value.members.find((m) => m.me) : null));

async function reload(fresh) {
  if (fresh) project.team = fresh; else await loadTeam();
  if (project.team) {
    name.value = name.value || project.team.suggest_name || '';
    roles.value = Object.fromEntries(project.team.members.filter((m) => m.role !== 'lead').map((m) => [m.author, m.role]));
    loadFiles().catch(() => {});
  }
  if (project.team && project.team.exists) { loadNews(); if (project.team.can_manage) loadStats(); }
}
onMounted(async () => { await reload(); loadGit(); });
async function act(route, body, done) {
  busy.value = true; error.value = ''; notice.value = '';
  try { await reload(await post(route, body)); if (done) notice.value = done; } catch (e) { error.value = e.message; } finally { busy.value = false; }
}
const join = () => act('team/join', { name: name.value });
async function create() { if (await requireFeature('team.manage')) act('team/create', {}); }
async function accept() { if (await requireFeature('team.manage')) act('team/accept', {}, t('You are the lead now.')); }
const saveRoles = () => act('team/set', { roles: roles.value, assign: st.value.assign }, t('Saved.'));

async function loadNews() {
  try { news.value = await post('team/changes', {}); } catch { news.value = null; }
}
async function seen() { try { await post('team/seen', {}); news.value = { total: 0, items: [], notes: 0 }; } catch (e) { error.value = e.message; } }
const WHAT = (i) => (i.what === 'edited' ? tn(i.n, 'changed {n} line', 'changed {n} lines') : i.what === 'approved' ? tn(i.n, 'approved {n} line', 'approved {n} lines')
  : i.what === 'marked' ? tn(i.n, 'marked {n} line', 'marked {n} lines') : tn(i.n, 'moved {n} file to a group', 'moved {n} files to groups'));

async function loadGit(s) {
  try { git.value = s || await post('git/state', {}); } catch (e) { git.value = { error: e.message }; }
}
async function runGit(route, doneText) {
  if (gitBusy.value) return;
  gitBusy.value = true; error.value = ''; notice.value = '';
  try { const r = await post(route, {}); await loadGit(r); notice.value = doneText(r); await reload(); } catch (e) { error.value = e.message; await loadGit(); } finally { gitBusy.value = false; }
}
const gitParts = computed(() => {
  const s = git.value; if (!s || !s.available || !s.repo) return [];
  const p = [t('Branch {branch}', { branch: s.branch || '–' })];
  p.push(s.changes ? tn(s.changes, '{n} changed file here, not sent yet', '{n} changed files here, not sent yet') : t('nothing waiting to be sent'));
  if (s.upstream && s.behind) p.push(tn(s.behind, '{n} newer change from the others (get it)', '{n} newer changes from the others (get them)'));
  if (s.upstream && s.ahead) p.push(tn(s.ahead, '{n} saved change not sent yet', '{n} saved changes not sent yet'));
  if (!s.remote) p.push(t('no server (remote) is set'));
  return p;
});

async function loadStats() {
  try { stats.value = await post('team/stats', { days: days.value || null }); } catch (e) { stats.value = { error: e.message }; }
}
</script>

<template>
  <main class="screen">
    <h2 class="title">{{ t('Team') }}</h2>
    <div v-if="error" class="notice danger" role="alert">{{ error }}</div>
    <div v-if="notice" class="notice" role="status">{{ notice }}</div>
    <div v-if="!st && !error" class="skeleton" style="height: 120px"></div>
    <div v-else-if="!st" class="notice danger">{{ t('The team could not be read.') }}</div>

    <template v-if="st">
      <section class="card">
        <div v-if="st.me.removed" class="notice danger">{{ t('The lead removed you from this team. Ask the lead to take you back.') }}</div>
        <template v-else-if="!st.me.joined">
          <p>{{ t('Say who you are in this project. Your name is shown to the others in the team; it is written in your own file in the shared folder.') }}</p>
          <div class="line"><label class="field nm"><input v-model="name" maxlength="40" :placeholder="t('Your name')" :aria-label="t('Your name')" @keydown.enter="join"></label><button class="btn primary" :disabled="busy" @click="join">{{ t('Join') }}</button></div>
        </template>
        <template v-else-if="!st.exists">
          <p>{{ t('This project has no team yet. A team is made by the person who leads it (this needs a Pro licence); the others join by opening the same folder and entering their name here.') }}</p>
          <div class="line"><button class="btn primary" :disabled="busy" @click="create">{{ t('Make this a team (I am the lead)') }}</button><SureButton :label="t('Forget my name here')" :question="t('Really?')" @yes="act('team/leave', {})" /></div>
        </template>

        <template v-if="st.exists">
          <div v-if="me" class="line"><span>{{ t('You are') }} <b>{{ me.name || me.author }}</b> · {{ roleLabel(me.role) }}</span><span class="grow"></span><SureButton v-if="me.role !== 'lead'" :label="t('Leave the team')" :question="t('Leave this team?')" @yes="act('team/leave', {}, t('You left the team.'))" /></div>
          <div v-if="st.me.offered" class="notice warn"><div>{{ t('The lead offers you the lead of this team. Taking it needs a Pro licence on this computer; until you accept, the lead stays the lead.') }}<div class="line"><button class="btn primary" @click="accept">{{ t('Accept the lead') }}</button></div></div></div>
          <div v-if="st.can_manage && st.offer" class="notice">{{ t('You offered the lead to {name}; you stay the lead until they accept.', { name: st.offer.name || st.offer.author }) }} <SureButton :label="t('Take the offer back')" :question="t('Take the offer back?')" @yes="act('team/offer_cancel', {})" /></div>

          <div class="wrap"><table>
            <thead><tr><th>{{ t('Name') }}</th><th>{{ t('Role') }}</th><th>{{ t('Files') }}</th><th v-if="st.can_manage"></th></tr></thead>
            <tbody><tr v-for="m in st.members" :key="m.author">
              <td><b>{{ m.name || m.author }}</b><span v-if="m.me" class="muted"> {{ t('(you)') }}</span></td>
              <td><span v-if="m.role === 'lead' || !st.can_manage">{{ roleLabel(m.role) }}</span>
                <select v-else class="sel" v-model="roles[m.author]" :aria-label="t('Role')"><option value="translator">{{ roleLabel('translator') }}</option><option value="proofreader">{{ roleLabel('proofreader') }}</option></select></td>
              <td class="num">{{ m.files }}</td>
              <td v-if="st.can_manage" class="nowrap"><template v-if="!m.me">
                <span v-if="st.offer && st.offer.author === m.author" class="muted">{{ t('offered the lead') }}</span>
                <SureButton v-else :label="t('Offer the lead')" :question="t('Offer the lead to this person?')" @yes="act('team/offer', { author: m.author }, t('The lead was offered.'))" />
                <SureButton :label="t('Remove')" :question="t('Remove this person?')" @yes="act('team/remove', { author: m.author })" /></template></td>
            </tr></tbody></table></div>
          <div v-if="st.can_manage" class="line"><button class="btn primary" :disabled="busy" @click="saveRoles">{{ t('Save roles') }}</button></div>
          <p v-if="st.can_manage && st.removed.length" class="muted">{{ t('Removed:') }} <span v-for="r in st.removed" :key="r.author"><b>{{ r.name || r.author }}</b> <SureButton :label="t('Take back')" :question="t('Take this person back?')" @yes="act('team/restore', { author: r.author })" /> </span></p>
          <p class="muted">{{ st.can_manage ? t('You are the lead: give roles here and files in the table of files (the column “Assigned to”). Everybody in the folder sees them.') : t('Roles and files are given by the lead. Your files are marked in the table of files.') }}</p>
          <p v-if="st.my_files.length"><b>{{ t('Your files:') }}</b> {{ st.my_files.map(baseName).join(', ') }}</p>
        </template>
      </section>

      <section v-if="st.exists" class="card">
        <h3 class="card-title">{{ t('News') }}</h3>
        <p v-if="!news" class="muted">{{ t('Looking for news…') }}</p>
        <p v-else-if="!news.total" class="muted">{{ t('Nothing new since your last look.') }}</p>
        <template v-else>
          <b>{{ t('Since your last look:') }}</b>
          <div v-for="(i, k) in news.items" :key="k"><b>{{ i.name }}</b>: {{ WHAT(i) }} · <span class="mono">{{ baseName(i.file) }}</span></div>
          <div v-if="news.notes">{{ tn(news.notes, '{n} new note to the team.', '{n} new notes to the team.') }}</div>
          <div class="line"><button class="btn small" @click="seen">{{ t('Mark as seen') }}</button></div>
        </template>
      </section>

      <section class="card">
        <h3 class="card-title">{{ t('Sharing with git') }}</h3>
        <p v-if="!git" class="muted">{{ t('Looking at git…') }}</p>
        <div v-else-if="git.error" class="notice danger">{{ git.error }}</div>
        <p v-else-if="!git.available" class="muted">{{ t('git is not installed on this computer. Install git to get and send changes with a button; any other way of sharing the folder (Dropbox, Syncthing) works without it.') }}</p>
        <p v-else-if="!git.repo" class="muted">{{ t('This folder is not a git repository, so the buttons are off. Clone the shared repository and open the translation folder inside it.') }}</p>
        <template v-else>
          <p class="muted">{{ gitParts.join(' · ') }}</p>
          <div class="line">
            <button class="btn" :disabled="gitBusy" :title="t('Gets what the others sent (git pull) and reads the files again. A conflict is not solved for you: nothing changes and you see what git said.')" @click="runGit('git/pull', () => t('Changes received.'))">{{ t('Get changes') }}</button>
            <button class="btn primary" :disabled="gitBusy" :title="t('Saves your changes of the translation folder as one commit and sends them (git push).')" @click="runGit('git/push', (r) => (r.committed ? t('Changes sent.') : t('Nothing new to send; the saved changes were sent.')))">{{ t('Send changes') }}</button>
            <span v-if="gitBusy" class="muted">{{ t('Working…') }}</span>
          </div>
        </template>
      </section>

      <section v-if="st.exists && st.can_manage" class="card">
        <div class="line"><h3 class="card-title grow">{{ t('Statistics') }}</h3>
          <select class="sel" v-model.number="days" :aria-label="t('Statistics')" @change="loadStats"><option :value="7">{{ t('Last 7 days') }}</option><option :value="30">{{ t('Last 30 days') }}</option><option :value="0">{{ t('All time') }}</option></select></div>
        <div v-if="stats && stats.error" class="notice danger">{{ stats.error }}</div>
        <template v-else-if="stats">
          <div class="wrap"><table>
            <thead><tr><th>{{ t('Name') }}</th><th>{{ t('Role') }}</th><th>{{ t('Files') }}</th><th>{{ t('Translated') }}</th><th>{{ t('Proofread') }}</th><th>{{ t('Changed lines') }}</th><th>{{ t('Approved') }}</th><th>{{ t('Marked') }}</th><th>{{ t('Last activity') }}</th></tr></thead>
            <tbody><tr v-for="m in stats.members" :key="m.author"><td><b>{{ m.name || m.author }}</b></td><td>{{ roleLabel(m.role) }}</td><td class="num">{{ m.files }}</td>
              <td class="num">{{ m.lines ? m.done_pct + ' %' : '–' }}</td><td class="num">{{ m.lines ? m.proof_pct + ' %' : '–' }}</td><td class="num">{{ m.edited }}</td><td class="num">{{ m.approved }}</td><td class="num">{{ m.marked }}</td>
              <td class="muted">{{ m.last ? String(m.last).slice(0, 16).replace('T', ' ') : '–' }}</td></tr></tbody></table></div>
          <p v-if="stats.unassigned.files" class="muted">{{ t('Files nobody has: {files} ({pct} % translated).', { files: stats.unassigned.files, pct: stats.unassigned.done_pct }) }}</p>
          <p class="muted">{{ t('“Changed lines” counts texts written in the app since the team was made; approved and marked come from the shared log. The period applies to these numbers, not to the progress of the files.') }}</p>
        </template>
      </section>

      <details v-if="st.exists && st.can_manage" class="card">
        <summary class="muted">{{ t('End the team') }}</summary>
        <p class="muted">{{ t('Ending the team removes the file of the lead; the people, their notes and their logs stay in the folder. You can make a new team later.') }}</p>
        <SureButton :label="t('End the team')" :question="t('End this team?')" danger @yes="act('team/disband', {}, t('The team was ended.'))" />
      </details>
    </template>
  </main>
</template>

<style scoped>
.screen { padding: var(--s-5) var(--s-6) var(--s-8); display: flex; flex-direction: column; gap: var(--s-4); max-width: 1100px; width: 100%; margin: 0 auto; }
.card { padding: var(--s-4); }
.title { margin: 0; font-size: var(--fs-title); font-weight: 500; }
.line { display: flex; flex-wrap: wrap; gap: var(--s-3); align-items: center; margin: var(--s-2) 0; }
.grow { flex: 1; margin: 0; }
.field.nm { width: 260px; }
.sel { height: var(--h-control-sm); background: var(--bg-input); color: var(--text); border: 1px solid var(--line); border-radius: var(--r-md); padding: 0 var(--s-2); }
.wrap { overflow: auto; border: 1px solid var(--line); border-radius: var(--r-md); margin: var(--s-2) 0; }
table { width: 100%; border-collapse: collapse; font-size: var(--fs-body); }
th { text-align: left; background: var(--bg-bar); font-size: var(--fs-caption); font-weight: 500; color: var(--text-muted); padding: var(--s-2) var(--s-3); }
td { padding: 6px var(--s-3); border-top: 1px solid var(--line-soft); }
.num { text-align: right; font-variant-numeric: tabular-nums; }
.nowrap { white-space: nowrap; }
summary { cursor: pointer; }
</style>
