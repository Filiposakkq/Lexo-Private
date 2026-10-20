<script setup>
import { computed } from 'vue';
import { t } from '../i18n.js';
import { ACTIONS, keysOf } from '../keys.js';
import Dialog from './Dialog.vue';

/* The shortcuts, as they are now (with the changes the user made). */
const emit = defineEmits(['close', 'settings']);
const general = computed(() => ACTIONS.filter((a) => a.scope !== 'lines' && keysOf(a).length));
const lines = computed(() => ACTIONS.filter((a) => a.scope === 'lines'));
const FIXED = computed(() => [['Ctrl+Enter', t('In the translation field: save to the file and go to the next untranslated line')], ['Esc', t('In the translation field: discard changes; elsewhere it clears the field selection')]]);
</script>

<template>
  <Dialog :title="t('Keyboard shortcuts')" width="680px" @close="emit('close')">
    <p class="muted">{{ t('The shortcuts can be changed in {where}. Actions without a shortcut are available through the command palette {palette}.', { where: t('Settings → Keyboard shortcuts'), palette: 'Ctrl+K' }) }} <a href="#" @click.prevent="emit('settings')">{{ t('Settings') }}</a></p>
    <dl>
      <dt class="group">{{ t('General') }}</dt>
      <template v-for="a in general" :key="a.id"><dt><template v-for="(k, i) in keysOf(a)" :key="k"><span v-if="i"> {{ t(' or ') }} </span><kbd>{{ k }}</kbd></template></dt><dd>{{ a.label }}</dd></template>
      <dt class="group">{{ t('Lines') }}</dt>
      <template v-for="a in lines" :key="a.id"><dt><template v-if="keysOf(a).length"><template v-for="(k, i) in keysOf(a)" :key="k"><span v-if="i"> {{ t(' or ') }} </span><kbd>{{ k }}</kbd></template></template><span v-else class="muted">—</span></dt><dd>{{ a.label }}</dd></template>
      <dt class="group">{{ t('Fixed') }}</dt>
      <template v-for="[k, d] in FIXED" :key="k"><dt><kbd>{{ k }}</kbd></dt><dd>{{ d }}</dd></template>
    </dl>
    <template #footer><button class="btn primary" @click="emit('close')">{{ t('Close') }}</button></template>
  </Dialog>
</template>

<style scoped>
dl { display: grid; grid-template-columns: 210px 1fr; gap: var(--s-1) var(--s-3); margin: var(--s-2) 0 0; font-size: var(--fs-body); }
dd { margin: 0; }
.group { grid-column: 1 / -1; margin-top: var(--s-3); font-weight: 500; color: var(--text-2); }
kbd { display: inline-block; padding: 1px 6px; border: 1px solid var(--line); border-radius: var(--r-sm); background: var(--bg-input); font-family: var(--mono); font-size: var(--fs-small); }
</style>
