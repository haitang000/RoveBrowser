<script setup lang="ts">
import { computed } from "vue";
import type { ExtensionProgress, ExtensionStep } from "../../lib/extensions";
import { useBrowser } from "../../stores/browser";

const props = defineProps<{ progress: ExtensionProgress }>();
const b = useBrowser();
const labels: Record<ExtensionStep, string> = {
  download: "下载扩展",
  verifySignature: "验证商店签名",
  unpack: "解包扩展",
  copyFiles: "复制扩展文件",
  readManifest: "读取扩展清单",
  install: "安装扩展",
  remove: "卸载扩展",
  enable: "启用扩展",
  disable: "停用扩展",
};

const rows = computed(() => {
  const current = props.progress.steps.indexOf(props.progress.current);
  return props.progress.steps.map((step, index) => {
    const value = index < current ? 100 : index === current ? props.progress.percent ?? undefined : 0;
    const status = index < current ? b.t("已完成") : index > current ? b.t("等待中")
      : value === undefined ? b.t("进行中") : `${value}%`;
    return { step, value, status };
  });
});
</script>

<template>
  <div class="extension-progress">
    <div class="extension-progress-announcement" role="status">
      {{ b.t('{step} · 进行中', { step: b.t(labels[progress.current]) }) }}
    </div>
    <ol class="extension-progress-steps">
      <li v-for="row in rows" :key="row.step" class="extension-progress-step"
        :aria-current="row.step === progress.current ? 'step' : undefined">
        <div class="extension-progress-label" aria-hidden="true">
          <span>{{ b.t(labels[row.step]) }}</span>
          <span>{{ row.status }}</span>
        </div>
        <div class="extension-progress-track" :class="{ 'is-indeterminate': row.value === undefined }"
          role="progressbar" :aria-label="b.t(labels[row.step])" :aria-valuenow="row.value" :aria-valuetext="row.status">
          <div class="extension-progress-fill" :style="{ '--extension-progress': (row.value ?? 0) / 100 }" />
        </div>
      </li>
    </ol>
  </div>
</template>

<style scoped>
.extension-progress-announcement {
  position: absolute;
  width: 1px;
  height: 1px;
  overflow: hidden;
  clip-path: inset(50%);
  white-space: nowrap;
}
.extension-progress-steps { display: grid; gap: 10px; margin: 0; padding: 0; list-style: none; }
.extension-progress-step { display: grid; gap: 5px; }
.extension-progress-label { display: flex; justify-content: space-between; gap: 12px; font-size: 12px; }
.extension-progress-label span + span { color: var(--quiet); font-variant-numeric: tabular-nums; white-space: nowrap; }
.extension-progress-track { height: 4px; overflow: hidden; border-radius: 2px; background: var(--separator); }
.extension-progress-fill {
  height: 100%;
  border-radius: inherit;
  background: var(--accent);
  transform: scaleX(var(--extension-progress));
  transform-origin: left;
  transition: transform 160ms linear;
}
.is-indeterminate .extension-progress-fill {
  width: 36%;
  transform: none;
  transition: none;
  animation: extension-progress-slide 1.2s linear infinite;
}
@keyframes extension-progress-slide {
  from { transform: translateX(-100%); }
  to { transform: translateX(278%); }
}
@media (prefers-reduced-motion: reduce) {
  .extension-progress-fill { transition: none; }
  .is-indeterminate .extension-progress-fill { animation: none; }
}
:global(:root[data-reduce-motion="true"]) .extension-progress-fill { transition: none; animation: none; }
</style>
