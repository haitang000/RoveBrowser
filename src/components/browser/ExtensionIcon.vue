<script setup lang="ts">
import Icon from "../common/Icon.vue";

withDefaults(defineProps<{ src?: string; size?: number }>(), { size: 32 });
</script>

<template>
  <span class="extension-icon" :style="{ '--extension-icon-size': size + 'px' }" aria-hidden="true">
    <img v-if="src" :src="src" alt="" :width="size" :height="size" draggable="false" />
    <Icon v-else name="extension" :size="size === 32 ? 17 : size" />
  </span>
</template>

<style scoped>
.extension-icon {
  display: grid;
  flex: 0 0 var(--extension-icon-size);
  width: var(--extension-icon-size);
  height: var(--extension-icon-size);
  place-items: center;
  color: var(--accent);
}
.extension-icon:has(svg) { background: var(--chosen); border-radius: 7px; }
.extension-icon img { object-fit: contain; }
</style>
