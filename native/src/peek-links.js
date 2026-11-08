// A link-disposition adapter only: no IPC, host objects, or privileged API.
// A standard named-window request is validated by the native NewWindowRequested handler.
(() => {
  const open = window.open.bind(window);
  document.addEventListener('click', event => {
    if (!event.isTrusted || event.defaultPrevented || event.button !== 0 ||
        !event.shiftKey || event.ctrlKey || event.altKey || event.metaKey) return;
    const link = event.composedPath().find(node => node instanceof HTMLAnchorElement || node instanceof SVGAElement);
    if (!link || link.hasAttribute('download')) return;
    const href = link instanceof HTMLAnchorElement ? link.href : new URL(link.getAttribute('href'), document.baseURI).href;
    let url;
    try { url = new URL(href); } catch { return; }
    if (!['http:', 'https:'].includes(url.protocol)) return;
    event.preventDefault();
    event.stopImmediatePropagation();
    // Only normalized geometry travels in the standard window name. Cross-origin
    // frames use the native pointer fallback, without a frame-to-host bridge.
    const point = window === window.top && innerWidth > 0 && innerHeight > 0
      ? `:${Math.max(0, Math.min(1, event.clientX / innerWidth))},${Math.max(0, Math.min(1, event.clientY / innerHeight))}`
      : '';
    open(url.href, '__era_peek__' + point);
  }, true);
})();
