<script setup lang="ts">
// State/actions belong to the host; the surface also runs in the native popup.
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useBrowser } from "../../stores/browser";
import { clearableDownloads, downloadFraction, downloadPercent, downloadStatus, isActiveDownload, isFinalizingDownload } from "../../lib/downloads";
import Icon from "../common/Icon.vue";
import DownloadFileIcon from "./DownloadFileIcon.vue";

const b = useBrowser();
const panel = ref<HTMLElement>();
const list = ref<HTMLElement>();
const search = ref<HTMLInputElement>();
const more = ref<HTMLButtonElement>();
const clear = ref<HTMLButtonElement>();
const searching = ref(false);
const query = ref("");
const menuOpen = ref(false);
const viewport = ref({ width: innerWidth, height: innerHeight });
const clearable = computed(() => clearableDownloads(b.downloads).length);
const filtered = computed(() => b.downloads.filter((item) => item.filename.toLocaleLowerCase().includes(query.value.trim().toLocaleLowerCase())));
const position = computed(() => {
  const width = Math.min(380, viewport.value.width - 24);
  const anchor = b.downloadsAnchor;
  const bottom = Math.max(24, viewport.value.height - (anchor?.y ?? viewport.value.height - 42) + 10);
  return {
    left: "12px", width: `${width}px`, bottom: `${bottom}px`,
    maxHeight: `${Math.max(100, Math.min(520, viewport.value.height - bottom - 40))}px`,
    "--downloads-tip": `${Math.max(24, Math.min(width - 24, (anchor ? anchor.x + anchor.width / 2 : 88) - 12))}px`,
  };
});
function resize() { viewport.value = { width: innerWidth, height: innerHeight }; }
function act(action: string, id?: string) {
  menuOpen.value = false;
  // Release the native popup so WebView2's security prompt can receive input.
  if (action === "review") b.closeDownloads();
  b.downloadAction(action, id);
}
function close() { b.closeDownloads(); }
async function toggleMenu() {
  menuOpen.value = !menuOpen.value;
  if (menuOpen.value) {
    await nextTick();
    if (clearable.value) clear.value?.focus();
  }
}
function closeMenu() { menuOpen.value = false; more.value?.focus(); }
function panelPointerDown(event: PointerEvent) {
  if (!(event.target as Element).closest(".downloads-more")) menuOpen.value = false;
}
async function toggleSearch() {
  searching.value = !searching.value;
  menuOpen.value = false;
  if (searching.value) { await nextTick(); search.value?.focus(); }
  else query.value = "";
}
function outside(event: PointerEvent) {
  if (!b.downloadsOpen) return;
  const target = event.target as Element | null;
  if (target && (panel.value?.contains(target) || target.closest(".downloads-button"))) return;
  close();
}
onMounted(() => {
  window.addEventListener("resize", resize);
  document.addEventListener("pointerdown", outside, true);
});
onBeforeUnmount(() => {
  window.removeEventListener("resize", resize);
  document.removeEventListener("pointerdown", outside, true);
});
watch(() => b.downloadsOpen, async (open) => {
  if (!open) return;
  query.value = ""; searching.value = false; menuOpen.value = false;
  await nextTick();
  panel.value?.focus({ preventScroll: true });
});
watch(() => b.downloadsFocus, async () => {
  query.value = "";
  await nextTick();
  list.value?.scrollTo({ top: 0 });
});
</script>
<template>
  <Transition name="downloads">
    <div v-if="b.downloadsOpen" class="downloads-layer" @pointerdown.self="close">
      <section
        id="downloads-panel" ref="panel" class="downloads-panel" :style="position"
        role="dialog" aria-label="下载" tabindex="-1"
        @wheel.stop @keydown.esc.stop.prevent="close"
        @pointerdown="panelPointerDown"
      >
        <header class="downloads-header">
          <h2 class="downloads-title">下载</h2>
          <div class="downloads-header-actions">
            <button class="download-action" title="打开下载文件夹" aria-label="打开下载文件夹" @click="act('openFolder')"><Icon name="folder" :size="18" /></button>
            <button class="download-action" title="搜索下载" aria-label="搜索下载" :aria-pressed="searching" @click="toggleSearch"><Icon name="search" :size="18" /></button>
            <div class="downloads-more">
              <button ref="more" class="download-action" title="更多下载操作" aria-label="更多下载操作" aria-haspopup="menu" :aria-expanded="menuOpen" @click="toggleMenu" @keydown.down.prevent="!menuOpen && toggleMenu()"><Icon name="more" :size="18" /></button>
              <div v-if="menuOpen" class="downloads-menu" role="menu" aria-label="下载操作" @keydown.esc.stop.prevent="closeMenu">
                <button ref="clear" role="menuitem" :disabled="!clearable" @click="act('clearFinished'); more?.focus()"><Icon name="trash" :size="15" />清除已结束的下载</button>
              </div>
            </div>
            <button class="download-action" aria-label="关闭下载面板" title="关闭" @click="close"><Icon name="close" :size="16" /></button>
          </div>
        </header>
        <div v-if="searching" class="downloads-search">
          <Icon name="search" :size="15" />
          <input ref="search" v-model="query" aria-label="搜索下载文件" placeholder="搜索下载文件" type="search" />
        </div>
        <div v-if="!b.downloads.length" class="downloads-empty"><Icon name="download" :size="30" /><span>暂无下载</span></div>
        <div v-else-if="!filtered.length" class="downloads-empty"><Icon name="search" :size="28" /><span>未找到匹配的下载</span></div>
        <div v-else ref="list" class="downloads-list" aria-label="下载文件">
          <article v-for="item in filtered" :key="item.id" class="download-row"
            :class="[item.state, { focus: b.downloadsFocus === item.id, missing: item.state === 'finished' && !item.exists }]"
          >
            <DownloadFileIcon :filename="item.filename" />
            <div class="download-body">
              <span class="download-name" :title="item.filename">{{ item.filename }}</span>
              <div v-if="isActiveDownload(item)" class="download-bar" :class="{ indeterminate: !item.total }"
                role="progressbar" :aria-label="`${item.filename} 下载进度`" :aria-valuenow="item.total > 0 ? downloadPercent(item) : undefined" :aria-valuemin="0" :aria-valuemax="100" :aria-valuetext="downloadStatus(item)">
                <span :style="{ transform: `scaleX(${item.total > 0 ? downloadFraction(item) : 0.3})` }" />
              </div>
              <span class="download-meta" :class="{ caution: item.state === 'failed' }" :title="downloadStatus(item)">{{ downloadStatus(item) }}</span>
              <button v-if="isFinalizingDownload(item)" class="download-open" @click="act('review', item.id)">查看下载提示</button>
              <button v-if="item.state === 'finished' && item.exists" class="download-open" @click="act('open', item.id)">打开文件</button>
            </div>
            <div class="download-actions">
              <template v-if="isActiveDownload(item)">
                <button class="download-action" :title="item.state === 'paused' ? '继续下载' : '暂停下载'" :aria-label="item.state === 'paused' ? '继续下载' : '暂停下载'" @click="act(item.state === 'paused' ? 'resume' : 'pause', item.id)"><Icon :name="item.state === 'paused' ? 'play' : 'pause'" :size="17" /></button>
                <button class="download-action" title="取消下载" aria-label="取消下载" @click="act('cancel', item.id)"><Icon name="close" :size="17" /></button>
              </template>
              <template v-else>
                <button v-if="item.state === 'finished' && item.exists" class="download-action" title="在文件夹中显示" aria-label="在文件夹中显示" @click="act('reveal', item.id)"><Icon name="folder" :size="18" /></button>
                <button class="download-action" title="从列表中移除" aria-label="从列表中移除" @click="act('remove', item.id)"><Icon name="trash" :size="18" /></button>
              </template>
            </div>
          </article>
        </div>
      </section>
    </div>
  </Transition>
</template>
