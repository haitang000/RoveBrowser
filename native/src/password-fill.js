// Runs only in the top document of the exact imported origin. No host bridge.
// The user picks a login before fields change; forms are never submitted.
function (expectedOrigin, accounts) {
  if (location.origin !== expectedOrigin || window !== window.top || !accounts.length) return;
  window.dispatchEvent(new Event('__rove_disable_imported_logins'));
  document.getElementById('__rove_imported_login')?.remove();
  let host;
  const eligible = () => [...document.querySelectorAll('input[type="password"]')].find(input => {
    if (input.disabled || input.readOnly || /new-password|one-time-code/.test(input.autocomplete) || !input.getClientRects().length) return false;
    try { return !input.form || new URL(input.form.action || location.href).origin === expectedOrigin; } catch { return false; }
  });
  function mount() {
    const password = eligible();
    if (!password) { host?.remove(); host = undefined; return; }
    if (host?.isConnected) return;
    host = document.createElement('div'); host.id = '__rove_imported_login';
    const shadow = host.attachShadow({ mode: 'open' });
    const style = document.createElement('style');
    style.textContent = ':host{all:initial;position:fixed;right:20px;bottom:20px;z-index:2147483647;font:13px "Segoe UI",sans-serif;color:#dae5f2}button{font:inherit;color:inherit;display:block;border:1px solid #ffffff38;background:#1e3a5f;border-radius:12px;padding:12px 16px;cursor:pointer;box-shadow:0 4px 24px #0003}button:hover{background:#2c4b70}button:focus-visible{outline:2px solid #7eafff;outline-offset:3px}.choices{display:none;max-height:240px;overflow:auto;margin-bottom:8px}.choices button{width:100%;text-align:left;margin-top:4px}';
    const choices = document.createElement('div'); choices.className = 'choices';
    const toggle = document.createElement('button'); toggle.type = 'button'; toggle.textContent = 'Rove · 填写登录信息'; toggle.setAttribute('aria-expanded', 'false');
    toggle.addEventListener('click', event => {
      if (!event.isTrusted || location.origin !== expectedOrigin) return;
      const open = choices.style.display !== 'block'; choices.style.display = open ? 'block' : 'none'; toggle.setAttribute('aria-expanded', String(open));
    });
    for (const login of accounts) {
      const button = document.createElement('button'); button.type = 'button'; button.textContent = login.username || '无用户名的登录信息';
      button.addEventListener('click', event => {
        if (!event.isTrusted || location.origin !== expectedOrigin) return;
        const pass = eligible(); if (!pass) return;
        const form = pass.form || document;
        const users = [...form.querySelectorAll('input')].filter(input => input !== pass && ['text','email','tel'].includes(input.type) && !input.disabled && !input.readOnly && input.getClientRects().length && (input.compareDocumentPosition(pass) & Node.DOCUMENT_POSITION_FOLLOWING));
        const user = users.find(input => input.autocomplete === 'username') || users.find(input => input.type === 'email') || users.at(-1);
        const set = (input, value) => {
          Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set.call(input, value);
          input.dispatchEvent(new Event('input', { bubbles: true })); input.dispatchEvent(new Event('change', { bubbles: true }));
        };
        if (user) set(user, login.username);
        set(pass, login.password); pass.focus(); cleanup();
      }); choices.append(button);
    }
    shadow.append(style, choices, toggle); document.documentElement.append(host);
  }
  const observer = new MutationObserver(mount); mount();
  observer.observe(document.body || document.documentElement, { childList: true, subtree: true });
  const timer = setTimeout(cleanup, 120000);
  function cleanup() {
    observer.disconnect(); host?.remove(); clearTimeout(timer); accounts.length = 0;
    window.removeEventListener('__rove_disable_imported_logins', cleanup);
  }
  window.addEventListener('__rove_disable_imported_logins', cleanup);
}
