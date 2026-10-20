<script setup>
import { computed, ref } from 'vue';
import { post } from '../api.js';
import { t, tn } from '../i18n.js';
import { baseName } from '../project.js';
import Dialog from './Dialog.vue';

/* Unify: set ONE chosen translation on the lines you tick. The same words can mean something else in another place, so every line of the group is listed
   (speaker, current translation) and only the ticked ones change. The change is one undoable step. */
const props = defineProps({ group: { type: Object, required: true }, variant: { type: Object, required: true }, loose: Boolean, bySpeaker: Boolean });
const emit = defineEmits(['close', 'done']);
const lines = props.group.variants.flatMap((x) => x.lines.map((l) => ({ ...l, current: x.target })));
const key = (l) => `${l.file}|${l.line}`;
const differs = (l) => l.current !== props.variant.target;
const sel = ref(new Set(lines.filter(differs).map(key)));
const speakers = new Set(props.variant.lines.map((l) => (l.who || '').split(' ')[0]));
const error = ref('');
const busy = ref(false);
const different = lines.filter(differs).length;

function toggle(l, on) { const s = new Set(sel.value); if (on) s.add(key(l)); else s.delete(key(l)); sel.value = s; }
const all = () => { sel.value = new Set(lines.filter(differs).map(key)); };
const none = () => { sel.value = new Set(); };
const sameSpeaker = () => { sel.value = new Set(lines.filter((l) => differs(l) && speakers.has((l.who || '').split(' ')[0])).map(key)); };
const count = computed(() => sel.value.size);
async function apply() {
  busy.value = true; error.value = '';
  try {
    const r = await post('consistency/unify', { key: props.group.key, target: props.variant.target, loose: props.loose, by_speaker: props.bySpeaker, lines: lines.filter((l) => sel.value.has(key(l))).map((l) => ({ file: l.file, line: l.line })) });
    emit('done', r.changed);
  } catch (e) { error.value = e.message; busy.value = false; }
}
</script>

<template>
  <Dialog :title="t('Unify the translation')" width="980px" @close="emit('close')">
    <p>{{ t('Original: {source} → new translation: {target}', { source: group.source, target: variant.target }) }}</p>
    <p class="muted">{{ t('The same text can mean something else (another speaker, gender, tone, context). Tick only the lines where it really fits.') }}</p>
    <div class="bar-row">
      <button class="btn small" @click="all">{{ t('Select all') }}</button>
      <button class="btn small" @click="none">{{ t('None') }}</button>
      <button class="btn small" :title="t('Only the lines of speakers who use this variant')" @click="sameSpeaker">{{ t('Only the same speaker') }}</button>
    </div>
    <div v-if="error" class="notice danger" role="alert">{{ error }}</div>
    <div class="wrap"><table>
      <thead><tr><th></th><th>{{ t('Where') }}</th><th>{{ t('Speaker') }}</th><th>{{ t('Now') }}</th></tr></thead>
      <tbody><tr v-for="l in lines" :key="key(l)">
        <td><input type="checkbox" :checked="sel.has(key(l))" :disabled="!differs(l)" :aria-label="baseName(l.file) + ':' + l.line" @change="toggle(l, $event.target.checked)"></td>
        <td>{{ baseName(l.file) }} <span class="muted mono">{{ t('line {n}', { n: l.line }) }}</span></td>
        <td>{{ l.who || '—' }}</td>
        <td>{{ l.current }} <span v-if="!differs(l)" class="pill checked">{{ t('already so') }}</span></td>
      </tr></tbody></table></div>
    <template #footer>
      <span class="muted grow">{{ t('Selected {n} of {total} different', { n: count, total: different }) }}</span>
      <button class="btn" @click="emit('close')">{{ t('Cancel') }}</button>
      <button class="btn primary" :disabled="!count || busy" @click="apply">{{ tn(count, 'Unify {n} line', 'Unify {n} lines') }}</button>
    </template>
  </Dialog>
</template>

<style scoped>
.bar-row { display: flex; flex-wrap: wrap; gap: var(--s-2); margin: var(--s-3) 0; }
.wrap { max-height: 46vh; overflow: auto; border: 1px solid var(--line); border-radius: var(--r-md); }
table { width: 100%; border-collapse: collapse; font-size: var(--fs-body); }
th { position: sticky; top: 0; background: var(--bg-bar); text-align: left; font-size: var(--fs-caption); font-weight: 500; color: var(--text-muted); padding: var(--s-2) var(--s-3); }
td { padding: 6px var(--s-3); border-top: 1px solid var(--line-soft); vertical-align: top; }
.grow { flex: 1; font-size: var(--fs-small); }
</style>
