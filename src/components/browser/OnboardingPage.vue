<script setup lang="ts">
// OnboardingView / HelloPage / StepPage / SitesPicker / ReadyPicker port.
// Copyright 2026 Zigao Wang, Apache-2.0; modified for Rove and Windows.
import { computed, nextTick, onMounted, ref, watch } from "vue";
import { useBrowser } from "../../stores/browser";
import { ONBOARDING_SITES, ONBOARDING_STEPS, otherFavorites, selectedSiteIds } from "../../lib/onboarding";
import { MAX_FAVORITES } from "../../lib/favorites";
import Icon from "../common/Icon.vue";
import DataImportPicker from "./DataImportPicker.vue";

const b = useBrowser();
const step = computed(() => b.state.onboarding?.step || "hello");
const index = computed(() => ONBOARDING_STEPS.indexOf(step.value));
const heading = ref<HTMLElement>();
const picked = ref<string[]>(selectedSiteIds(b.state));
const makeDefault = ref(true);
const limitNotice = ref("");
const busy = ref(false);
const importer = ref<InstanceType<typeof DataImportPicker>>();
const remaining = computed(() => Math.max(0, MAX_FAVORITES - otherFavorites(b.state).length));
const titles = {
  hello: "欢迎使用 Rove", bring: "把熟悉的内容带过来", sites: "置顶你常用的网站", ads: "关于广告拦截", ready: "将 Rove 设为默认浏览器？",
};
const subtitles = {
  hello: "", bring: "把书签、历史记录和密码带到 Rove。原来的浏览器保持原样。", sites: "它们会一直留在侧栏顶部，随时都能打开。", ads: "了解当前的广告拦截支持情况。", ready: "在 Windows 默认应用设置中选择链接的打开方式。",
};
const primaryTitle = computed(() => step.value === "hello" ? "开始使用" : step.value === "ready" ? "开始浏览" : step.value === "bring" && importer.value?.canImport ? "导入所选数据" : "继续");
watch(step, async () => {
  busy.value = false; limitNotice.value = "";
  if (step.value === "sites") picked.value = selectedSiteIds(b.state);
  await nextTick(); heading.value?.focus({ preventScroll: true });
});
// A native rejection leaves the current step in place and permits retry.
watch(() => b.error, (value) => { if (value) busy.value = false; });
function select(id: string) {
  limitNotice.value = "";
  if (picked.value.includes(id)) picked.value = picked.value.filter((item) => item !== id);
  else if (picked.value.length < remaining.value) picked.value.push(id);
  else limitNotice.value = `最多置顶 ${MAX_FAVORITES} 个网站，请先取消一个已选网站。`;
}
function advance(skip = false) {
  if (busy.value || b.importBusy) return;
  if (step.value === "bring" && !skip && importer.value?.canImport) { importer.value.run(); return; }
  busy.value = true;
  b.send({ type: "onboardingAdvance", step: step.value, skip, sites: [...picked.value], makeDefault: step.value === "ready" && makeDefault.value && b.native });
  if (!b.native) busy.value = false;
}
function back() {
  if (!busy.value && !b.importBusy && index.value > 0) b.send({ type: "onboardingBack", step: step.value });
}
function key(event: KeyboardEvent) {
  if (event.defaultPrevented || event.isComposing || event.ctrlKey || event.altKey || event.metaKey) return;
  const target = event.target as HTMLElement;
  if (target.closest('input, textarea, select, [contenteditable="true"]')) return;
  if (event.key === "Escape" && index.value > 0) { event.preventDefault(); back(); }
  else if (event.key === "Enter" && !target.closest("button, a")) { event.preventDefault(); advance(); }
}
function focusHeading() { heading.value?.focus({ preventScroll: true }); }
onMounted(focusHeading);
</script>

