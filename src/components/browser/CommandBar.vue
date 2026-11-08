<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, watch } from "vue";
import { useBrowser } from "../../stores/browser";
import { address, host } from "../../lib/address";
import Favicon from "../common/Favicon.vue";
import Icon from "../common/Icon.vue";
const b = useBrowser(),
  query = ref(""),
  selected = ref(0),
  field = ref<HTMLInputElement>(),
  panel = ref<HTMLElement>();
let previous: HTMLElement | null = null,
  timer: ReturnType<typeof setTimeout>;
const results = computed(() => {
  const q = query.value.trim().toLowerCase();
  const list: {
    title: string;
    detail: string;
    url: string;
    id?: string;
    kind: string;
  }[] = [];
  if (q)
    list.push({
      title: query.value,
      detail: /[.\/]/.test(q) ? "打开网址" : "使用 Google 搜索",
      url: query.value,
      kind: "navigate",
    });
  list.push(
    ...b.state.tabs
      .filter(
        (t) =>
          t.url !== "era://newtab" &&
          (!q || `${t.title} ${t.url}`.toLowerCase().includes(q)),
      )
      .map((t) => ({
        title: t.title,
        detail: `切换标签页 · ${host(t.url)}`,
        url: t.url,
        id: t.id,
        kind: "tab",
      })),
  );
  list.push(
    ...b.history
      .filter(
        (h) =>
          (!q || `${h.title} ${h.url}`.toLowerCase().includes(q)) &&
          !list.some((x) => x.url === h.url),
      )
      .map((h) => ({
        ...h,
        detail: `历史记录 · ${host(h.url)}`,
        kind: "history",
      })),
  );
  if (!q)
    list.push(
      ...b.state.favorites
        .filter(
          (f) =>
            !list.some(
              (x) => x.url.replace(/\/$/, "") === f.url.replace(/\/$/, ""),
            ),
        )
        .map((f) => ({ ...f, detail: host(f.url), kind: "favorite" })),
    );
  return list.slice(0, 8);
});
watch(query, () => {
  selected.value = 0;
  clearTimeout(timer);
  timer = setTimeout(
    () => b.send({ type: "history", query: query.value }),
    100,
  );
});
watch(
  () => b.commandOpen,
  async (open) => {
    if (open) {
      previous = document.activeElement as HTMLElement;
      query.value =
        b.commandMode === "edit" && (b.peek ?? b.active)?.url !== "era://newtab"
          ? (b.peek ?? b.active)?.url || ""
          : "";
      selected.value = 0;
      await nextTick();
      field.value?.focus();
      field.value?.select();
    } else {
      previous?.focus();
      clearTimeout(timer);
    }
  },
);
watch(selected, async () => {
  await nextTick();
  panel.value
    ?.querySelector('[aria-selected="true"]')
    ?.scrollIntoView({ block: "nearest" });
});
function run(index = selected.value) {
  const r = results.value[index];
  if (!r) return;
  try {
    if (r.id) b.send({ type: "activate", id: r.id });
    else
      b.send({
        type: b.commandMode === "edit" ? "navigate" : "new",
        url: address(r.url),
      });
    b.commandOpen = false;
  } catch (e) {
    b.error = String(e);
  }
}
function key(e: KeyboardEvent) {
  if (e.key === "ArrowDown" || e.key === "ArrowUp") {
    e.preventDefault();
    selected.value =
      (selected.value +
        (e.key === "ArrowDown" ? 1 : -1) +
        results.value.length) %
      Math.max(results.value.length, 1);
  }
  if (e.key === "Enter") {
    e.preventDefault();
    run();
  }
  if (e.key === "Escape") {
    e.preventDefault();
    b.commandOpen = false;
  }
  if (e.key === "Tab") {
    e.preventDefault();
    field.value?.focus();
  }
}
onBeforeUnmount(() => clearTimeout(timer));
</script>
<template>
  <Transition name="panel"
    ><div
      v-if="b.commandOpen"
      class="command-scrim"
      @pointerdown.self="b.commandOpen = false"
      @keydown="key"
    >
      <section
        ref="panel"
        class="command-panel"
        role="dialog"
        aria-modal="true"
        aria-label="搜索或输入地址"
      >
        <div class="command-input">
          <Icon name="search" :size="18" /><input
            ref="field"
            v-model="query"
            role="combobox"
            aria-label="搜索或输入地址"
            aria-controls="command-results"
            :aria-activedescendant="`command-${selected}`"
            aria-expanded="true"
            placeholder="Search or enter address"
            autocomplete="off"
            spellcheck="false"
          /><kbd>Esc</kbd>
        </div>
        <div id="command-results" class="command-results" role="listbox">
          <div
            v-for="(r, i) in results"
            :id="`command-${i}`"
            :key="r.kind + r.url + r.id"
            class="command-result"
            :class="{ chosen: i === selected }"
            role="option"
            :aria-selected="i === selected"
            @pointermove="selected = i"
            @click="run(i)"
          >
            <Icon
              v-if="r.kind === 'navigate'"
              name="search"
              :size="18"
            /><Favicon v-else :url="r.url" /><span class="result-title">{{
              r.title
            }}</span
            ><span class="result-detail">{{ r.detail }}</span
            ><span v-if="i === selected" class="result-action"
              >{{ r.kind === "tab" ? "Switch" : "Open" }} <kbd>↵</kbd></span
            >
          </div>
        </div>
        <div v-if="b.error" class="inline-error">{{ b.error }}</div>
      </section>
    </div></Transition
  >
</template>
