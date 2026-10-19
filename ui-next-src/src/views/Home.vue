<script setup>
import { computed, ref } from 'vue';
import AddGameDialog from '../components/AddGameDialog.vue';
import FirstRun from '../components/FirstRun.vue';
import GameCard from '../components/GameCard.vue';
import { post } from '../api.js';
import { t, tn } from '../i18n.js';
import { app, loadApp, loadGames } from '../state.js';

const emit = defineEmits(['prepare']);

/* Home: the library of games, what needs attention (the licence), and the way into a game. The screens of a game are still the classic ones, so opening a game continues there. */
const query = ref('');
const sort = ref('recent');
const adding = ref(false);
const busy = ref(false);
const message = ref('');

const games = computed(() => {
  const q = query.value.trim().toLowerCase();
  const list = (app.games || []).filter((g) => !q || g.name.toLowerCase().includes(q));
  const by = {
    recent: () => 0,
    name: (a, b) => a.name.localeCompare(b.name),
    progress: (a, b) => (b.total ? b.done / b.total : 0) - (a.total ? a.done / a.total : 0),
  }[sort.value];
  return [...list].sort((a, b) => (b.pinned ? 1 : 0) - (a.pinned ? 1 : 0) || by(a, b));
});
const latestPath = computed(() => {
  const recent = (app.games || []).filter((g) => g.opened && g.state === 'ready').sort((a, b) => b.opened.localeCompare(a.opened))[0];
  return recent ? recent.path : '';
});

/* what needs attention: the licence */
const alerts = computed(() => {
  const l = app.licence || {};
  const out = [];
  if (l.state === 'grace') out.push({ key: 'grace', text: tn(l.days_left, 'The check with the licence server is overdue. Pro keeps working for {n} more day.', 'The check with the licence server is overdue. Pro keeps working for {n} more days.'), act: t('Check now') });
  else if (l.state === 'expired') out.push({ key: 'expired', text: t('The licence has expired because it could not be checked for too long. Pro works with its limits.'), act: t('Check now') });
  else if (l.state === 'refused') out.push({ key: 'refused', text: t('The licence file cannot be used.'), act: t('Check now') });
  else if (l.state === 'licensed' && l.days_to_check <= 3) out.push({ key: 'due', text: tn(Math.max(l.days_to_check, 0), 'The licence check with the server is due in {n} day. Connect to the internet.', 'The licence check with the server is due in {n} days. Connect to the internet.'), act: t('Check now') });
  return out;
});

async function checkLicence() {
  try { await post('licence/refresh', {}); } catch (e) { message.value = e.message; }
  await loadApp();
}
async function open(game) {
  busy.value = true; message.value = '';
  try {
    await post('project/set', { path: game.path });
    location.href = '/';           // the screens of a game are the classic ones for now
  } catch (e) { message.value = e.message; busy.value = false; }
}
async function pin(game) { try { await post('library/pin', { path: game.path, pinned: !game.pinned }); } catch (e) { message.value = e.message; } await loadGames(); }
async function remove(game) { try { await post('library/remove', { path: game.path }); } catch (e) { message.value = e.message; } await loadGames(); }
function prepare(game) { emit('prepare', game.path); }
async function added() { adding.value = false; await loadGames(); }
</script>

<template>
  <main class="home">
    <template v-if="app.games === null"><div class="skeleton" style="height: 120px; max-width: 520px"></div></template>
    <FirstRun v-else-if="!app.games.length" @add="adding = true" />
    <template v-else>
      <div class="alerts" v-if="alerts.length || message">
        <div v-for="a in alerts" :key="a.key" class="notice warn" role="status">{{ a.text }}<span class="act" role="button" tabindex="0" @click="checkLicence" @keydown.enter="checkLicence">{{ a.act }}</span></div>
        <div v-if="message" class="notice" role="status">{{ message }}<span class="act" role="button" tabindex="0" @click="message = ''" @keydown.enter="message = ''">{{ t('Close') }}</span></div>
      </div>
      <div class="bar-top">
        <h2 class="heading">{{ t('Your games') }}</h2>
        <label class="field sort">
          <select v-model="sort" :aria-label="t('Sort the games')">
            <option value="recent">{{ t('Sort: last used') }}</option>
            <option value="name">{{ t('Sort: name') }}</option>
            <option value="progress">{{ t('Sort: progress') }}</option>
          </select>
        </label>
        <span class="grow"></span>
        <label class="field search"><input v-model="query" type="search" :placeholder="t('Search for a game…')" :aria-label="t('Search for a game…')"></label>
        <button class="btn primary" @click="adding = true">{{ t('Open a game folder') }}</button>
      </div>
      <div class="grid">
        <GameCard v-for="g in games" :key="g.path" :game="g" :latest="g.path === latestPath" :busy="busy" @open="open" @pin="pin" @remove="remove" @prepare="prepare" />
        <button class="card add" @click="adding = true"><span class="plus">+</span><span class="heading">{{ t('Add a game') }}</span><span class="muted">{{ t('Choose the folder of a game.') }}</span></button>
      </div>
      <p v-if="!games.length" class="muted">{{ t('No game matches the search.') }}</p>
    </template>
    <AddGameDialog v-if="adding" @added="added" @close="adding = false" />
  </main>
</template>

<style scoped>
.home { padding: var(--s-5) var(--s-6) var(--s-8); max-width: 1100px; margin: 0 auto; width: 100%; }
.alerts { display: flex; flex-direction: column; gap: var(--s-2); margin-bottom: var(--s-4); }
.bar-top { display: flex; align-items: center; gap: var(--s-3); margin-bottom: var(--s-4); flex-wrap: wrap; }
.grow { flex: 1; }
.sort { height: 30px; width: auto; }
.search { min-width: 190px; }
.grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(220px, 1fr)); gap: var(--s-4); }
.add { border: 1.5px dashed var(--line-accent); background: transparent; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: var(--s-1); min-height: 190px; cursor: pointer; color: var(--text); text-align: center; }
.add:hover { background: var(--bg-card); }
.plus { font-size: 26px; color: var(--accent-hi); line-height: 1; }
</style>