<template>
  <main class="onboarding" :data-step="step" aria-label="Rove 欢迎引导" @keydown="key">
    <div v-if="step !== 'hello'" class="onboarding-top">
      <ol class="onboarding-progress" aria-label="引导进度">
        <li v-for="(name, i) in ONBOARDING_STEPS.slice(1)" :key="name" :class="{ current: index === i + 1, done: index > i + 1 }" :aria-current="index === i + 1 ? 'step' : undefined">
          <span class="sr-only">第 {{ i + 1 }} 步，共 4 步：{{ titles[name] }}</span>
        </li>
      </ol>
      <button v-if="step !== 'ready'" class="onboarding-quiet onboarding-skip" :disabled="busy || b.importBusy" @click="advance(true)">跳过</button>
    </div>
    <Transition name="onboarding-page" mode="out-in" @after-enter="focusHeading">
      <section v-if="step === 'hello'" key="hello" class="onboarding-hello" aria-labelledby="onboarding-title">
        <p class="onboarding-welcome">欢迎使用</p>
        <h1 id="onboarding-title" ref="heading" class="onboarding-wordmark" tabindex="-1">Rove</h1>
        <button class="onboarding-primary onboarding-start" @click="advance()">开始使用 <Icon name="arrow" :size="16" /></button>
      </section>
      <section v-else :key="step" class="onboarding-step" aria-labelledby="onboarding-title" aria-describedby="onboarding-subtitle">
        <h1 id="onboarding-title" ref="heading" tabindex="-1">{{ titles[step] }}</h1>
        <p id="onboarding-subtitle" class="onboarding-subtitle">{{ subtitles[step] }}</p>

        <DataImportPicker v-if="step === 'bring'" ref="importer" />

        <div v-else-if="step === 'sites'" class="onboarding-card onboarding-sites">
          <div class="onboarding-sites-label"><h2>你的日常，从这里开始</h2><span aria-live="polite">已选 {{ picked.length }} / {{ remaining }}</span></div>
          <div class="onboarding-site-grid" role="group" aria-label="选择常用网站">
            <button v-for="site in ONBOARDING_SITES" :key="site.id" class="onboarding-site" :class="{ selected: picked.includes(site.id) }" :aria-pressed="picked.includes(site.id)" :aria-label="site.title" @click="select(site.id)">
              <span class="onboarding-site-glyph" :style="{ '--site-color': site.color }" aria-hidden="true">{{ site.glyph }}</span>
              <span>{{ site.title }}</span><span class="onboarding-site-check" aria-hidden="true"><Icon v-if="picked.includes(site.id)" name="check" :size="12" /></span>
            </button>
          </div>
          <p class="onboarding-card-note" :class="{ 'onboarding-limit': limitNotice }" aria-live="polite">{{ limitNotice || (remaining < MAX_FAVORITES ? `保留了 ${MAX_FAVORITES - remaining} 个其他常用网站。` : '最多选择 8 个；之后可以在侧栏中随时调整。') }}</p>
        </div>

        <div v-else-if="step === 'ads'" class="onboarding-question onboarding-ads">
          <span class="onboarding-shield"><Icon name="shield" :size="46" /></span>
          <h2>目前，网页会正常显示广告</h2>
          <p>Rove 暂未内置广告拦截。<br />这一步无需设置，继续即可开始下一步。</p>
        </div>

        <div v-else class="onboarding-question">
          <div class="onboarding-answers" role="group" aria-label="默认浏览器偏好">
            <button :aria-pressed="makeDefault" :class="{ chosen: makeDefault }" @click="makeDefault = true"><Icon name="check" v-if="makeDefault" :size="16" />设为默认</button>
            <button :aria-pressed="!makeDefault" :class="{ chosen: !makeDefault }" @click="makeDefault = false"><Icon name="check" v-if="!makeDefault" :size="16" />暂时不用</button>
          </div>
          <p v-if="b.native">{{ makeDefault ? '点击“开始浏览”后，将打开 Windows 默认应用设置。' : '你可以稍后在设置中更改默认浏览器。' }}</p>
          <p v-else>当前为界面预览，不会更改系统默认浏览器。</p>
        </div>
      </section>
    </Transition>
    <p v-if="b.error" class="onboarding-error" role="alert">{{ b.error }}</p>
    <footer v-if="step !== 'hello'" class="onboarding-footer">
      <button class="onboarding-quiet onboarding-back" :disabled="busy || b.importBusy" @click="back"><Icon name="back" :size="14" />返回</button>
      <button class="onboarding-primary" :disabled="busy || b.importBusy" @click="advance()">{{ b.importBusy ? '请稍候…' : primaryTitle }}<Icon name="arrow" :size="16" /></button>
    </footer>
  </main>
</template>
