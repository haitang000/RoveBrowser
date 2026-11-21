(pause, expectedPage) => {
    const page = location.href.split("#", 1)[0];
    if (expectedPage !== undefined && page !== expectedPage) return "cancelled";

    const key = Symbol.for("rove.nowPlaying.paused");
    if (pause) {
        const paused = [];
        const roots = [document];
        for (const root of roots) {
            for (const element of root.querySelectorAll("*")) {
                if (element.shadowRoot) roots.push(element.shadowRoot);
                if (!(element instanceof HTMLMediaElement) || element.paused || element.ended) continue;
                element.pause();
                if (element.paused) {
                    paused.push({ element: new WeakRef(element), source: element.currentSrc, stream: element.srcObject });
                }
            }
        }
        window[key] = { page, paused };
        return paused.length ? "applied" : "unavailable";
    }

    const remembered = window[key];
    if (remembered?.page !== page) return "unavailable";
    const targets = remembered.paused.filter(({ element, source, stream }) => {
        const media = element.deref();
        return media?.isConnected && !media.ended && media.currentSrc === source && media.srcObject === stream;
    });
    remembered.paused = targets;
    let resumed = false;
    for (const { element } of targets) {
        const media = element.deref();
        if (media.paused) media.play().catch(() => {});
        // The HTML play algorithm changes paused synchronously when it accepts a request.
        resumed ||= !media.paused;
    }
    return resumed ? "applied" : targets.length ? "blocked" : "unavailable";
}
