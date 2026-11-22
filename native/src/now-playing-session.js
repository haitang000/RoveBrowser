(() => {
    const session = navigator.mediaSession;
    if (!session) return;

    // MediaSession has no API to read the handlers a website registered.
    const actions = new Map();
    const setActionHandler = session.setActionHandler;
    session.setActionHandler = function (action, handler) {
        const result = Reflect.apply(setActionHandler, this, arguments);
        if (this === session && ["previoustrack", "nexttrack"].includes(action)) {
            if (typeof handler === "function") actions.set(action, handler);
            else actions.delete(action);
        }
        return result;
    };
    window[Symbol.for("rove.nowPlaying.actions")] = actions;
})();
