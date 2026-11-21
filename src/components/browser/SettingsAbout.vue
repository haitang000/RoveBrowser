<script setup lang="ts">
import { computed } from "vue";
import roveIcon from "../../../native/assets/icon.png";
import { UPDATE_CHANNELS, updateChannelDetail } from "../../lib/settings";
import { useBrowser } from "../../stores/browser";
import Icon from "../common/Icon.vue";

const props = defineProps<{ tabId: string }>();
const emit = defineEmits<{ confirmUpdate: [offerId: string] }>();
const b = useBrowser();
const version = __APP_VERSION__;
const statusIcon = computed(() => {
  if (b.updating) return "spinner";
  switch (b.updateStatus.phase) {
    case "available": return "download";
    case "current": return "check";
    case "error": return "warning";
    default: return "reload";
  }
});
const channelId = computed(() => `update-channel-${props.tabId}`);

function changeChannel(event: Event) {
  const channel = (event.target as HTMLSelectElement).value;
  b.send({ type: "settingsUpdate", settings: { ...b.settings, updateChannel: channel } });
}
</script>

<template>
  <div class="settings-about">
    <section class="about-card" aria-label="版本与软件更新">
      <div class="about-brand">
        <img :src="roveIcon" alt="" width="52" height="52" draggable="false" />
        <div>
          <h3>Rove</h3>
          <p>版本 {{ version }} <span aria-hidden="true">·</span> Windows / WebView2</p>
        </div>
      </div>

      <div class="about-update" :data-phase="b.updateStatus.phase">
        <span class="about-update-icon">
          <Icon :name="statusIcon" :size="19" :class="{ 'reload-spinner': b.updating }" />
        </span>
        <div class="setting-description">
          <strong>软件更新</strong>
          <p role="status">{{ b.updateStatus.message || "检查所选通道是否有新版本。" }}</p>
        </div>
        <button
          v-if="b.updateStatus.phase === 'available' && b.updateStatus.offerId"
          class="primary-button"
          @click="emit('confirmUpdate', b.updateStatus.offerId)"
        >
          更新并重启…
        </button>
        <button v-else class="setting-button" :disabled="b.updating" @click="b.checkUpdates()">
          检查更新
        </button>
      </div>

      <div class="setting-row about-channel">
        <div class="setting-description">
          <label :for="channelId">更新通道</label>
          <p :id="`${channelId}-detail`">{{ updateChannelDetail(b.settings.updateChannel) }}</p>
        </div>
        <select
          :id="channelId"
          :aria-describedby="`${channelId}-detail`"
          :disabled="b.updating"
          :value="b.settings.updateChannel"
          @change="changeChannel"
        >
          <option v-for="[id, title] in UPDATE_CHANNELS" :key="id" :value="id">{{ title }}</option>
        </select>
      </div>
    </section>

    <section class="about-card" aria-label="欢迎引导">
      <div class="setting-row">
        <div class="setting-description">
          <strong>欢迎引导</strong>
          <p>重新选择常用网站，或了解首次使用流程。</p>
        </div>
        <button class="setting-button" aria-label="打开欢迎引导" @click="b.openOnboarding()">打开</button>
      </div>
    </section>
  </div>
</template>
