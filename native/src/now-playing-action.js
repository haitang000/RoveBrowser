(action, token, expectedPage) => {
    if (expectedPage !== undefined && location.href.split("#", 1)[0] !== expectedPage) return "cancelled";
    const target = window[Symbol.for("rove.nowPlaying.target")];
    if (target?.token !== token) return "cancelled";

    try {
        if (action === "previous" || action === "next") {
            const name = action === "previous" ? "previoustrack" : "nexttrack";
            const handler = window[Symbol.for("rove.nowPlaying.actions")]?.get(name);
            if (!handler) return "unavailable";
            Promise.resolve(handler({ action: name })).catch(() => {});
            return "applied";
        }

        const player = target.player?.deref();
        if (!player?.isConnected || player.ended) return "unavailable";
        if (action.seek) {
            const { position } = action.seek;
            if (!Number.isFinite(position) || !Number.isFinite(player.duration) || !player.seekable.length) {
                return "unavailable";
            }
            player.currentTime = Math.min(player.duration, Math.max(0, position));
            return "applied";
        }
        if (action === "pictureInPicture" && document.pictureInPictureEnabled && player instanceof HTMLVideoElement
            && !player.disablePictureInPicture && player.videoWidth) {
            const result = document.pictureInPictureElement === player
                ? document.exitPictureInPicture()
                : player.requestPictureInPicture();
            result.catch(() => {});
            return "applied";
        }
    } catch {
        return "blocked";
    }
    return "unavailable";
}
