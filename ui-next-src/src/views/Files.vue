<script setup>
import { computed, onMounted, ref } from 'vue';
import GroupsDialog from '../components/GroupsDialog.vue';
import { t, tn } from '../i18n.js';
import { assignLabel, baseName, fileState, fmtN, fmtPct, groupName, groupNumbers, loadFiles, loadOverview, loadTeam, openCheck, openLines, pctOf, pillOf, post, project, reloadAll } from '../project.js';

/* Files: every file with its progress, assignment and state; the detail of the chosen file on the right. Double-click (or "Show lines") opens its lines. */
const emit = defineEmits(['go']);
const error = ref('');
const q = ref('');
const filter = ref('all');         // all | open | rev | done
const assign = ref('*');           // * (all) | '' (unassigned) | gen | ep:N
const sortKey = ref('name');
const sortDir = ref(1);
const selected = ref('');
const picked = ref(new Set());
const bulk = ref('');
const busy = ref(false);
const groups = ref(false);
const who = ref('*');                // * | mine | none | the author of a member

onMounted(async () => {
  loadTeam();
  try { await Promise.all([loadFiles(), loadOverview()]); } catch (e) { error.value = e.message; }
});

const files = computed(() => project.files || []);
const team = computed(() => (project.team && project.team.exists ? project.team : null));
const whoOf = (r) => (team.value ? team.value.assign[r.name] || [] : []);
const nameOf = (a) => { const m = team.value && team.value.members.find((x) => x.author === a); return m ? (m.name || a) : a; };
const assignKey = (a) => (!a ? 1000 : a === 'gen' ? 900 : parseInt(a.slice(3), 10));
const value = {
  who: (r) => whoOf(r).map(nameOf).join(',').toLowerCase(), name: (r) => r.name.toLowerCase(), assign: (r) => assignKey(r.assign), tp: (r) => r.done / r.total, cp: (r) => r.corr / r.total,
  todo: (r) => r.todo, rev: (r) => Math.max(0, r.total - r.corr), order: (r) => r.order, warn: (r) => r.errors * 1000 + r.warnings,
};
const rows = computed(() => {
  const text = q.value.toLowerCase();
  const list = files.value.filter((r) => (!text || r.name.toLowerCase().includes(text))
    && (assign.value === '*' || assign.value === r.assign)
    && (who.value === '*' || !team.value || (who.value === 'mine' ? whoOf(r).includes(team.value.me.author) : who.value === 'none' ? !whoOf(r).length : whoOf(r).includes(who.value)))
    && (filter.value === 'all' || (filter.value === 'open' && r.todo > 0) || (filter.value === 'rev' && r.todo === 0 && r.corr < r.total) || (filter.value === 'done' && r.corr >= r.total)));
  const get = value[sortKey.value];
  return list.slice().sort((a, b) => { const x = get(a), y = get(b); const c = typeof x === 'string' ? x.localeCompare(y, undefined, { numeric: true }) : x < y ? -1 : x > y ? 1 : 0; return (c || a.name.localeCompare(b.name, undefined, { numeric: true })) * sortDir.value; });
});
const totals = computed(() => rows.value.reduce((s, r) => ({ done: s.done + r.done, total: s.total + r.total, corr: s.corr + r.corr, todo: s.todo + r.todo }), { done: 0, total: 0, corr: 0, todo: 0 }));
const current = computed(() => files.value.find((r) => r.name === selected.value));
const allPicked = computed(() => rows.value.length > 0 && rows.value.every((r) => picked.value.has(r.name)));

const COLS = computed(() => [
  ['name', () => t('File')], ['assign', () => t('Assignment')], ...(team.value ? [['who', () => t('Assigned to')]] : []), ['tp', () => t('Translation')], ['cp', () => t('Proofreading')],
  ['todo', () => t('Left to translate')], ['rev', () => t('Left to proofread')], ['order', () => t('State')], ['warn', () => t('Check')],
]);
function sortBy(k) { if (sortKey.value === k) sortDir.value = -sortDir.value; else { sortKey.value = k; sortDir.value = 1; } }
function togglePick(name, on) { const s = new Set(picked.value); if (on) s.add(name); else s.delete(name); picked.value = s; }
function toggleAll(on) { picked.value = on ? new Set(rows.value.map((r) => r.name)) : new Set(); }

