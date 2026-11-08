<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useBrowser } from "../../stores/browser";
import Favicon from "../common/Favicon.vue";
import Icon from "../common/Icon.vue";
import {
  commandContext,
  completionAllowed,
  completionFor,
  displayUrl,
  matchSite,
  nextCompletion,
  rankCommands,
  scopedResults,
  suggestionQuery,
  type CommandRow,
  type CompletionMatch,
  type SiteSearch,
} from "../../lib/command";
const b = useBrowser(),
  typed = ref(""),
  selected = ref(0),
  field = ref<HTMLInputElement>(),
  panel = ref<HTMLElement>(),
  scope = ref<SiteSearch>(),
  completion = ref<CompletionMatch>(),
  composing = ref(false);
let previous: HTMLElement | null = null,
  historyTimer: ReturnType<typeof setTimeout>,
  suggestTimer: ReturnType<typeof setTimeout>,
  typedBeforeScope = "",
  pointer: { x: number; y: number } | undefined;
const currentUrl = computed(() => (b.peek ?? b.active)?.url || "");
const hinted = computed(() => scope.value ? undefined : matchSite(typed.value));
const suffix = computed(() => {
  const match = completion.value;
  if (!match || match.text.length <= typed.value.length) return "";
  return match.text.toLowerCase().startsWith(typed.value.toLowerCase()) ? match.text.slice(typed.value.length) : "";
});
const kept = () => [...b.state.tabs.map((tab) => tab.url), ...b.state.favorites.map((item) => item.url), ...(b.state.pins || []).map((item) => item.url)];
const results = computed(() => {
  if (scope.value) return scopedResults(scope.value, typed.value, b.suggestions, b.history);
  const editing = b.commandMode === "edit" && typed.value === currentUrl.value;
  const ranked = rankCommands(typed.value, { state: b.state, recent: b.recent, history: b.history, suggestions: b.suggestions, editingCurrent: editing });
  const match = completion.value;
  if (!match || !typed.value.trim()) return ranked;
  const top: CommandRow = { kind: { type: "open", url: match.url }, title: displayUrl(match.url), detail: "", icon: "globe", action: "Open", key: `completion:${match.url}` };
  const next = ranked.filter((row) => row.title !== top.title && !(row.kind.type === "open" && row.kind.url === match.url));
  const existing = b.state.tabs.find((tab) => tab.url.replace(/\/$/, "") === match.url.replace(/\/$/, "") && tab.id !== b.state.activeTab);
  if (existing) next.unshift({ kind: { type: "switchTo", id: existing.id }, title: existing.title, detail: displayUrl(match.url), icon: "globe", action: "Switch to Tab", key: `switch:${existing.id}` });
  return [top, ...next].slice(0, 8);
});
function domain(site: SiteSearch) { return site.home.replace(/^https?:\/\//, "").split(".").slice(-2).join("."); }
function refreshRemote() {
  clearTimeout(historyTimer);
  clearTimeout(suggestTimer);
  const query = scope.value && !typed.value.trim() ? domain(scope.value) : typed.value;
  historyTimer = setTimeout(() => b.send({ type: "history", query }), 100);
  const suggest = suggestionQuery(typed.value);
  if (suggest) suggestTimer = setTimeout(() => b.suggest(suggest), 90);
  else b.suggestions = [];
}
function onInput(event: Event) {
  const input = event.target as HTMLInputElement;
  const text = input.value;
  const previousMatch = completion.value;
  const deleting = (event as InputEvent).inputType?.startsWith("delete") === true;
  const caretAtEnd = input.selectionStart === text.length && input.selectionEnd === text.length;
  const allowed = !scope.value && completionAllowed(text, typed.value + suffix.value, typed.value, deleting, caretAtEnd, composing.value);
  typed.value = text;
  completion.value = allowed ? nextCompletion(previousMatch, completionFor(text, b.history, kept()), text) : undefined;
  selected.value = 0;
  refreshRemote();
}
function acceptCompletion() {
  if (!completion.value) return;
  typed.value = completion.value.text;
  completion.value = nextCompletion(completion.value, completion.value, typed.value);
}
function enterScope(site: SiteSearch) {
  typedBeforeScope = typed.value;
  scope.value = site;
  typed.value = "";
  completion.value = undefined;
  selected.value = 0;
  refreshRemote();
  nextTick(() => field.value?.focus());
}
function leaveScope() {
  scope.value = undefined;
  typed.value = typedBeforeScope;
  completion.value = undefined;
  selected.value = 0;
  refreshRemote();
  nextTick(() => {
    const end = typed.value.length;
    field.value?.setSelectionRange(end, end);
  });
}
function step(delta: number) {
  if (!results.value.length) return;
  selected.value = (selected.value + delta + results.value.length) % results.value.length;
}
function runAction(id: string) {
  const active = b.peek ?? b.active;
  if (id === "settings") { b.commandOpen = false; b.settingsOpen = true; return; }
  if (id === "pin" || id === "unpin") b.send({ type: "pin", id: active?.id });
  else if (id === "favorite" && active) b.send({ type: "favoriteTab", id: active.id });
  else if (id === "separate") b.send({ type: "unsplit" });
  else if (id === "split") {
    const partner = commandContext(b.state, b.recent).splitPartner;
    if (partner && active) b.send({ type: "split", id: active.id, other: partner.id });
  } else if (id === "close-others") b.send({ type: "closeOthers" });
  else if (id === "close-below") b.send({ type: "closeBelow" });
  else if (id === "reopen") b.send({ type: "restore" });
  else if (id === "new-space") {
    const colors = ["sky", "sage", "sand", "rose", "lavender", "mint", "peach", "graphite"];
    b.send({ type: "spaceCreate", name: `Space ${b.state.spaces.length + 1}`, color: colors[b.state.spaces.length % colors.length] });
  } else if (id.startsWith("space:")) b.send({ type: "space", id: id.slice(6) });
  else if (id.startsWith("move:") && active) b.send({ type: "move", id: active.id, spaceId: id.slice(5) });
  b.commandOpen = false;
}
function open(url: string) {
  b.send({ type: b.commandMode === "edit" ? "navigate" : "new", url });
  b.commandOpen = false;
}
function run(index = selected.value) {
  const item = results.value[index];
  if (!item) {
    if (b.commandMode === "edit" && typed.value === currentUrl.value) b.send({ type: "reload" });
    b.commandOpen = false;
    return;
  }
  const kind = item.kind;
  try {
    if (kind.type === "open" || kind.type === "history" || kind.type === "searchOn") open(kind.url);
    else if (kind.type === "search" || kind.type === "suggestion") open(`https://www.google.com/search?q=${encodeURIComponent(kind.query)}`);
    else if (kind.type === "switchTo" || kind.type === "bookmark") { b.send({ type: "activate", id: kind.id }); b.commandOpen = false; }
    else if (kind.type === "action") runAction(kind.id);
  } catch (error) { b.error = String(error); }
}
function rowUrl(item: CommandRow) {
  const kind = item.kind;
  if (kind.type === "open" || kind.type === "history" || kind.type === "searchOn") return kind.url;
  if (kind.type === "switchTo") return b.state.tabs.find((tab) => tab.id === kind.id)?.url || "";
  if (kind.type === "bookmark") return [...b.state.favorites, ...(b.state.pins || [])].find((mark) => mark.id === kind.id)?.url || "";
  return "";
}
function point(event: PointerEvent, index: number) {
  if (!pointer) { pointer = { x: event.clientX, y: event.clientY }; return; }
  if (event.clientX !== pointer.x || event.clientY !== pointer.y) selected.value = index;
}
function key(event: KeyboardEvent) {
  if (event.key === "ArrowDown" || event.key === "ArrowUp") { event.preventDefault(); step(event.key === "ArrowDown" ? 1 : -1); }
  if (event.key === "Enter") { event.preventDefault(); run(); }
  if (event.key === "Escape") {
    event.preventDefault();
    if (scope.value) leaveScope();
    else if (suffix.value) completion.value = undefined;
    else b.commandOpen = false;
  }
  if (event.key === "Tab") {
    event.preventDefault();
    if (event.shiftKey) step(-1);
    else if (hinted.value) enterScope(hinted.value);
    else if (suffix.value) acceptCompletion();
    else step(1);
  }
  if (event.key === "ArrowRight" && suffix.value && field.value?.selectionStart === typed.value.length) acceptCompletion();
  if (event.key === "Backspace" && scope.value && !typed.value) { event.preventDefault(); leaveScope(); }
}
watch([typed, scope, () => b.history], () => {
  if (scope.value || !completion.value) return;
  completion.value = nextCompletion(completion.value, completionFor(typed.value, b.history, kept()), typed.value);
});
watch(selected, async () => {
  await nextTick();
  panel.value?.querySelector('[aria-selected="true"]')?.scrollIntoView({ block: "nearest" });
});
watch(() => b.commandOpen, async (open) => {
  if (open) {
    previous = document.activeElement as HTMLElement;
    scope.value = undefined;
    completion.value = undefined;
    pointer = undefined;
    typed.value = b.commandMode === "edit" && currentUrl.value !== "era://newtab" ? currentUrl.value : "";
    selected.value = 0;
    refreshRemote();
    await nextTick();
    field.value?.focus();
    field.value?.select();
  } else {
    clearTimeout(historyTimer);
    clearTimeout(suggestTimer);
    previous?.focus();
  }
});
function dismissOutside(event: PointerEvent) {
  if (b.commandOpen && panel.value && event.target instanceof Node && !panel.value.contains(event.target)) {
    if (event.target instanceof Element && event.target.closest(".command-scrim")) event.preventDefault();
    b.commandOpen = false;
  }
}
onMounted(() => document.addEventListener("pointerdown", dismissOutside, true));
onBeforeUnmount(() => {
  clearTimeout(historyTimer);
  clearTimeout(suggestTimer);
  document.removeEventListener("pointerdown", dismissOutside, true);
});
</script>
<template>
  <Transition name="panel"
    ><div
      v-if="b.commandOpen"
      class="command-scrim"
      data-presented="true"
      @pointerdown.self="b.commandOpen = false"
      @keydown="key"
    >
      <section ref="panel" class="command-panel" role="dialog" aria-modal="true" aria-label="搜索或输入地址">
        <div class="command-input">
          <Icon v-if="!scope" name="search" :size="18" />
          <span v-if="scope" class="command-chip">{{ scope.name }}</span>
          <span class="command-field">
            <span class="command-ghost" aria-hidden="true">{{ typed }}<span>{{ suffix }}</span></span>
            <input
              ref="field"
              :value="typed"
              role="combobox"
              aria-label="搜索或输入地址"
              aria-controls="command-results"
              :aria-activedescendant="results.length ? `command-${selected}` : undefined"
              :aria-expanded="results.length > 0"
              :placeholder="scope ? `Search ${scope.name}` : 'Search or enter address'"
              autocomplete="off"
              spellcheck="false"
              @input="onInput"
              @compositionstart="composing = true"
              @compositionend="composing = false"
            />
          </span>
          <span v-if="hinted" class="command-hint">Search {{ hinted.name }} <kbd>Tab</kbd></span>
          <kbd v-else>Esc</kbd>
        </div>
        <div id="command-results" class="command-results" role="listbox">
          <div
            v-for="(item, index) in results"
            :id="`command-${index}`"
            :key="item.key"
            class="command-result"
            :class="{ chosen: index === selected }"
            role="option"
            :aria-selected="index === selected"
            @pointermove="point($event, index)"
            @click="run(index)"
          >
            <Favicon v-if="rowUrl(item)" :url="rowUrl(item)" /><Icon v-else :name="item.icon" :size="18" />
            <span class="result-title">{{ item.title }}</span>
            <span class="result-detail">{{ item.detail }}</span>
            <span v-if="index === selected" class="result-action">{{ item.action }} <kbd>↵</kbd></span>
          </div>
        </div>
        <div v-if="b.error" class="inline-error">{{ b.error }}</div>
      </section>
    </div></Transition
  >
</template>
