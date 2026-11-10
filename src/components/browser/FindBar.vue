<script setup lang="ts">
// A port of Zigzag's Window/FindBar.swift (Copyright 2026 Zigao Wang,
// Apache-2.0), adapted to WebView2's own Find API and the floating trusted
// layer. WebView2 reports matches, so this component only owns the field,
// the counter, stepping, closing, and the entrance/exit motion.
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useBrowser } from "../../stores/browser";
import { findMatchLabel, normalizeFindQuery } from "../../lib/find";
import Icon from "../common/Icon.vue";
const b = useBrowser();
const field = ref<HTMLInputElement>();
const typed = ref("");
let timer: ReturnType<typeof setTimeout> | undefined;
const label = computed(() =>
  findMatchLabel(b.findCount, b.findActive, Boolean(b.findQuery.trim())),
);
const hasMatches = computed(() => b.findCount > 0);
const reducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)");
const calm = ref(b.settings.reduceMotion || reducedMotion.matches);
function syncCalm() {
  calm.value = b.settings.reduceMotion || reducedMotion.matches;
}
watch(() => b.settings.reduceMotion, syncCalm);
onMounted(() => reducedMotion.addEventListener("change", syncCalm));
onBeforeUnmount(() => reducedMotion.removeEventListener("change", syncCalm));
// The pill settles in from its top-right corner over 160ms and leaves in
// 120ms; Reduce Motion keeps only an 80ms fade, as elsewhere in Rove. The
// explicit duration lets Vue unmount the outer box even though the visible
// animation lives on the inner surface.
const motion = computed(() => (calm.value ? 80 : { enter: 160, leave: 120 }));
watch(
  () => [b.findOpen, b.findTicket, b.findFocusTicket] as const,
  async ([open]) => {
    if (!open) return;
    typed.value = b.findQuery;
    await nextTick();
    field.value?.focus();
    field.value?.select();
  },
  { immediate: true },
);
function changed(event: Event) {
  typed.value = (event.target as HTMLInputElement).value;
  clearTimeout(timer);
  // Zigzag searches on every edit; a short quiet window keeps a fast typist
  // from starting one WebView2 find session per key.
  timer = setTimeout(() => b.setFindQuery(normalizeFindQuery(typed.value)), 60);
}
function commit() {
  clearTimeout(timer);
  const next = normalizeFindQuery(typed.value);
  // Sending the same term would start a fresh session and reset the active
  // match; only a changed term needs a new search.
  if (next !== b.findQuery) b.setFindQuery(next);
}
function key(event: KeyboardEvent) {
  if (event.key === "Enter") {
    event.preventDefault();
    commit();
    b.findStep(event.shiftKey);
  } else if (event.key === "Escape") {
    event.preventDefault();
    clearTimeout(timer);
    b.closeFind();
  }
}
onBeforeUnmount(() => clearTimeout(timer));
</script>
<template>
  <Transition name="find" :duration="motion">
    <div
      v-if="b.findOpen"
      class="find-bar"
      role="search"
      aria-label="页内查找"
      data-find-bar=""
    >
      <div class="find-bar-surface">
        <Icon class="find-icon" name="search" :size="14" />
        <input
          ref="field"
          :value="typed"
          type="text"
          placeholder="在页面中查找"
          aria-label="查找内容"
          autocomplete="off"
          spellcheck="false"
          @input="changed"
          @keydown="key"
        />
        <span v-if="label" class="find-count" role="status">{{ label }}</span>
        <button
          class="find-button"
          type="button"
          title="上一个匹配 (Shift+Enter)"
          aria-label="上一个匹配"
          :disabled="!hasMatches"
          @click="b.findStep(true)"
        >
          <Icon name="chevronUp" :size="12" />
        </button>
        <button
          class="find-button"
          type="button"
          title="下一个匹配 (Enter)"
          aria-label="下一个匹配"
          :disabled="!hasMatches"
          @click="b.findStep(false)"
        >
          <Icon name="chevronDown" :size="12" />
        </button>
        <button
          class="find-button find-done"
          type="button"
          title="关闭查找 (Esc)"
          aria-label="关闭查找"
          @click="b.closeFind()"
        >
          <Icon name="close" :size="12" />
        </button>
      </div>
    </div>
  </Transition>
</template>