async function guarded(fn) {
  busy.value = true; error.value = '';
  try { await fn(); await reloadAll(); } catch (e) { error.value = e.message; } finally { busy.value = false; }
}
const setAssign = (r, v) => guarded(() => post('assign', { name: r.name, v }));
const assignPicked = () => guarded(async () => {
  for (const name of picked.value) await post('assign', { name, v: bulk.value });
  picked.value = new Set();
});
async function setPerson(r, author) {
  const assign = { ...team.value.assign };
  if (author) assign[r.name] = [author]; else delete assign[r.name];
  busy.value = true; error.value = '';
  try { project.team = await post('team/set', { roles: Object.fromEntries(team.value.members.filter((m) => m.role !== 'lead').map((m) => [m.author, m.role])), assign }); } catch (e) { error.value = e.message; } finally { busy.value = false; }
}
const proofreadAll = (r) => guarded(() => post('file/save', { name: r.name, c: 'ano', t: r.done !== r.auto ? String(r.done) : '' }));
function lines(r, status) { openLines({ file: r.name, status }); emit('go', 'lines'); }
async function copyPath(r) { try { await navigator.clipboard.writeText(r.name); } catch { error.value = t('Copying failed.'); } }
async function openEditor(r) { try { await post('open-editor', { file: r.name, line: 1 }); } catch (e) { error.value = e.message; } }
</script>

