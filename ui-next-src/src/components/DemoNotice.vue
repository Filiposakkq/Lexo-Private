<script setup>
import { t } from '../i18n.js';

/* The one notice of every Pro dialog when the demo cut the work to its limit ({feature, limit, total} from the pack). */
defineProps({ limited: { type: Object, default: null } });
</script>

<template>
  <div v-if="limited" class="notice warn">{{ limited.feature === 'bulk.categorize'
    ? t('The demo handles only the first {limit} files ({total} found). A licence removes the limit.', { limit: limited.limit, total: limited.total })
    : t('The demo handles only the first {limit} lines ({total} found). A licence removes the limit.', { limit: limited.limit, total: limited.total }) }}</div>
</template>
