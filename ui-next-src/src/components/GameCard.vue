<script setup>
import { computed, ref } from 'vue';
import { locale, t, tn } from '../i18n.js';
import { ago } from '../state.js';

/* One game on Home: its numbers, its state, the main action and the menu (open, pin, remove from the list). */
const props = defineProps({ game: { type: Object, required: true }, latest: Boolean, busy: Boolean });
const emit = defineEmits(['open', 'pin', 'remove', 'prepare']);
const menu = ref(false);
const sure = ref(false);

const percent = computed(() => (props.game.total ? Math.round((props.game.done / props.game.total) * 100) : 0));
const when = computed(() => ago(props.game.opened, locale()));
const sub = computed(() => {
  const g = props.game;
  if (g.state === 'needs_prepare') return t('version {v} · no translation', { v: g.version || '–' });
  const language = g.language ? g.language.charAt(0).toUpperCase() + g.language.slice(1) : '–';
  return t('version {v} · {language}', { v: g.version || '–', language });
});
const close = () => { menu.value = false; sure.value = false; };
</script>

<template>
  <article class="card game" :class="{ active: latest, missing: game.state === 'missing' }" @mouseleave="close">
    <header>
      <span class="art" aria-hidden="true">{{ (game.name || '?').slice(0, 1).toUpperCase() }}</span>
      <span v-if="game.pinned" class="tag-pro mark">{{ t('Pinned') }}</span>
      <span v-else-if="latest" class="tag-pro mark">{{ t('Last used') }}</span>
      <button class="btn ghost sm more" :aria-label="t('Menu of the game')" @click.stop="menu = !menu">⋯</button>
      <div v-if="menu" class="menu" role="menu">
        <div class="menu-item" role="menuitem" @click="emit('open', game); close()">{{ t('Open') }}</div>
        <div class="menu-item" role="menuitem" @click="emit('pin', game); close()">{{ game.pinned ? t('Unpin') : t('Pin to the top') }}</div>
        <hr>
        <div v-if="!sure" class="menu-item danger" role="menuitem" @click.stop="sure = true">{{ t('Remove from the list') }}</div>
        <div v-else class="menu-item danger" role="menuitem" @click="emit('remove', game); close()">{{ t('Remove it (the files stay)') }}</div>
      </div>
    </header>
    <div class="body">
      <div class="heading">{{ game.name }}</div>
      <div class="muted small">{{ sub }}</div>

      <template v-if="game.state === 'ready' || game.state === 'choose'">
        <div class="bar" role="progressbar" :aria-valuenow="percent" aria-valuemin="0" aria-valuemax="100"><i :style="{ width: percent + '%' }"></i></div>
        <div class="nums muted small"><span>{{ percent }} %</span><span>{{ (game.done || 0).toLocaleString(locale()) }} / {{ (game.total || 0).toLocaleString(locale()) }}</span></div>
        <div class="faint small when" v-if="when">{{ when }}</div>
        <button v-if="game.state === 'ready'" class="btn go" :class="{ primary: latest }" :disabled="busy" @click="emit('open', game)">{{ t('Continue') }}</button>
        <div v-else class="muted small">{{ t('Several translations: open it to choose one.') }}</div>
        <button v-if="game.state === 'choose'" class="btn go" :disabled="busy" @click="emit('open', game)">{{ t('Open') }}</button>
      </template>
      <template v-else-if="game.state === 'needs_prepare'">
        <div class="warn small"><span class="pill todo">{{ t('Not prepared yet') }}</span></div>
        <button class="btn go" :disabled="busy" @click="emit('prepare', game)">{{ t('Prepare the game') }}</button>
      </template>
      <template v-else>
        <div class="warn small"><span class="pill danger">{{ t('The folder is gone') }}</span></div>
        <button class="btn go" @click="emit('remove', game)">{{ t('Remove from the list') }}</button>
      </template>
    </div>
  </article>
</template>

<style scoped>
.game { padding: 0; position: relative; display: flex; flex-direction: column; min-width: 0; }
header { position: relative; height: 64px; background: var(--bg-hover); border-radius: var(--r-lg) var(--r-lg) 0 0; display: flex; align-items: center; justify-content: center; }
.game.active header { background: var(--accent-soft); }
.art { font-size: 28px; font-weight: 500; color: var(--text-faint); }
.game.active .art { color: var(--accent-text); }
.mark { position: absolute; left: var(--s-2); top: var(--s-2); }
.more { position: absolute; right: var(--s-2); top: var(--s-2); font-size: 18px; line-height: 1; padding: 0 var(--s-2); }
.menu { position: absolute; right: var(--s-2); top: 36px; z-index: 5; }
.body { padding: var(--s-3) var(--s-4) var(--s-4); display: flex; flex-direction: column; gap: var(--s-2); }
.small { font-size: var(--fs-small); }
.nums { display: flex; justify-content: space-between; }
.go { margin-top: var(--s-2); }
.warn { margin: var(--s-1) 0; }
.missing { opacity: .8; }
.when { margin-top: -2px; }
</style>
