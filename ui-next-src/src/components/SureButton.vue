<script setup>
import { ref } from 'vue';
import { t } from '../i18n.js';

/* A button that asks "really?" first, in place (one question at a time, no dialog on top of a dialog). */
defineProps({ label: { type: String, required: true }, question: { type: String, required: true }, danger: { type: Boolean, default: false }, disabled: { type: Boolean, default: false } });
const emit = defineEmits(['yes']);
const asking = ref(false);
</script>

<template>
  <span class="sure">
    <button v-if="!asking" class="btn small" :class="danger ? 'danger' : 'ghost'" :disabled="disabled" @click="asking = true">{{ label }}</button>
    <template v-else><span class="muted">{{ question }}</span> <button class="btn small danger" @click="asking = false; emit('yes')">{{ t('Yes') }}</button> <button class="btn small" @click="asking = false">{{ t('No') }}</button></template>
  </span>
</template>

<style scoped>
.sure { display: inline-flex; gap: var(--s-2); align-items: center; font-size: var(--fs-small); }
</style>
