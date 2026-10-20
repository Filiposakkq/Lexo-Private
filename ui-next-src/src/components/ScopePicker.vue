<script setup>
import { computed, ref } from 'vue';
import { t } from '../i18n.js';
import { project } from '../project.js';

/* "Where": all files, or only the files you tick. The dialogs that work on many lines share it. */
const props = defineProps({ scope: { type: String, required: true }, chosen: { type: Array, required: true }, label: { type: String, default: '' } });
const emit = defineEmits(['update:scope', 'update:chosen']);
const q = ref('');
const all = computed(() => (project.files || []).map((x) => x.name));
const shown = computed(() => all.value.filter((n) => !q.value || n.toLowerCase().includes(q.value.toLowerCase())));
function toggle(name, on) { emit('update:chosen', on ? [...props.chosen, name] : props.chosen.filter((n) => n !== name)); }
function setShown(on) {
  const rest = props.chosen.filter((n) => !shown.value.includes(n));
  emit('update:chosen', on ? [...rest, ...shown.value] : rest);
}
</script>

<template>
  <div class="scope">
    <div class="line">
      <b>{{ label || t('Where:') }}</b>
      <div class="seg" role="group">
        <button class="chip" :class="{ on: scope === 'all' }" @click="emit('update:scope', 'all')">{{ t('All files ({n})', { n: all.length }) }}</button>
        <button class="chip" :class="{ on: scope === 'chosen' }" @click="emit('update:scope', 'chosen')">{{ t('Only selected files') }}</button>
      </div>
      <template v-if="scope === 'chosen'">
        <span class="muted small">{{ t('selected {n}', { n: chosen.length }) }}</span>
        <button class="btn small" @click="setShown(true)">{{ t('All shown') }}</button><button class="btn small" @click="setShown(false)">{{ t('None') }}</button>
        <label class="field"><input v-model="q" type="search" :placeholder="t('Search files…')"></label>
      </template>
    </div>
    <div v-if="scope === 'chosen'" class="list"><label v-for="n in shown" :key="n" class="check"><input type="checkbox" :checked="chosen.includes(n)" @change="toggle(n, $event.target.checked)"> {{ n }}</label></div>
  </div>
</template>

<style scoped>
.line { display: flex; flex-wrap: wrap; gap: var(--s-2); align-items: center; margin: var(--s-2) 0; }
.seg { display: flex; gap: var(--s-1); }
.field { width: 220px; }
.list { display: grid; grid-template-columns: repeat(auto-fill, minmax(220px, 1fr)); gap: 2px var(--s-3); max-height: 160px; overflow: auto; padding: var(--s-2); border: 1px solid var(--line); border-radius: var(--r-md); background: var(--bg-input); }
.check { display: flex; gap: var(--s-2); align-items: center; font-size: var(--fs-small); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.small { font-size: var(--fs-small); }
</style>
