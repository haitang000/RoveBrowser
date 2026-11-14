<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from "vue";
import { calendarDays, localDate, validGithubUsername, type NewTabWidget } from "../../lib/new-tab";
import { githubContributions, type Contribution } from "../../lib/github-widget";
import Icon from "../common/Icon.vue";

const props = defineProps<{ widget: NewTabWidget; now: Date; editing: boolean }>();
const emit = defineEmits<{
  patch: [value: Partial<NewTabWidget>];
  navigate: [url: string];
}>();
const clock = computed(() => props.now.toLocaleTimeString("en-GB", { hour: "2-digit", minute: "2-digit" }));
const monthOffset = ref(0);
const month = computed(() => new Date(props.now.getFullYear(), props.now.getMonth() + monthOffset.value, 1));
const days = computed(() => calendarDays(month.value));
const monthTitle = computed(() => month.value.toLocaleDateString("en-US", { month: "long", year: "numeric" }));
const isToday = (day: number | null) => day === props.now.getDate() && month.value.getMonth() === props.now.getMonth() && month.value.getFullYear() === props.now.getFullYear();
const note = ref(props.widget.text || "");
const noteFocused = ref(false);
let noteTimer: ReturnType<typeof setTimeout> | undefined;
watch(() => props.widget.text, (value) => { if (!noteFocused.value && noteTimer === undefined) note.value = value || ""; });
function saveNote() {
  if (noteTimer === undefined) return;
  clearTimeout(noteTimer);
  noteTimer = undefined;
  emit("patch", { text: note.value });
}
function queueNote() {
  clearTimeout(noteTimer);
  noteTimer = setTimeout(saveNote, 350);
}
const username = ref(props.widget.username || "");
const configuring = ref(!props.widget.username);
const usernameError = ref("");
const contributionError = ref("");
const contributions = ref<Contribution[]>([]);
const loading = ref(false);
const retry = ref(0);
const recentDays = computed(() => {
  const values = contributions.value.slice(-119);
  if (!values.length) return [];
  const offset = new Date(`${values[0].date}T12:00:00`).getDay();
  return [...Array<null>(offset).fill(null), ...values];
});
const total = computed(() => contributions.value.reduce((sum, day) => sum + day.count, 0));
function setUsername() {
  if (!validGithubUsername(username.value)) { usernameError.value = "请输入有效的 GitHub 用户名"; return; }
  usernameError.value = "";
  configuring.value = false;
  emit("patch", { username: username.value.trim() });
}
watch(() => props.widget.username, (value) => { username.value = value || ""; configuring.value = !value; });
watch([() => props.widget.username, retry], async ([user], _, onCleanup) => {
  if (props.widget.kind !== "github" || !user) return;
  const controller = new AbortController();
  let active = true;
  const timer = setTimeout(() => controller.abort(), 12_000);
  onCleanup(() => { active = false; clearTimeout(timer); controller.abort(); });
  loading.value = true;
  contributionError.value = "";
  contributions.value = [];
  try { const result = await githubContributions(user, controller.signal); if (active) contributions.value = result; }
  catch (error) { if (active) contributionError.value = error instanceof Error && error.name !== "AbortError" ? error.message : "连接超时，请重试"; }
  finally { clearTimeout(timer); if (active) loading.value = false; }
}, { immediate: true });
onBeforeUnmount(saveNote);
</script>

<template>
  <div v-if="widget.kind === 'clock'" class="nt-clock">
    <time :datetime="now.toISOString()">{{ clock }}</time>
    <span>{{ localDate(now) }}</span>
  </div>

  <div v-else-if="widget.kind === 'calendar'" class="nt-calendar">
    <header>
      <button class="nt-month-title" aria-label="回到本月" @click="monthOffset = 0">{{ monthTitle }}</button>
      <div class="nt-month-nav">
        <button aria-label="上个月" @click="monthOffset--"><Icon name="back" :size="13" /></button>
        <button aria-label="下个月" @click="monthOffset++"><Icon name="forward" :size="13" /></button>
      </div>
    </header>
    <div class="nt-calendar-grid">
      <span v-for="(day, index) in ['M', 'T', 'W', 'T', 'F', 'S', 'S']" :key="`weekday-${index}`" class="nt-weekday">{{ day }}</span>
      <span v-for="(day, index) in days" :key="index" :class="{ 'nt-today': isToday(day) }" :aria-current="isToday(day) ? 'date' : undefined">{{ day }}</span>
    </div>
  </div>

  <div v-else-if="widget.kind === 'notes'" class="nt-notes">
    <label :for="`note-${widget.id}`">Notes <Icon name="edit" :size="16" /></label>
    <textarea :id="`note-${widget.id}`" v-model="note" aria-label="便签内容" placeholder="A little thought…" maxlength="10000" spellcheck="false" @focus="noteFocused = true" @input="queueNote" @blur="noteFocused = false; saveNote()" />
  </div>

  <div v-else class="nt-github">
    <header>
      <span>GitHub <span v-if="widget.username" class="nt-github-user">@{{ widget.username }}</span></span>
      <button v-if="widget.username" class="nt-widget-setting" aria-label="修改 GitHub 用户名" @click="configuring = !configuring"><Icon name="settings" :size="16" /></button>
    </header>
    <form v-if="configuring" class="nt-github-form" @submit.prevent="setUsername">
      <label :for="`github-${widget.id}`">你的 GitHub 用户名</label>
      <div><input :id="`github-${widget.id}`" v-model="username" placeholder="octocat" maxlength="39" autocomplete="off" :aria-invalid="Boolean(usernameError)" :aria-describedby="`github-help-${widget.id}`" /><button type="submit" aria-label="显示贡献图"><Icon name="arrow" :size="18" /></button></div>
      <p :id="`github-help-${widget.id}`" :class="{ 'nt-form-error': usernameError }">{{ usernameError || '仅查询公开贡献记录 · 数据由 GitHub Contributions API 提供' }}</p>
    </form>
    <div v-else-if="loading" class="nt-widget-message" role="status">正在加载贡献记录…</div>
    <div v-else-if="contributionError" class="nt-widget-message" role="status"><span>{{ contributionError }}</span><button class="nt-text-button" @click="retry++">重试</button></div>
    <template v-else>
      <button class="nt-contribution-link" aria-label="打开 GitHub 个人主页" @click="emit('navigate', `https://github.com/${widget.username}`)">
        <span class="nt-contribution-summary"><strong>{{ total.toLocaleString() }}</strong> contributions in the last year</span>
        <span class="nt-contribution-grid" role="img" :aria-label="`${widget.username} 最近一年的 GitHub 贡献次数：${total}`">
          <span v-for="(day, i) in recentDays" :key="i" class="nt-contribution-day" :data-level="day?.level || 0" :style="day ? undefined : { visibility: 'hidden' }" :title="day ? `${day.date}: ${day.count} contributions` : undefined" />
        </span>
      </button>
      <div class="nt-contribution-legend"><span>Last 17 weeks</span><span>Less <i v-for="level in 5" :key="level" class="nt-contribution-day" :data-level="level - 1" /> More</span></div>
    </template>
  </div>
</template>
