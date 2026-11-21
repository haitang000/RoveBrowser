<script setup lang="ts">
import { useBrowser } from "../../stores/browser";
import Icon from "../common/Icon.vue";

const b = useBrowser();
</script>
<template>
  <Transition name="toast">
    <div
      v-if="b.zoomNotice"
      class="toast status-toast"
      :data-theme="b.state.theme"
      role="status"
    >
      <div class="toast-surface">
        <Icon name="search" :size="15" />
        <span>{{ Math.round(b.zoomNotice.factor * 100) }}%</span>
      </div>
    </div>
  </Transition>
</template>
