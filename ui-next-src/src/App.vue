<script setup>
import { computed, onMounted, ref } from 'vue';
import ProLocked from './components/ProLocked.vue';
import Splash from './components/Splash.vue';
import Home from './views/Home.vue';
import Prepare from './views/Prepare.vue';
import Overview from './views/Overview.vue';
import Files from './views/Files.vue';
import Lines from './views/Lines.vue';
import Check from './views/Check.vue';
import Glossary from './views/Glossary.vue';
import Team from './views/Team.vue';
import ToolsScreen from './views/ToolsScreen.vue';
import { t } from './i18n.js';
import { app, loadApp, loadGames } from './state.js';
import { loadOverview, loadTeam, project } from './project.js';

/* The shell: the icon rail, the top bar with the name (Aside, or Aside Pro when Pro is on) and the screen. Home and the first screens of a game (Overview, Files, Lines) are in the new interface;
   the other screens of a game (Check, Glossary, Tools, Team, Release, Settings) are still the classic ones and are linked from the menu of the game. */
const splash = ref(true);
const view = ref('home');          // home | prepare | overview | files | lines
const prepPath = ref('');
function prepare(path) { prepPath.value = path || ''; view.value = 'prepare'; }
function home() { view.value = 'home'; loadGames(); }
const PROJECT_VIEWS = ['overview', 'files', 'lines', 'check', 'glossary', 'tools', 'team'];
const inProject = computed(() => PROJECT_VIEWS.includes(view.value));
async function opened() { view.value = 'overview'; loadTeam(); try { await loadOverview(); } catch { /* the screen shows the error itself */ } }
const projectName = computed(() => project.overview?.project?.name || project.overview?.project?.dir || '');
const MENU = [['overview', () => t('Overview'), '◔'], ['files', () => t('Files'), '▤'], ['lines', () => t('Lines'), '≡'], ['check', () => t('Check'), '✓'], ['glossary', () => t('Glossary'), 'Aa'], ['tools', () => t('Tools'), '⚒'], ['team', () => t('Team'), '☺']];
const CLASSIC = [() => t('Publishing'), () => t('Settings')];
let seen = false;
try { seen = sessionStorage.getItem('aside.splash') === '1'; } catch { /* no storage: the animation plays */ }
if (seen) splash.value = false;
function splashDone() {
  splash.value = false;
  try { sessionStorage.setItem('aside.splash', '1'); } catch { /* nothing to remember */ }
}
onMounted(loadApp);
</script>

<template>
  <Splash v-if="splash" @done="splashDone" />
  <ProLocked />
  <div class="shell">
    <nav class="rail" :aria-label="t('Main menu')">
      <a class="rail-item" :class="{ on: view === 'home' }" href="#" :title="t('Home')" :aria-current="view === 'home' ? 'page' : null" @click.prevent="home">⌂</a>
      <a class="rail-item" :class="{ on: view === 'prepare' }" href="#" :title="t('Prepare a game')" :aria-current="view === 'prepare' ? 'page' : null" @click.prevent="prepare()">⚒</a>
      <a class="rail-item" href="/" :title="t('The rest of the application is in the classic interface for now')">☰</a>
      <span class="grow"></span>
      <a class="rail-item" href="/" :title="t('Settings (in the classic interface for now)')">⚙</a>
    </nav>
    <div class="main">
      <header class="topbar">
        <span class="logo">A</span>
        <span class="name">Aside<span v-if="app.pro"> <b class="pro">Pro</b></span></span>
        <span class="ver faint" v-if="app.version">{{ app.version }}</span>
        <span class="grow"></span>
        <span v-if="app.pro" class="tag-pro">{{ t('Pro is active') }}</span>
      </header>
      <div class="body">
        <nav v-if="inProject" class="nav" :aria-label="t('Game menu')">
          <div class="game" :title="projectName">{{ projectName }}</div>
          <a v-for="[k, label, glyph] in MENU" :key="k" class="nav-item" :class="{ on: view === k }" href="#" :aria-current="view === k ? 'page' : null" @click.prevent="view = k"><span aria-hidden="true">{{ glyph }}</span>{{ label() }}</a>
          <hr class="sep">
          <a v-for="(label, i) in CLASSIC" :key="i" class="nav-item" href="/" :title="t('This screen is in the classic interface for now')">{{ label() }}</a>
        </nav>
        <div class="screen-host">
          <Home v-if="view === 'home'" @prepare="prepare" @opened="opened" />
          <Prepare v-else-if="view === 'prepare'" :path="prepPath" @back="home" @done="loadGames" @opened="opened" />
          <Overview v-else-if="view === 'overview'" @go="view = $event" />
          <Files v-else-if="view === 'files'" @go="view = $event" />
          <Lines v-else-if="view === 'lines'" @go="view = $event" />
          <Check v-else-if="view === 'check'" @go="view = $event" />
          <Glossary v-else-if="view === 'glossary'" @go="view = $event" />
          <ToolsScreen v-else-if="view === 'tools'" />
          <Team v-else-if="view === 'team'" />
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.shell { display: flex; min-height: 100vh; }
.main { flex: 1; min-width: 0; display: flex; flex-direction: column; }
.body { flex: 1; display: flex; min-height: 0; }
.screen-host { flex: 1; min-width: 0; display: flex; }
.screen-host > :deep(*) { flex: 1; min-width: 0; }
.nav-item { text-decoration: none; }
.nav { flex: none; min-height: calc(100vh - 52px); }
.game { padding: var(--s-2) var(--s-3) var(--s-3); font-weight: 500; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.rail { position: sticky; top: 0; height: 100vh; }
.rail-item { text-decoration: none; font-size: 20px; }
.grow { flex: 1; }
.name { font-size: var(--fs-title); font-weight: 500; }
.pro { font-weight: 500; color: var(--accent-text); }
.ver { font-size: var(--fs-small); margin-top: 4px; }
</style>
