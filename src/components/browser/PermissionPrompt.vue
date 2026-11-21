<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { permissions, type PermissionPrompt } from "../../lib/permissions";
import { useBrowser } from "../../stores/browser";
import Icon from "../common/Icon.vue";

const props = defineProps<{ prompt: PermissionPrompt }>();
const b = useBrowser();
const remember = ref(props.prompt.canRemember && props.prompt.remember);
const canAllow = ref(false);
const permission = computed(() => permissions[props.prompt.scope.kind]);
const paneStyle = computed(() => {
  const { x, y, width, height } = props.prompt.pane;
  return { left: `${x}px`, top: `${y}px`, width: `${width}px`, height: `${height}px` };
});
let timer: ReturnType<typeof setTimeout>;
onMounted(() => { timer = setTimeout(() => { canAllow.value = true; }, 500); });
onBeforeUnmount(() => clearTimeout(timer));

function answer(allowed: boolean | null) {
  b.send({ type: "permissionAnswer", id: props.prompt.id, allowed, remember: remember.value });
}
</script>

<template>
  <div class="permission-layer" :style="paneStyle" @click.self="answer(null)">
    <section class="permission-prompt" role="dialog" tabindex="-1"
      aria-labelledby="permission-question" aria-describedby="permission-requester">
      <div class="permission-heading">
        <div class="permission-icon"><Icon :name="permission.icon" :size="19" /></div>
        <div class="permission-source">
          <h2 id="permission-question">{{ permission.question }}</h2>
          <p id="permission-requester">
            <bdi>{{ prompt.scope.requester }}</bdi>
            <template v-if="prompt.scope.requester !== prompt.scope.page">
              嵌入在 <bdi>{{ prompt.scope.page }}</bdi>
            </template>
          </p>
        </div>
        <button class="icon-button permission-close" type="button" aria-label="暂不处理" title="暂不处理（Esc）" @click="answer(null)">
          <Icon name="close" :size="13" />
        </button>
      </div>
      <label v-if="prompt.canRemember" class="permission-remember">
        <input v-model="remember" type="checkbox" /> 记住对此网站的选择
      </label>
      <div class="permission-actions">
        <small v-if="prompt.waiting">另有 {{ prompt.waiting }} 个请求待处理</small>
        <button type="button" @click="answer(false)">不允许</button>
        <button type="button" class="permission-allow" :disabled="!canAllow" @click="answer(true)">允许</button>
      </div>
    </section>
  </div>
</template>

<style scoped>
.permission-layer {
  position: fixed;
  z-index: 88;
  display: flex;
  justify-content: center;
  align-items: flex-start;
  padding: 10px;
}
.permission-prompt {
  box-sizing: border-box;
  width: min(100%, 380px);
  max-height: 100%;
  padding: 14px;
  overflow: auto;
  color: var(--ink);
  background: var(--panel);
  border: 1px solid var(--panel-edge);
  border-radius: var(--radius-panel);
  box-shadow: 0 6px 18px var(--panel-shadow);
  outline: none;
}
.permission-heading { display: flex; align-items: flex-start; gap: 10px; }
.permission-icon {
  display: grid;
  flex: 0 0 34px;
  height: 34px;
  place-items: center;
  border-radius: 50%;
  color: var(--accent);
  background: var(--chosen);
}
.permission-source { flex: 1; min-width: 0; }
.permission-heading h2 { margin: 0; font-size: 14px; font-weight: 600; }
.permission-heading p { margin: 3px 0 0; color: var(--quiet); font-size: 12px; overflow-wrap: anywhere; }
.permission-close { flex: 0 0 auto; width: 24px; height: 24px; color: var(--quiet); }
.permission-remember { display: flex; align-items: center; gap: 7px; margin-top: 12px; font-size: 12px; }
.permission-remember input { margin: 0; accent-color: var(--accent); }
.permission-actions { display: flex; flex-wrap: wrap; justify-content: flex-end; align-items: center; gap: 7px; margin-top: 12px; }
.permission-actions small { margin-right: auto; color: var(--quiet); }
.permission-actions button {
  min-height: 30px;
  padding: 0 13px;
  border: 1px solid var(--panel-edge);
  border-radius: 7px;
  background: var(--control);
  color: var(--ink);
  font-weight: 550;
}
.permission-actions button:hover { background: var(--chosen); }
.permission-actions .permission-allow {
  background: var(--accent);
  color: light-dark(white, #17171a);
}
.permission-actions .permission-allow:hover { background: color-mix(in srgb, var(--accent), var(--ink) 12%); }
.permission-actions button:disabled { opacity: 0.55; }
</style>
