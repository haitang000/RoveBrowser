<script setup lang="ts">
// The session, site-cluster and clear-range interactions follow Zigzag's
// HistoryPage.swift, Copyright 2026 Zigao Wang, Apache-2.0.
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { host } from "../../lib/address";
import { displayUrl } from "../../lib/command";
import { groupHistory, historyPeriod, historyRangeStart, type HistoryCluster, type HistoryCursor, type HistoryEdit, type HistoryRange, type HistoryVisit } from "../../lib/history";
import { tabDisplayTitle } from "../../lib/tab-meta";
import { useBrowser } from "../../stores/browser";
import Favicon from "../common/Favicon.vue";
import Icon from "../common/Icon.vue";
import HistoryRow from "./HistoryRow.vue";
import "../../styles/history.css";

const props = defineProps<{ tabId: string }>();
const b = useBrowser();
const scroller = ref<HTMLElement>(), search = ref<HTMLInputElement>(), more = ref<HTMLButtonElement>();
const clearButton = ref<HTMLButtonElement>(), menu = ref<HTMLElement>(), cancel = ref<HTMLButtonElement>(), confirmButton = ref<HTMLButtonElement>();
const pageRows = ref<HistoryVisit[]>([]);
const cursor = ref<HistoryCursor | null>(null);
const openClusters = ref<Record<string, boolean>>({});
const shown = ref<Record<string, number>>({});
const query = ref(""), resultQuery = ref(""), loading = ref(true), error = ref(""), notice = ref("");
const selection = ref(false), menuOpen = ref(false);
const confirm = ref<{ range: HistoryRange } | { items: HistoryVisit[] }>();
const ranges = [
  ["hour", "最近一小时"], ["today", "今天"], ["yesterday", "今天和昨天"], ["all", "全部历史记录"],
] as const;
let read: { id: string; append: boolean; query: string; openFirst?: boolean } | undefined;
let editId: string | undefined;
const editing = ref(false);
let timer: ReturnType<typeof setTimeout>;
let previousFocus: HTMLElement | null = null;

const sessions = computed(() => groupHistory(pageRows.value));
const recent = computed(() => b.state.closed.filter((tab) => host(tab.url)).slice(-10).reverse());
const icons = computed(() => {
  const cached = new Map<string, string>();
  for (const tab of [...b.state.tabs, ...recent.value, ...b.state.favorites, ...(b.state.pins ?? [])]) {
    if (tab.favicon?.startsWith("data:image/")) cached.set(host(tab.url), tab.favicon);
  }
  return cached;
});
const timeFormat = computed(() => new Intl.DateTimeFormat(b.language, { hour: "numeric", minute: "2-digit" }));
const weekdayFormat = computed(() => new Intl.DateTimeFormat(b.language, { weekday: "long" }));
function time(visit: HistoryVisit) { return timeFormat.value.format(visit.visited * 1000); }
function span(cluster: HistoryCluster) {
  return timeFormat.value.formatRange(cluster.items.at(-1)!.visited * 1000, cluster.items[0]!.visited * 1000);
}
function sessionLabel(date: Date) {
  const today = new Date();
  const yesterday = new Date(today.getFullYear(), today.getMonth(), today.getDate() - 1);
  const lastWeek = new Date(today.getFullYear(), today.getMonth(), today.getDate() - 6);
  let day: string;
  if (date.toDateString() === today.toDateString()) day = b.t("今天");
  else if (date.toDateString() === yesterday.toDateString()) day = b.t("昨天");
  else if (date >= lastWeek && date <= today) day = weekdayFormat.value.format(date);
  else day = date.toLocaleDateString(b.language, { month: "long", day: "numeric", year: date.getFullYear() === today.getFullYear() ? undefined : "numeric" });
  return b.t("{day} · {period}", { day, period: b.t(historyPeriod(date)) });
}
function clusterKey(cluster: HistoryCluster) { return cluster.items[0]!.url; }
function expanded(cluster: HistoryCluster) { return openClusters.value[clusterKey(cluster)] ?? Boolean(resultQuery.value); }
function toggle(cluster: HistoryCluster) { openClusters.value[clusterKey(cluster)] = !expanded(cluster); }
function count(items: number) { return b.t(items === 1 ? "1 个页面" : "{count} 个页面", { count: items }); }

