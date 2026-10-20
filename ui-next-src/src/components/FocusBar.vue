<script setup>
import { t } from '../i18n.js';
import { setFocus, ui } from '../layout.js';
import LayoutMenu from './LayoutMenu.vue';

/* The small floating bar of the focus mode: bring everything back, the layout, the quick mode (translating / proofreading) with what is left, full screen and undo. In the proofreading
   mode it also has the buttons of the check (✓ Proofread, ⚑ Needs a fix, the arrows). The screen that shows the lines does what the buttons ask through the handlers of the keyboard. */
const props = defineProps({ left: { type: Object, default: () => ({}) } });
const emit = defineEmits(['act']);
const quick = (k) => { if (ui.quick !== k) emit('act', k === 'proof' ? 'proofmode' : 'quick'); };
</script>

<template>
  <div class="focusbar" role="toolbar" :aria-label="t('Focus')">
    <button class="btn small" :title="t('Bring back the top bar and the tabs (Ctrl+Shift+F)')" @click="setFocus(false)">{{ t('⤢ Show everything') }}</button>
    <LayoutMenu @quick="emit('act', 'quick')" @window="emit('act', 'new_window')" />
    <div v-if="ui.quick" class="seg" role="group" :aria-label="t('Quick mode')">
      <button class="btn small" :class="{ primary: ui.quick === 'tr' }" :title="t('Only untranslated lines, the editor focuses itself')" @click="quick('tr')">{{ t('Translation') }}</button>
      <button class="btn small" :class="{ primary: ui.quick === 'proof' }" :title="t('Only lines to check, P marks one as checked and moves on')" @click="quick('proof')">{{ t('Proofreading') }}</button>
    </div>
    <button class="btn small icon" :title="t('Full screen (F11)')" @click="emit('act', 'fullscreen')">⛶</button>
    <button class="btn small icon" :title="t('Undo the last change (Ctrl+Z)')" @click="emit('act', 'undo')">↶</button>
    <template v-if="ui.quick === 'proof'">
      <button class="btn small primary" :title="t('Marks the line as checked and moves to the next one (P)')" @click="emit('act', 'proof')">{{ t('✓ Proofread') }}</button>
      <button class="btn small" :title="t('Flags the line for a fix (F)')" @click="emit('act', 'flag')">{{ t('⚑ Needs a fix') }}</button>
      <button class="btn small icon" :title="t('Previous line (←)')" @click="emit('act', 'prev')">←</button>
      <button class="btn small icon" :title="t('Next line (→)')" @click="emit('act', 'next')">→</button>
      <span class="muted">{{ t('Proofreading') }}<template v-if="left.review != null"> · {{ t('left to proofread: {n}', { n: left.review }) }}</template></span>
    </template>
    <span v-else-if="ui.quick" class="muted">{{ t('Quick translating') }}<template v-if="left.todo != null"> · {{ t('left to translate: {n}', { n: left.todo }) }}</template><template v-if="left.review != null"> · {{ t('to proofread: {n}', { n: left.review }) }}</template></span>
  </div>
</template>

<style scoped>
.focusbar { position: fixed; top: var(--s-2); right: var(--s-3); z-index: 15; display: flex; flex-wrap: wrap; gap: var(--s-2); align-items: center; padding: var(--s-2) var(--s-3); background: var(--bg-bar); border: 1px solid var(--line-accent); border-radius: var(--r-lg); box-shadow: var(--shadow-float); font-size: var(--fs-body); max-width: calc(100vw - 2 * var(--s-3)); }
.seg { display: flex; gap: var(--s-1); }
</style>
