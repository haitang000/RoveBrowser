<script setup lang="ts">
// Welcome flow adapted from Zigzag, Copyright 2026 Zigao Wang, Apache-2.0.
// Rove's split-panel onboarding reuses the live space editor and native settings.
import { computed, nextTick, onMounted, ref, watch } from "vue";
import { useBrowser } from "../../stores/browser";
import { ONBOARDING_SITES, ONBOARDING_STEPS, otherFavorites, selectedSiteIds } from "../../lib/onboarding";
import { MAX_FAVORITES } from "../../lib/favorites";
import { frameOf } from "../../lib/space-look";
import Icon from "../common/Icon.vue";
import OnboardingLogo from "./OnboardingLogo.vue";
import DataImportPicker from "./DataImportPicker.vue";
import SpaceEditor from "./SpaceEditor.vue";

const b = useBrowser();
const step = computed(() => b.state.onboarding?.step || "hello");
const index = computed(() => ONBOARDING_STEPS.indexOf(step.value));
const heading = ref<HTMLElement>();
const picked = ref<string[]>(selectedSiteIds(b.state));
const makeDefault = ref(false);
const limitNotice = ref("");
const busy = ref(false);
const importer = ref<InstanceType<typeof DataImportPicker>>();
const remaining = computed(() => Math.max(0, MAX_FAVORITES - otherFavorites(b.state).length));
const titles = {
  hello: "欢迎使用 Rove", bring: "把熟悉的内容\n带过来。", spaces: "你的空间，\n你的色彩。",
  search: "选择你的\n搜索引擎。", sites: "把常用的网站，\n放在手边。", ready: "将 Rove 设为\n默认浏览器？",
};
const subtitles = {
  hello: "", bring: "选择一个浏览器，导入书签、历史记录和密码，从熟悉的地方继续。",
  spaces: "为你的空间挑选专属色彩，让每一次打开浏览器，都更像你。",
  search: "选择你喜欢的默认搜索引擎。之后也可以随时在设置中更改。",
  sites: "选择你最常用的网站，把它们留在侧栏顶部，随时打开。",
  ready: "让来自其他应用的链接在 Rove 中打开。你可以随时更改这个选择。",
};
const engines = [
  { id: "google", title: "Google" }, { id: "bing", title: "Bing" }, { id: "duckDuckGo", title: "DuckDuckGo" },
];
const primaryTitle = computed(() => step.value === "ready" ? "开始浏览" : step.value === "bring" && importer.value?.canImport ? "导入所选数据" : "继续");
const previewStyle = computed(() => {
  const space = b.spacePreview?.id === b.space.id ? { ...b.space, ...b.spacePreview.look } : b.space;
  const frame = frameOf(space, b.state.theme, false);
  return frame.custom ? { background: frame.background } : undefined;
});
watch(step, async () => {
  busy.value = false; limitNotice.value = "";
  if (step.value === "sites") picked.value = selectedSiteIds(b.state);
  await nextTick(); heading.value?.focus({ preventScroll: true });
});
watch(() => b.error, (value) => { if (value) busy.value = false; });
function select(id: string) {
  limitNotice.value = "";
  if (picked.value.includes(id)) picked.value = picked.value.filter((item) => item !== id);
  else if (picked.value.length < remaining.value) picked.value.push(id);
  else limitNotice.value = `最多置顶 ${MAX_FAVORITES} 个网站，请先取消一个已选网站。`;
}
function selectEngine(id: string) {
  b.send({ type: "settingsUpdate", settings: { ...b.settings, searchEngine: id } });
}
function advance(skip = false) {
  if (busy.value || b.importBusy) return;
  if (step.value === "bring" && !skip && importer.value?.canImport) { importer.value.run(); return; }
  busy.value = true;
  b.send({ type: "onboardingAdvance", step: step.value, skip, sites: [...picked.value], makeDefault: step.value === "ready" && makeDefault.value && b.native && !(b.defaultBrowser?.http && b.defaultBrowser?.https) });
  if (!b.native) busy.value = false;
}
function back() {
  if (!busy.value && !b.importBusy && index.value > 0) b.send({ type: "onboardingBack", step: step.value });
}
function key(event: KeyboardEvent) {
  if (event.defaultPrevented || event.isComposing || event.ctrlKey || event.altKey || event.metaKey) return;
  const target = event.target as HTMLElement;
  if (target.closest('input, textarea, select, [contenteditable="true"], [role="slider"]')) return;
  if (event.key === "Escape" && index.value > 0) { event.preventDefault(); back(); }
  else if (event.key === "Enter" && !target.closest("button, a")) { event.preventDefault(); advance(); }
}
function focusHeading() { heading.value?.focus({ preventScroll: true }); }
onMounted(focusHeading);
</script>

