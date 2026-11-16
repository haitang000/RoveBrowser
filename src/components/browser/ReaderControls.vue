<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useBrowser } from "../../stores/browser";
import { readerPreferences, type ReaderPreferences } from "../../lib/reader";
import Icon from "../common/Icon.vue";
const b = useBrowser();
const open = ref(false);
const preferences = computed(() => readerPreferences(b.state.readerPreferences));
const theme = computed(() => preferences.value.theme === "auto" ? b.state.theme : preferences.value.theme);
const position = computed(() => {
  const r = b.reader.rect;
  return r ? { left: `${r.x + r.width - 100}px`, top: `${r.y + 16}px` } : {};
});
function update(patch: Partial<ReaderPreferences>) {
  b.send({ type: "readerPreferences", preferences: { ...preferences.value, ...patch } });
}
function step(delta: number) { update({ size: Math.max(14, Math.min(32, preferences.value.size + delta)) }); }
watch(() => [b.reader.id, b.state.activeTab, b.peek], () => { open.value = false; });
</script>
<template>
  <div v-if="b.reader.id && b.reader.rect && !b.peek && !b.overlay" class="reader-controls" :style="position" :data-reader-theme="theme">
    <button class="reader-style-button" aria-label="阅读外观" title="文字和颜色" :aria-expanded="open" @click="open = !open">Aa</button>
    <button class="reader-style-button" aria-label="退出阅读模式" :title="b.shortcutTitle('退出阅读模式', 'reader')" @click="b.send({ type: 'readerClose' })"><Icon name="close" :size="14" /></button>
    <section v-if="open" class="reader-options" aria-label="阅读外观设置" @keydown.esc.stop.prevent="open = false">
      <div class="reader-size"><button aria-label="缩小阅读字号" :disabled="preferences.size <= 14" @click="step(-2)">A−</button><span>{{ preferences.size }} px</span><button aria-label="放大阅读字号" :disabled="preferences.size >= 32" @click="step(2)">A+</button></div>
      <fieldset><legend>字体</legend><div class="reader-choices"><button :aria-pressed="preferences.face === 'serif'" style="font-family:Georgia,serif" @click="update({ face: 'serif' })">衬线</button><button :aria-pressed="preferences.face === 'sans'" @click="update({ face: 'sans' })">无衬线</button></div></fieldset>
      <fieldset><legend>颜色</legend><div class="reader-choices"><button v-for="item in ([['auto', '自动'], ['light', '浅色'], ['sepia', '纸色'], ['dark', '深色']] as const)" :key="item[0]" :aria-pressed="preferences.theme === item[0]" @click="update({ theme: item[0] })">{{ item[1] }}</button></div></fieldset>
      <fieldset><legend>栏宽</legend><div class="reader-choices"><button v-for="item in ([['narrow', '窄'], ['medium', '适中'], ['wide', '宽']] as const)" :key="item[0]" :aria-pressed="preferences.width === item[0]" @click="update({ width: item[0] })">{{ item[1] }}</button></div></fieldset>
    </section>
  </div>
</template>
<style scoped>
.reader-controls { position:fixed; display:flex; gap:4px; z-index:20; --reader-paper:#fbfaf8; --reader-ink:#1f1e1c; --reader-rule:rgba(31,30,28,.12); color:var(--reader-ink); }
.reader-controls[data-reader-theme=dark] { --reader-paper:#1e1e20; --reader-ink:#e6e4df; --reader-rule:rgba(255,255,255,.15); }
.reader-controls[data-reader-theme=sepia] { --reader-paper:#f4ecde; --reader-ink:#3b2f24; --reader-rule:rgba(80,55,25,.16); }
.reader-style-button { width:32px; height:32px; display:grid; place-items:center; border-radius:8px; background:var(--reader-paper); color:inherit; font:16px/1 Georgia,serif; }
.reader-style-button:hover { background:color-mix(in srgb,var(--reader-ink) 8%,var(--reader-paper)); }
.reader-options { position:absolute; right:0; top:40px; width:248px; padding:16px; border:1px solid var(--reader-rule); border-radius:16px; background:var(--reader-paper); box-shadow:0 8px 28px #0002; font:13px/1.4 system-ui,sans-serif; }
fieldset { margin:16px 0 0; padding:0; border:0; }
legend { padding:0; margin-bottom:7px; opacity:.65; }
.reader-choices,.reader-size { display:flex; align-items:center; gap:4px; }
.reader-options button { flex:1; min-height:32px; border:1px solid transparent; border-radius:7px; color:inherit; }
.reader-options button:hover { background:var(--reader-rule); }
.reader-options button[aria-pressed=true] { border-color:var(--reader-rule); background:var(--reader-rule); }
.reader-options button:disabled { opacity:.3; }
.reader-size span { flex:1; text-align:center; font-variant-numeric:tabular-nums; }
.reader-size button { font:18px/1 Georgia,serif; }
button:focus-visible { outline:2px solid var(--reader-ink); outline-offset:2px; }
</style>
