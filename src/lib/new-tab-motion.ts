import { onBeforeUnmount, watch, type Ref } from "vue";

const moveEase = "cubic-bezier(0.25, 0.8, 0.25, 1)";
const placementEase = "cubic-bezier(0.4, 0, 0.2, 1)";
const layoutItems = ".nt-section-header h2, .nt-widget-grid > .nt-widget";

function invert(from: DOMRect, to: DOMRect) {
  const x = from.x - to.x;
  const y = from.y - to.y;
  return `translate(${x}px, ${y}px) scale(${from.width / to.width}, ${from.height / to.height})`;
}

export function useNewTabMotion(root: Ref<HTMLElement | undefined>, reduceMotion: () => boolean) {
  const systemMotion = matchMedia("(prefers-reduced-motion: reduce)");
  const layoutAnimations = new Set<Animation>();
  let cancelPlacement: (() => void) | undefined;

  function clear() {
    for (const animation of layoutAnimations) animation.cancel();
    layoutAnimations.clear();
    cancelPlacement?.();
  }

  watch(reduceMotion, reduced => { if (reduced) clear(); });
  function onSystemMotionChange() { if (systemMotion.matches) clear(); }
  systemMotion.addEventListener("change", onSystemMotionChange);
  onBeforeUnmount(() => {
    clear();
    systemMotion.removeEventListener("change", onSystemMotionChange);
  });

  function captureLayout() {
    const reduced = reduceMotion() || systemMotion.matches;
    const positions = reduced ? [] : Array.from(root.value?.querySelectorAll<HTMLElement>(layoutItems) ?? [],
      element => ({ element, rect: element.getBoundingClientRect() }));
    clear();
    return positions;
  }

  function moveLayout(positions: ReturnType<typeof captureLayout>) {
    if (reduceMotion() || systemMotion.matches) return;
    // Read every destination before applying transforms to the live cards.
    const destinations = positions.map(({ element, rect }) => ({ element, rect, target: element.getBoundingClientRect() }));
    for (const { element, rect, target } of destinations) {
      if (!rect.width || !rect.height || !target.width || !target.height) continue;
      if (rect.x === target.x && rect.y === target.y && rect.width === target.width && rect.height === target.height) continue;
      const animation = element.animate({
        transformOrigin: ["0 0", "0 0"],
        transform: [invert(rect, target), "none"],
      }, { duration: 360, easing: moveEase, fill: "backwards" });
      layoutAnimations.add(animation);
      const forget = () => layoutAnimations.delete(animation);
      void animation.finished.then(forget, forget);
    }
  }

  function placeWidget(widget: HTMLElement, from: DOMRect) {
    cancelPlacement?.();
    const layer = root.value?.querySelector<HTMLElement>(".nt-flight-layer");
    const canvas = root.value?.querySelector<HTMLElement>(".nt-canvas");
    const layout = root.value?.querySelector<HTMLElement>(".nt-layout");
    if (!layer || !canvas || !layout) return;

    const scroller = getComputedStyle(canvas).overflowY === "auto" ? canvas : layout;
    const viewport = scroller.getBoundingClientRect();
    const target = widget.getBoundingClientRect();
    const initialScroll = scroller.scrollTop;
    const offset = target.top < viewport.top + 16 || target.height + 32 > scroller.clientHeight
      ? target.top - viewport.top - 16
      : Math.max(0, target.bottom - viewport.top - scroller.clientHeight + 16);
    const endScroll = Math.max(0, Math.min(initialScroll + offset, scroller.scrollHeight - scroller.clientHeight));
    if (reduceMotion() || systemMotion.matches || !from.width || !from.height || !target.width || !target.height) {
      scroller.scrollTop = endScroll;
      return;
    }

    const frame = layer.getBoundingClientRect();
    const to = new DOMRect(target.x, target.y + initialScroll - endScroll, target.width, target.height);
    // The flight must paint above the menu and outside the canvas scroller.
    const copy = document.createElement("div");
    copy.className = `${widget.className} nt-flight-card`;
    copy.append(...Array.from(widget.children, child => child.cloneNode(true)));
    for (const element of copy.querySelectorAll("[id]")) element.removeAttribute("id");
    Object.assign(copy.style, {
      left: `${to.x - frame.x}px`, top: `${to.y - frame.y}px`,
      width: `${to.width}px`, height: `${to.height}px`,
    });
    widget.classList.add("nt-widget-placing");
    layer.append(copy);
    const animation = copy.animate({ transform: [invert(from, to), "none"] }, {
      duration: 420, easing: placementEase, fill: "both",
    });

    let scrollFrame = 0;
    function syncScroll() {
      const progress = animation.effect?.getComputedTiming().progress ?? 0;
      scroller.scrollTop = initialScroll + (endScroll - initialScroll) * progress;
      scrollFrame = requestAnimationFrame(syncScroll);
    }
    if (endScroll !== initialScroll) scrollFrame = requestAnimationFrame(syncScroll);

    function cleanup() {
      cancelAnimationFrame(scrollFrame);
      widget.removeEventListener("focusin", finish);
      scroller.removeEventListener("wheel", cleanup);
      scroller.removeEventListener("touchstart", cleanup);
      widget.classList.remove("nt-widget-placing");
      copy.remove();
      animation.cancel();
      cancelPlacement = undefined;
    }
    function finish() {
      if (!copy.isConnected) return;
      scroller.scrollTop = endScroll;
      cleanup();
    }
    cancelPlacement = cleanup;
    widget.addEventListener("focusin", finish, { once: true });
    scroller.addEventListener("wheel", cleanup, { passive: true });
    scroller.addEventListener("touchstart", cleanup, { passive: true });
    void animation.finished.then(finish, () => {});
  }

  return { captureLayout, moveLayout, placeWidget, stopPlacement: () => cancelPlacement?.() };
}
