<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref } from "vue";
import { useBrowser } from "../../stores/browser";
import { commandContext } from "../../lib/command";
import Icon from "../common/Icon.vue";

const b = useBrowser();
const panel = ref<HTMLElement>();
const partner = computed(() => commandContext(b.state, b.recent).splitPartner);
const position = computed(() => {
  const anchor = b.creationMenu!;
  return { left: `${Math.max(8, anchor.x + anchor.width - 224)}px`, bottom: `${innerHeight - anchor.y + 8}px` };
});
function close(restoreFocus = false) {
  b.creationMenu = undefined;
  if (restoreFocus) document.querySelector<HTMLElement>(".creation-trigger")?.focus();
}
function outside(event: PointerEvent) {
  const target = event.target as HTMLElement;
  if (!panel.value?.contains(target) && !target.closest(".creation-trigger")) close();
}
function key(event: KeyboardEvent) {
  if (event.isComposing) return;
  if (event.key === "Escape") {
    event.preventDefault();
    event.stopPropagation();
    close(true);
  } else if (["ArrowDown", "ArrowUp", "Home", "End"].includes(event.key)) {
    event.preventDefault();
    event.stopPropagation();
    const items = Array.from(panel.value?.querySelectorAll<HTMLButtonElement>("button:not(:disabled)") ?? []);
    const current = items.indexOf(document.activeElement as HTMLButtonElement);
    const index = event.key === "Home" ? 0 : event.key === "End" ? items.length - 1
      : (current + (event.key === "ArrowDown" ? 1 : -1) + items.length) % items.length;
    items[index]?.focus();
  } else if (event.key === "Tab") close(true);
}
function split() {
  const other = partner.value;
  close();
  if (b.active && other) b.send({ type: "split", id: b.active.id, other: other.id });
}
onMounted(async () => {
  window.addEventListener("pointerdown", outside, true);
  window.addEventListener("keydown", key, true);
  window.addEventListener("resize", dismiss);
  await nextTick();
  panel.value?.querySelector<HTMLButtonElement>("button:not(:disabled)")?.focus();
});
function dismiss() { close(); }
onBeforeUnmount(() => {
  window.removeEventListener("pointerdown", outside, true);
  window.removeEventListener("keydown", key, true);
  window.removeEventListener("resize", dismiss);
});
</script>

<template>
  <div class="menu-scrim creation-menu-scrim" @pointerdown.self="close()">
    <section ref="panel" id="creation-menu" class="context-menu creation-menu" role="menu" aria-label="新建" :style="position">
      <button role="menuitem" :disabled="b.state.spaces.length >= 16" :title="b.state.spaces.length >= 16 ? '最多创建 16 个工作区' : undefined" @click="b.openSpaceCreation()">
        <Icon name="spaces" :size="18" /><span>新建 Space</span>
      </button>
      <hr role="separator" />
      <button role="menuitem" :disabled="!partner || Boolean(b.spaceCreation)" @click="split">
        <Icon name="split" :size="18" /><span>新建分屏</span>
      </button>
      <button role="menuitem" @click="b.openCommand()">
        <Icon name="plus" :size="18" /><span>新建标签页</span>
      </button>
    </section>
  </div>
</template>