<template>
  <main class="screen">
    <div v-if="error" class="notice danger" role="alert">{{ error }}</div>
    <div class="toolbar">
      <label class="field search"><input v-model="q" type="search" :placeholder="t('Search files… (Ctrl+F)')"></label>
      <div class="seg" role="group" :aria-label="t('State')">
        <button v-for="[k, l] in [['all', t('All')], ['open', t('Unfinished')], ['rev', t('To proofread')], ['done', t('Finished')]]" :key="k" class="chip" :class="{ on: filter === k }" @click="filter = k">{{ l }}</button>
      </div>
      <select class="sel" v-model="assign" :title="t('Filter by assignment')">
        <option value="*">{{ t('Assignment: all') }}</option><option value="">{{ t('Unassigned') }}</option><option value="gen">{{ t('General') }}</option>
        <option v-for="n in groupNumbers()" :key="n" :value="'ep:' + n">{{ groupName(n) }}</option>
      </select>
      <select v-if="team" class="sel" v-model="who" :title="t('Filter by person')" :aria-label="t('Person')"><option value="*">{{ t('Person: all') }}</option><option value="mine">{{ t('My files') }}</option><option value="none">{{ t('Nobody') }}</option><option v-for="m in team.members" :key="m.author" :value="m.author">{{ m.name || m.author }}</option></select>
      <button class="btn" :title="t('What the groups (episodes, chapters…) are called and which numbers exist')" @click="groups = true">{{ t('Groups…') }}</button>
      <button class="btn" :title="t('Who is in the team of this project and who has which files')" @click="emit('go', 'team')">{{ t('Team…') }}</button>
      <span class="grow"></span>
      <template v-if="picked.size">
        <select class="sel" v-model="bulk" :aria-label="t('Assignment')">
          <option value="">{{ t('Unassigned') }}</option><option value="gen">{{ t('General') }}</option>
          <option v-for="n in groupNumbers()" :key="n" :value="'ep:' + n">{{ groupName(n) }}</option>
        </select>
        <button class="btn" :disabled="busy" @click="assignPicked">{{ t('Assign selected ({n})…', { n: picked.size }) }}</button>
      </template>
      <a class="btn" href="/" :title="t('Suggested assignment, groups and the team are in the classic interface for now')">{{ t('Suggest assignment…') }}</a>
      <span class="muted small">{{ t('Showing {n} of {total} files', { n: rows.length, total: files.length }) }}</span>
    </div>

    <div class="split">
      <div class="tablewrap">
        <table class="grid" v-if="files.length">
          <thead><tr>
            <th class="pick"><input type="checkbox" :checked="allPicked" :title="t('Select all shown')" @change="toggleAll($event.target.checked)"></th>
            <th v-for="[k, label] in COLS" :key="k" :aria-sort="sortKey === k ? (sortDir > 0 ? 'ascending' : 'descending') : 'none'"><button class="th" @click="sortBy(k)">{{ label() }}<span v-if="sortKey === k" aria-hidden="true"> {{ sortDir > 0 ? '▲' : '▼' }}</span></button></th>
          </tr></thead>
          <tbody>
            <tr v-for="r in rows" :key="r.name" :class="{ sel: selected === r.name, picked: picked.has(r.name) }" @click="selected = r.name" @dblclick="lines(r, 'all')">
              <td class="pick"><input type="checkbox" :checked="picked.has(r.name)" :aria-label="t('Select ') + r.name" @click.stop @change="togglePick(r.name, $event.target.checked)"></td>
              <td class="name"><span :title="r.name">{{ baseName(r.name) }}</span><span v-if="r.proof_stale" class="pill none stale">↻ {{ r.proof_stale }}</span></td>
              <td @click.stop><select class="sel" :value="r.assign" :disabled="busy" @change="setAssign(r, $event.target.value)">
                <option value="">{{ t('Unassigned') }}</option><option value="gen">{{ t('General') }}</option>
                <option v-for="n in groupNumbers()" :key="n" :value="'ep:' + n">{{ groupName(n) }}</option>
              </select></td>
              <td v-if="team" @click.stop><select v-if="team.can_manage" class="sel" :value="whoOf(r)[0] || ''" :disabled="busy" :aria-label="t('Assigned to')" @change="setPerson(r, $event.target.value)"><option value="">–</option><option v-for="m in team.members" :key="m.author" :value="m.author">{{ m.name || m.author }}</option></select>
                <span v-else :class="{ muted: !(team.me && whoOf(r).includes(team.me.author)) }" :title="team.me && whoOf(r).includes(team.me.author) ? t('This file is yours.') : ''">{{ whoOf(r).length ? whoOf(r).map(nameOf).join(', ') : '–' }}</span></td>
              <td><div class="cell"><div class="bar"><i :style="{ width: pctOf(r.done, r.total) + '%' }"></i></div><span class="num">{{ fmtN(r.done) }} · {{ fmtPct(pctOf(r.done, r.total)) }}</span></div></td>
              <td><div class="cell"><div class="bar two"><i :style="{ width: pctOf(r.corr, r.total) + '%' }"></i></div><span class="num">{{ fmtN(r.corr) }} · {{ fmtPct(pctOf(r.corr, r.total)) }}</span>
                <button v-if="r.corr < r.total" class="icon-btn" :title="t('Mark the whole file as proofread')" :aria-label="t('Mark the whole file as proofread')" :disabled="busy" @click.stop="proofreadAll(r)">✓</button></div></td>
              <td class="num">{{ fmtN(r.todo) }}</td>
              <td class="num">{{ fmtN(Math.max(0, r.total - r.corr)) }}</td>
              <td><span class="pill" :class="pillOf(r.status)">{{ fileState(r.status) }}</span></td>
              <td><span v-if="r.errors" class="pill danger" :title="t('Errors')">{{ r.errors }}</span> <span v-if="r.warnings" class="pill todo" :title="t('Warnings')">{{ r.warnings }}</span></td>
            </tr>
          </tbody>
          <tfoot><tr>
            <td></td><td>{{ t('Total ({n} shown)', { n: rows.length }) }}</td><td></td><td v-if="team"></td>
            <td class="num">{{ fmtN(totals.done) }} / {{ fmtN(totals.total) }} ({{ fmtPct(pctOf(totals.done, totals.total)) }})</td>
            <td class="num">{{ fmtN(totals.corr) }} ({{ fmtPct(pctOf(totals.corr, totals.total)) }})</td>
            <td class="num">{{ fmtN(totals.todo) }}</td><td class="num">{{ fmtN(Math.max(0, totals.total - totals.corr)) }}</td><td></td><td></td>
          </tr></tfoot>
        </table>
        <div v-else-if="project.files" class="empty muted">{{ t('No files') }}</div>
        <div v-else class="skeleton" style="height: 160px"></div>
        <div v-if="project.files && files.length && !rows.length" class="empty muted"><b>{{ t('No files') }}</b> {{ t('Change the filter or the search text.') }}</div>
      </div>

      <aside class="detail card">
        <template v-if="current">
          <h3 class="card-title">{{ t('File') }}</h3>
          <div class="path mono small">{{ current.name }}</div>
          <div class="actions">
            <button class="btn primary" @click="lines(current, 'all')">{{ t('Show lines') }}</button>
            <button class="btn" @click="lines(current, 'todo')">{{ t('Only untranslated') }}</button>
            <button v-if="current.proof_stale" class="btn" :title="t('Lines that changed after proofreading')" @click="lines(current, 'stale')">{{ t('Changed after proofreading ({n})', { n: current.proof_stale }) }}</button>
            <button class="btn" @click="lines(current, 'review')">{{ t('Continue proofreading') }}</button>
            <button class="btn" @click="openEditor(current)">{{ t('Open in the editor') }}</button>
            <button class="btn" @click="copyPath(current)">{{ t('Copy the path') }}</button>
          </div>
          <dl class="kv">
            <dt>{{ t('Assignment') }}</dt><dd>{{ assignLabel(current.assign) }}</dd>
            <dt>{{ t('Lines in total') }}</dt><dd>{{ fmtN(current.total) }}</dd>
            <dt>{{ t('Translated') }}</dt><dd>{{ fmtN(current.done) }} ({{ fmtPct(pctOf(current.done, current.total)) }})</dd>
            <dt>{{ t('of which approved by hand') }}</dt><dd>{{ fmtN(current.approved) }}</dd>
            <dt>{{ t('Counted automatically') }}</dt><dd>{{ fmtN(current.auto) }}</dd>
            <dt>{{ t('Left to translate') }}</dt><dd>{{ fmtN(current.todo) }}</dd>
            <dt>{{ t('Left to proofread') }}</dt><dd>{{ fmtN(Math.max(0, current.total - current.corr)) }}</dd>
            <dt>{{ t('Proofread') }}</dt><dd>{{ fmtN(current.corr) }} ({{ fmtPct(pctOf(current.corr, current.total)) }})</dd>
            <dt>{{ t('of which line by line') }}</dt><dd>{{ fmtN(current.proof_marked) }}</dd>
            <dt>{{ t('To fix') }}</dt><dd><button v-if="current.proof_fix" class="btn small" @click="lines(current, 'fix')">{{ fmtN(current.proof_fix) }}</button><template v-else>0</template></dd>
            <dt>{{ t('Errors / warnings') }}</dt><dd>{{ current.errors }} / {{ current.warnings }}</dd>
          </dl>
          <button v-if="current.errors + current.warnings" class="btn small" @click="openCheck({ file: current.name }); emit('go', 'check')">{{ t('Show the problems') }}</button>
        </template>
        <template v-else><h3 class="card-title">{{ t('File detail') }}</h3><p class="muted">{{ t('Select a file in the table. Double-click opens its lines.') }}</p></template>
      </aside>
    </div>
    <GroupsDialog v-if="groups" @close="groups = false" @done="groups = false; reloadAll()" />
  </main>
