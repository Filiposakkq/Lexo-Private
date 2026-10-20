<script setup>
import { nextTick, onMounted, ref, watch } from 'vue';
import { contextFor, peekContext } from '../gamewindow.js';
import { t } from '../i18n.js';
import { speakerColor, speakerInitial, speakerLabel } from '../project.js';

/* The lines around the open one as a horizontal strip of cards (speaker, number, text, a mark when checked, to fix or too long), the current one centred. A click opens the line. */
const props = defineProps({ detail: { type: Object, required: true }, radius: { type: Number, default: 6 } });
const emit = defineEmits(['goto']);
const items = ref(peekContext(props.detail.file, props.detail.line, props.radius)?.items || null);
const error = ref('');
const strip = ref(null);

async function load() {
  const d = props.detail, peek = peekContext(d.file, d.line, props.radius);
  if (peek) { items.value = peek.items; } else {
    try { const r = await contextFor(d.file, d.line, props.radius); if (props.detail === d) items.value = r.items; } catch (e) { error.value = e.message; }
  }
  await nextTick(); center();
}
function center() {
  const el = strip.value, cur = el && el.querySelector('.cur');
  if (el && cur) el.scrollLeft = cur.offsetLeft - (el.clientWidth - cur.offsetWidth) / 2;
}
onMounted(() => { load(); requestAnimationFrame(center); });
watch(() => props.detail, load);
</script>

<template>
  <div ref="strip" class="filmstrip" role="list" :aria-label="t('Lines of the scene')">
    <div v-if="error" class="notice danger">{{ error }}</div>
    <div v-else-if="!items" class="muted small">{{ t('Loading…') }}</div>
    <button v-for="it in items" :key="it.line" class="fcard" :class="{ cur: it.current, over: it.over, todo: it.status === 'todo' }" role="listitem" :title="t('Line {n}', { n: it.line })" @click="emit('goto', { file: detail.file, line: it.line })">
      <span class="fhead">
        <span v-if="it.who" class="sbadge" :style="{ background: speakerColor(it.who) }" :title="speakerLabel(it.who)">{{ speakerInitial(it.who) }}</span>
        <span class="fnum">{{ it.line }}</span>
        <span v-if="it.over" class="fmark over" :title="t('The text does not fit the game window')">⤓</span>
        <span v-if="it.proof === 'ok'" class="fmark ok" :title="t('Proofread')">✓</span><span v-else-if="it.proof === 'fix'" class="fmark fix" :title="t('To fix')">⚑</span>
      </span>
      <span class="ftxt" :class="{ muted: it.status === 'todo' }">{{ it.status === 'todo' ? (it.source ?? '') : (it.target ?? '') }}</span>
    </button>
  </div>
</template>

<style scoped>
.filmstrip { display: flex; gap: var(--s-2); overflow-x: auto; padding: var(--s-1) 2px var(--s-2); }
.fcard { flex: 0 0 190px; text-align: left; border: 1px solid var(--line); background: var(--bg-card); color: var(--text); border-radius: var(--r-lg); padding: var(--s-2) var(--s-3); cursor: pointer; font: inherit; display: flex; flex-direction: column; gap: var(--s-1); transition: border-color var(--t-fast), background var(--t-fast); }
.fcard:hover { border-color: var(--line-accent); }
.fcard.cur { border-color: var(--accent-hi); background: var(--accent-soft); }
.fcard.todo { border-style: dashed; }
.fcard.over { border-color: var(--danger); background: var(--danger-bg); }
.fhead { display: flex; align-items: center; gap: var(--s-1); }
.sbadge { width: 20px; height: 20px; border-radius: 50%; display: inline-flex; align-items: center; justify-content: center; font-size: 11px; font-weight: 500; color: #fff; flex: none; }
.fnum { color: var(--text-muted); font-size: var(--fs-small); font-family: var(--mono); }
.fmark { margin-left: auto; font-size: var(--fs-small); font-weight: 500; } .fmark + .fmark { margin-left: var(--s-1); }
.fmark.ok { color: var(--checked); } .fmark.fix { color: var(--todo); } .fmark.over { color: var(--danger); }
.ftxt { font-size: var(--fs-body); line-height: var(--lh-tight); max-height: 4.1em; overflow: hidden; word-break: break-word; }
.small { font-size: var(--fs-small); }
</style>
