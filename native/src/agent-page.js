// Runs only in the "Rove Agent Page" world. The page cannot replace these
// helpers or the references that connect a snapshot to its DOM elements.
(params) => {
  const MAX_ELEMENTS = 100;
  const MAX_TEXT = 12000;
  const TEXT_INPUTS = new Set(["text", "search", "email", "tel", "url"]);
  const CLICK_INPUTS = new Set(["button", "submit", "reset", "image", "checkbox", "radio"]);
  const ROLES = new Set(["button", "link", "checkbox", "radio", "switch", "menuitem", "tab", "option"]);
  const TARGETS = "a[href],button,input,textarea,summary,[role]";
  const error = (code) => ({ error: code });

  function checkPage() {
    if (Date.now() >= params.expires) return "timeout";
    if (location.href !== params.url) return "page_changed";
    if (document.visibilityState !== "visible") return "tab_not_visible";
    if (document.readyState === "loading" || !document.body) return "page_not_ready";
    return null;
  }

  const initial = checkPage();
  if (initial) return error(initial);

  function elementKind(element) {
    if (element instanceof HTMLInputElement) {
      if (TEXT_INPUTS.has(element.type) && !element.readOnly) return "text";
      return CLICK_INPUTS.has(element.type) ? "click" : null;
    }
    if (element instanceof HTMLTextAreaElement) return element.readOnly ? null : "text";
    return element.matches("a[href],button,summary") || ROLES.has(element.getAttribute("role"))
      ? "click" : null;
  }

  function actionableKind(element) {
    if (!(element instanceof HTMLElement) || !element.isConnected) return null;
    const kind = elementKind(element);
    if (!kind || element.closest('[inert],[aria-hidden="true"],[aria-disabled="true"]') ||
        element.matches(":disabled") || !element.getClientRects().length ||
        !element.checkVisibility({ checkOpacity: true, checkVisibilityCSS: true })) return null;
    return kind;
  }

  function normalizeLabel(value) {
    return value?.slice(0, 1024).replace(/\s+/g, " ").trim();
  }

  function referencedLabel(element) {
    const ids = element.getAttribute("aria-labelledby")?.trim();
    if (!ids) return "";
    return ids.split(/\s+/).map(id => document.getElementById(id)?.textContent || "").join(" ");
  }

  function describe(element, kind) {
    const label = normalizeLabel(referencedLabel(element)) ||
      normalizeLabel(element.getAttribute("aria-label")) ||
      normalizeLabel(Array.from(element.labels || [], label => label.innerText).join(" ")) ||
      normalizeLabel(element.innerText) ||
      normalizeLabel(element.getAttribute("placeholder")) ||
      normalizeLabel(element.getAttribute("alt")) ||
      (element instanceof HTMLInputElement && kind === "click" && normalizeLabel(element.getAttribute("value"))) ||
      normalizeLabel(element.getAttribute("title")) ||
      normalizeLabel(element.getAttribute("name")) ||
      element.localName;
    const href = element instanceof HTMLAnchorElement ? element.href : "";
    return { kind, label: label.slice(0, 180).toWellFormed(), href };
  }

  if (params.op === "snapshot") {
    const entries = [];
    let skipped = 0;
    let nextOffset = null;
    for (const element of document.querySelectorAll(TARGETS)) {
      const kind = actionableKind(element);
      if (!kind) continue;
      if (skipped < params.offset) {
        skipped += 1;
        continue;
      }
      if (entries.length === MAX_ELEMENTS) {
        nextOffset = params.offset + MAX_ELEMENTS;
        break;
      }
      entries.push({ element, ...describe(element, kind) });
    }

    if (!globalThis.__roveAgentPageSnapshot) {
      addEventListener("pagehide", () => { globalThis.__roveAgentPageSnapshot = null; }, { once: true });
    }
    globalThis.__roveAgentPageSnapshot = { token: params.snapshot, url: location.href, entries };
    const text = getComputedStyle(document.body).display === "none" ? "" : document.body.innerText;
    return { result: {
      url: location.href,
      title: document.title.slice(0, 256).toWellFormed(),
      text: text.slice(0, MAX_TEXT).toWellFormed(),
      textTruncated: text.length > MAX_TEXT,
      snapshot: params.snapshot,
      elements: entries.map(({ kind, label }, index) => ({ element: index + 1, kind, label })),
      nextOffset,
    } };
  }

  const snapshot = globalThis.__roveAgentPageSnapshot;
  if (!snapshot || snapshot.token !== params.snapshot || snapshot.url !== location.href) {
    return error("stale_snapshot");
  }
  const entry = snapshot.entries[params.element - 1];
  if (!entry) return error("element_not_found");
  if (entry.kind !== (params.op === "fill" ? "text" : "click")) return error("unsupported_element");
  const { element } = entry;

  function checkElement() {
    const page = checkPage();
    if (page) return page;
    const kind = actionableKind(element);
    if (!kind) return "stale_element";
    const current = describe(element, kind);
    return current.kind === entry.kind && current.label === entry.label && current.href === entry.href
      ? null : "stale_element";
  }

  let issue = checkElement();
  if (issue) return error(issue);
  element.scrollIntoView({ behavior: "instant", block: "nearest", inline: "nearest" });
  if (params.op === "fill") element.focus({ preventScroll: true });
  issue = checkElement();
  if (issue) return error(issue);

  if (params.op === "click") {
    HTMLElement.prototype.click.call(element);
  } else {
    const prototype = element instanceof HTMLTextAreaElement
      ? HTMLTextAreaElement.prototype : HTMLInputElement.prototype;
    // The native setter lets framework input listeners see the change.
    Object.getOwnPropertyDescriptor(prototype, "value").set.call(element, params.text);
    element.dispatchEvent(new Event("input", { bubbles: true, composed: true }));
    element.dispatchEvent(new Event("change", { bubbles: true }));
  }
  return { result: { applied: true } };
}
