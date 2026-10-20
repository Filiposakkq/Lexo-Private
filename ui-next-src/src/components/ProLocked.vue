<script setup>
import { t } from '../i18n.js';
import { featureTitle, pro } from '../pro.js';
import Dialog from './Dialog.vue';

/* "This is a Pro feature": shown by requireFeature() when the pack does not offer a feature. */
</script>

<template>
  <Dialog v-if="pro.locked" :title="t('Pro feature')" width="440px" @close="pro.locked = null">
    <p>{{ t('“{title}” is a Pro feature.', { title: featureTitle(pro.locked.id) }) }}</p>
    <p class="muted">{{ t('It is not available in this installation. The rest of the application works without it.') }}</p>
    <div v-if="pro.locked.problems.length" class="notice warn">{{ pro.locked.problems.map(String).join(' ') }}</div>
    <template #footer><button class="btn primary" @click="pro.locked = null">{{ t('Close') }}</button></template>
  </Dialog>
</template>
