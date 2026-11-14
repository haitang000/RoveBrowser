<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref } from "vue";
import { useBrowser } from "../../stores/browser";
import { greeting, MAX_WIDGETS, newTabSettings, newTabSuggestions, WIDGET_CATALOG, type NewTabWidget, type WidgetKind } from "../../lib/new-tab";
import Favicon from "../common/Favicon.vue";
import Icon from "../common/Icon.vue";
import NewTabWidgetView from "./NewTabWidget.vue";
import githubLogo from "../../assets/onboarding/github.png";
import chatgptLogo from "../../assets/onboarding/chatgpt.png";
import notionLogo from "../../assets/onboarding/notion.png";
import youtubeLogo from "../../assets/onboarding/youtube.png";
import claudeLogo from "../../assets/onboarding/claude.png";
import gmailLogo from "../../assets/onboarding/gmail.png";

const props = defineProps<{ tabId?: string }>();
const b = useBrowser();
const root = ref<HTMLElement>();
const settings = computed(() => newTabSettings(b.state.newTab));
const sites = computed(() => newTabSuggestions(b.history, b.state.favorites, b.state.pins));
const editing = ref(false);
const now = ref(new Date());
const editButton = ref<HTMLButtonElement>();
const discoverTitle = ref<HTMLElement>();
const name = ref("");
const nameError = ref("");
const dragged = ref<string>();
const dropTarget = ref<string>();
const announcement = ref("");
const placeholders = computed(() => Math.min(2, Math.max(0, 4 - settings.value.widgets.reduce((sum, widget) => sum + (widget.size === "wide" ? 2 : 1), 0))));
const logos: Record<string, string> = { "github.com": githubLogo, "chatgpt.com": chatgptLogo, "notion.so": notionLogo, "notion.com": notionLogo, "youtube.com": youtubeLogo, "claude.ai": claudeLogo, "mail.google.com": gmailLogo };
const hostnameFor = (url: string) => new URL(url).hostname.replace(/^www\./, "");
const logoFor = (url: string) => logos[hostnameFor(url)];
const widgetTitle = (kind: WidgetKind) => WIDGET_CATALOG.find((entry) => entry.kind === kind)!.title;
let clock: ReturnType<typeof setInterval>;
onMounted(() => {
  b.send({ type: "history", query: "" });
  clock = setInterval(() => { now.value = new Date(); }, 1000);
});
onBeforeUnmount(() => clearInterval(clock));
function save(widgets = settings.value.widgets) {
  b.send({ type: "newTabUpdate", settings: { ...settings.value, widgets } });
}
function patchWidget(id: string, value: Partial<NewTabWidget>) {
  if (settings.value.widgets.some((widget) => widget.id === id)) save(settings.value.widgets.map((widget) => widget.id === id ? { ...widget, ...value, id } : widget));
}
async function beginEditing() {
  name.value = settings.value.displayName;
  nameError.value = "";
  editing.value = true;
  await nextTick();
  discoverTitle.value?.focus();
}
async function finishEditing() {
  if (!editing.value) return;
  const displayName = name.value.trim();
  if (!displayName) { nameError.value = "请输入你的称呼"; return; }
  if (displayName !== settings.value.displayName) b.send({ type: "newTabUpdate", settings: { ...settings.value, displayName } });
  editing.value = false;
  dragged.value = undefined;
  dropTarget.value = undefined;
  await nextTick();
  editButton.value?.focus();
}
function addWidget(kind: WidgetKind) {
  if (settings.value.widgets.length >= MAX_WIDGETS) return;
  const definition = WIDGET_CATALOG.find((entry) => entry.kind === kind)!;
  save([...settings.value.widgets, { id: crypto.randomUUID(), kind, size: definition.size }]);
  announcement.value = `已添加${definition.title}`;
}
function removeWidget(id: string) {
  save(settings.value.widgets.filter((widget) => widget.id !== id));
  announcement.value = "已移除小组件";
  nextTick(() => root.value?.querySelector<HTMLButtonElement>(".nt-add-button:not(:disabled)")?.focus());
}
function moveWidget(id: string, offset: number) {
  const widgets = [...settings.value.widgets];
  const index = widgets.findIndex((widget) => widget.id === id);
  const target = index + offset;
  if (index < 0 || target < 0 || target >= widgets.length) return;
  [widgets[index], widgets[target]] = [widgets[target], widgets[index]];
  save(widgets);
  announcement.value = `已移动到第 ${target + 1} 个位置`;
}
function dragStart(event: DragEvent, id: string) {
  if (!editing.value || !event.dataTransfer) return;
  dragged.value = id;
  event.dataTransfer.effectAllowed = "move";
  event.dataTransfer.setData("text/rove-widget", id);
}
function dragOver(event: DragEvent, id?: string) {
  if (!event.dataTransfer?.types.includes("text/rove-widget")) return;
  event.preventDefault();
  event.stopPropagation();
  event.dataTransfer.dropEffect = "move";
  if (id && id !== dragged.value) dropTarget.value = id;
}
function dropWidget(event: DragEvent, target: string) {
  const id = event.dataTransfer?.getData("text/rove-widget");
  if (!id) return;
  event.preventDefault();
  event.stopPropagation();
  const widgets = [...settings.value.widgets];
  const from = widgets.findIndex((widget) => widget.id === id);
  const to = widgets.findIndex((widget) => widget.id === target);
  if (from >= 0 && to >= 0 && from !== to) { widgets.splice(to, 0, widgets.splice(from, 1)[0]); save(widgets); }
  dragged.value = undefined;
  dropTarget.value = undefined;
}
function navigate(url: string, event?: MouseEvent) {
  if (event?.ctrlKey || event?.metaKey || event?.shiftKey || event?.button === 1) b.send({ type: "new", url });
  else b.send({ type: "navigate", id: props.tabId || b.state.activeTab, url });
}
</script>

