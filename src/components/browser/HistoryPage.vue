<script setup lang="ts">
// The session and site-cluster interactions follow Zigzag's
// HistoryPage.swift, Copyright 2026 Zigao Wang, Apache-2.0.
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { host } from "../../lib/address";
import { displayUrl } from "../../lib/command";
import { compareHistory, groupHistory, groupHistoryBySite, historyBounds, historyMatches, historyPeriod, type HistoryCluster, type HistoryCursor, type HistoryEdit, type HistoryFilters as FilterOptions, type HistoryQuery, type HistoryVisit } from "../../lib/history";
import { tabDisplayTitle } from "../../lib/tab-meta";
import { useBrowser } from "../../stores/browser";
import Favicon from "../common/Favicon.vue";
import Icon from "../common/Icon.vue";
import HistoryFilters from "./HistoryFilters.vue";
import HistoryRow from "./HistoryRow.vue";
import "../../styles/history.css";

const props = defineProps<{ tabId: string }>();
const b = useBrowser();
const scroller = ref<HTMLElement>(), search = ref<HTMLInputElement>(), more = ref<HTMLButtonElement>();
const cancel = ref<HTMLButtonElement>(), confirmButton = ref<HTMLButtonElement>();
const pageRows = ref<HistoryVisit[]>([]);
const cursor = ref<HistoryCursor | null>(null);
const openClusters = ref<Record<string, boolean>>({});
const shown = ref<Record<string, number>>({});
const query = ref(""), resultQuery = ref(""), loading = ref(true), error = ref(""), notice = ref("");
const selection = ref(false);
const confirm = ref<{ items: HistoryVisit[] } | { all: true }>();
const filters = ref<FilterOptions>({ period: null, sort: "date", source: "history" });
let read: { id: string; append: boolean; query: string; openFirst?: boolean } | undefined;
let editId: string | undefined;
const editing = ref(false);
let timer: ReturnType<typeof setTimeout>;
let previousFocus: HTMLElement | null = null;

const showClosed = computed(() => filters.value.source === "closed");
const sessions = computed(() => {
  if (showClosed.value || !pageRows.value.length) return [];
  if (filters.value.sort === "date") return groupHistory(pageRows.value);
  const clusters = filters.value.sort === "site"
    ? groupHistoryBySite(pageRows.value)
    : pageRows.value.map((visit) => ({ host: host(visit.url), items: [visit] }));
  return [{ start: undefined, clusters, count: pageRows.value.length }];
});
const closed = computed(() => b.state.closed.filter((tab) => host(tab.url)).reverse());
const closedRows = computed(() => {
  const counts = showClosed.value && filters.value.sort === "popular"
    ? new Map(pageRows.value.map((visit) => [visit.url, visit.visitCount])) : undefined;
  const entries = closed.value.map((tab) => {
    const visited = tab.lastActivated ?? 0;
    return {
      id: tab.id, url: tab.url, title: tabDisplayTitle(tab, b.language),
      visited: Number.isFinite(new Date(visited * 1000).getTime()) ? visited : 0,
      visitCount: counts?.get(tab.url) ?? 0,
    };
  });
  if (!showClosed.value) return !query.value.trim() && !filters.value.period && filters.value.sort === "date" ? entries.slice(0, 10) : [];
  const words = query.value.toLowerCase().split(/\s+/).filter(Boolean);
  const { since, until } = historyBounds(filters.value.period);
  return entries.filter((entry) => historyMatches(entry, words)
    && (since === undefined || entry.visited >= since)
    && (until === undefined || entry.visited < until))
    .sort((a, b) => compareHistory(a, b, filters.value.sort));
});
const hasResults = computed(() => showClosed.value ? closedRows.value.length > 0 : sessions.value.length > 0);
const icons = computed(() => {
  const cached = new Map<string, string>();
  for (const tab of [...b.state.tabs, ...closed.value, ...b.state.favorites, ...(b.state.pins ?? [])]) {
    if (tab.favicon?.startsWith("data:image/")) cached.set(host(tab.url), tab.favicon);
  }
  return cached;
});
const timeFormat = computed(() => new Intl.DateTimeFormat(b.language, {
  hour: "numeric", minute: "2-digit",
  ...(showClosed.value || filters.value.sort !== "date" ? { month: "numeric", day: "numeric" } : {}),
}));
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
function expanded(cluster: HistoryCluster) { return openClusters.value[clusterKey(cluster)] ?? Boolean(resultQuery.value || filters.value.sort === "site"); }
function toggle(cluster: HistoryCluster) { openClusters.value[clusterKey(cluster)] = !expanded(cluster); }
function count(items: number) { return b.t(items === 1 ? "1 个页面" : "{count} 个页面", { count: items }); }