<template>
  <main class="onboarding" :data-step="step" aria-label="Rove 欢迎引导" @keydown="key">
    <Transition name="onboarding-page" mode="out-in" @after-enter="focusHeading">
      <section v-if="step === 'hello'" key="hello" class="onboarding-hello" aria-labelledby="onboarding-title">
        <p class="onboarding-welcome">欢迎来到你的新空间</p>
        <h1 id="onboarding-title" ref="heading" class="onboarding-wordmark" tabindex="-1">Rove</h1>
        <p class="onboarding-hello-note">少一点纷扰，多一点自由。</p>
        <button class="onboarding-primary onboarding-start" :disabled="busy" @click="advance()">开始使用 <Icon name="arrow" :size="18" /></button>
        <p v-if="b.error" class="onboarding-error" role="alert">{{ b.error }}</p>
      </section>
      <section v-else :key="step" class="onboarding-step" aria-labelledby="onboarding-title" aria-describedby="onboarding-subtitle">
        <div class="onboarding-copy">
          <button class="onboarding-back" :disabled="busy || b.importBusy" @click="back"><Icon name="back" :size="18" />返回</button>
          <div class="onboarding-introduction">
            <h1 id="onboarding-title" ref="heading" tabindex="-1">{{ titles[step] }}</h1>
            <p id="onboarding-subtitle" class="onboarding-subtitle">{{ subtitles[step] }}</p>
          </div>
          <footer class="onboarding-footer">
            <p v-if="b.error" class="onboarding-error" role="alert">{{ b.error }}</p>
            <button class="onboarding-primary" :disabled="busy || b.importBusy" @click="advance()">{{ b.importBusy ? '请稍候…' : primaryTitle }}</button>
            <button v-if="step === 'sites' || step === 'bring'" class="onboarding-skip" :disabled="busy || b.importBusy" @click="advance(true)">{{ step === 'sites' ? '暂时跳过' : '跳过' }}</button>
            <ol class="onboarding-progress" aria-label="引导进度">
              <li v-for="(name, i) in ONBOARDING_STEPS.slice(1)" :key="name" :class="{ current: index === i + 1, done: index > i + 1 }" :aria-current="index === i + 1 ? 'step' : undefined"><span class="sr-only">第 {{ i + 1 }} 步，共 5 步</span></li>
            </ol>
          </footer>
        </div>
        <div class="onboarding-visual" :class="`onboarding-visual-${step}`" :style="step === 'spaces' ? previewStyle : undefined">
          <DataImportPicker v-if="step === 'bring'" ref="importer" />
          <div v-else-if="step === 'spaces'" class="onboarding-space-preview">
            <div class="onboarding-theme" role="group" aria-label="外观模式">
              <button :aria-pressed="b.state.theme === 'light'" aria-label="浅色模式" @click="b.state.theme !== 'light' && b.send({ type: 'theme' })"><Icon name="sun" :size="19" /></button>
              <button :aria-pressed="b.state.theme === 'dark'" aria-label="深色模式" @click="b.state.theme !== 'dark' && b.send({ type: 'theme' })"><Icon name="moon" :size="19" /></button>
            </div>
            <SpaceEditor embedded />
          </div>
          <div v-else-if="step === 'search'" class="onboarding-choices" role="radiogroup" aria-label="默认搜索引擎">
            <label v-for="engine in engines" :key="engine.id" class="onboarding-choice" :class="{ selected: b.settings.searchEngine === engine.id }">
              <input type="radio" name="search-engine" :value="engine.id" :checked="b.settings.searchEngine === engine.id" @change="selectEngine(engine.id)" />
              <strong>{{ engine.title }}</strong><span class="onboarding-choice-art" :class="`brand-${engine.id}`"><OnboardingLogo :name="engine.id" /></span>
            </label>
            <p v-if="!engines.some(engine => engine.id === b.settings.searchEngine)" class="onboarding-existing-engine">已保留你在设置中选择的搜索引擎。</p>
          </div>
          <div v-else-if="step === 'sites'" class="onboarding-sites-window">
            <div class="onboarding-window-dots" aria-hidden="true"><i /><i /><i /></div>
            <div class="onboarding-site-grid" role="group" aria-label="选择常用网站">
              <button v-for="site in ONBOARDING_SITES" :key="site.id" class="onboarding-site" :class="{ selected: picked.includes(site.id) }" :aria-pressed="picked.includes(site.id)" :aria-label="site.title" @click="select(site.id)">
                <OnboardingLogo :name="site.id" :glyph="site.glyph" :color="site.color" />
                <span class="onboarding-site-title">{{ site.title }}</span><span class="onboarding-site-check" aria-hidden="true"><Icon v-if="picked.includes(site.id)" name="check" :size="12" /></span>
              </button>
            </div>
            <div class="onboarding-sites-status" aria-live="polite"><span>已选 {{ picked.length }} / {{ remaining }}</span><p :class="{ 'onboarding-limit': limitNotice }">{{ limitNotice || '之后也可以随时在侧栏中调整。' }}</p></div>
          </div>
          <div v-else class="onboarding-ready">
            <div class="onboarding-choices" role="radiogroup" aria-label="默认浏览器偏好">
              <label class="onboarding-choice" :class="{ selected: makeDefault }"><input v-model="makeDefault" type="radio" name="make-default" :value="true" aria-label="设为默认" /><span><strong>是的，</strong>将 Rove 设为默认。</span></label>
              <label class="onboarding-choice" :class="{ selected: !makeDefault }"><input v-model="makeDefault" type="radio" name="make-default" :value="false" aria-label="暂时不用" /><span><strong>暂时不用，</strong>我想先探索一下。</span></label>
            </div>
            <p class="onboarding-ready-note">{{ !b.native ? '当前为界面预览，不会更改系统默认浏览器。' : b.defaultBrowser?.http && b.defaultBrowser?.https ? 'Rove 已是默认浏览器。' : makeDefault ? '点击“开始浏览”后，在 Windows 设置中选择 Rove。' : '你可以稍后在设置中更改默认浏览器。' }}</p>
          </div>
        </div>
      </section>
    </Transition>
  </main>
</template>
