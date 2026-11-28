<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, watch } from "vue";
import type { Rect } from "../../lib/types";
import { useBrowser } from "../../stores/browser";
import ExtensionIcon from "./ExtensionIcon.vue";
import ExtensionProgress from "./ExtensionProgress.vue";
import ExtensionReviewDetails from "./ExtensionReviewDetails.vue";

const b = useBrowser();
const dialog = ref<HTMLDialogElement>();
const viewport = ref<HTMLElement>();
const content = ref<HTMLElement>();
const cancel = ref<HTMLButtonElement>();
const submitting = ref(false);
let resize: ResizeObserver | undefined;

function bounds(rect: Rect) {
  return { left: `${rect.x}px`, top: `${rect.y}px`, width: `${rect.width}px`, height: `${rect.height}px` };
}

function dismiss() {
  if (!submitting.value || b.extensions.error) b.send({ type: "extensionsDismissPopup" });
}
function confirm() {
  const { removal, review } = b.extensions;
  if (submitting.value || (!removal && !review)) return;
  submitting.value = true;
  if (removal) b.send({ type: "extensionsRemove", ticket: removal.ticket });
  else if (review) b.send({ type: "extensionsReview", ticket: review.ticket, install: true });
}

watch(() => b.extensions.error, error => {
  if (error) cancel.value?.focus({ preventScroll: true });
}, { flush: "post" });

onMounted(() => {
  dialog.value?.showModal();
  if (!content.value || !viewport.value) return;
  const body = content.value;
  const frame = viewport.value;
  const measure = () => frame.style.setProperty("--extension-dialog-height", `${body.getBoundingClientRect().height}px`);
  measure();
  resize = new ResizeObserver(measure);
  resize.observe(body);
});
onBeforeUnmount(() => {
  resize?.disconnect();
  dialog.value?.close();
});
</script>

<template>
  <div class="extension-dialog-scrim">
    <div v-for="({ rect, image }, index) in b.extensionBackground" :key="index" class="extension-dialog-backdrop"
      :style="bounds(rect)" aria-hidden="true">
      <img :src="image" alt="" draggable="false" />
    </div>
    <dialog ref="dialog" class="extension-dialog"
      :aria-label="b.t(b.extensions.removal ? '确认卸载扩展' : '确认安装扩展')"
      :aria-describedby="b.extensions.removal ? 'extension-removal-description' : undefined"
      @cancel.prevent="dismiss">
      <div ref="viewport" class="extension-dialog-viewport">
        <div ref="content" class="extension-dialog-content">
          <div v-if="b.extensions.removal" class="extension-removal-details">
            <ExtensionIcon :src="b.extensions.removal.item.icon" />
            <div>
              <h2>{{ b.t('卸载 {name}？', { name: b.extensions.removal.item.name }) }}</h2>
              <p id="extension-removal-description">{{ b.t('扩展及其应用内副本会被移除。') }}</p>
            </div>
          </div>
          <ExtensionReviewDetails v-else-if="b.extensions.review" :review="b.extensions.review" />
          <h2 v-else>{{ b.t('确认安装扩展') }}</h2>
          <p v-if="b.extensions.error" class="extension-message" role="alert">{{ b.extensions.error }}</p>
          <ExtensionProgress v-else-if="b.extensions.progress" :progress="b.extensions.progress" class="extension-dialog-progress" />
          <p v-else-if="b.extensions.busy || submitting" class="extension-message" role="status">{{ b.t('正在处理扩展…') }}</p>
          <div class="extension-dialog-actions">
            <button ref="cancel" type="button" autofocus :disabled="submitting && !b.extensions.error" @click="dismiss">
              {{ b.extensions.error ? b.t('关闭') : b.t('取消') }}
            </button>
            <button v-if="(b.extensions.removal || b.extensions.review) && !b.extensions.error" type="button" class="primary-button" :disabled="submitting || b.extensions.busy" @click="confirm">
              {{ b.t(b.extensions.removal ? '确认卸载' : '安装扩展') }}
            </button>
          </div>
        </div>
      </div>
    </dialog>
  </div>
</template>

<style scoped>
.extension-dialog-scrim { position: fixed; inset: 0; }
.extension-dialog-scrim::after { content: ""; position: absolute; inset: 0; background: rgb(0 0 0 / 0.24); }
.extension-dialog-backdrop { position: absolute; overflow: hidden; }
.extension-dialog-backdrop img { display: block; width: 100%; height: 100%; filter: blur(8px); transform: scale(1.03); }
.extension-dialog {
  box-sizing: border-box;
  width: min(440px, calc(100vw - 32px));
  max-width: none;
  max-height: calc(100vh - 32px);
  margin: auto;
  padding: 0;
  overflow: auto;
  overflow-wrap: anywhere;
  color: var(--ink);
  background: color-mix(in srgb, var(--panel) 87%, transparent);
  backdrop-filter: blur(18px) saturate(115%);
  border: 1px solid var(--panel-edge);
  border-radius: var(--radius-panel);
  box-shadow: 0 12px 42px var(--panel-shadow);
}
.extension-dialog::backdrop { background: transparent; }
.extension-dialog-viewport {
  height: var(--extension-dialog-height, auto);
  overflow: clip;
  transition: height 230ms cubic-bezier(0.25, 0.8, 0.25, 1);
}
.extension-dialog-content { padding: 20px; }
h2 { margin: 0; font-size: 16px; font-weight: 650; }
.extension-removal-details { display: flex; align-items: flex-start; gap: 12px; }
.extension-removal-details > div { min-width: 0; }
.extension-removal-details p { margin: 4px 0 0; color: var(--quiet); font-size: 13px; }
.extension-message { margin: 8px 0 0; color: var(--quiet); font-size: 13px; }
.extension-dialog-progress { margin-top: 14px; }
.extension-dialog-actions { display: flex; flex-wrap: wrap; justify-content: flex-end; gap: 8px; margin-top: 18px; }
.extension-dialog-actions button { min-height: 36px; min-width: 78px; padding: 6px 12px; border-radius: 7px; font-size: 13px; }
.extension-dialog-actions button:not(.primary-button) { background: var(--separator); }
.extension-dialog-actions .primary-button { color: var(--surface); background: var(--accent); }
.extension-dialog-actions .primary-button:hover { background: color-mix(in srgb, var(--accent) 90%, var(--ink)); }
.extension-dialog-actions button:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
@media (prefers-reduced-motion: reduce) {
  .extension-dialog-viewport { transition: none; }
}
:global(:root[data-reduce-motion="true"]) .extension-dialog-viewport { transition: none; }
</style>
