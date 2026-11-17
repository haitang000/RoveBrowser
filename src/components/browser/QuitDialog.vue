<script setup lang="ts">
import { onMounted, onBeforeUnmount, ref } from "vue";
import { useBrowser } from "../../stores/browser";

const b = useBrowser();
const dialog = ref<HTMLDialogElement>();
function respond(confirm: boolean, always = false) {
  b.send({ type: "quitResponse", confirm, always });
}
function keydown(event: KeyboardEvent) {
  event.stopPropagation();
  if (event.key === "Tab") {
    const buttons = dialog.value!.querySelectorAll<HTMLButtonElement>("button");
    const first = buttons[0]!, last = buttons[buttons.length - 1]!;
    if (event.target === dialog.value || (event.shiftKey ? event.target === first : event.target === last)) {
      event.preventDefault();
      (event.shiftKey ? last : first).focus();
    }
    return;
  }
  if (event.isComposing || event.repeat || event.ctrlKey || event.altKey || event.metaKey || event.shiftKey) return;
  if (event.key === "Escape") {
    event.preventDefault();
    respond(false);
  } else if (event.key === "Enter" && event.target === dialog.value) {
    event.preventDefault();
    respond(true);
  }
}
onMounted(() => {
  dialog.value?.showModal();
  dialog.value?.focus();
});
onBeforeUnmount(() => dialog.value?.close());
</script>

<template>
  <div class="quit-scrim">
    <dialog ref="dialog" class="quit-dialog" role="alertdialog" aria-modal="true" tabindex="-1"
      aria-labelledby="quit-title" aria-describedby="quit-description"
      @cancel.prevent="respond(false)" @keydown="keydown">
      <h2 id="quit-title">确定要退出 Rove 吗？</h2>
      <p id="quit-description">标签页中未保存的内容可能会丢失。</p>
      <div class="quit-actions">
        <button class="quit-always" @click="respond(true, true)">始终退出</button>
        <button class="quit-cancel" aria-label="取消" @click="respond(false)">取消 <kbd aria-hidden="true">ESC</kbd></button>
        <button class="quit-confirm" aria-label="退出" @click="respond(true)">退出 <kbd aria-hidden="true">ENTER</kbd></button>
      </div>
    </dialog>
  </div>
</template>

<style scoped>
.quit-scrim {
  position: fixed;
  inset: 0;
  z-index: 1000;
  background: rgb(0 0 0 / 0.32);
}
.quit-dialog {
  color-scheme: dark;
  width: min(568px, calc(100vw - 40px));
  max-width: none;
  max-height: calc(100vh - 40px);
  margin: auto;
  padding: 32px;
  overflow: auto;
  border: 1px solid rgb(255 255 255 / 0.08);
  border-radius: 12px;
  background: #212121;
  color: #fafafa;
  box-shadow: 0 24px 80px rgb(0 0 0 / 0.3);
  outline: none;
}
.quit-dialog::backdrop { background: transparent; }
.quit-dialog:focus-visible { outline: none; }
.quit-dialog h2 {
  margin: 0;
  font-size: 24px;
  line-height: 1.4;
  font-weight: 600;
  letter-spacing: -0.025em;
  text-wrap: balance;
}
.quit-dialog p {
  margin: 16px 0 32px;
  color: #c4c4c4;
  font-size: 17px;
  line-height: 1.6;
}
.quit-actions {
  display: flex;
  align-items: center;
  gap: 12px;
}
.quit-actions button {
  display: inline-flex;
  justify-content: center;
  align-items: center;
  gap: 10px;
  min-height: 50px;
  padding: 0 20px;
  border: 1px solid rgb(255 255 255 / 0.025);
  border-radius: 7px;
  background: #2c2c2c;
  color: #fafafa;
  font-size: 18px;
  line-height: 1.2;
  white-space: nowrap;
  transition: background-color 120ms ease, transform 120ms ease;
}
.quit-actions .quit-always { margin-right: auto; }
.quit-actions .quit-confirm { background: #ff98a8; color: #171214; }
.quit-actions button:focus-visible { outline: 2px solid #ffb3c0; outline-offset: 3px; }
.quit-actions button:active { transform: scale(0.97); background: #383838; }
.quit-actions .quit-confirm:active { background: #ed8798; }
.quit-actions kbd {
  padding: 4px 8px;
  border: 1px solid rgb(255 255 255 / 0.06);
  border-radius: 6px;
  color: #c4c4c4;
  font-size: 12px;
  font-weight: 600;
  line-height: 1.2;
}
.quit-confirm kbd { background: rgb(255 255 255 / 0.07); color: #642f38; }
@media (hover: hover) and (pointer: fine) {
  .quit-actions button:hover { background: #353535; }
  .quit-actions .quit-confirm:hover { background: #ffabb8; }
}
@media (max-width: 560px) {
  .quit-dialog { padding: 24px; }
  .quit-dialog h2 { font-size: 22px; }
  .quit-actions { flex-wrap: wrap; }
  .quit-actions .quit-always { width: 100%; margin: 0; }
  .quit-cancel, .quit-confirm { flex: 1; }
}
@media (prefers-reduced-motion: reduce) {
  .quit-actions button { transition: none; }
  .quit-actions button:active { transform: none; }
}
:global([data-reduce-motion="true"]) .quit-actions button { transition: none; }
:global([data-reduce-motion="true"]) .quit-actions button:active { transform: none; }
</style>
