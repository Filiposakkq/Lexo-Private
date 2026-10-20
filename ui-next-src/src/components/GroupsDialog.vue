<script setup>
import { computed, onMounted, ref } from 'vue';
import { get, post } from '../api.js';
import { t } from '../i18n.js';
import Dialog from './Dialog.vue';

/* The groups of a project (episodes, chapters, parts, days...): what they are called and which numbers exist. Suggested from the file names. */
const emit = defineEmits(['close', 'done']);
const data = ref(null);
const label = ref('Episode');
const list = ref('');
const useSeries = ref(false);
const useFolders = ref(false);
const error = ref('');
const busy = ref(false);

const WORD = () => ({ Episode: t('Episode'), Chapter: t('Chapter'), Part: t('Part'), Day: t('Day'), Update: t('Update'), Act: t('Act'), Folder: t('Folder') });
/* "1-4, 6, 8-9" -> [1,2,3,4,6,8,9]; null when the text is not a list of numbers */
function parseGroups(text) {
  const out = new Set();
  for (const part of String(text).split(/[,;\s]+/).filter(Boolean)) {
    const m = part.match(/^(\d{1,3})(?:\s*[-–]\s*(\d{1,3}))?$/);
    if (!m) return null;
    const a = +m[1], b = m[2] ? +m[2] : a;
    if (a < 1 || b < a || b - a > 500) return null;
    for (let i = a; i <= b; i++) out.add(i);
  }
  return [...out].sort((x, y) => x - y);
}
/* [1,2,3,4,6] -> "1-4, 6" */
function formatGroups(l) {
  const out = [];
  for (let i = 0; i < l.length; i++) {
    let j = i;
    while (j + 1 < l.length && l[j + 1] === l[j] + 1) j++;
    out.push(j > i + 1 ? `${l[i]}-${l[j]}` : l.slice(i, j + 1).join(', '));
    i = j;
  }
  return out.join(', ');
}
const p = computed(() => data.value.proposal);
const cur = computed(() => data.value.current);
const stem = computed(() => (data.value ? (cur.value.series || p.value.series || '') : ''));
const folders = computed(() => (data.value ? (cur.value.folders.length ? cur.value.folders : p.value.folders || []) : []));
const locked = computed(() => data.value && cur.value.sections > 1);

onMounted(async () => {
  try {
    data.value = await get('groups/suggest');
    const start = cur.value.groups.length ? cur.value : p.value;
    label.value = start.label; list.value = formatGroups(start.groups);
    useSeries.value = cur.value.groups.length ? !!cur.value.series : !!p.value.series;
    useFolders.value = !!cur.value.folders.length || (!p.value.groups.length && folders.value.length > 0);
  } catch (e) { error.value = e.message; }
});
async function save() {
  const groups = folders.value.length && useFolders.value ? [1] : parseGroups(list.value);
  if (!groups || !groups.length) { error.value = t('Write the numbers like 1-8 or 1, 2, 5-7.'); return; }
  busy.value = true; error.value = '';
  try {
    await post('groups/apply', { label: label.value, groups, series: stem.value && useSeries.value ? stem.value : '', folders: folders.value.length && useFolders.value ? folders.value : [] });
    emit('done');
  } catch (e) { error.value = e.message; } finally { busy.value = false; }
}
</script>

<template>
  <Dialog :title="t('Groups of the project')" width="640px" @close="emit('close')">
    <div v-if="error" class="notice danger" role="alert">{{ error }}</div>
    <div v-if="!data && !error" class="skeleton" style="height: 100px"></div>
    <template v-if="data">
      <p class="muted">{{ t('Groups are the numbered units you want statistics for, such as episodes or chapters. Files are assigned to them in the Files tab; the suggestion there reads the numbers from the file and folder names.') }}</p>
      <p class="muted">{{ locked ? t('This project has several sections (seasons) in aside.json; edit the file by hand to change them.')
        : p.groups.length ? t('Suggested from the file names: {label} {list} ({matched} of {total} files).', { label: WORD()[p.label] || p.label, list: formatGroups(p.groups), matched: p.matched, total: p.total })
          : t('No numbered groups (ep1, chapter2, part3, day4, update5…) were found in the file names; write them yourself if the game has them.') }}</p>
      <div class="line">
        <b>{{ t('They are called:') }}</b>
        <select class="sel" v-model="label" :disabled="locked" :aria-label="t('They are called:')"><option v-for="w in data.words" :key="w" :value="w">{{ WORD()[w] || w }}</option></select>
        <b>{{ t('Numbers:') }}</b>
        <label class="field num"><input v-model="list" placeholder="1-8" :disabled="locked || (folders.length && useFolders)" :aria-label="t('Numbers:')"></label>
      </div>
      <label v-if="stem" class="check"><input type="checkbox" v-model="useSeries" :disabled="locked"> {{ t('Files called {a}, {b}2, {b}3… are groups 1, 2, 3…', { a: stem, b: stem }) }}</label>
      <label v-if="folders.length" class="check"><input type="checkbox" v-model="useFolders" :disabled="locked"> {{ t('One group per folder ({n} folders): {list}', { n: folders.length, list: folders.slice(0, 6).join(', ') + (folders.length > 6 ? '…' : '') }) }}</label>
      <p v-if="cur.groups.length" class="muted">{{ t('Now: {label} {list}', { label: WORD()[cur.label] || cur.label, list: formatGroups(cur.groups) }) }}</p>
      <div v-else class="empty muted"><b>{{ t('No groups yet') }}</b> {{ t('Until groups exist, files can only be “General” or unassigned.') }}</div>
    </template>
    <template #footer>
      <button class="btn" @click="emit('close')">{{ t('Cancel') }}</button>
      <button class="btn primary" :disabled="busy || locked || !data" @click="save">{{ t('Save the groups') }}</button>
    </template>
  </Dialog>
</template>

<style scoped>
.line { display: flex; flex-wrap: wrap; gap: var(--s-3); align-items: center; margin: var(--s-3) 0; }
.sel { height: var(--h-control-sm); background: var(--bg-input); color: var(--text); border: 1px solid var(--line); border-radius: var(--r-md); padding: 0 var(--s-2); }
.field.num { width: 280px; }
.check { display: flex; gap: var(--s-2); align-items: center; font-size: var(--fs-body); margin: var(--s-2) 0; }
.empty { padding: var(--s-3) 0; }
</style>
