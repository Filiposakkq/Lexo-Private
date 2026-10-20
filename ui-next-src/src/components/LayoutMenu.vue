<script setup>
import { ref } from 'vue';
import { t } from '../i18n.js';
import { setLayout, ui } from '../layout.js';

/* The layout of the list of lines and the detail: automatic, detail below, detail on the right, scene and film strip (large picture, editor, strip of lines), and the quick mode
   (translating or proofreading with no distractions). */
const emit = defineEmits(['quick', 'window']);
const open = ref(false);
const ITEMS = () => [['auto', t('Automatic'), t('by window width')], ['rows', t('Detail below'), ''], ['cols', t('Detail on the right'), ''], ['film', t('Scene and filmstrip'), t('large picture, editor, strip of lines')]];
function pick(k) { setLayout(k); open.value = false; }
function quick() { open.value = false; emit('quick', 'tr'); }
function second() { open.value = false; emit('window'); }
</script>

<template>
  <div class="wrap">
    <button class="btn" :aria-expanded="open" aria-haspopup="menu" :title="t('Layout of the list and the detail')" @click="open = !open" @blur="open = false">{{ t('Layout') }} ▾</button>
    <div v-if="open" class="menu" role="menu" @mousedown.prevent>
      <div v-for="[k, label, hint] in ITEMS()" :key="k" class="menu-item" role="menuitem" @click="pick(k)"><span>{{ (ui.layout === k ? '✓ ' : '') + label }}</span><span class="faint">{{ hint }}</span></div>
      <div class="menu-item" role="menuitem" @click="second"><span>{{ t('Two monitors') }}</span><span class="faint">{{ t('detail in a second window') }}</span></div>
      <div class="menu-item" role="menuitem" @click="quick"><span>{{ t('Quick mode') }}</span><span class="faint">{{ t('translating or proofreading: scene + editor, no distractions') }}</span></div>
    </div>
  </div>
</template>

<style scoped>
.wrap { position: relative; }
.menu { position: absolute; z-index: 6; top: 100%; right: 0; margin-top: 4px; min-width: 300px; }
.menu-item { justify-content: space-between; gap: var(--s-4); cursor: pointer; }
.faint { font-size: var(--fs-small); }
</style>
