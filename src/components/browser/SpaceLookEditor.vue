<script setup lang="ts">
// Color wheel, shade, presets and effects, from Zigzag/Sidebar/SpaceEditor.swift.
// Copyright 2026 Zigao Wang, Apache-2.0. Modified for Rove.
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import Icon from "../common/Icon.vue";
import type { SpaceColor } from "../../lib/types";
import { paint } from "../../lib/space-color";
import {
  PRESETS,
  type SpaceLook,
  applyPreset,
  committed,
  depthAt,
  hasDefaultColors,
  lookMatches,
  lookOf,
  moveSpot,
  openDraft,
  shadeTo,
  spotAt,
  spotPoint,
  spotsOf,
  swatchBackground,
  toggleSecond,
  useDefaultColors,
  type LookDraft,
  type LookFields,
  type Spot,
} from "../../lib/space-look";

const props = defineProps<{ space: LookFields; dark: boolean }>();
const emit = defineEmits<{ preview: [LookFields]; commit: [LookFields] }>();

const original = lookOf(props.space);
const draft = ref<LookDraft>(openDraft(props.space, props.dark));
const effects = ref(props.space.hue2 != null || (props.space.translucency ?? 0) > 0.005 || (props.space.grain ?? 0) > 0.01);
const wheel = ref<HTMLElement>();
const canvas = ref<HTMLCanvasElement>();
const wheelSize = ref({ width: 252, height: 200 });
let dragging = false;

function saved(): LookFields {
  return committed(draft.value, original);
}
function preview() {
  emit("preview", saved());
}
function commit() {
  emit("commit", saved());
}

const shown = computed(() => spotsOf(draft.value, props.dark));
const selectedSpot = computed(() => shown.value.spots[draft.value.selected] ?? shown.value.spots[0]!);
const colors = computed(() => {
  const spot = selectedSpot.value;
  return [0, 0.2, 0.4, 0.6, 0.8, 1].map((depth) => paint(spot.hue, depth, spot.amount));
});
const frameColors = computed(() => {
  const look = saved();
  const first = paint(shown.value.spots[0]!.hue, shown.value.depths[0]!, shown.value.spots[0]!.amount);
  const second = shown.value.spots[1]
    ? paint(shown.value.spots[1].hue, shown.value.depths[1]!, shown.value.spots[1].amount)
    : first;
  return { first, second, look };
});
const shadeStops = computed(() => `linear-gradient(90deg, ${colors.value.join(", ")})`);
const clearStops = computed(
  () => `linear-gradient(90deg, ${frameColors.value.first}, color-mix(in srgb, ${frameColors.value.second} 20%, transparent))`,
);
const canReset = computed(() => !hasDefaultColors(saved()));
const two = computed(() => draft.value.hue2 != null);

function paintWheel() {
  const box = wheel.value;
  const surface = canvas.value;
  if (!box || !surface) return;
  const width = box.clientWidth;
  const height = box.clientHeight;
  wheelSize.value = { width, height };
  const scale = window.devicePixelRatio || 1;
  surface.width = Math.max(1, Math.round(width * scale));
  surface.height = Math.max(1, Math.round(height * scale));
  const context = surface.getContext("2d");
  if (!context) return;
  context.setTransform(scale, 0, 0, scale, 0, 0);
  context.clearRect(0, 0, width, height);
  const depth = shown.value.depths[draft.value.selected] ?? shown.value.depths[0]!;
  const cx = width / 2;
  const cy = height / 2;
  const radius = Math.min(width, height) / 2 - 14;
  const step = 9.6;
  const rowStep = step * 0.866;
  const dot = 6;
  const rows = Math.floor(radius / rowStep);
  for (let row = -rows; row <= rows; row++) {
    const y = cy + row * rowStep;
    const offset = row % 2 === 0 ? 0 : step / 2;
    const columns = Math.floor(radius / step) + 1;
    for (let column = -columns; column <= columns; column++) {
      const x = cx + column * step + offset;
      if (Math.hypot(x - cx, y - cy) > radius - dot / 2 + 0.5) continue;
      const spot = spotAt(x, y, width, height, 0);
      context.fillStyle = paint(spot.hue, depth, spot.amount);
      context.beginPath();
      context.ellipse(x, y, dot / 2, dot / 2, 0, 0, Math.PI * 2);
      context.fill();
    }
  }
}

function knobStyle(spot: Spot) {
  const { width, height } = wheelSize.value;
  const point = spotPoint(spot, width, height);
  return { left: `${point.x}px`, top: `${point.y}px` };
}

function linkStyle() {
  const [a, b] = shown.value.spots;
  if (!a || !b) return { display: "none" };
  const { width, height } = wheelSize.value;
  const from = spotPoint(a, width, height);
  const to = spotPoint(b, width, height);
  const dx = to.x - from.x;
  const dy = to.y - from.y;
  const length = Math.hypot(dx, dy);
  const angle = (Math.atan2(dy, dx) * 180) / Math.PI;
  return {
    left: `${from.x}px`,
    top: `${from.y}px`,
    width: `${length}px`,
    transform: `rotate(${angle}deg)`,
  };
}

