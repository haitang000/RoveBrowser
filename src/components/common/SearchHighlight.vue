<script setup lang="ts">
import { computed } from "vue";
import { settingsSearchParts } from "../../lib/settings-search";

const props = defineProps<{ text: string; words: readonly string[] }>();
const parts = computed(() => settingsSearchParts(props.text, props.words));
</script>

<template>
  <template v-for="(part, index) in parts" :key="index"><mark v-if="part.match" class="settings-search-match">{{ part.text }}</mark><template v-else>{{ part.text }}</template></template>
</template>
