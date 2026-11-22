if (url === null ? !document.documentElement.hasAttribute("data-reader-phase") : location.href !== url) {
  return null;
}

const language = (element) => element?.closest("[lang]")?.lang || document.documentElement.lang;
const result = (text, element) => ({
  text: text.trim().slice(0, limit).replace(/[\uD800-\uDBFF]$/u, ""),
  lang: language(element).slice(0, 85),
});
const selection = window.getSelection();
const anchor = selection?.anchorNode;
const selected = anchor?.nodeType === Node.ELEMENT_NODE ? anchor : anchor?.parentElement;
if (selectionOnly) return result("", selected);
if (!selection?.isCollapsed && !selected?.isContentEditable && !document.activeElement?.isContentEditable) {
  const text = selection?.toString();
  if (text?.trim()) return result(text, selected);
}

const roots = url === null ? [document.querySelector("main article")] : [
  ...document.querySelectorAll("main, [role=main]"),
  ...document.querySelectorAll("article"),
  document.body,
];
for (const root of roots) {
  if (!(root instanceof HTMLElement) || root.isContentEditable || !root.getClientRects().length) continue;
  const text = root.innerText.trim();
  if (text) {
    const title = url === null ? document.querySelector("main h1")?.innerText.trim() : "";
    return result(title ? `${title}.\n${text}` : text, root);
  }
}
return null;
