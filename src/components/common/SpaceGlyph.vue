<script setup lang="ts">
import { computed } from "vue";
import { cleanIcon } from "../../lib/space-icon";
const props = defineProps<{ icon: string; size?: number }>();
const emoji = computed(() => cleanIcon(props.icon));
const side = computed(() => props.size || 16);
</script>
<template>
  <span
    v-if="emoji"
    class="space-glyph space-emoji"
    :style="{ width: side + 'px', height: side + 'px', '--glyph': side + 'px' }"
    >{{ emoji }}</span
  >
</template>
