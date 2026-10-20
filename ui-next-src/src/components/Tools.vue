<script setup>
import { computed, ref } from 'vue';
import { t } from '../i18n.js';
import { requireFeature } from '../pro.js';
import { project } from '../project.js';
import CarryDialog from './CarryDialog.vue';
import FixDialog from './FixDialog.vue';
import MissedTextsDialog from './MissedTextsDialog.vue';
import MtDialog from './MtDialog.vue';
import ReleaseDialog from './ReleaseDialog.vue';
import ReplaceDialog from './ReplaceDialog.vue';
import RuleApproveDialog from './RuleApproveDialog.vue';
import SpellDialog from './SpellDialog.vue';
import StyleDialog from './StyleDialog.vue';

/* The Tools menu of the lines: bulk edits of the translation. Every one opens a dialog with a preview; nothing is written before you confirm, and a change can be undone. */
const props = defineProps({ picked: { type: Array, default: () => [] }, anchor: { type: Object, default: null }, file: { type: String, default: '' }, mode: { type: String, default: 'menu' } });
const emit = defineEmits(['changed']);
const open = ref(false);
const tool = ref('');

const scoped = computed(() => (props.picked.length ? [...new Set(props.picked.map((p) => p.file))] : props.file ? [props.file] : []));
const anchor = computed(() => (props.anchor ? { file: props.anchor.file, line: props.anchor.line } : null));
const ITEMS = computed(() => [
  { id: 'replace', label: t('Find and replace…'), hint: t('in the translations') },
  { id: 'rules', label: t('Approve by rule…'), hint: t('names, interjections') },
  { id: 'missed', label: t('Texts the generator missed…'), hint: t('show text, input prompts, titles') },
  { id: 'carry', label: t('Carry over from the previous version…'), hint: t('Pro: translation of the old game version'), feature: 'carry.over' },
  { id: 'similar', label: t('Fill from similar translations…'), hint: t('Pro: similar originals of this project'), feature: 'memory.fuzzy' },
  { id: 'fix', label: t('Fix…'), hint: t('speaker, spaces, typos'), feature: 'fixes.safe' },
  { id: 'mt', label: t('Translate with a machine…'), hint: t('selected lines, scene, file'), feature: 'mt.batch' },
  { id: 'style', label: t('Check the style…'), hint: t('Pro: address, gender, tone (language model)'), feature: 'style.check' },
  { id: 'spell', label: t('Check the spelling…'), hint: t('Pro: with a dictionary of the language'), feature: 'spell.hunspell' },
  { id: 'release', label: t('Build a release…'), hint: t('Pro: a zip for the players'), feature: 'release.build' },
]);
async function choose(item) {
  open.value = false;
  if (item.feature && !await requireFeature(item.feature)) return;
  tool.value = item.id;
}
function done() { tool.value = ''; emit('changed'); }
const toSettings = () => { location.href = '/'; };
</script>

<template>
  <div class="tools" :class="{ grid: mode === 'grid' }">
    <div v-if="mode === 'grid'" class="cards">
      <button v-for="item in ITEMS" :key="item.id" class="tool-card card" @click="choose(item)">
        <span class="tool-glyph" aria-hidden="true">⚒</span>
        <span class="tool-name">{{ item.label }}<span v-if="item.feature" class="pill none pro">Pro</span></span>
        <span class="muted small">{{ item.hint }}</span>
      </button>
    </div>
    <template v-else>
      <button class="btn tool-btn" :aria-expanded="open" aria-haspopup="menu" :title="t('Bulk edits of translations')" @click="open = !open" @blur="open = false"><span aria-hidden="true">⚒</span> {{ t('Tools') }} ▾</button>
      <div v-if="open" class="menu" role="menu" @mousedown.prevent>
        <div class="menu-head faint">{{ t('Bulk edits of translations') }}</div>
        <div v-for="item in ITEMS" :key="item.id" class="menu-item" role="menuitem" @click="choose(item)"><span>{{ item.label }}<span v-if="item.feature" class="pill none pro">Pro</span></span><span class="faint hint">{{ item.hint }}</span></div>
      </div>
    </template>

    <ReplaceDialog v-if="tool === 'replace'" :preselected="scoped" @close="tool = ''" @done="done" />
    <RuleApproveDialog v-if="tool === 'rules'" :preselected="scoped" @close="tool = ''" @done="done" />
    <MissedTextsDialog v-if="tool === 'missed'" @close="tool = ''" @done="done" />
    <CarryDialog v-if="tool === 'carry'" @close="tool = ''" @done="done" />
    <CarryDialog v-if="tool === 'similar'" own @close="tool = ''" @done="done" />
    <FixDialog v-if="tool === 'fix'" :file="file" @close="tool = ''" @done="done" />
    <MtDialog v-if="tool === 'mt'" :picked="picked" :anchor="anchor" :file="file" @close="tool = ''" @done="done" @settings="toSettings" />
    <StyleDialog v-if="tool === 'style'" :picked="picked" :anchor="anchor" :file="file" @close="tool = ''" @done="done" @settings="toSettings" />
    <SpellDialog v-if="tool === 'spell'" :file="file" @close="tool = ''" @done="done" />
    <ReleaseDialog v-if="tool === 'release'" @close="tool = ''" />
  </div>
</template>

<style scoped>
.tools { position: relative; }
.menu { position: absolute; z-index: 10; top: 100%; left: 0; margin-top: 4px; min-width: 380px; }
.menu-item { justify-content: space-between; gap: var(--s-4); cursor: pointer; }
.menu-head { padding: var(--s-2) var(--s-3) var(--s-1); font-size: var(--fs-caption); text-transform: uppercase; letter-spacing: .06em; }
.tool-btn { border-color: var(--line-accent); color: var(--accent-text); }
.tool-btn:hover { background: var(--accent-soft); }
.pro { margin-left: var(--s-2); }
.cards { display: grid; grid-template-columns: repeat(auto-fill, minmax(260px, 1fr)); gap: var(--s-3); }
.tool-card { all: unset; box-sizing: border-box; display: flex; flex-direction: column; gap: var(--s-1); padding: var(--s-4); cursor: pointer; background: var(--bg-card); border: 1px solid var(--line); border-radius: var(--r-lg); }
.tool-card:hover { border-color: var(--line-accent); background: var(--bg-hover); }
.tool-card:focus-visible { box-shadow: var(--focus); }
.tool-glyph { color: var(--accent-text); font-size: var(--fs-heading); }
.tool-name { font-size: var(--fs-text); font-weight: 500; }
.small { font-size: var(--fs-small); }
.hint { font-size: var(--fs-small); }
</style>
