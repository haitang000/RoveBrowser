<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useBrowser } from "../../stores/browser";
import { readerPreferences, type ReaderPreferences } from "../../lib/reader";
import Icon from "../common/Icon.vue";
const b = useBrowser();
const open = ref(false);
const appearanceButton = ref<HTMLButtonElement>();
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
function closeOptions() { open.value = false; appearanceButton.value?.focus({ preventScroll: true }); }
watch(() => [b.reader.id, b.state.activeTab, b.peek], () => { open.value = false; });
</script>
<template>
  <Transition name="reader-controls" appear>
  <div v-if="b.reader.id && b.reader.rect && !b.peek && !b.overlay" :key="b.reader.id" class="reader-controls" :class="{ 'is-closing': b.reader.closing }" :inert="b.reader.closing" :style="position" :data-reader-theme="theme">
    <button ref="appearanceButton" class="reader-style-button" :aria-label="b.t('阅读外观')" :title="b.t('文字和颜色')" :aria-expanded="open" aria-controls="reader-options" @click="open = !open">Aa</button>
    <button class="reader-style-button" :aria-label="b.t('退出阅读模式')" :title="b.shortcutTitle(b.t('退出阅读模式'), 'reader')" @click="b.send({ type: 'readerClose' })"><Icon name="close" :size="14" /></button>
    <Transition name="bubble">
      <div v-if="open" class="reader-options-layer" @keydown.esc.stop.prevent="closeOptions">
        <section id="reader-options" class="reader-options bubble-surface" :aria-label="b.t('阅读外观设置')">
          <div class="reader-size"><button :aria-label="b.t('缩小阅读字号')" :disabled="preferences.size <= 14" @click="step(-2)">A−</button><span class="reader-size-value" aria-live="polite"><Transition name="reader-value"><span :key="preferences.size">{{ preferences.size }} px</span></Transition></span><button :aria-label="b.t('放大阅读字号')" :disabled="preferences.size >= 32" @click="step(2)">A+</button></div>
          <fieldset><legend>{{ b.t('字体') }}</legend><div class="reader-choices" :style="{ '--choice-count': 2, '--choice-index': preferences.face === 'sans' ? 1 : 0 }"><span class="reader-choice-highlight" aria-hidden="true" /><button :aria-pressed="preferences.face === 'serif'" style="font-family:Georgia,serif" @click="update({ face: 'serif' })">{{ b.t('衬线') }}</button><button :aria-pressed="preferences.face === 'sans'" @click="update({ face: 'sans' })">{{ b.t('无衬线') }}</button></div></fieldset>
          <fieldset><legend>{{ b.t('颜色') }}</legend><div class="reader-choices" :style="{ '--choice-count': 4, '--choice-index': ['auto', 'light', 'sepia', 'dark'].indexOf(preferences.theme) }"><span class="reader-choice-highlight" aria-hidden="true" /><button v-for="item in ([['auto', b.t('自动')], ['light', b.t('浅色')], ['sepia', b.t('纸色')], ['dark', b.t('深色')]] as const)" :key="item[0]" :aria-pressed="preferences.theme === item[0]" @click="update({ theme: item[0] })">{{ item[1] }}</button></div></fieldset>
          <fieldset><legend>{{ b.t('栏宽') }}</legend><div class="reader-choices" :style="{ '--choice-count': 3, '--choice-index': ['narrow', 'medium', 'wide'].indexOf(preferences.width) }"><span class="reader-choice-highlight" aria-hidden="true" /><button v-for="item in ([['narrow', b.t('窄')], ['medium', b.t('适中')], ['wide', b.t('宽')]] as const)" :key="item[0]" :aria-pressed="preferences.width === item[0]" @click="update({ width: item[0] })">{{ item[1] }}</button></div></fieldset>
        </section>
      </div>
    </Transition>
  </div>
  </Transition>
