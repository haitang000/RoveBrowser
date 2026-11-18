<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import type { DownloadItem } from "../../lib/types";
import Icon from "../common/Icon.vue";

const props = defineProps<{ item: DownloadItem }>();
const emit = defineEmits<{ dismiss: []; cancelDownload: [id: string] }>();
const dialog = ref<HTMLDialogElement>();
const blocked = computed(() => props.item.state === "blocked");
const reason = computed(() => {
  if (!blocked.value) return "文件已接收完毕，浏览器还在确认能否保存此文件。";
  return props.item.failure ? `${props.item.failure}。` : "浏览器已阻止下载此文件。";
});

function cancelDownload() {
  emit("dismiss");
  emit("cancelDownload", props.item.id);
}

onMounted(() => dialog.value?.showModal());
onBeforeUnmount(() => dialog.value?.close());
</script>

<template>
  <div class="download-security-scrim">
    <dialog ref="dialog" class="download-security-dialog" :class="{ blocked }" :role="blocked ? 'alertdialog' : 'dialog'"
      aria-modal="true" aria-labelledby="download-security-title" aria-describedby="download-security-file download-security-reason"
      @cancel.prevent="emit('dismiss')" @keydown.stop>
      <div class="security-heading">
        <span class="security-icon" aria-hidden="true"><Icon name="shield" :size="20" /></span>
        <h2 id="download-security-title">{{ blocked ? '下载已被拦截' : '正在等待下载结果' }}</h2>
      </div>
      <bdi id="download-security-file">{{ item.filename }}</bdi>
      <p id="download-security-reason">{{ reason }}</p>
      <div class="security-actions">
        <button v-if="!blocked" @click="cancelDownload">取消下载</button>
        <button class="security-primary" autofocus @click="emit('dismiss')">{{ blocked ? '知道了' : '继续等待' }}</button>
      </div>
    </dialog>
  </div>
</template>

<style scoped>
.download-security-scrim {
  position: fixed;
  inset: 0;
  z-index: 900;
  background: rgb(0 0 0 / 0.32);
}
.download-security-dialog {
  box-sizing: border-box;
  width: min(420px, calc(100vw - 32px));
  max-height: calc(100vh - 32px);
  margin: auto;
  padding: 24px;
  overflow: auto;
  border: 1px solid var(--panel-edge);
  border-radius: 16px;
  background: var(--panel);
  color: var(--ink);
  box-shadow: 0 24px 64px var(--panel-shadow), 0 4px 12px var(--shadow);
}
.download-security-dialog::backdrop { background: transparent; }
.security-heading { display: flex; align-items: center; gap: 12px; }
.security-icon {
  display: grid;
  flex: 0 0 36px;
  height: 36px;
  place-items: center;
  border-radius: 10px;
  background: var(--chosen);
  color: var(--accent);
}
.blocked .security-icon {
  color: light-dark(#a63330, #ff9c92);
  background: color-mix(in srgb, currentColor 14%, transparent);
}
.security-heading h2 { margin: 0; font-size: 19px; line-height: 1.35; }
bdi { display: block; margin-top: 16px; font-size: 14px; font-weight: 600; overflow-wrap: anywhere; }
p { margin: 6px 0 0; color: var(--quiet); font-size: 13px; line-height: 1.6; }
.security-actions { display: flex; flex-wrap: wrap; justify-content: flex-end; gap: 8px; margin-top: 20px; }
.security-actions button {
  min-height: 36px;
  padding: 6px 14px;
  border-radius: 8px;
  background: var(--separator);
  font-size: 13px;
}
.security-actions .security-primary { background: var(--chosen); }
.security-actions button:hover { background: color-mix(in srgb, var(--ink) 12%, var(--panel)); }
.security-actions .security-primary:hover { background: color-mix(in srgb, var(--accent) 25%, var(--panel)); }
.security-actions button:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
</style>
