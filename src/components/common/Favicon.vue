<script setup lang="ts">
import { ref, watch } from "vue";
import Icon from "./Icon.vue";
import { host } from "../../lib/address";
const props = defineProps<{ url: string; favicon?: string }>();
const failed = ref(false);
watch(
  () => [props.url, props.favicon],
  () => (failed.value = false),
);
</script>
<template>
  <img
    v-if="!failed && url.startsWith('http')"
    class="favicon"
    :src="favicon || `https://${host(url)}/favicon.ico`"
    alt=""
    referrerpolicy="no-referrer"
    @error="failed = true"
  /><Icon v-else name="globe" :size="16" />
</template>