</template>
<style scoped>
.reader-controls { position:fixed; display:flex; gap:4px; z-index:20; --reader-paper:#fbfaf8; --reader-ink:#1f1e1c; --reader-rule:rgba(31,30,28,.12); color:var(--reader-ink); opacity:1; transition:opacity 160ms cubic-bezier(.23,1,.32,1),color 220ms ease; }
.reader-controls-enter-active { transition-duration:220ms; }
.reader-controls-enter-from,.reader-controls.is-closing { opacity:0; pointer-events:none; }
.reader-controls-leave-active { transition:none; }
:global(html[data-reduce-motion="true"] .reader-controls) { transition-duration:80ms !important; }
@media (prefers-reduced-motion: reduce) { .reader-controls { transition-duration:80ms !important; } }
.reader-controls[data-reader-theme=dark] { --reader-paper:#1e1e20; --reader-ink:#e6e4df; --reader-rule:rgba(255,255,255,.15); }
.reader-controls[data-reader-theme=sepia] { --reader-paper:#f4ecde; --reader-ink:#3b2f24; --reader-rule:rgba(80,55,25,.16); }
.reader-style-button { width:32px; height:32px; display:grid; place-items:center; border-radius:8px; background:var(--reader-paper); color:inherit; font:16px/1 Georgia,serif; transition:background-color 220ms ease,transform 120ms ease; }
/* Global `svg { color: var(--icon) }` follows the workspace. The close glyph follows the reader ink. */
.reader-style-button :deep(svg) { color: inherit; }
.reader-style-button:hover { background:color-mix(in srgb,var(--reader-ink) 8%,var(--reader-paper)); }
.reader-style-button:active { transform:scale(.97); }
/* Keep native clip bounds stable while the inner surface scales/overshoots. */
.reader-options-layer { position:absolute; right:0; top:40px; width:248px; --bubble-enter-y:-7px; --bubble-exit-y:-5px; }
.reader-options { position:relative; width:100%; padding:16px; border:1px solid var(--reader-rule); border-radius:16px; background:var(--reader-paper); box-shadow:0 8px 28px #0002; font:13px/1.4 system-ui,sans-serif; transform-origin:calc(100% - 52px) -8px; transition:background-color 220ms ease,border-color 220ms ease; }
.reader-options::before { content:""; position:absolute; top:-5px; right:46px; width:8px; height:8px; background:inherit; border-top:1px solid var(--reader-rule); border-left:1px solid var(--reader-rule); transform:rotate(45deg); transition:border-color 220ms ease; }
fieldset { margin:16px 0 0; padding:0; border:0; }
legend { padding:0; margin-bottom:7px; opacity:.65; }
.reader-choices,.reader-size { position:relative; display:flex; align-items:center; gap:4px; }
.reader-choice-highlight { position:absolute; inset:0 auto 0 0; width:calc((100% - (var(--choice-count) - 1) * 4px) / var(--choice-count)); border:1px solid var(--reader-rule); border-radius:7px; background:var(--reader-rule); transform:translateX(calc(var(--choice-index) * (100% + 4px))); transition:transform 220ms cubic-bezier(.23,1,.32,1),background-color 220ms ease,border-color 220ms ease; pointer-events:none; }
.reader-options button { position:relative; flex:1; min-width:0; min-height:32px; border:1px solid transparent; border-radius:7px; color:inherit; transition:background-color 160ms ease,opacity 160ms ease,transform 120ms ease; }
.reader-options button:hover { background:var(--reader-rule); }
.reader-options button[aria-pressed=true]:hover { background:transparent; }
.reader-options button:active:enabled { transform:scale(.97); }
.reader-options button:disabled { opacity:.3; }
.reader-size-value { flex:1; display:grid; text-align:center; font-variant-numeric:tabular-nums; }
.reader-size-value > span { grid-area:1 / 1; }
.reader-value-enter-active,.reader-value-leave-active { transition:opacity 140ms ease,transform 140ms cubic-bezier(.23,1,.32,1); }
.reader-value-enter-from { opacity:0; transform:translateY(3px); }
.reader-value-leave-to { opacity:0; transform:translateY(-3px); }
.reader-size button { font:18px/1 Georgia,serif; }
:global(html[data-reduce-motion="true"] .reader-choice-highlight) { transition:background-color 80ms ease,border-color 80ms ease !important; }
:global(html[data-reduce-motion="true"] .reader-size-value > span) { transform:none; }
@media (prefers-reduced-motion: reduce) { .reader-choice-highlight { transition:background-color 80ms ease,border-color 80ms ease !important; } .reader-size-value > span { transform:none; } }
button:focus-visible { outline:2px solid var(--reader-ink); outline-offset:2px; }
</style>
