import { createApp } from 'vue';
import App from './App.vue';
import { availableLanguages, chooseLanguage, loadLanguage, setLanguage } from './i18n.js';

/* The language first (the texts are English keys; a pack of the chosen language is loaded from the application), then the application. */
async function start() {
  try {
    const langs = await availableLanguages();
    let saved = '';
    try { saved = localStorage.getItem('aside.lang') || ''; } catch { /* no storage: the browser's language is used */ }
    setLanguage(await loadLanguage(chooseLanguage(saved, langs)));
  } catch {
    setLanguage(null);
  }
  createApp(App).mount('#app');
}
start();
