<script setup lang="ts">
import { computed, onMounted, onBeforeUnmount, ref } from "vue";
import { useBrowser } from "../../stores/browser";
import { liveTab } from "../../lib/bookmarks";
import type { Tab } from "../../lib/types";
import Icon from "../common/Icon.vue";
import PinnedTabs from "./PinnedTabs.vue";
import TabList from "./TabList.vue";

const props = defineProps<{ spaceId: string; savedScroll: number }>();
const emit = defineEmits<{ menu: [event: MouseEvent, tab: Tab]; scroll: [id: string, top: number] }>();
const b = useBrowser(), list = ref<HTMLElement>();
const space = computed(() => b.state.spaces.find((s) => s.id === props.spaceId));
const tabs = computed(() => b.state.tabs.filter((t) => t.spaceId === props.spaceId && !t.pinned && !b.state.favorites.some((f) => f.id === t.bookmarkId)));
const pins = computed<Tab[]>(() => (b.state.pins || []).filter((f) => f.spaceId === props.spaceId)
  .map((f) => ({ ...(liveTab(b.state, f.id) || { id: f.id, spaceId: props.spaceId, url: f.url, unloaded: true }), title: f.title, pinned: true, bookmarkId: f.id })));
onMounted(() => { if (list.value) list.value.scrollTop = props.savedScroll; });
onBeforeUnmount(() => { if (list.value) emit("scroll", props.spaceId, list.value.scrollTop); });
</script>

<template>
  <section ref="list" class="space-list" :data-space-id="spaceId" @scroll="emit('scroll', spaceId, list!.scrollTop)">
    <h1 class="space-name">{{ space?.name }}</h1>
    <PinnedTabs :tabs="pins" :space-id="spaceId" @menu="(e, t) => emit('menu', e, t)" />
    <div class="section-label"><span>Tabs</span><span>{{ tabs.length }}</span></div>
    <button class="new-tab" @click="b.openCommand()"><Icon name="plus" :size="16" /><span>New Tab</span><kbd>Ctrl T</kbd></button>
    <TabList :tabs="tabs" aria-label="标签页" @menu="(e, t) => emit('menu', e, t)" />
  </section>
</template>
