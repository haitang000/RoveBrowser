<script setup lang="ts">
import { useBrowser } from "../../stores/browser";
import Icon from "../common/Icon.vue";

const b = useBrowser();
function undo() {
  b.undoBatch();
  b.dismissToast();
}
</script>
<template>
  <Transition name="toast">
    <div
      v-if="b.actionToast"
      class="toast action-toast"
      :data-theme="b.state.theme"
      role="status"
    >
      <div class="toast-surface">
        <Icon :name="b.actionToast.action === 'undoBatch' ? 'check' : 'bell'" :size="16" />
        <span class="toast-message">{{ b.actionToast.message }}</span>
        <button
          v-if="b.actionToast.action === 'undoBatch'"
          class="toast-button action-toast-undo"
          type="button"
          @click="undo"
        >
          撤销
        </button>
      </div>
    </div>
  </Transition>
</template>
