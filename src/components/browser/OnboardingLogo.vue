<script setup lang="ts">
import { computed, ref, watch } from "vue";
const props = defineProps<{ name: string; glyph?: string; color?: string }>();
const assets = import.meta.glob<string>("../../assets/onboarding/*.{png,svg}", { eager: true, query: "?url", import: "default" });
const logo = computed(() => assets[`../../assets/onboarding/${props.name}.svg`] || assets[`../../assets/onboarding/${props.name}.png`]);
const failed = ref(false);
watch(() => props.name, () => { failed.value = false; });
</script>
<template>
  <span v-if="name.toLowerCase().includes('chrome')" class="onboarding-logo logo-chrome" aria-hidden="true" />
  <img v-else-if="logo && !failed" class="onboarding-logo" :src="logo" alt="" draggable="false" @error="failed = true" />
  <span v-else class="onboarding-logo logo-fallback" :style="{ color: color || '#008fb3' }" aria-hidden="true">{{ glyph || name.charAt(0).toUpperCase() }}</span>
</template>