</template>

<style scoped>
.screen { padding: var(--s-4) var(--s-5) var(--s-6); display: flex; flex-direction: column; gap: var(--s-3); width: 100%; min-width: 0; }
.toolbar { display: flex; flex-wrap: wrap; gap: var(--s-2); align-items: center; }
.search { width: 240px; }
.seg { display: flex; gap: var(--s-1); }
.grow { flex: 1; }
.sel { height: var(--h-control-sm); background: var(--bg-input); color: var(--text); border: 1px solid var(--line); border-radius: var(--r-md); padding: 0 var(--s-2); max-width: 130px; }
.split { display: grid; grid-template-columns: minmax(0, 1fr) 340px; gap: var(--s-3); align-items: start; }
.tablewrap { overflow: auto; border: 1px solid var(--line); border-radius: var(--r-lg); background: var(--bg-card); max-height: calc(100vh - 170px); }
.grid { width: 100%; border-collapse: collapse; font-size: var(--fs-body); }
.grid th { position: sticky; top: 0; background: var(--bg-bar); text-align: left; font-size: var(--fs-caption); font-weight: 500; color: var(--text-muted); padding: 0; border-bottom: 1px solid var(--line); z-index: 1; }
.th { all: unset; cursor: pointer; padding: var(--s-2) var(--s-3); display: block; }
.th:focus-visible { box-shadow: var(--focus); }
.grid td { padding: 0 var(--s-3); height: var(--h-row); border-bottom: 1px solid var(--line-soft); white-space: nowrap; }
.grid tbody tr { cursor: default; }
.grid tbody tr:hover { background: var(--bg-hover); }
.grid tbody tr.sel { background: var(--bg-hover); }
.grid tbody tr.sel td:first-child { box-shadow: inset 3px 0 0 var(--accent-hi); }
.grid tfoot td { font-weight: 500; color: var(--text-2); background: var(--bg-bar); position: sticky; bottom: 0; }
.pick { width: 34px; text-align: center; padding: 0 var(--s-2); }
.name { max-width: 300px; overflow: hidden; text-overflow: ellipsis; }
.stale { margin-left: var(--s-2); }
.cell { display: flex; align-items: center; gap: var(--s-2); min-width: 190px; }
.cell .bar { flex: 1; min-width: 60px; }
.num { font-variant-numeric: tabular-nums; text-align: right; color: var(--text-2); }
.icon-btn { all: unset; cursor: pointer; width: 22px; height: 22px; text-align: center; border-radius: var(--r-sm); color: var(--text-muted); }
.icon-btn:hover { background: var(--bg-hover); color: var(--accent-text); }
.icon-btn:focus-visible { box-shadow: var(--focus); }
.detail { padding: var(--s-4); position: sticky; top: var(--s-3); }
.path { word-break: break-all; margin-bottom: var(--s-3); }
.actions { display: flex; flex-wrap: wrap; gap: var(--s-2); margin-bottom: var(--s-3); }
.kv { display: grid; grid-template-columns: auto 1fr; gap: var(--s-1) var(--s-3); font-size: var(--fs-body); margin: 0 0 var(--s-3); }
.kv dt { color: var(--text-muted); } .kv dd { margin: 0; text-align: right; }
.empty { padding: var(--s-6); text-align: center; }
.small { font-size: var(--fs-small); }
@media (max-width: 1000px) { .split { grid-template-columns: 1fr; } .detail { position: static; } }
</style>
