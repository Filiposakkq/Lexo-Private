<script setup>
import { computed, ref } from 'vue';
import { t } from '../i18n.js';
import { baseName, fmtN, speakerInitial, speakerColor } from '../project.js';

/* The film layout has no list of lines, so a search or a filter needs its own way to move between the hits: "Result 3 of 16", previous / next, and a list of all hits to pick from.
   It is shown only when something narrows the lines down (search text, a file, a line type, a kind, a check) and there are at least two hits. */
const props = defineProps({ items: { type: Array, required: true }, total: { type: Number, default: 0 }, offset: { type: Number, default: 0 }, sel: { type: Number, default: -1 }, summary: { type: String, default: '' }, more: Boolean });
const emit = defineEmits(['select', 'more']);
function stored() { try { return localStorage.getItem('aside.resOpen') === '1'; } catch { return false; } }
const open = ref(stored());
function toggle() { open.value = !open.value; try { localStorage.setItem('aside.resOpen', open.value ? '1' : '0'); } catch { /* only for this page */ } }
const at = computed(() => props.offset + props.sel + 1);
</script>

<template>
  <div class="resbar">
    <div class="row-r">
      <span>{{ t('Result {n} of {total}', { n: fmtN(at), total: fmtN(total) }) }}</span>
      <button class="btn small icon" :title="t('Previous line (←)')" @click="emit('select', sel - 1)">←</button>
      <button class="btn small icon" :title="t('Next line (→)')" @click="emit('select', sel + 1)">→</button>
      <button class="btn small" @click="toggle">{{ open ? t('Hide the list') : t('Show the list') }}</button>
      <span class="muted small">{{ summary }}</span>
    </div>
    <div v-if="open" class="reslist">
      <div v-for="(it, i) in items" :key="it.file + ':' + it.line" class="resrow" :class="{ cur: i === sel, todo: it.status === 'todo' }" @click="emit('select', i)">
        <span class="num muted">{{ it.line }}</span>
        <span class="who"><span v-if="it.who" class="sbadge" :style="{ background: speakerColor(it.who) }">{{ speakerInitial(it.who) }}</span></span>
        <span class="txt">{{ it.source }}</span><span class="txt">{{ it.status === 'todo' ? '–' : it.target }}</span>
        <span class="muted small file">{{ baseName(it.file) }}</span>
      </div>
      <button v-if="more" class="btn small" @click="emit('more')">{{ t('Load more') }}</button>
    </div>
  </div>
</template>

<style scoped>
.resbar { border: 1px solid var(--line); border-radius: var(--r-lg); background: var(--bg-card); padding: var(--s-2) var(--s-3); display: flex; flex-direction: column; gap: var(--s-2); }
.row-r { display: flex; flex-wrap: wrap; gap: var(--s-2); align-items: center; font-size: var(--fs-body); }
.reslist { max-height: 30vh; overflow: auto; border-top: 1px solid var(--line-soft); padding-top: var(--s-2); }
.resrow { display: grid; grid-template-columns: 56px 28px minmax(0, 1fr) minmax(0, 1fr) 110px; gap: var(--s-2); align-items: center; padding: 4px var(--s-2); border-radius: var(--r-md); cursor: pointer; font-size: var(--fs-body); }
.resrow:hover { background: var(--bg-hover); }
.resrow.cur { background: var(--bg-hover); box-shadow: inset 3px 0 0 var(--accent-hi); }
.txt { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.num { text-align: right; font-variant-numeric: tabular-nums; }
.sbadge { width: 20px; height: 20px; border-radius: 50%; display: inline-flex; align-items: center; justify-content: center; font-size: 11px; color: #fff; }
.small { font-size: var(--fs-small); }
</style>
