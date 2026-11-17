// Polled by the host only while an image is pending, for at most ten seconds.
const root = document.documentElement;
if (root.dataset.readerPhase !== 'open') return 0;
if (cleanup) {
  for (const element of document.querySelectorAll('[data-reader-reveal]')) {
    delete element.dataset.readerReveal;
    element.style.removeProperty('--reader-delay');
  }
}
for (const image of document.querySelectorAll('img[data-reader-pending]')) {
  if (!image.complete) continue;
  delete image.dataset.readerPending;
  const rect = image.getBoundingClientRect();
  if (!image.naturalWidth || root.dataset.reduceMotion === 'true' ||
      matchMedia('(prefers-reduced-motion: reduce)').matches || rect.bottom <= 0 || rect.top >= innerHeight) continue;
  image.animate([{ opacity: 0, filter: 'blur(6px)' }, { opacity: 1, filter: 'blur(0)' }], {
    duration: 280, easing: 'cubic-bezier(.23,1,.32,1)',
  });
}
return document.querySelectorAll('img[data-reader-pending]').length;
