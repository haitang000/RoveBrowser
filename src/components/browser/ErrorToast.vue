<script setup lang="ts">
import { useBrowser } from "../../stores/browser";
const b = useBrowser();
</script>
<template>
  <div v-if="b.error && !b.commandOpen" class="error-toast" role="alert">
    {{ b.error }}<button aria-label="关闭提示" @click="b.error = ''">×</button>
  </div>
</template>
