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
const revealed = ref<Record<string, "instant" | "shown">>({});
let animation: Animation | undefined, epoch = 0, revealGeneration = 0, alive = true;
const queued = new Set<string>();
function pageKey(page: { key: string; tabId?: string }) { return page.tabId || page.key; }
function seed() {
  revealGeneration += 1;
  queued.clear();
  const next: Record<string, "instant" | "shown"> = {};
  for (const page of pages.value) {
    const key = pageKey(page);
    if (b.thumbnails[key]) next[key] = "instant";
  }
  revealed.value = next;
}
watch(() => b.hoverPreview?.id, seed, { flush: "sync" });
function reveal(el: HTMLImageElement, key: string) {
  if (revealed.value[key] || queued.has(key) || !b.thumbnails[key]) return;
  const id = b.hoverPreview?.id;
  const gen = revealGeneration;
  queued.add(key);
  void el.decode().then(() => {
    if (!alive || gen !== revealGeneration || b.hoverPreview?.id !== id || !el.isConnected || !b.thumbnails[key]) {
      queued.delete(key);
      return;
    }
    // Paint the hidden frame first, then fade. One frame is not enough when
    // decode finishes in the same turn the image is inserted.
    requestAnimationFrame(() => requestAnimationFrame(() => {
      queued.delete(key);
      if (!alive || gen !== revealGeneration || b.hoverPreview?.id !== id || !b.thumbnails[key] || revealed.value[key]) return;
      revealed.value = { ...revealed.value, [key]: "shown" };
    }));
  }, () => { queued.delete(key); });
}
function prime(el: unknown, key: string) {
  if (el instanceof HTMLImageElement) reveal(el, key);
}
watch(() => pages.value.map((page) => `${pageKey(page)}:${b.thumbnails[pageKey(page)] ? 1 : 0}`).join("|"), () => {
  const next = { ...revealed.value };
  let changed = false;
  for (const key of Object.keys(next)) {
    if (!pages.value.some((page) => pageKey(page) === key && b.thumbnails[key])) {
      delete next[key];
      queued.delete(key);
      changed = true;
    }
  }
  if (changed) revealed.value = next;
});
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
  const duration = matchMedia("(prefers-reduced-motion: reduce)").matches || b.settings.reduceMotion ? TAB_PREVIEW.reduced : closing ? TAB_PREVIEW.leave : preview.instant ? 0 : TAB_PREVIEW.enter;
  const from = getComputedStyle(card.value).opacity;
  animation = card.value.animate([{ opacity: from }, { opacity: closing ? 0 : 1 }], { duration, easing: TAB_PREVIEW.ease, fill: "both" });
  try { await animation.finished; } catch { return; }
  if (generation !== epoch) return;
  b.previewAnimationDone = preview.ticket;
});
onBeforeUnmount(() => { alive = false; revealGeneration += 1; epoch++; freeze(); });
</script>
<template><Teleport to="body">
    <section v-if="b.hoverPreview && pages.length" id="tab-hover-preview" ref="card" class="tab-hover-preview" role="tooltip"
      :data-preview-id="b.hoverPreview.id" :data-closing="b.hoverPreview.closing" :data-phase="b.hoverPreview.phase"
      :style="{ left: b.hoverPreview.frame.x + 'px', top: b.hoverPreview.frame.y + 'px', width: b.hoverPreview.frame.width + 'px', height: b.hoverPreview.frame.height + 'px', opacity: displayOpacity, '--snapshot-reveal': `${TAB_PREVIEW.reveal}ms ${TAB_PREVIEW.ease}` }">
      <div class="tab-preview-picture" :class="{ 'is-split': pages.length === 2 }">
        <div v-for="page in pages" :key="page.key" class="tab-preview-pane" :class="{ 'is-instant': revealed[pageKey(page)] === 'instant' }">
          <div v-if="page.url === 'era://newtab'" class="tab-preview-empty tab-preview-newtab" :class="{ 'is-covered': revealed[pageKey(page)] }" :aria-hidden="revealed[pageKey(page)] ? true : undefined"><span>Rove</span><small>A little room to explore.</small></div>
          <div v-else class="tab-preview-empty" :class="{ 'is-covered': revealed[pageKey(page)] }" :aria-hidden="revealed[pageKey(page)] ? true : undefined"><Favicon :url="page.url" :favicon="page.favicon" /><small>{{ !b.native ? b.t('界面预览') : b.previewFailures[page.tabId || page.key] ? b.t('暂时无法生成预览') : b.t('正在生成预览…') }}</small></div>
          <img v-if="b.thumbnails[pageKey(page)]" class="tab-preview-snapshot" :class="{ 'is-shown': revealed[pageKey(page)] }" :src="b.thumbnails[pageKey(page)]" :alt="`${page.title} 的网页缩略图`" :aria-hidden="revealed[pageKey(page)] ? undefined : true" draggable="false" :ref="(el) => prime(el, pageKey(page))" @load="prime($event.target, pageKey(page))" />
        </div>
      </div>
      <div class="tab-preview-captions">
        <div v-for="page in pages" :key="page.key" class="tab-preview-caption">
          <span>{{ page.title }}</span><small v-if="b.asleep[page.tabId || page.key]" class="tab-preview-sleep">{{ b.t('已休眠以节省内存') }}</small><small v-else>{{ displayUrl(page.url) }}</small>
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
.tab-preview-pane { position: relative; flex: 1; min-width: 0; overflow: hidden; }
.tab-preview-pane + .tab-preview-pane { border-left: 1px solid var(--panel-edge); }
.tab-preview-snapshot {
  position: absolute;
  inset: 0;
  z-index: 1;
  display: block;
  width: 100%;
  height: 100%;
  object-fit: cover;
  object-position: top;
  opacity: 0;
  transition: opacity var(--snapshot-reveal, 200ms cubic-bezier(0.23, 1, 0.32, 1));
}
.tab-preview-snapshot.is-shown { opacity: 1; }
.tab-preview-pane.is-instant .tab-preview-snapshot,
.tab-preview-pane.is-instant .tab-preview-empty { transition: none; }
.tab-preview-empty { height: 100%; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 12px; color: var(--tertiary); transition: opacity var(--snapshot-reveal, 200ms cubic-bezier(0.23, 1, 0.32, 1)); }
.tab-preview-empty.is-covered { opacity: 0; }
.tab-preview-empty .favicon { width: 32px; height: 32px; }
.tab-preview-empty small { font-size: 11px; }
.tab-preview-captions { display: flex; flex: 0 0 43px; align-items: center; }
.tab-preview-caption { flex: 1; min-width: 0; padding: 0 8px; }
.tab-preview-caption span, .tab-preview-caption small { display: block; overflow: hidden; white-space: nowrap; text-overflow: ellipsis; }
.tab-preview-caption span { font-size: 13px; line-height: 19px; }
.tab-preview-caption small { font-size: 11px; line-height: 16px; color: var(--quiet); }
</style>
