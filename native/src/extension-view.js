function (extensionOrigin, activeUrl, optionsPage) {
  if (location.origin !== extensionOrigin) return;

  // WebView2 opens runtime.openOptionsPage in an unmanaged WebView.
  // Use its ordinary, user-initiated window request for the same page instead.
  if (optionsPage && chrome.runtime?.openOptionsPage) {
    const openOptionsPage = chrome.runtime.openOptionsPage.bind(chrome.runtime);
    const open = window.open.bind(window);
    chrome.runtime.openOptionsPage = function (callback) {
      if (!navigator.userActivation.isActive) {
        return callback === undefined ? openOptionsPage() : openOptionsPage(callback);
      }
      open(optionsPage, "_blank", "noopener");
      if (callback === undefined) return Promise.resolve();
      if (typeof callback === "function") queueMicrotask(callback);
    };
  }

  if (!activeUrl || !chrome.tabs?.query) return;

  const tabs = chrome.tabs;
  const query = tabs.query.bind(tabs);
  const hasOwn = Object.hasOwn;

  function findActive(all) {
    let active;
    for (let i = 0; i < all.length; i++) {
      const tab = all[i];
      if (!hasOwn(tab, "url") || tab.url !== activeUrl || !tab.active || tab.id < 0) continue;
      if (active) return [];
      active = tab;
    }
    return active ? [active] : [];
  }

  // WebView2 makes every controller the active tab in its own browser window.
  // Use only tabs and URLs the runtime already returns to this extension.
  tabs.query = function (options, callback) {
    const choosesWindow = options?.currentWindow === true && options.lastFocusedWindow === undefined ||
      options?.lastFocusedWindow === true && options.currentWindow === undefined;
    if (options?.active !== true ||
        !choosesWindow ||
        Object.keys(options).some(key => !["active", "currentWindow", "lastFocusedWindow"].includes(key))) {
      return callback === undefined ? query(options) : query(options, callback);
    }

    if (callback === undefined) {
      return (async () => {
        try { return findActive(await query({})); }
        catch { return []; }
      })();
    }
    return query({}, all => {
      // Read lastError when enumeration fails so the caller can handle an empty result.
      callback(chrome.runtime.lastError ? [] : findActive(all));
    });
  };
}
