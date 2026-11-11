<script setup lang="ts">
import { useBrowser } from "../../stores/browser";
import Icon from "../common/Icon.vue";
const b = useBrowser();
defineProps<{ swipeShares?: Record<string, number> | null }>();
</script>
<template>
  <footer class="workspace-switcher">
    <button
      class="icon-button"
      :title="b.shortcutTitle('设置', 'settings')"
      aria-label="设置"
      @click="b.openSettings()"
    ><Icon name="settings" :size="14" /></button>
    <button
      class="icon-button"
      :title="b.shortcutTitle('全部标签页', 'desk')"
      aria-label="全部标签页"
      @click="b.deskOpen = true"
    >
      <Icon name="grid" :size="14" />
    </button>
    <button
      class="icon-button downloads-button"
      :class="{ active: b.activeDownloads > 0 }"
      :title="b.activeDownloads ? `下载 · ${b.activeDownloads} 个进行中 (Ctrl+J)` : '下载 (Ctrl+J)'"
      aria-label="下载"
      @click="b.toggleDownloads()"
    >
      <Icon name="download" :size="14" />
      <span v-if="b.activeDownloads" class="downloads-badge">{{ b.activeDownloads }}</span>
    </button>
    <div class="space-keys" :class="{ 'space-keys-tracking': swipeShares }" aria-label="工作区" title="在侧栏左右轻扫切换工作区">
      <button
        v-for="(s, i) in b.state.spaces"
        :key="s.id"
        class="space-key"
        :class="{ current: s.id === b.state.activeSpace }"
        :style="swipeShares ? { '--space-share': swipeShares[s.id] || 0 } : undefined"
        :title="s.name"
        :aria-label="`工作区 ${s.name}`"
        :aria-pressed="s.id === b.state.activeSpace"
        @click="b.send({ type: 'space', id: s.id })"
        @dragover.prevent
        @drop.prevent="
          b.send({
            type: 'move',
            id: $event.dataTransfer?.getData('text/era-tab'),
            spaceId: s.id,
          });
          b.dragging = false;
        "
      >
        {{ i + 1 }}
      </button>
    </div>
    <button
      class="icon-button"
      title="工作区与外观"
      aria-label="工作区与外观"
      @click="b.settingsOpen = true"
    >
      <Icon name="plus" :size="14" />
    </button>
  </footer>
</template>
