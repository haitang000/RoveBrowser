// Executed by the host after DOMContentLoaded, never by article markup.
const root = document.documentElement;
root.dataset.reduceMotion = String(reduceMotion);
const reduced = reduceMotion || matchMedia('(prefers-reduced-motion: reduce)').matches;
const initial = root.dataset.readerPhase === 'loading';
if (opening && initial && !reduced) {
  const candidates = [...document.querySelectorAll('main > header > *, article :is(h1,h2,h3,h4,h5,h6,p,ul,ol,dl,blockquote,pre,table,figure,details,img), main > footer')];
  const groups = new Set(candidates);
  let order = 0;
  for (const element of candidates) {
    // Animate a figure/list/quote as one unit, avoiding nested blur layers.
    let nested = false;
    for (let parent = element.parentElement; parent; parent = parent.parentElement) {
      if (groups.has(parent)) { nested = true; break; }
    }
    if (nested) continue;
    const rect = element.getBoundingClientRect();
    // Offscreen text stays readable immediately, even in very long articles.
    if (!rect.width || !rect.height || rect.bottom <= 0 || rect.top >= innerHeight + 80) continue;
    element.dataset.readerReveal = '';
    element.style.setProperty('--reader-delay', `${Math.min(order++ * 24, 120)}ms`);
  }
  // Establish the hidden state before changing the transition target.
  getComputedStyle(root).opacity;
  for (const element of document.querySelectorAll('[data-reader-reveal]')) getComputedStyle(element).filter;
  // The host polls incomplete images: timers and load handlers do not run
  // inside a controller with webpage JavaScript disabled.
  for (const image of document.querySelectorAll('article img')) {
    if (image.complete) continue;
    image.dataset.readerPending = '';
  }
}
root.dataset.readerPhase = opening ? 'open' : 'closing';
// Reading input remains available during entry. Only the departing document
// becomes inert; shortcuts are still handled by the native controller.
document.querySelector('main').inert = !opening;
const duration = reduced ? 80 : opening ? 220 : 160;
return duration;
