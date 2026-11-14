<script setup lang="ts">
// BringPicker / ImportCoordinator presentation adapted from Zigzag (Apache-2.0).
import { computed, onMounted, ref, watch } from "vue";
import { useBrowser } from "../../stores/browser";
import Icon from "../common/Icon.vue";
import OnboardingLogo from "./OnboardingLogo.vue";
const b = useBrowser();
const selected = ref<string[]>([]), bookmarks = ref(true), history = ref(true), attempted = ref(false);
const reports = computed(() => b.state.onboarding?.imports || []);
const canImport = computed(() => b.native && !attempted.value && selected.value.length > 0 && (bookmarks.value || history.value));
const reportFor = (id: string) => reports.value.find((r) => r.id === id);
watch(() => b.importSources, (sources) => {
  selected.value = sources.filter((s) => !reportFor(s.id) || reportFor(s.id)!.problems.length > 0).slice(0, 1).map((s) => s.id);
}, { immediate: true });
function run(ids = selected.value) {
  if (b.importBusy || !ids.length || (!bookmarks.value && !history.value)) return;
  attempted.value = true;
  b.runImport({ sources: [...ids], bookmarks: bookmarks.value, history: history.value });
}
function choose() { attempted.value = false; }
function startFresh() { selected.value = []; choose(); }
function browserLogo(browser: string) {
  return browser.includes('Edge') ? 'edge' : browser.toLowerCase().includes('chrome') ? 'chrome' : browser.toLowerCase();
}
defineExpose({ canImport, run });
onMounted(() => b.scanImports());
</script>
<template>
  <div class="onboarding-card onboarding-bring" :aria-busy="b.importBusy">
    <div class="import-browser-choices">
      <button class="onboarding-choice import-fresh" :class="{ selected: !selected.length }" :aria-pressed="!selected.length" :disabled="b.importBusy" @click="startFresh"><span class="onboarding-radio" aria-hidden="true" /><strong>全新开始</strong></button>
      <div v-if="!b.native" class="import-preview-sources" aria-label="可导入的浏览器">
        <div v-for="browser in [{ id: 'edge', title: 'Microsoft Edge' }, { id: 'chrome', title: 'Chrome' }]" :key="browser.id" class="onboarding-choice import-preview-source" aria-disabled="true"><span class="onboarding-radio" aria-hidden="true" /><strong>{{ browser.title }}</strong><span class="onboarding-choice-art" :class="`brand-${browser.id}`"><OnboardingLogo :name="browser.id" /></span></div>
      </div>
      <div v-if="b.importSources.length" class="import-source-list" role="group" aria-label="选择来源浏览器">
        <div v-for="source in b.importSources" :key="source.id" class="import-source">
          <label class="onboarding-choice import-source-choice" :class="{ selected: selected.includes(source.id) }"><input type="checkbox" v-model="selected" :value="source.id" :disabled="b.importBusy" @change="choose" /><span><strong>{{ source.browser }}</strong><small>{{ source.name }}</small></span><span class="onboarding-choice-art" :class="`brand-${browserLogo(source.browser)}`"><OnboardingLogo :name="browserLogo(source.browser)" /></span></label>
          <div v-if="reportFor(source.id)" class="import-result" role="status">
            <p>已导入 {{ reportFor(source.id)!.bookmarks }} 个书签、{{ reportFor(source.id)!.history }} 条历史<span v-if="reportFor(source.id)!.skipped">；跳过 {{ reportFor(source.id)!.skipped }} 个重复或无效项目</span>。</p>
            <p v-for="problem in reportFor(source.id)!.problems" :key="problem" class="import-problem">{{ problem }}</p>
            <button v-if="reportFor(source.id)!.problems.length" class="import-link" :disabled="b.importBusy || (!bookmarks && !history)" @click="run([source.id])">重试这个来源</button>
          </div>
        </div>
      </div>
    </div>
    <p v-if="!b.native" class="onboarding-card-note">浏览器预览无法读取本机数据。请在 Rove 桌面版中导入。</p>
    <p v-else-if="b.importBusy && b.importPhase === 'checking'" class="onboarding-card-note" role="status">正在检查浏览器配置…</p>
    <p v-else-if="!b.importSources.length" class="onboarding-card-note">未找到可导入的浏览器数据。支持 Edge、Chrome、Firefox、Brave、Vivaldi 和 Opera；也可单独导入密码文件。</p>
    <div v-if="b.native" class="import-heading"><button class="import-link" :disabled="b.importBusy" @click="b.scanImports()">重新检查浏览器</button></div>
    <details class="import-options" :open="b.native && b.importSources.length > 0">
      <summary>导入选项与密码</summary>
    <fieldset v-if="b.importSources.length" class="import-kinds" :disabled="b.importBusy" @change="choose"><legend>选择导入内容</legend><label><input type="checkbox" v-model="bookmarks" />书签</label><label><input type="checkbox" v-model="history" />历史记录</label></fieldset>
    <div class="import-passwords"><div><h2>密码</h2><p>在原浏览器中导出密码 CSV，再选择文件。密码使用当前 Windows 账户加密保存。</p></div><button class="import-file-button" :disabled="b.importBusy || !b.native" @click="b.importPasswords()"><Icon name="key" :size="14" />导入密码文件…</button></div>
    <div v-if="reportFor('password-file')" class="import-result" role="status"><p>已导入 {{ reportFor('password-file')!.passwords }} 个密码<span v-if="reportFor('password-file')!.skipped">，跳过 {{ reportFor('password-file')!.skipped }} 个重复或无效项目</span>。</p><p v-for="problem in reportFor('password-file')!.problems" :key="problem" class="import-problem">{{ problem }}</p></div>
    <p class="onboarding-card-note">书签放入各来源的独立工作区；历史合并并去重。原浏览器的数据保持原样。导入的密码可在对应网站登录时点击“Rove · 填写登录信息”使用。</p>
    </details>
    <p v-if="b.importBusy && b.importPhase === 'importing'" class="import-progress" role="status">正在导入，请稍候…</p>
  </div>
</template>
