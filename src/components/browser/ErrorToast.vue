<script setup lang="ts">
import { useBrowser } from "../../stores/browser";
import Icon from "../common/Icon.vue";

const b = useBrowser();
</script>
<template>
  <Transition name="toast">
    <div
      v-if="b.error && !b.commandOpen"
      class="toast error-toast"
      :data-theme="b.state.theme"
      role="alert"
    >
      <div class="toast-surface">
        <Icon name="warning" :size="16" />
        <span class="toast-message">{{ b.error }}</span>
        <button class="toast-button icon-button" type="button" aria-label="关闭提示" @click="b.error = ''">
          <Icon name="close" :size="13" />
        </button>
      </div>
    </div>
  </Transition>
</template>
