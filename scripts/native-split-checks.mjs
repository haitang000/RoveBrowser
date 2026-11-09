import assert from 'node:assert/strict';

export async function checkSplitResize(ui, pages, { waitFor, capturePaints = false }) {
  const divider = ui.getByRole('separator', { name: '调整分屏比例' });
  const start = await divider.boundingBox();
  const paints = pages.map(() => []);
  const sessions = capturePaints ? await Promise.all(pages.map(page => page.context().newCDPSession(page))) : [];
  for (let i = 0; i < sessions.length; i++) {
    sessions[i].on('Page.screencastFrame', frame => {
      // PNG dimensions come from the painted bitmap, rather than innerWidth or
      // a bounds acknowledgement that can arrive before Chromium paints.
      const png = Buffer.from(frame.data, 'base64');
      const scale = png.readUInt32BE(20) / frame.metadata.deviceHeight;
      paints[i].push({ width: png.readUInt32BE(16) / scale, time: frame.metadata.timestamp * 1000 });
      void sessions[i].send('Page.screencastFrameAck', { sessionId: frame.sessionId }).catch(() => {});
    });
    await sessions[i].send('Page.startScreencast', { format: 'png', maxWidth: 1280, maxHeight: 800, everyNthFrame: 1 });
  }
  const latencies = [];
  const markers = await Promise.all(pages.map(page => page.evaluate(() => {
    window.__splitMarker = crypto.randomUUID();
    window.__splitVisibility = [];
    window.__splitResizes = [];
    window.__splitOnVisibility = () => window.__splitVisibility.push(document.visibilityState);
    window.__splitOnResize = () => window.__splitResizes.push(innerWidth);
    document.addEventListener('visibilitychange', window.__splitOnVisibility);
    window.addEventListener('resize', window.__splitOnResize);
    return window.__splitMarker;
  })));
  await ui.evaluate(() => {
    window.__splitOverlays = [];
    window.__splitLayout = null;
    // Wry freezes window.ipc. Intercept the underlying trusted WebView bridge.
    window.__splitPostMessage = window.chrome.webview.postMessage.bind(window.chrome.webview);
    const record = message => {
      const command = JSON.parse(message);
      if (command.type === 'overlay') window.__splitOverlays.push(command.visible);
      if (command.type === 'layout') window.__splitLayout = { ...command, time: Date.now() };
      window.__splitPostMessage(message);
    };
    window.chrome.webview.postMessage = record;
    if (window.chrome.webview.postMessage !== record) throw Error('IPC recording was not installed');
  });
  try {
    await ui.mouse.move(start.x + 1, start.y + 100);
    await ui.mouse.down();
    for (const delta of [120, -180, 2000, -2000, 80]) {
      await ui.mouse.move(start.x + 1 + delta, start.y + 100, { steps: 8 });
      const widths = await ui.locator('.page-pane > .webview-slot').evaluateAll(slots =>
        slots.map(slot => slot.getBoundingClientRect().width));
      const requested = await ui.evaluate(() => window.__splitLayout);
      assert(requested, 'recorded a real layout request');
      for (let i = 0; i < pages.length; i++) {
        await waitFor(async () => Math.abs(await pages[i].evaluate(() => innerWidth) - widths[i]) <= 1,
          `live split viewport during drag (${delta}, pane ${i})`);
        assert.equal(await pages[i].evaluate(() => document.visibilityState), 'visible');
        assert.equal(await pages[i].evaluate(() => window.__splitMarker), markers[i]);
        if (capturePaints) try {
          await waitFor(() => paints[i].some(paint => paint.time >= requested.time && Math.abs(paint.width - widths[i]) <= 1),
            `painted split pixels (${delta}, pane ${i})`);
        } catch (error) {
          console.log(JSON.stringify({ requested, widths, pane: i, paints: paints[i].slice(-6) }));
          throw error;
        }
        if (capturePaints) {
          const painted = paints[i].find(paint => paint.time >= requested.time && Math.abs(paint.width - widths[i]) <= 1);
          latencies.push(painted.time - requested.time);
        }
      }
      assert.deepEqual(await ui.evaluate(() => window.__splitOverlays), []);
      const ratio = Number(await divider.getAttribute('aria-valuenow'));
      assert(ratio >= 20 && ratio <= 80);
    }
    await ui.mouse.up();
    const settled = await divider.getAttribute('aria-valuenow');
    await ui.mouse.move(start.x - 250, start.y + 100);
    assert.equal(await divider.getAttribute('aria-valuenow'), settled);
    for (const page of pages) {
      assert.deepEqual(await page.evaluate(() => window.__splitVisibility), []);
      assert(await page.evaluate(() => new Set(window.__splitResizes).size >= 4));
    }
    latencies.sort((a, b) => a - b);
    return capturePaints ? { splitPaintLatencyMs: { median: latencies[Math.floor(latencies.length / 2)], max: latencies.at(-1), samples: latencies.length } } : {};
  } finally {
    await ui.mouse.up();
    await ui.evaluate(() => {
      window.chrome.webview.postMessage = window.__splitPostMessage;
      delete window.__splitPostMessage;
    });
    for (const page of pages) await page.evaluate(() => {
      document.removeEventListener('visibilitychange', window.__splitOnVisibility);
      window.removeEventListener('resize', window.__splitOnResize);
    });
    for (const session of sessions) {
      await session.send('Page.stopScreencast');
      await session.detach();
    }
  }
}
