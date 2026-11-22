<script setup lang="ts">
import { displayUrl } from "../../lib/command";
import { useBrowser } from "../../stores/browser";
import type { HistoryVisit } from "../../lib/history";
import Favicon from "../common/Favicon.vue";
import Icon from "../common/Icon.vue";

defineProps<{ visit: HistoryVisit; time: string; favicon?: string; indented?: boolean; disabled?: boolean; showCount?: boolean }>();
defineEmits<{ open: [event: MouseEvent]; remove: [] }>();
const b = useBrowser();
</script>

<template>
  <div class="history-row" :class="{ 'history-indent': indented }">
    <a class="history-row-main" :href="visit.url" :title="visit.url" data-history-row
      aria-keyshortcuts="Delete" @click.prevent="$emit('open', $event)" @auxclick.middle.prevent="$emit('open', $event)"
      @keydown.delete.exact="!disabled && $emit('remove')" @keydown.backspace.exact="!disabled && $emit('remove')">
      <span class="history-site-icon">
        <Favicon v-if="favicon" :url="visit.url" :favicon="favicon" /><Icon v-else name="globe" :size="16" />
      </span>
      <span class="history-title">{{ visit.title.trim() || displayUrl(visit.url) }}</span>
      <span class="history-address">{{ showCount ? b.t('访问 {count} 次', { count: visit.visitCount }) : displayUrl(visit.url) }}</span>
      <time class="history-time" :datetime="new Date(visit.visited * 1000).toISOString()">{{ time }}</time>
    </a>
    <button class="history-remove" :aria-label="b.t('移除 {title}', { title: visit.title.trim() || displayUrl(visit.url) })"
      :title="b.t('从历史记录移除')" :disabled="disabled" @click="$emit('remove')"><Icon name="close" :size="12" /></button>
  </div>
</template>
