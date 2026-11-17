// Trusted host update. CSS handles continuous size/measure/color changes;
// font families are discrete, so soften their switch with a brief reveal.
const root = document.documentElement;
const main = document.querySelector('main');
const reduced = root.dataset.reduceMotion === 'true' || matchMedia('(prefers-reduced-motion: reduce)').matches;
if (root.dataset.face !== face && root.dataset.readerPhase === 'open' && !reduced && main) {
  const previous = main.getAnimations().find(animation => animation.id === 'reader-typeface');
  const current = previous ? getComputedStyle(main) : null;
  const from = { opacity: current?.opacity ?? '.65', filter: current?.filter ?? 'blur(2px)' };
  previous?.cancel();
  main.animate([from, { opacity: 1, filter: 'blur(0)' }], {
    id: 'reader-typeface', duration: 220, easing: 'cubic-bezier(.23,1,.32,1)',
  });
}
root.dataset.theme = theme;
root.dataset.face = face;
root.style.setProperty('--size', size);
root.style.setProperty('--measure', measure);
