<script setup>
import { onMounted, ref } from 'vue';
import { post } from '../api.js';
import { t } from '../i18n.js';
import Dialog from './Dialog.vue';

/* The profile of a character (a Pro feature): the gender, the way the character speaks and the form in which they address the other characters. The batch machine translation
   sends it to the language model with every line of that speaker. It is kept in the note of the glossary entry (the glossary page does not show it). */
const props = defineProps({ code: { type: String, required: true }, name: { type: String, default: '' } });
const emit = defineEmits(['close', 'saved']);
const loaded = ref(false);
const error = ref('');
const gender = ref('');
const style = ref('');
const others = ref([]);
const forms = ref({});
const busy = ref(false);

onMounted(async () => {
  try {
    const list = await post('profile/list', {});
    const me = list.items.find((i) => i.code === props.code) || { profile: { gender: '', style: '', to: {} } };
    gender.value = me.profile.gender || ''; style.value = me.profile.style || '';
    others.value = list.items.filter((i) => i.code !== props.code && (i.in_project || me.profile.to[i.code])).slice(0, 40);
    forms.value = { ...(me.profile.to || {}) };
    loaded.value = true;
  } catch (e) { error.value = e.message; }
});
async function save() {
  busy.value = true; error.value = '';
  const to = {};
  for (const [c, v] of Object.entries(forms.value)) if (v) to[c] = v;
  try { await post('profile/set', { code: props.code, gender: gender.value, style: style.value.trim(), to }); emit('saved'); } catch (e) { error.value = e.message; busy.value = false; }
}
</script>

<template>
  <Dialog :title="t('Profile of {name}', { name: name || code })" width="620px" @close="emit('close')">
    <p class="muted">{{ t('The batch machine translation tells the language model who speaks: it keeps the feminine and masculine forms right and the form of address (tykání / vykání) the same. Only a language model (OpenAI-compatible service) can use it; DeepL and LibreTranslate cannot.') }}</p>
    <div v-if="error" class="notice danger" role="alert">{{ error }}</div>
    <div v-if="!loaded && !error" class="skeleton" style="height: 80px"></div>
    <template v-if="loaded">
      <label class="field-row">{{ t('Gender') }}
        <select v-model="gender" class="sel"><option value="">{{ t('not set') }}</option><option value="f">{{ t('female') }}</option><option value="m">{{ t('male') }}</option><option value="n">{{ t('neither') }}</option></select></label>
      <label class="field-row">{{ t('How the character speaks') }}
        <input v-model="style" class="text" type="text" maxlength="200" :placeholder="t('e.g. sarcastic, short sentences, speaks slowly')"></label>
      <template v-if="others.length">
        <b>{{ t('Addresses the others') }}</b>
        <div class="wrap"><table>
          <thead><tr><th>{{ t('Code') }}</th><th>{{ t('Name') }}</th><th>{{ t('Form') }}</th></tr></thead>
          <tbody><tr v-for="o in others" :key="o.code"><td class="mono">{{ o.code }}</td><td>{{ o.name || '' }}</td>
            <td><select v-model="forms[o.code]" class="sel"><option value="">{{ t('not set') }}</option><option value="informal">{{ t('informal (tykání)') }}</option><option value="formal">{{ t('formal (vykání)') }}</option></select></td></tr></tbody></table></div>
      </template>
      <p v-else class="muted">{{ t('There are no other speakers in the project yet.') }}</p>
    </template>
    <template #footer>
      <button class="btn" @click="emit('close')">{{ t('Cancel') }}</button>
      <button class="btn primary" :disabled="!loaded || busy" @click="save">{{ t('Save') }}</button>
    </template>
  </Dialog>
</template>

<style scoped>
.field-row { display: flex; align-items: center; gap: var(--s-3); margin: var(--s-2) 0; font-size: var(--fs-body); }
.sel { height: var(--h-control-sm); background: var(--bg-input); color: var(--text); border: 1px solid var(--line); border-radius: var(--r-md); padding: 0 var(--s-2); }
.text { flex: 1; height: var(--h-control); padding: 0 var(--s-3); background: var(--bg-input); color: var(--text); border: 1px solid var(--line); border-radius: var(--r-md); font: inherit; }
.wrap { max-height: 36vh; overflow: auto; border: 1px solid var(--line); border-radius: var(--r-md); margin-top: var(--s-2); }
table { width: 100%; border-collapse: collapse; font-size: var(--fs-body); }
th { position: sticky; top: 0; background: var(--bg-bar); text-align: left; font-size: var(--fs-caption); font-weight: 500; color: var(--text-muted); padding: var(--s-2) var(--s-3); }
td { padding: 4px var(--s-3); border-top: 1px solid var(--line-soft); }
</style>
