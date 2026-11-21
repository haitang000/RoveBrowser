<script setup lang="ts">
import Icon from "../common/Icon.vue";

defineProps<{ src?: string }>();
</script>

<template>
  <span class="extension-icon" aria-hidden="true">
    <img v-if="src" :src="src" alt="" width="32" height="32" draggable="false" />
    <Icon v-else name="extension" :size="17" />
  </span>
</template>

<style scoped>
.extension-icon {
  display: grid;
  flex: 0 0 32px;
  width: 32px;
  height: 32px;
  place-items: center;
  color: var(--accent);
}
.extension-icon:has(svg) { background: var(--chosen); border-radius: 7px; }
.extension-icon img { object-fit: contain; }
</style>
