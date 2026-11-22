<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, useId } from "vue";
import type { HistoryFilters } from "../../lib/history";
import { useBrowser } from "../../stores/browser";
import Icon from "../common/Icon.vue";

const props = defineProps<{ value: HistoryFilters; disabled?: boolean }>();
const emit = defineEmits<{ apply: [value: HistoryFilters] }>();
const b = useBrowser();
const root = ref<HTMLElement>(), trigger = ref<HTMLButtonElement>(), done = ref<HTMLButtonElement>();
const draft = ref<HistoryFilters>();
const panelId = useId();
const active = computed(() => props.value.period !== null || props.value.sort !== "date" || props.value.source !== "history");
const periods = [["today", "今天"], ["week", "本周"], ["month", "本月"]] as const;
const sorts = [["date", "按日期"], ["site", "按站点"], ["popular", "按访问次数"], ["last", "按最近访问"]] as const;
const sources = [["history", "历史记录"], ["closed", "已关闭的标签页"]] as const;

function open() {
  if (draft.value) return;
  draft.value = { ...props.value };
  void nextTick(() => done.value?.focus());
}
function close(restoreFocus = false) {
  draft.value = undefined;
  if (restoreFocus) trigger.value?.focus();
}
function apply() {
  if (draft.value) emit("apply", draft.value);
  close(true);
}
function outside(event: Event) {
  if (draft.value && event.target instanceof Node && !root.value?.contains(event.target)) close();
}
function escape(event: KeyboardEvent) {
  if (!draft.value) return;
  event.preventDefault();
  event.stopPropagation();
  close(true);
}
onMounted(() => {
  document.addEventListener("pointerdown", outside, true);
  document.addEventListener("focusin", outside);
});
onBeforeUnmount(() => {
  document.removeEventListener("pointerdown", outside, true);
  document.removeEventListener("focusin", outside);
});
</script>

<template>
  <div ref="root" class="history-filter" @keydown.esc="escape">
    <button ref="trigger" class="history-button history-filter-trigger" :class="{ 'is-active': active }"
      :disabled="disabled" aria-haspopup="dialog" :aria-expanded="!!draft" :aria-controls="draft ? panelId : undefined"
      @click="draft ? close() : open()" @keydown.down.prevent="open">
      <Icon name="filter" :size="16" />{{ b.t('筛选') }}
    </button>
    <section v-if="draft" :id="panelId" class="history-filter-panel" role="dialog" :aria-labelledby="`${panelId}-label`">
      <header>
        <h2 :id="`${panelId}-label`">{{ b.t('筛选历史记录') }}</h2>
        <button ref="done" class="history-button history-filter-done" @click="apply">{{ b.t('完成') }}</button>
      </header>
      <fieldset>
        <legend>{{ b.t('访问时间') }}</legend>
        <button v-for="[period, label] in periods" :key="period" :aria-pressed="draft.period === period"
          @click="draft.period = draft.period === period ? null : period">{{ b.t(label) }}</button>
      </fieldset>
      <fieldset class="history-filter-sort">
        <legend>{{ b.t('排序方式') }}</legend>
        <button v-for="[sort, label] in sorts" :key="sort" :aria-pressed="draft.sort === sort"
          @click="draft.sort = sort">{{ b.t(label) }}</button>
      </fieldset>
      <fieldset>
        <legend>{{ b.t('筛选范围') }}</legend>
        <button v-for="[source, label] in sources" :key="source" :aria-pressed="draft.source === source"
          @click="draft.source = source">{{ b.t(label) }}</button>
      </fieldset>
    </section>
  </div>
</template>