function request(append = false) {
  clearTimeout(timer);
  const value = query.value.trim();
  error.value = "";
  if (showClosed.value && (filters.value.sort !== "popular" || !closed.value.length)) {
    read = undefined;
    resultQuery.value = value;
    loading.value = false;
    return;
  }
  const id = crypto.randomUUID();
  read = { id, append, query: value };
  loading.value = true;
  const options: HistoryQuery = showClosed.value
    ? { query: "", sort: "popular", urls: closed.value.map((tab) => tab.url) }
    : { query: value, sort: filters.value.sort, ...historyBounds(filters.value.period), before: append ? cursor.value ?? undefined : undefined };
  b.send({ type: "historyPage", ticket: id, ...options });
}
function applyFilters(value: FilterOptions) {
  filters.value = value;
  pageRows.value = [];
  cursor.value = null;
  selection.value = false;
  openClusters.value = {};
  shown.value = {};
  if (scroller.value) scroller.value.scrollTop = 0;
  request();
}
function loadMore() {
  if (cursor.value && !loading.value && !editing.value) request(true);
}
function searchChanged(event: Event) {
  if (event instanceof InputEvent && event.isComposing) return;
  selection.value = false;
  notice.value = "";
  if (showClosed.value) return;
  read = undefined;
  loading.value = true;
  clearTimeout(timer);
  timer = setTimeout(request, 120);
}
function openVisit(visit: HistoryVisit, event?: MouseEvent) {
  if (selection.value) selection.value = false;
  if (event?.ctrlKey || event?.metaKey || event?.button === 1) b.send({ type: "new", url: visit.url });
  else b.send({ type: "navigate", id: props.tabId, url: visit.url });
}
function restore(id: string) { b.send({ type: "restore", id }); }
function openFirstResult() {
  if (error.value) return;
  if (showClosed.value) {
    if (closedRows.value[0]) restore(closedRows.value[0].id);
  } else if (pageRows.value[0]) openVisit(pageRows.value[0]);
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
  confirm.value = value;
  void nextTick(() => cancel.value?.focus());
}
function closeConfirm() {
  confirm.value = undefined;
  void nextTick(() => {
    if (previousFocus?.isConnected) previousFocus.focus();
    else search.value?.focus();
  });
}
const confirmTitle = computed(() => {
  if (!confirm.value) return "";
  if ("items" in confirm.value) return b.t("移除这 {count} 个页面？", { count: confirm.value.items.length });
  return b.t("清除全部历史记录？");
});
function confirmChange() {
  const pending = confirm.value;
  if (!pending) return;
  closeConfirm();
  if ("all" in pending) change({ action: "clear", since: null });
  else change({ action: "remove", urls: pending.items.map((item) => item.url) });
}
function cycleConfirmFocus() {
  (document.activeElement === cancel.value ? confirmButton.value : cancel.value)?.focus();
}
function key(event: KeyboardEvent) {
  if (event.defaultPrevented || event.isComposing || event.keyCode === 229 || confirm.value
    || (event.target instanceof Element && event.target.closest(".history-filter"))) return;
  const isSearch = event.target === search.value;
  if (event.ctrlKey && !event.shiftKey && !event.altKey && !event.metaKey) {
    if (event.key.toLowerCase() === "a" && !showClosed.value && pageRows.value.length && !loading.value && (!isSearch || !query.value)) {
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
    startConfirm(resultQuery.value || filters.value.period ? { items: pageRows.value } : { all: true });
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
    else openFirstResult();
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
  if (openFirst) openFirstResult();
  else if (hadFocus) void nextTick(() => {
    if (document.activeElement === document.body) scroller.value?.focus({ preventScroll: true });
  });
}, { flush: "sync" });
watch(() => b.historyVersion, () => { selection.value = false; request(); }, { immediate: true });
watch(() => closed.value.map((tab) => tab.id).join(","), () => {
  if (showClosed.value && filters.value.sort === "popular") request();
});
let observer: IntersectionObserver | undefined;
onMounted(() => {
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
});
</script>

<template>
  <section class="history-page" :aria-label="b.t('历史记录')">
    <div ref="scroller" class="history-scroll" tabindex="-1" :inert="!!confirm" @keydown="key" @pointerdown="selection = false">
      <header class="history-header">
        <div class="history-header-inner">
          <h1><Icon name="history" :size="20" />{{ b.t('历史记录') }}</h1>
          <div class="history-tools">
            <label class="history-search">
              <Icon name="search" :size="16" /><input ref="search" v-model="query" type="search" maxlength="500"
                :aria-label="b.t('搜索历史记录')" :placeholder="b.t('搜索历史记录')" @input="searchChanged" @focus="selection = false" />
            </label>
            <button v-if="b.canUndoHistory" class="history-button" :disabled="editing" @click="change({ action: 'undo' })">{{ b.t('撤销') }}</button>
            <HistoryFilters :value="filters" :disabled="editing" @apply="applyFilters" />
          </div>
        </div>
      </header>

      <div class="history-list" :aria-busy="loading || editing">
        <component :is="showClosed ? 'section' : 'details'" v-if="closedRows.length && (!showClosed || !loading)" class="history-recent">
          <summary v-if="!showClosed"><Icon name="chevronDown" :size="12" /><Icon name="history" :size="16" />{{ b.t('最近关闭的标签页') }}</summary>
          <button v-for="tab in closedRows" :key="tab.id" class="history-row history-recent-tab" data-history-row @click="restore(tab.id)">
            <span class="history-site-icon"><Favicon v-if="icons.get(host(tab.url))" :url="tab.url" :favicon="icons.get(host(tab.url))" /><Icon v-else name="globe" :size="16" /></span>
            <span class="history-title">{{ tab.title || displayUrl(tab.url) }}</span>
            <span class="history-address" :title="tab.url">{{ showClosed && filters.sort === 'popular' && tab.visitCount ? b.t('访问 {count} 次', { count: tab.visitCount }) : displayUrl(tab.url) }}</span>
            <span v-if="showClosed && tab.visited > 0" class="history-time">{{ time(tab) }}</span>
          </button>
        </component>

        <p v-if="error" class="history-status" role="alert">{{ error }} <button class="history-button" @click="request()">{{ b.t('刷新') }}</button></p>
        <p class="history-feedback" role="status">{{ notice }}</p>
        <div v-if="!hasResults || (showClosed && loading)" class="history-empty">
          <Icon name="history" :size="36" />
          <p v-if="loading">{{ b.t('正在加载历史记录…') }}</p>
          <p v-else-if="!error">{{ b.t(showClosed ? '没有匹配的已关闭标签页' : resultQuery || filters.period ? '没有匹配的历史记录' : '访问过的网页会显示在这里。') }}</p>
        </div>
        <section v-for="session in sessions" :key="session.clusters[0]!.items[0]!.url" class="history-session">
          <header v-if="session.start" class="history-session-head">
            <h2>{{ sessionLabel(session.start) }}</h2>
            <span v-if="session.clusters.length > 1">{{ count(session.count) }}</span>
            <button class="history-remove" :aria-label="b.t('移除此浏览会话')" :title="b.t('移除此浏览会话')"
              :disabled="editing" @click="remove(session.clusters.flatMap((cluster) => cluster.items))"><Icon name="close" :size="12" /></button>
          </header>
          <div class="history-session-rows" :class="{ 'history-selected': selection }">
            <template v-for="cluster in session.clusters" :key="clusterKey(cluster)">
              <HistoryRow v-if="cluster.items.length === 1" :visit="cluster.items[0]!" :time="time(cluster.items[0]!)"
                :favicon="icons.get(cluster.host)" :disabled="editing" :show-count="filters.sort === 'popular'"
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