function request(append = false) {
  clearTimeout(timer);
  const id = crypto.randomUUID();
  read = { id, append, query: query.value.trim() };
  error.value = "";
  loading.value = true;
  b.send({ type: "historyPage", ticket: id, query: read.query, before: append ? cursor.value : undefined });
}
function loadMore() {
  if (cursor.value && !loading.value && !editing.value) request(true);
}
function searchChanged(event: Event) {
  if (event instanceof InputEvent && event.isComposing) return;
  read = undefined;
  selection.value = false;
  loading.value = true;
  notice.value = "";
  clearTimeout(timer);
  timer = setTimeout(request, 120);
}
function openVisit(visit: HistoryVisit, event?: MouseEvent) {
  if (selection.value) selection.value = false;
  if (event?.ctrlKey || event?.metaKey || event?.button === 1) b.send({ type: "new", url: visit.url });
  else b.send({ type: "navigate", id: props.tabId, url: visit.url });
}

function change(change: HistoryEdit) {
  if (editing.value) return;
  editId = crypto.randomUUID();
  editing.value = true;
  selection.value = false;
  notice.value = "";
  error.value = "";
  b.send({ type: "historyEdit", ticket: editId, change });
}
function remove(items: HistoryVisit[]) {
  if (!selection.value) change({ action: "remove", urls: items.map((item) => item.url) });
}
function startConfirm(value: NonNullable<typeof confirm.value>) {
  previousFocus = document.activeElement as HTMLElement | null;
  setMenu(false);
  confirm.value = value;
  void nextTick(() => cancel.value?.focus());
}
function closeConfirm() {
  confirm.value = undefined;
  void nextTick(() => {
    if (previousFocus?.isConnected) previousFocus.focus();
    else clearButton.value?.focus();
  });
}
const confirmTitle = computed(() => {
  if (!confirm.value) return "";
  if ("items" in confirm.value) return b.t("移除这 {count} 个页面？", { count: confirm.value.items.length });
  return b.t({
    hour: "清除最近一小时的历史记录？",
    today: "清除今天的历史记录？",
    yesterday: "清除今天和昨天的历史记录？",
    all: "清除全部历史记录？",
  }[confirm.value.range]);
});
function confirmChange() {
  const pending = confirm.value;
  if (!pending) return;
  closeConfirm();
  if ("range" in pending) change({ action: "clear", since: historyRangeStart(pending.range) });
  else change({ action: "remove", urls: pending.items.map((item) => item.url) });
}
function cycleConfirmFocus() {
  (document.activeElement === cancel.value ? confirmButton.value : cancel.value)?.focus();
}
function setMenu(open: boolean, focus = false) {
  menuOpen.value = open;
  if (open && focus) void nextTick(() => menu.value?.querySelector("button")?.focus());
}
function dismissMenu(event: MouseEvent) {
  if (menuOpen.value && event.target instanceof Element && !event.target.closest(".history-clear")) setMenu(false);
}
function menuKey(event: KeyboardEvent) {
  const items = [...(menu.value?.querySelectorAll("button") ?? [])];
  if (event.key === "Escape") {
    event.preventDefault();
    setMenu(false);
    clearButton.value?.focus();
  } else if (event.key === "Tab") {
    setMenu(false);
  } else if (event.key === "ArrowDown" || event.key === "ArrowUp") {
    event.preventDefault();
    const at = items.findIndex((item) => item === document.activeElement);
    items[(at + (event.key === "ArrowDown" ? 1 : -1) + items.length) % items.length]?.focus();
  }
}
function key(event: KeyboardEvent) {
  if (event.defaultPrevented || event.isComposing || event.keyCode === 229 || menuOpen.value || confirm.value) return;
  const isSearch = event.target === search.value;
  if (event.ctrlKey && !event.shiftKey && !event.altKey && !event.metaKey) {
    if (event.key.toLowerCase() === "a" && pageRows.value.length && !loading.value && (!isSearch || !query.value)) {
      event.preventDefault();
      selection.value = true;
      if (isSearch) scroller.value?.focus();
    } else if (event.key.toLowerCase() === "z" && !isSearch && b.canUndoHistory) {
      event.preventDefault();
      change({ action: "undo" });
    }
    return;
  }
  if (isSearch && event.key === "Escape" && query.value) {
    event.preventDefault();
    query.value = "";
    request();
  } else if (event.key === "Escape" && selection.value) {
    event.preventDefault();
    selection.value = false;
  } else if (selection.value && ["Delete", "Backspace"].includes(event.key)) {
    event.preventDefault();
    startConfirm(resultQuery.value ? { items: pageRows.value } : { range: "all" });
  } else if (event.key === "ArrowDown" || event.key === "ArrowUp") {
    const rows = [...(scroller.value?.querySelectorAll<HTMLElement>("[data-history-row]") ?? [])];
    const at = rows.findIndex((row) => row === document.activeElement);
    if (isSearch && event.key === "ArrowDown" && rows.length) {
      event.preventDefault();
      rows[0]?.focus();
    } else if (at >= 0) {
      event.preventDefault();
      if (at === 0 && event.key === "ArrowUp") search.value?.focus();
      else rows[at + (event.key === "ArrowDown" ? 1 : -1)]?.focus();
    }
  } else if (isSearch && event.key === "Enter") {
    event.preventDefault();
    if (!read && loading.value) request();
    if (read) read.openFirst = true;
    else if (pageRows.value[0] && !error.value) openVisit(pageRows.value[0]);
  }
}

