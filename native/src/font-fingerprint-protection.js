(() => {
  if (!window.FontFace || location.protocol === "chrome-extension:") return;

  const sheet = new CSSStyleSheet();
  sheet.insertRule("@font-face {}");
  const descriptor = sheet.cssRules[0].style;
  const clearSource = descriptor.removeProperty.bind(descriptor, "src");
  const setSource = descriptor.setProperty.bind(descriptor, "src");
  const readSource = descriptor.getPropertyValue.bind(descriptor, "src");
  const isView = ArrayBuffer.isView;
  const bufferLength = Object.getOwnPropertyDescriptor(ArrayBuffer.prototype, "byteLength").get;

  function filterSources(source) {
    clearSource();
    setSource(source);
    const parsed = readSource();
    if (!parsed.includes("local(")) return source;

    // CSSOM normalizes function names and quotes URL and local font names.
    // Split the serialized list only at commas outside strings and functions.
    const entries = parsed.match(/(?:[^,()"\\]|"(?:[^"\\]|\\[\s\S])*"|\((?:[^")]|"(?:[^"\\]|\\[\s\S])*")*\))+/g);
    const allowed = entries.map(entry => entry.trim()).filter(entry => !entry.startsWith("local("));
    return allowed.length === entries.length ? source : allowed.join(", ");
  }

  function isBinary(source) {
    if (isView(source)) return true;
    if (source === null || typeof source !== "object") return false;
    try {
      bufferLength.call(source);
      return true;
    } catch {
      return false;
    }
  }

  const native = window.FontFace;
  const protectedFontFace = new Proxy(native, {
    construct(target, args, newTarget) {
      if (args.length >= 2 && !isBinary(args[1])) {
        args[1] = filterSources(`${args[1]}`);
      }
      return Reflect.construct(target, args, newTarget);
    },
  });
  Object.defineProperty(native.prototype, "constructor", { value: protectedFontFace });
  window.FontFace = protectedFontFace;
})();
