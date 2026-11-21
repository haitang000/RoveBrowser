(() => {
  const prefix = location.origin === 'https://chromewebstore.google.com' ? '/detail/'
    : location.origin === 'https://chrome.google.com' ? '/webstore/detail/' : '';
  if (!prefix || window !== window.top) return;

  const isListing = () => location.pathname.startsWith(prefix) &&
    /^(?:[^/]+\/)?[a-p]{32}\/*$/.test(location.pathname.slice(prefix.length));
  const open = window.open.bind(window);
  const host = document.createElement('span');
  host.dataset.roveStoreInstall = '';
  const shadow = host.attachShadow({ mode: 'open' });
  const style = new CSSStyleSheet();
  style.replaceSync(`
    :host { display: inline-block; max-width: 100%; }
    button {
      background: #202124;
      color: #fff;
      border: 1px solid transparent;
      border-radius: 8px;
      padding: 9px 16px;
      min-height: 40px;
      font: 600 14px/20px system-ui, sans-serif;
      cursor: pointer;
    }
    button:hover { background: #3c4043; }
    button:focus-visible { outline: 2px solid #1a73e8; outline-offset: 2px; }
    @media (prefers-color-scheme: dark) {
      button { background: #e8eaed; color: #202124; }
      button:hover { background: #fff; }
    }
  `);
  shadow.adoptedStyleSheets = [style];
  const button = document.createElement('button');
  button.type = 'button';
  button.addEventListener('click', event => {
    if (event.isTrusted && isListing()) {
      event.stopPropagation();
      open(location.href, '__rove_chrome_store_install__');
    }
  });
  shadow.append(button);

  const replacementStyle = new CSSStyleSheet();
  replacementStyle.replaceSync('[data-rove-store-install][data-replaces-button] + button { display: none !important; }');
  document.adoptedStyleSheets = [...document.adoptedStyleSheets, replacementStyle];

  const sync = () => {
    if (!isListing()) {
      host.remove();
      return;
    }
    // Anchor to the store's action slot when present and to the title if the layout changes.
    const storeButton = document.querySelector('[jscontroller="ri2s0b"] button');
    const title = document.querySelector('h1');
    if (!storeButton && !title) return;
    if (host.isConnected && (storeButton ? host.nextElementSibling === storeButton
      : host.previousElementSibling === title)) return;

    const language = document.documentElement.lang.toLowerCase();
    button.textContent = /^zh-(?:hant|tw|hk|mo)/.test(language) ? '加到 Rove'
      : language.startsWith('zh') ? '添加到 Rove' : 'Add to Rove';
    host.toggleAttribute('data-replaces-button', Boolean(storeButton));
    if (storeButton) storeButton.before(host);
    else title.after(host);
  };
  new MutationObserver(sync).observe(document, { childList: true, subtree: true });
  window.navigation?.addEventListener('currententrychange', sync);
  sync();
})();
