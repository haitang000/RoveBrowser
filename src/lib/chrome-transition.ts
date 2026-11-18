import { nextTick, onBeforeUnmount, watch, type Ref } from "vue";

const chromeItems = "[data-chrome-motion]";
const easeOut = "cubic-bezier(0.23, 1, 0.32, 1)";

export function useChromeTransition(
  shell: Ref<HTMLElement | undefined>,
  split: Ref<boolean>,
  onboarding: Ref<boolean>,
  reducedMotion: Ref<boolean>,
) {
  let transition: { layer: HTMLElement; animations: Animation[] } | undefined;

  function clear() {
    transition?.layer.remove();
    transition?.animations.forEach(animation => animation.cancel());
    transition = undefined;
  }
  onBeforeUnmount(clear);
  watch(reducedMotion, reduced => { if (reduced) clear(); });

  watch([split, onboarding], async ([, onboard], [, wasOnboard]) => {
    const root = shell.value;
    if (!root || onboard || wasOnboard) {
      clear();
      return;
    }
    const previousOpacity = new Map(
      [...(transition?.layer.children ?? [])].map(element => [
        (element as HTMLElement).dataset.chromeCopy,
        getComputedStyle(element).opacity,
      ]),
    );
    const layer = document.createElement("div");
    layer.className = "chrome-outgoing";
    layer.inert = true;
    layer.setAttribute("aria-hidden", "true");
    const frame = root.getBoundingClientRect();
    for (const element of root.querySelectorAll<HTMLElement>(chromeItems)) {
      if (element.closest("[inert]")) continue;
      const { x, y, width, height, right } = element.getBoundingClientRect();
      const { opacity, padding, visibility } = getComputedStyle(element);
      if (!width || !height || opacity === "0" || visibility !== "visible") continue;

      // Keep the old toolbar visible while its live layout and
      // native hit regions switch to the new mode in the same update.
      const copy = element.cloneNode(true) as HTMLElement;
      copy.dataset.chromeCopy = element.dataset.chromeMotion;
      delete copy.dataset.chromeMotion;
      delete copy.dataset.windowDrag;
      Object.assign(copy.style, {
        left: `${x - frame.x}px`,
        top: `${y - frame.y}px`,
        width: `${width}px`,
        height: `${height}px`,
        margin: "0",
        padding,
        opacity,
      });
      const track = element.closest(".sidebar-track");
      if (track) {
        const bounds = track.getBoundingClientRect();
        copy.style.clipPath = `inset(0 ${Math.max(0, right - bounds.right)}px 0 ${Math.max(0, bounds.left - x)}px)`;
      }
      layer.append(copy);
    }
    clear();
    root.append(layer);
    const current = { layer, animations: [] as Animation[] };
    transition = current;
    await nextTick();
    if (transition !== current) return;

    const reduced = reducedMotion.value;
    for (const copy of layer.children) {
      current.animations.push(copy.animate({ opacity: [getComputedStyle(copy).opacity, "0"] }, {
        duration: reduced ? 80 : 120, easing: easeOut, fill: "forwards",
      }));
    }
    for (const element of root.querySelectorAll<HTMLElement>(chromeItems)) {
      if (element.closest("[inert]")) continue;
      current.animations.push(element.animate({
        opacity: [previousOpacity.get(element.dataset.chromeMotion) ?? "0", "1"],
      }, { duration: reduced ? 80 : 180, easing: easeOut }));
    }
    void Promise.allSettled(current.animations.map(animation => animation.finished)).then(() => {
      if (transition === current) clear();
    });
  });
}
