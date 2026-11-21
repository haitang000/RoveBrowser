(() => {
  const root = document.documentElement;
  const body = document.body;
  if (!root || !body) return null;

  const px = value => Number.parseFloat(value) || 0;
  const bodyStyle = getComputedStyle(body);
  const height = Math.max(
    root.getBoundingClientRect().height,
    body.getBoundingClientRect().top + window.scrollY +
      Math.max(body.offsetHeight, body.scrollHeight) + px(bodyStyle.marginBottom),
  );

  // Measure the intrinsic width without wrapping the live page in a different
  // layout container. Restore the stylesheet before the next rendering frame.
  const measureWidth = () => Math.max(
    root.getBoundingClientRect().width,
    body.getBoundingClientRect().left + window.scrollX +
      Math.max(body.offsetWidth, body.scrollWidth) + px(bodyStyle.marginRight),
  );
  // Keep an absolute width explicitly set on the root by the extension.
  if (root.computedStyleMap().get("width")?.unit === "px") return [measureWidth(), height];

  const sheets = document.adoptedStyleSheets;
  const measure = new CSSStyleSheet();
  measure.replaceSync("html:root { width: max-content !important; }");
  try {
    document.adoptedStyleSheets = [...sheets, measure];
    return [measureWidth(), height];
  } finally {
    document.adoptedStyleSheets = sheets;
  }
})()
