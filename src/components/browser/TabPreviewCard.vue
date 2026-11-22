<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, watch } from "vue";
import { useBrowser } from "../../stores/browser";
import { previewPages, TAB_PREVIEW } from "../../lib/tab-preview";
import { displayUrl } from "../../lib/command";
const b = useBrowser();
const pages = computed(() => b.hoverPreview ? previewPages(b.state, b.hoverPreview.id) : []);
const card = ref<HTMLElement>();
const displayOpacity = ref(0);
const snapshots = ref<Record<string, { src: string; instant: boolean }>>({});
const requested = new Map<string, { src: string }>();
let animation: Animation | undefined, epoch = 0;
function pageKey(page: { key: string; tabId?: string }) { return page.tabId || page.key; }
const pictureVisible = computed(() => pages.value.some(page => page.url === "era://newtab" || snapshots.value[pageKey(page)]));
const animatePicture = computed(() => pages.value.some(page => snapshots.value[pageKey(page)]?.instant === false));
watch(() => [b.hoverPreview?.id, pages.value.map(page => [pageKey(page), b.thumbnails[pageKey(page)]] as const)] as const, ([id, entries], [previousId]) => {
  const switched = id !== previousId;
  if (switched) {
    snapshots.value = {};
    requested.clear();
  }
  const sources = new Map(entries);
  for (const key of requested.keys()) {
    if (!sources.get(key)) {
      requested.delete(key);
      delete snapshots.value[key];
    }
  }
  for (const [key, src] of sources) {
    if (!src || requested.get(key)?.src === src) continue;
    const request = { src };
    requested.set(key, request);
    const image = new Image();
    image.src = src;
    // Keep the last decoded frame visible while a refresh is decoding.
    void image.decode().then(() => {
      if (requested.get(key) !== request || b.hoverPreview?.id !== id || b.thumbnails[key] !== src) return;
      const instant = snapshots.value[key]?.instant ?? (switched && b.hoverPreview?.phase === "preparing");
      snapshots.value[key] = { src, instant };
    }, () => {});
  }
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
onBeforeUnmount(() => { requested.clear(); epoch++; freeze(); });
</script>
<template><Teleport to="body">
    <section v-if="b.hoverPreview && pages.length" id="tab-hover-preview" ref="card" class="tab-hover-preview" role="tooltip"
      :class="{ 'has-picture': pictureVisible, 'animate-picture': animatePicture }"
      :data-preview-id="b.hoverPreview.id" :data-closing="b.hoverPreview.closing" :data-phase="b.hoverPreview.phase"
      :style="{ left: b.hoverPreview.frame.x + 'px', top: b.hoverPreview.frame.y + 'px', bottom: `calc(100% - ${b.hoverPreview.frame.y + b.hoverPreview.frame.height}px)`, width: b.hoverPreview.frame.width + 'px', opacity: displayOpacity, '--preview-height': b.hoverPreview.frame.height + 'px', '--snapshot-reveal': `${TAB_PREVIEW.reveal}ms ${TAB_PREVIEW.ease}` }">
      <div class="tab-preview-picture">
        <div v-for="page in pages" :key="page.key" class="tab-preview-pane"
          :class="{ 'has-snapshot': snapshots[pageKey(page)], 'is-instant': snapshots[pageKey(page)]?.instant, 'is-newtab': page.url === 'era://newtab' }">
          <img v-if="snapshots[pageKey(page)]" class="tab-preview-snapshot" :src="snapshots[pageKey(page)]?.src" :alt="`${page.title} 的网页缩略图`" draggable="false" />
          <div v-else-if="page.url === 'era://newtab'" class="tab-preview-newtab"><span>Rove</span><small>A little room to explore.</small></div>
        </div>
      </div>
      <div class="tab-preview-captions">
        <div v-for="page in pages" :key="page.key" class="tab-preview-caption">
          <span>{{ page.title }}</span><small v-if="b.asleep[pageKey(page)]" class="tab-preview-sleep">{{ b.t('已休眠以节省内存') }}</small><small v-else-if="b.previewFailures[pageKey(page)] && !snapshots[pageKey(page)]" :title="displayUrl(page.url)">{{ b.t('暂时无法生成预览') }}</small><small v-else>{{ displayUrl(page.url) }}</small>
        </div>
      </div>
    </section>
</Teleport></template>
<style>
.tab-preview-newtab > span { font-size: 36px; font-weight: 300; letter-spacing: -1px; color: var(--ink); }
.tab-hover-preview {
  --preview-caption-height: 43px;
  --preview-padding: 4px;
  --preview-border: 1px;
  --preview-compact-height: calc(var(--preview-caption-height) + 2 * (var(--preview-padding) + var(--preview-border)));
  position: fixed;
  z-index: 45;
  display: flex;
  flex-direction: column;
  height: var(--preview-compact-height);
  margin-block: auto;
  padding: var(--preview-padding);
  border: var(--preview-border) solid var(--panel-edge);
  border-radius: var(--radius-panel);
  background: var(--panel);
  box-shadow: 0 3px 16px var(--panel-shadow);
  overflow: hidden;
  pointer-events: none;
}
.tab-hover-preview.has-picture { height: var(--preview-height); }
.tab-hover-preview.animate-picture { transition: height var(--snapshot-reveal); }
.tab-preview-picture { display: flex; flex: 1; min-height: 0; overflow: hidden; border-radius: 10px; background: var(--surface); }
.tab-preview-pane {
  flex: 1;
  min-width: 0;
  height: calc(var(--preview-height) - var(--preview-compact-height));
  overflow: hidden;
  opacity: 0;
}
.tab-preview-pane.has-snapshot, .tab-preview-pane.is-newtab { opacity: 1; }
.tab-preview-pane.has-snapshot:not(.is-instant) { transition: opacity var(--snapshot-reveal); }
.tab-preview-pane + .tab-preview-pane { border-left: 1px solid var(--panel-edge); }
.tab-preview-snapshot {
  display: block;
  width: 100%;
  height: 100%;
  object-fit: cover;
  object-position: top;
}
.tab-preview-newtab { height: 100%; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 12px; color: var(--tertiary); }
.tab-preview-newtab small { font-size: 11px; }
.tab-preview-captions { display: flex; flex: 0 0 var(--preview-caption-height); align-items: center; }
.tab-preview-caption { flex: 1; min-width: 0; padding: 0 8px; }
.tab-preview-caption span, .tab-preview-caption small { display: block; overflow: hidden; white-space: nowrap; text-overflow: ellipsis; }
.tab-preview-caption span { font-size: 13px; line-height: 19px; }
.tab-preview-caption small { font-size: 11px; line-height: 16px; color: var(--quiet); }
:root[data-reduce-motion="true"] :is(.tab-hover-preview.animate-picture, .tab-preview-pane.has-snapshot) { transition: none !important; }
@media (prefers-reduced-motion: reduce) {
  .tab-hover-preview.animate-picture, .tab-preview-pane.has-snapshot:not(.is-instant) { transition: none; }
}
</style>