watch(() => b.historyReply, (reply) => {
  if (!reply) return;
  if (reply.requestId === editId) {
    editing.value = false;
    editId = undefined;
    if (!reply.ok) error.value = b.t("无法修改历史记录，请重试。");
    else notice.value = b.t("历史记录已更新");
    return;
  }
  if (reply.requestId !== read?.id) return;
  const { append, query: requested, openFirst } = read;
  read = undefined;
  loading.value = false;
  if (!reply.page) {
    error.value = b.t("无法读取历史记录，请重试。");
    return;
  }
  resultQuery.value = requested;
  cursor.value = reply.page.next;
  const hadFocus = scroller.value?.contains(document.activeElement);
  const entries = reply.page.entries.filter((entry) => host(entry.url) && Number.isFinite(new Date(entry.visited * 1000).getTime()));
  if (append) {
    const existing = new Set(pageRows.value.map((entry) => entry.url));
    pageRows.value = [...pageRows.value, ...entries.filter((entry) => !existing.has(entry.url))];
  } else {
    pageRows.value = entries;
    openClusters.value = {};
    shown.value = {};
    if (scroller.value) scroller.value.scrollTop = 0;
  }
  if (openFirst && pageRows.value[0]) openVisit(pageRows.value[0]);
  else if (hadFocus) void nextTick(() => {
    if (document.activeElement === document.body) scroller.value?.focus({ preventScroll: true });
  });
}, { flush: "sync" });
watch(() => b.historyVersion, () => { selection.value = false; request(); }, { immediate: true });
let observer: IntersectionObserver | undefined;
onMounted(() => {
  document.addEventListener("click", dismissMenu);
  observer = new IntersectionObserver(([entry]) => { if (entry?.isIntersecting) loadMore(); }, { root: scroller.value, rootMargin: "0px 0px 200px" });
  if (more.value) observer.observe(more.value);
  if (b.active?.id === props.tabId) search.value?.focus({ preventScroll: true });
});
watch(more, (element, old) => {
  if (old) observer?.unobserve(old);
  if (element) observer?.observe(element);
});
onBeforeUnmount(() => {
  clearTimeout(timer);
  observer?.disconnect();
  document.removeEventListener("click", dismissMenu);
});
</script>

