<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, watch } from "vue";
import { useBrowser } from "../../stores/browser";
import { previewPages, TAB_PREVIEW } from "../../lib/tab-preview";
import { displayUrl } from "../../lib/command";
import Favicon from "../common/Favicon.vue";
const b = useBrowser();
const pages = computed(() => b.hoverPreview ? previewPages(b.state, b.hoverPreview.id) : []);
const card = ref<HTMLElement>();
const displayOpacity = ref(0);
let animation: Animation | undefined, epoch = 0;
function freeze() {
  if (animation && card.value) displayOpacity.value = Number(getComputedStyle(card.value).opacity);
  animation?.cancel(); animation = undefined;
}
watch(() => [b.hoverPreview?.id, b.hoverPreview?.ticket, b.hoverPreview?.phase], async () => {
  const generation = ++epoch;
  freeze();
  const preview = b.hoverPreview;
  if (!preview) { displayOpacity.value = 0; return; }
  if (!["opening", "closing"].includes(preview.phase)) return;
  await nextTick();
  if (!card.value || generation !== epoch) return;
  const closing = preview.phase === "closing";
  const duration = matchMedia("(prefers-reduced-motion: reduce)").matches ? TAB_PREVIEW.reduced : closing ? TAB_PREVIEW.leave : preview.instant ? 0 : TAB_PREVIEW.enter;
  const from = getComputedStyle(card.value).opacity;
  animation = card.value.animate([{ opacity: from }, { opacity: closing ? 0 : 1 }], { duration, easing: TAB_PREVIEW.ease, fill: "both" });
  try { await animation.finished; } catch { return; }
  if (generation !== epoch) return;
  b.previewAnimationDone = preview.ticket;
});
onBeforeUnmount(() => { epoch++; freeze(); });
</script>
<template><Teleport to="body">
    <section v-if="b.hoverPreview && pages.length" id="tab-hover-preview" ref="card" class="tab-hover-preview" role="tooltip"
      :data-preview-id="b.hoverPreview.id" :data-closing="b.hoverPreview.closing" :data-phase="b.hoverPreview.phase"
      :style="{ left: b.hoverPreview.frame.x + 'px', top: b.hoverPreview.frame.y + 'px', width: b.hoverPreview.frame.width + 'px', height: b.hoverPreview.frame.height + 'px', opacity: displayOpacity }">
      <div class="tab-preview-picture" :class="{ 'is-split': pages.length === 2 }">
        <div v-for="page in pages" :key="page.key" class="tab-preview-pane">
          <img v-if="b.thumbnails[page.tabId || page.key]" :src="b.thumbnails[page.tabId || page.key]" :alt="`${page.title} 的网页缩略图`" draggable="false" />
          <div v-else-if="page.url === 'era://newtab'" class="tab-preview-empty tab-preview-newtab"><span>Rove</span><small>A little room to explore.</small></div>
          <div v-else class="tab-preview-empty"><Favicon :url="page.url" :favicon="page.favicon" /><small>{{ !b.native ? '界面预览' : b.previewFailures[page.tabId || page.key] ? '暂时无法生成预览' : '正在生成预览…' }}</small></div>
        </div>
      </div>
      <div class="tab-preview-captions">
        <div v-for="page in pages" :key="page.key" class="tab-preview-caption">
          <span>{{ page.title }}</span><small>{{ displayUrl(page.url) }}</small>
        </div>
      </div>
    </section>
</Teleport></template>
<style>
.tab-preview-newtab > span { font-size: 36px; font-weight: 300; letter-spacing: -1px; color: var(--ink); }
.tab-hover-preview {
  position: fixed;
  z-index: 45;
  display: flex;
  flex-direction: column;
  padding: 4px;
  border: 1px solid var(--panel-edge);
  border-radius: var(--radius-panel);
  background: var(--panel);
  box-shadow: 0 3px 16px var(--panel-shadow);
  overflow: hidden;
  pointer-events: none;
}
.tab-preview-picture { display: flex; flex: 1; min-height: 0; overflow: hidden; border-radius: 10px; background: var(--surface); }
.tab-preview-pane { flex: 1; min-width: 0; overflow: hidden; }
.tab-preview-pane + .tab-preview-pane { border-left: 1px solid var(--panel-edge); }
.tab-preview-pane img { display: block; width: 100%; height: 100%; object-fit: cover; object-position: top; }
.tab-preview-empty { height: 100%; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 12px; color: var(--tertiary); }
.tab-preview-empty .favicon { width: 32px; height: 32px; }
.tab-preview-empty small { font-size: 11px; }
.tab-preview-captions { display: flex; flex: 0 0 43px; align-items: center; }
.tab-preview-caption { flex: 1; min-width: 0; padding: 0 8px; }
.tab-preview-caption span, .tab-preview-caption small { display: block; overflow: hidden; white-space: nowrap; text-overflow: ellipsis; }
.tab-preview-caption span { font-size: 13px; line-height: 19px; }
.tab-preview-caption small { font-size: 11px; line-height: 16px; color: var(--quiet); }
</style>
