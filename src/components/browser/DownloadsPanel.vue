<script setup lang="ts">
// Zigzag's downloads panel (Window/DownloadsPanel.swift, Apache-2.0), adapted
// to the sidebar lane: newest downloads first, a progress line while a file is
// moving, pause/resume/cancel, reveal once finished, and Clear that never
// throws away a half-finished file. The host owns the list; this component only
// renders it and asks for actions.
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useBrowser } from "../../stores/browser";
import {
  clearableDownloads,
  downloadFraction,
  downloadStatus,
} from "../../lib/downloads";
import type { DownloadItem } from "../../lib/types";
import Icon from "../common/Icon.vue";

const b = useBrowser();
const panel = ref<HTMLElement>();
const list = ref<HTMLElement>();
const clearable = computed(() => clearableDownloads(b.downloads).length);

function act(action: string, id?: string) {
  b.downloadAction(action, id);
}
function close() {
  b.closeDownloads();
}
function rowClick(item: DownloadItem) {
  if (item.state === "finished" && item.exists) act("open", item.id);
}
function outside(event: PointerEvent) {
  if (!b.downloadsOpen) return;
  const target = event.target as Element | null;
  if (target && (panel.value?.contains(target) || target.closest(".downloads-button"))) return;
  close();
}
onMounted(() => document.addEventListener("pointerdown", outside, true));
onBeforeUnmount(() => document.removeEventListener("pointerdown", outside, true));
// A download that lands while the panel is open should scroll its newest row
// into view, as the panel does when it reveals itself for a new download.
watch(
  () => b.downloadsFocus,
  async () => {
    await Promise.resolve();
    list.value?.scrollTo({ top: 0 });
  },
);
</script>
<template>
  <Transition name="downloads">
    <section
      v-if="b.downloadsOpen"
      ref="panel"
      class="downloads-panel"
      role="dialog"
      aria-label="下载"
      @wheel.stop
      @keydown.esc="close"
    >
      <header class="downloads-header">
        <span class="downloads-title">下载</span>
        <div class="downloads-header-actions">
          <button
            v-if="clearable"
            class="downloads-clear"
            title="清除已结束的下载"
            @click="act('clearFinished')"
          >
            清除
          </button>
          <button class="icon-button" aria-label="关闭下载面板" title="关闭" @click="close">
            <Icon name="close" :size="12" />
          </button>
        </div>
      </header>
      <div v-if="!b.downloads.length" class="downloads-empty">
        <Icon name="download" :size="26" />
        <span>暂无下载</span>
      </div>
      <div v-else ref="list" class="downloads-list">
        <div
          v-for="item in b.downloads"
          :key="item.id"
          class="download-row"
          :class="[item.state, { focus: b.downloadsFocus === item.id, missing: item.state === 'finished' && !item.exists }]"
          :title="`${item.filename}\n${item.path || item.url}`"
          @click="rowClick(item)"
        >
          <span class="download-icon"><Icon name="download" :size="14" /></span>
          <span class="download-body">
            <span class="download-name">{{ item.filename }}</span>
            <span
              class="download-meta"
              :class="{ caution: item.state === 'failed' }"
              :title="downloadStatus(item)"
              >{{ downloadStatus(item) }}</span
            >
            <span
              v-if="item.state === 'inProgress' || item.state === 'paused'"
              class="download-bar"
            >
              <span :style="{ width: `${downloadFraction(item) * 100}%` }" />
            </span>
          </span>
          <span class="download-actions">
            <button
              v-if="item.state === 'inProgress' || item.state === 'paused'"
              class="download-action"
              :title="item.state === 'paused' ? '继续下载' : '暂停下载'"
              :aria-label="item.state === 'paused' ? '继续下载' : '暂停下载'"
              @click.stop="act(item.state === 'paused' ? 'resume' : 'pause', item.id)"
            >
              <Icon :name="item.state === 'paused' ? 'play' : 'pause'" :size="11" />
            </button>
            <button
              v-if="item.state === 'inProgress' || item.state === 'paused'"
              class="download-action"
              title="取消下载"
              aria-label="取消下载"
              @click.stop="act('cancel', item.id)"
            >
              <Icon name="close" :size="11" />
            </button>
            <button
              v-else-if="item.state === 'finished' && item.exists"
              class="download-action reveal"
              title="在文件夹中显示"
              aria-label="在文件夹中显示"
              @click.stop="act('reveal', item.id)"
            >
              <Icon name="folder" :size="11" />
            </button>
            <button
              v-else
              class="download-action"
              title="从列表中移除"
              aria-label="从列表中移除"
              @click.stop="act('remove', item.id)"
            >
              <Icon name="close" :size="11" />
            </button>
          </span>
        </div>
      </div>
      <button class="downloads-folder" @click="act('openFolder')">
        <Icon name="folder" :size="13" />打开下载文件夹
      </button>
    </section>
  </Transition>
</template>