<template>
  <section class="history-page" :aria-label="b.t('历史记录')">
    <div ref="scroller" class="history-scroll" tabindex="-1" :inert="!!confirm" @keydown="key" @pointerdown="selection = false">
      <header class="history-header">
        <div class="history-header-inner">
          <h1><Icon name="history" :size="20" />{{ b.t('历史记录') }}</h1>
          <label class="history-search">
            <Icon name="search" :size="16" /><input ref="search" v-model="query" type="search" maxlength="500"
              :aria-label="b.t('搜索历史记录')" :placeholder="b.t('搜索历史记录')" @input="searchChanged" @focus="selection = false" />
          </label>
          <div class="history-actions">
            <button v-if="b.canUndoHistory" class="history-button" :disabled="editing" @click="change({ action: 'undo' })">{{ b.t('撤销') }}</button>
            <div class="history-clear">
              <button ref="clearButton" class="history-button" aria-haspopup="menu" :aria-expanded="menuOpen" :disabled="editing"
                @click="setMenu(!menuOpen, $event.detail === 0)" @keydown.down.prevent="setMenu(true, true)">
                {{ b.t('清除历史记录') }}<Icon name="chevronDown" :size="12" />
              </button>
              <div v-if="menuOpen" ref="menu" role="menu" :aria-label="b.t('清除历史记录')" class="history-menu" @keydown="menuKey">
                <button v-for="[range, label] in ranges" :key="range" role="menuitem" :class="{ 'history-danger': range === 'all' }"
                  @click="startConfirm({ range })">{{ b.t(label) }}</button>
              </div>
            </div>
          </div>
        </div>
      </header>

      <div class="history-list" :aria-busy="loading || editing">
        <details v-if="!query.trim() && recent.length" class="history-recent">
          <summary><Icon name="chevronDown" :size="12" /><Icon name="history" :size="16" />{{ b.t('最近关闭的标签页') }}</summary>
          <button v-for="tab in recent" :key="tab.id" class="history-row history-recent-tab" @click="b.send({ type: 'restore', id: tab.id })">
            <span class="history-site-icon"><Favicon v-if="icons.get(host(tab.url))" :url="tab.url" :favicon="icons.get(host(tab.url))" /><Icon v-else name="globe" :size="16" /></span>
            <span class="history-title">{{ tabDisplayTitle(tab, b.language) }}</span>
            <span class="history-address">{{ displayUrl(tab.url) }}</span>
          </button>
        </details>

        <p v-if="error" class="history-status" role="alert">{{ error }} <button class="history-button" @click="request()">{{ b.t('刷新') }}</button></p>
        <p class="history-feedback" role="status">{{ notice }}</p>
        <div v-if="!sessions.length" class="history-empty">
          <Icon name="history" :size="36" />
          <p v-if="loading">{{ b.t('正在加载历史记录…') }}</p>
          <p v-else-if="!error">{{ b.t(resultQuery ? '没有匹配的历史记录' : '访问过的网页会显示在这里。') }}</p>
        </div>
        <section v-for="session in sessions" :key="session.clusters[0]!.items[0]!.url" class="history-session">
          <header class="history-session-head">
            <h2>{{ sessionLabel(session.start) }}</h2>
            <span v-if="session.clusters.length > 1">{{ count(session.count) }}</span>
            <button class="history-remove" :aria-label="b.t('移除此浏览会话')" :title="b.t('移除此浏览会话')"
              :disabled="editing" @click="remove(session.clusters.flatMap((cluster) => cluster.items))"><Icon name="close" :size="12" /></button>
          </header>
          <div class="history-session-rows" :class="{ 'history-selected': selection }">
            <template v-for="cluster in session.clusters" :key="clusterKey(cluster)">
              <HistoryRow v-if="cluster.items.length === 1" :visit="cluster.items[0]!" :time="time(cluster.items[0]!)"
                :favicon="icons.get(cluster.host)" :disabled="editing"
                @open="openVisit(cluster.items[0]!, $event)" @remove="remove(cluster.items)" />
              <template v-else>
                <div class="history-row">
                  <button class="history-row-main history-cluster" data-history-row :aria-expanded="expanded(cluster)" aria-keyshortcuts="Delete"
                    @click="toggle(cluster)" @keydown.delete.exact="remove(cluster.items)" @keydown.backspace.exact="remove(cluster.items)">
                    <Icon name="chevronDown" class="history-disclosure" :class="{ open: expanded(cluster) }" :size="12" />
                    <span class="history-site-icon"><Favicon v-if="icons.get(cluster.host)" :url="cluster.items[0]!.url" :favicon="icons.get(cluster.host)" /><Icon v-else name="globe" :size="16" /></span>
                    <span class="history-title">{{ cluster.host }}</span>
                    <span class="history-address">{{ count(cluster.items.length) }}</span>
                    <span class="history-time">{{ span(cluster) }}</span>
                  </button>
                  <button class="history-remove" :aria-label="b.t('移除 {count} 个页面', { count: cluster.items.length })"
                    :title="b.t('移除 {count} 个页面', { count: cluster.items.length })" :disabled="editing"
                    @click="remove(cluster.items)"><Icon name="close" :size="12" /></button>
                </div>
                <template v-if="expanded(cluster)">
                  <HistoryRow v-for="visit in cluster.items.slice(0, shown[clusterKey(cluster)] ?? 100)" :key="visit.url"
                    :visit="visit" :time="time(visit)" :favicon="icons.get(cluster.host)" :disabled="editing" indented
                    @open="openVisit(visit, $event)" @remove="remove([visit])" />
                  <button v-if="cluster.items.length > (shown[clusterKey(cluster)] ?? 100)" class="history-more-sites"
                    @click="shown[clusterKey(cluster)] = (shown[clusterKey(cluster)] ?? 100) + 100">{{ b.t('显示更多页面') }}</button>
                </template>
              </template>
            </template>
          </div>
        </section>
        <button v-if="cursor" ref="more" class="history-button history-more" :disabled="loading || editing" @click="loadMore">{{ b.t('加载更多历史记录') }}</button>
      </div>
    </div>

    <div v-if="confirm" class="history-confirm-backdrop" @pointerdown.self="closeConfirm">
      <section class="history-confirm" role="alertdialog" aria-modal="true" :aria-label="confirmTitle"
        @keydown.esc.stop.prevent="closeConfirm" @keydown.tab.prevent="cycleConfirmFocus">
        <h2>{{ confirmTitle }}</h2>
        <p>{{ b.t('所选网页会从此设备的历史记录和地址栏建议中移除。') }}</p>
        <div>
          <button ref="cancel" class="history-button history-cancel" @click="closeConfirm">{{ b.t('取消') }}</button>
          <button ref="confirmButton" class="history-button history-danger" @click="confirmChange">{{ b.t('确定移除') }}</button>
        </div>
      </section>
    </div>
  </section>
</template>