<template>
  <section ref="root" class="new-tab-page" :class="{ 'nt-editing': editing }" aria-label="新标签页" @keydown.esc.stop="finishEditing" @dragover="dragOver($event)">
    <aside v-if="editing" class="nt-discover" aria-label="发现小组件">
      <h1 ref="discoverTitle" tabindex="-1">Discover</h1>
      <div class="nt-catalog">
        <article v-for="entry in WIDGET_CATALOG" :key="entry.kind" class="nt-catalog-card">
          <div class="nt-catalog-preview" aria-hidden="true">
            <div v-if="entry.kind === 'github'" class="nt-preview-graph"><i v-for="cell in 77" :key="cell" :class="{ filled: cell === 71, soft: cell === 72 || cell === 64 }" /></div>
            <div v-else-if="entry.kind === 'clock'" class="nt-preview-clock"><strong>{{ now.toLocaleTimeString('en-GB', { hour: '2-digit', minute: '2-digit' }) }}</strong><span>{{ now.toLocaleDateString('en-US', { month: 'short', day: 'numeric' }) }}</span></div>
            <div v-else-if="entry.kind === 'calendar'" class="nt-preview-calendar"><span>{{ now.toLocaleDateString('en-US', { month: 'long' }) }}</span><strong>{{ now.getDate() }}</strong><small>{{ now.toLocaleDateString('en-US', { weekday: 'long' }) }}</small></div>
            <div v-else class="nt-preview-notes"><Icon name="edit" :size="18" /><i /><i /><i /></div>
          </div>
          <div class="nt-catalog-info"><h2>{{ entry.title }}</h2><p>{{ entry.description }}</p><button class="nt-add-button" :aria-label="`添加${entry.title}`" :disabled="settings.widgets.length >= MAX_WIDGETS" @click="addWidget(entry.kind)"><Icon name="plus" :size="14" /> Add</button></div>
        </article>
      </div>
      <p v-if="settings.widgets.length >= MAX_WIDGETS" class="nt-limit" role="status">最多添加 {{ MAX_WIDGETS }} 个小组件</p>
      <form class="nt-name-form" @submit.prevent="finishEditing"><label :for="`new-tab-name-${tabId || 'empty'}`">Your name</label><input :id="`new-tab-name-${tabId || 'empty'}`" v-model="name" maxlength="40" autocomplete="off" aria-label="首页称呼" :aria-invalid="Boolean(nameError)" @input="nameError = ''" /><span v-if="nameError" class="nt-form-error" role="alert">{{ nameError }}</span></form>
    </aside>

    <button v-if="editing" class="nt-done-button" aria-label="完成编辑" @click="finishEditing">Done</button>
    <div class="nt-canvas">
      <div class="nt-home">
        <header v-if="!editing" class="nt-greeting">
          <svg class="nt-brand" viewBox="0 0 64 64" fill="currentColor" aria-hidden="true"><path d="M43 2h19v19a19 19 0 1 1-19-19ZM21 24a19 19 0 1 1 0 38H2V43a19 19 0 0 1 19-19ZM12 2a10 10 0 0 0 0 20V12h10A10 10 0 0 0 12 2ZM52 42v10H42a10 10 0 1 0 10-10Z" /><circle cx="43" cy="21" r="11" fill="var(--nt-bg)" /><circle cx="21" cy="43" r="11" fill="var(--nt-bg)" /></svg>
          <h1><span>{{ greeting(now) }}</span><strong>{{ settings.displayName }}.</strong></h1>
        </header>

        <div class="nt-columns">
          <section v-if="!editing" class="nt-suggestions" aria-label="推荐网站">
            <h2>Suggestions</h2>
            <div class="nt-suggestion-list"><a v-for="site in sites" :key="site.url" class="nt-suggestion" :href="site.url" @click.prevent="navigate(site.url, $event)" @auxclick.middle.prevent="navigate(site.url, $event)"><span class="nt-site-icon"><svg v-if="hostnameFor(site.url) === 'figma.com'" class="nt-figma-logo" viewBox="0 0 38 57" aria-hidden="true"><path fill="#f24e1e" d="M9.5 0a9.5 9.5 0 0 0 0 19H19V0Z" /><path fill="#ff7262" d="M19 0h9.5a9.5 9.5 0 0 1 0 19H19Z" /><path fill="#a259ff" d="M9.5 19a9.5 9.5 0 0 0 0 19H19V19Z" /><circle fill="#1abcfe" cx="28.5" cy="28.5" r="9.5" /><path fill="#0acf83" d="M9.5 38a9.5 9.5 0 1 0 9.5 9.5V38Z" /></svg><img v-else-if="logoFor(site.url)" :src="logoFor(site.url)" alt="" draggable="false" /><Favicon v-else :url="site.url" :favicon="site.favicon" /></span><span class="nt-site-text"><strong>{{ site.title }}</strong><small>{{ site.url }}</small></span><Icon class="nt-site-arrow" name="arrow" :size="18" /></a></div>
          </section>

          <section class="nt-widgets" aria-label="小组件">
            <header class="nt-section-header"><h2>Widgets</h2><button v-if="!editing" ref="editButton" class="nt-edit-button" aria-label="编辑小组件" @click="beginEditing">Edit</button></header>
            <div class="nt-widget-grid">
              <article v-for="(widget, index) in settings.widgets" :key="widget.id" class="nt-widget" :class="{ 'nt-widget-wide': widget.size === 'wide', 'nt-widget-dragging': dragged === widget.id, 'nt-widget-drop': dropTarget === widget.id }" :aria-label="widgetTitle(widget.kind)" @dragover="dragOver($event, widget.id)" @drop="dropWidget($event, widget.id)">
                <div v-if="editing" class="nt-widget-toolbar">
                  <button class="nt-drag-handle" draggable="true" :aria-label="`拖动${widgetTitle(widget.kind)}排序`" title="拖动排序" @dragstart="dragStart($event, widget.id)" @dragend="dragged = undefined; dropTarget = undefined"><Icon name="more" :size="16" /></button>
                  <div><button :disabled="index === 0" :aria-label="`前移${widgetTitle(widget.kind)}`" @click="moveWidget(widget.id, -1)"><Icon name="back" :size="13" /></button><button :disabled="index === settings.widgets.length - 1" :aria-label="`后移${widgetTitle(widget.kind)}`" @click="moveWidget(widget.id, 1)"><Icon name="forward" :size="13" /></button><button :aria-label="`调整${widgetTitle(widget.kind)}宽度`" @click="patchWidget(widget.id, { size: widget.size === 'wide' ? 'small' : 'wide' })"><Icon name="expand" :size="13" /></button><button :aria-label="`移除${widgetTitle(widget.kind)}`" @click="removeWidget(widget.id)"><Icon name="close" :size="14" /></button></div>
                </div>
                <NewTabWidgetView :widget="widget" :now="now" :editing="editing" @patch="patchWidget(widget.id, $event)" @navigate="navigate($event)" />
              </article>
              <button v-for="slot in placeholders" :key="`empty-${slot}`" class="nt-widget nt-widget-empty" aria-label="添加小组件" @click="editing ? addWidget('notes') : beginEditing()"><span><Icon name="plus" :size="22" /><small>{{ editing ? '添加便签' : 'Add a widget' }}</small></span></button>
            </div>
          </section>
        </div>
      </div>
    </div>
    <span v-if="editing" class="nt-sr-only" role="status" aria-live="polite">{{ announcement }}</span>
  </section>
</template>