function pointerSpot(event: PointerEvent): Spot {
  const box = wheel.value!;
  const rect = box.getBoundingClientRect();
  const current = shown.value.spots[draft.value.selected] ?? shown.value.spots[0]!;
  return spotAt(event.clientX - rect.left, event.clientY - rect.top, rect.width, rect.height, current.hue);
}

function onWheelDown(event: PointerEvent) {
  if (event.button !== 0) return;
  const target = event.target as HTMLElement;
  const index = target.dataset.knob === "1" ? 1 : draft.value.selected;
  if (target.dataset.knob != null) draft.value = { ...draft.value, selected: index as 0 | 1 };
  dragging = true;
  (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
  draft.value = moveSpot(draft.value, (target.dataset.knob === "1" ? 1 : draft.value.selected) as 0 | 1, pointerSpot(event));
  preview();
}
function onWheelMove(event: PointerEvent) {
  if (!dragging) return;
  draft.value = moveSpot(draft.value, draft.value.selected, pointerSpot(event));
  preview();
}
function onWheelUp() {
  if (!dragging) return;
  dragging = false;
  commit();
}
function nudge(event: KeyboardEvent, index: 0 | 1) {
  const spot = shown.value.spots[index];
  if (!spot) return;
  const step = event.key === "ArrowUp" || event.key === "ArrowRight" ? 1 : event.key === "ArrowDown" || event.key === "ArrowLeft" ? -1 : 0;
  if (!step) return;
  event.preventDefault();
  const vertical = event.key === "ArrowUp" || event.key === "ArrowDown";
  draft.value = moveSpot(draft.value, index, {
    hue: (spot.hue + (vertical ? 0 : step / 36) + 1) % 1,
    amount: Math.min(1, Math.max(0, spot.amount + (vertical ? step * 0.05 : 0))),
  });
  commit();
}

function pickPreset(index: number) {
  const look = PRESETS[index];
  if (!look) return;
  const next = openDraft(applyPreset(draft.value, look), props.dark);
  next.colorChosen = true;
  next.brightness = look.depth;
  draft.value = next;
  commit();
}
function onShade(event: Event, final: boolean) {
  const input = event.target as HTMLInputElement;
  const alt = (event as MouseEvent).altKey || input.dataset.alt === "1";
  draft.value = shadeTo(draft.value, Number(input.value), alt, props.dark);
  if (final) commit();
  else preview();
}
function setAmount(key: "translucency" | "grain", value: number, final: boolean) {
  const next = { ...draft.value };
  if (key === "translucency") next.translucency = value > 0.005 ? value : undefined;
  else next.grain = value > 0.01 ? value : undefined;
  draft.value = next;
  if (final) commit();
  else preview();
}
function setAngle(value: number, final: boolean) {
  const nearest = Math.round(value / 45) * 45;
  const angle = Math.abs(nearest - value) < 8 ? ((nearest % 360) + 360) % 360 : (value + 360) % 360;
  draft.value = { ...draft.value, angle };
  if (final) commit();
  else preview();
}
function dragDial(event: PointerEvent) {
  const dial = event.currentTarget as HTMLElement;
  const move = (e: PointerEvent, final: boolean) => {
    const rect = dial.getBoundingClientRect();
    let angle = (Math.atan2(e.clientY - rect.top - rect.height / 2, e.clientX - rect.left - rect.width / 2) * 180) / Math.PI;
    if (angle < 0) angle += 360;
    setAngle(angle, final);
  };
  move(event, false);
  const end = (e: PointerEvent) => {
    move(e, true);
    window.removeEventListener("pointermove", drag);
    window.removeEventListener("pointerup", end);
  };
  const drag = (e: PointerEvent) => move(e, false);
  window.addEventListener("pointermove", drag);
  window.addEventListener("pointerup", end);
}
function swatchStyle(look: SpaceLook) {
  const image = swatchBackground(look);
  if (look.seeThrough <= 0.005) return { background: image };
  return { backgroundImage: image, backgroundSize: "auto, 8px 8px" };
}
function resetColors() {
  draft.value = useDefaultColors(original.color as SpaceColor, props.dark);
  commit();
}
function second() {
  draft.value = toggleSecond(draft.value, props.dark);
  commit();
}

let observer: ResizeObserver | undefined;
onMounted(() => {
  paintWheel();
  if (wheel.value) {
    observer = new ResizeObserver(() => paintWheel());
    observer.observe(wheel.value);
  }
});
onBeforeUnmount(() => observer?.disconnect());
watch(shown, () => paintWheel(), { deep: true });
watch(
  () => props.dark,
  (dark) => {
    if (!draft.value.colorChosen) draft.value = { ...draft.value, brightness: openDraft(saved(), dark).brightness };
  },
);
</script>
<template>
  <div class="space-look">
    <div class="look-presets" role="group" aria-label="预设外观">
      <button
        v-for="(look, index) in PRESETS"
        :key="look.name"
        type="button"
        class="look-swatch"
        :class="{ chosen: lookMatches(look, saved()), through: look.seeThrough > 0 }"
        :aria-label="look.name"
        :aria-pressed="lookMatches(look, saved())"
        :title="look.name"
        :style="swatchStyle(look)"
        @click="pickPreset(index)"
      />
    </div>
    <div
      ref="wheel"
      class="color-wheel"
      @pointerdown="onWheelDown"
      @pointermove="onWheelMove"
      @pointerup="onWheelUp"
      @pointercancel="onWheelUp"
    >
      <canvas ref="canvas" aria-hidden="true" />
      <span v-if="two" class="wheel-link" :style="linkStyle()" />
      <button
        v-for="(spot, index) in shown.spots"
        :key="index"
        type="button"
        class="wheel-knob"
        :class="{ selected: draft.selected === index }"
        :data-knob="index"
        role="slider"
        :aria-label="index === 0 ? '颜色' : '第二种颜色'"
        :style="{ ...knobStyle(spot), background: paint(spot.hue, shown.depths[index]!, spot.amount) }"
        @keydown="nudge($event, index as 0 | 1)"
      />
    </div>
    <label class="look-row">
      <span>明暗</span>
      <span class="look-slider" :style="{ '--track': shadeStops }">
        <input
          :value="depthAt(draft, draft.selected, dark)"
          type="range"
          min="0"
          max="1"
          step="0.01"
          aria-label="明暗"
          @pointerdown="($event.target as HTMLInputElement).dataset.alt = $event.altKey ? '1' : ''"
          @input="onShade($event, false)"
          @change="onShade($event, true)"
        />
      </span>
    </label>
    <div class="look-fold">
      <button type="button" class="look-disclosure" :aria-expanded="effects" @click="effects = !effects">
        <span>效果</span><Icon name="chevronDown" :size="12" />
      </button>
      <button type="button" class="look-reset" :disabled="!canReset" @click="resetColors">恢复默认颜色</button>
    </div>
    <div v-show="effects" class="look-effects">
      <div class="look-row look-gradient">
        <span>渐变</span>
        <button
          type="button"
          class="color-chip"
          :class="{ selected: two && draft.selected === 0 }"
          aria-label="颜色"
          :style="{ background: frameColors.first }"
          @click="draft.selected = 0"
        />
        <button
          v-if="two"
          type="button"
          class="color-chip second"
          :class="{ selected: draft.selected === 1 }"
          aria-label="第二种颜色"
          :style="{ background: frameColors.second }"
          @click="draft.selected = 1"
        >
          <span class="chip-remove" aria-label="去掉第二种颜色" @click.stop="second" @keydown.enter.stop="second">×</span>
        </button>
        <button v-else type="button" class="color-chip add" aria-label="添加第二种颜色" @click="second">+</button>
        <button
          v-show="two"
          type="button"
          class="direction-dial"
          role="slider"
          aria-label="渐变方向"
          :aria-valuemin="0"
          :aria-valuemax="360"
          :aria-valuenow="Math.round(draft.angle ?? 45)"
          :style="{ '--dial': `linear-gradient(${(draft.angle ?? 45) + 90}deg, ${frameColors.first}, ${frameColors.second})` }"
          @pointerdown.prevent="dragDial"
        >
          <span class="dial-ticks" />
          <span class="dial-needle" :style="{ transform: `rotate(${draft.angle ?? 45}deg)` }" />
        </button>
      </div>
      <label class="look-row">
        <span>清透</span>
        <span class="look-slider through" :style="{ '--track': clearStops }">
          <input
            :value="draft.translucency ?? 0"
            type="range"
            min="0"
            max="1"
            step="0.01"
            aria-label="清透"
            @input="setAmount('translucency', Number(($event.target as HTMLInputElement).value), false)"
            @change="setAmount('translucency', Number(($event.target as HTMLInputElement).value), true)"
          />
        </span>
      </label>
      <label class="look-row">
        <span>颗粒</span>
        <span class="look-slider grain" :style="{ '--track': `linear-gradient(90deg, ${frameColors.first}, ${frameColors.second})` }">
          <input
            :value="draft.grain ?? 0"
            type="range"
            min="0"
            max="1"
            step="0.01"
            aria-label="颗粒"
            @input="setAmount('grain', Number(($event.target as HTMLInputElement).value), false)"
            @change="setAmount('grain', Number(($event.target as HTMLInputElement).value), true)"
          />
        </span>
      </label>
    </div>
  </div>
</template>
<style src="../../styles/space-editor.css"></style>
