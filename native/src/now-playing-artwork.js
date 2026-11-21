(() => {
    const session = navigator.mediaSession;
    const metadata = session?.metadata;
    const media = Array.from(document.querySelectorAll("audio, video"))
        .filter((item) => !item.paused && !item.ended);
    const playing = session?.playbackState === "playing" || media.length > 0;
    if (!playing && !metadata) return null;

    const sizeDistance = ({ sizes }) => {
        const size = Number.parseInt(sizes, 10);
        return size > 0 ? Math.abs(Math.log2(size / 160)) : 2;
    };
    const artwork = Array.from(metadata?.artwork ?? []).slice(0, 12)
        .sort((a, b) => sizeDistance(a) - sizeDistance(b)).slice(0, 2).map(({ src }) => src);
    const poster = media.find((item) => item instanceof HTMLVideoElement && item.poster)?.poster;
    const pageImage = playing ? document.querySelector('meta[property="og:image"]')?.content : null;
    const sources = [];
    for (const source of [...artwork, poster, pageImage]) {
        if (!source || source.length > 256 * 1024) continue;
        try {
            const url = new URL(source, document.baseURI);
            if (url.username || url.password) continue;
            if (["https:", "http:", "blob:"].includes(url.protocol) || url.href.startsWith("data:image/")) {
                if (!sources.includes(url.href)) sources.push(url.href);
            }
        } catch {}
    }
    const title = metadata?.title?.trim().slice(0, 320) || null;
    if (!title && !sources.length) return null;

    const key = Symbol.for("rove.nowPlaying.artwork");
    const signature = sources.join("\n");
    let sample = window[key];
    if (sample?.signature !== signature) {
        sample = { signature, colors: null, sampling: sources.length > 0 };
        window[key] = sample;
        if (sources.length) loadArtwork(sources, sample);
    }
    return {
        priority: (artwork.length ? 4 : 0) + (playing ? 2 : 0) + (title ? 1 : 0),
        title, colors: sample.colors, sampling: sample.sampling,
    };

    async function loadArtwork(urls, sample) {
        for (const url of urls) {
            if (window[key] !== sample) return;
            const image = new Image();
            image.crossOrigin = "anonymous";
            image.referrerPolicy = "no-referrer";
            image.src = url;
            let timeout;
            try {
                await Promise.race([
                    image.decode(),
                    new Promise((_, reject) => { timeout = setTimeout(reject, 2400); }),
                ]);
                const colors = paletteFrom(image);
                if (colors) {
                    sample.colors = colors;
                    break;
                }
            } catch {
                image.removeAttribute("src");
            } finally {
                clearTimeout(timeout);
            }
        }
        sample.sampling = false;
    }

    function paletteFrom(image) {
        const canvas = document.createElement("canvas");
        canvas.width = canvas.height = 24;
        const context = canvas.getContext("2d");
        if (!context) return null;
        context.drawImage(image, 0, 0, 24, 24);
        const { data } = context.getImageData(0, 0, 24, 24);
        const buckets = new Map();
        for (let i = 0; i < data.length; i += 4) {
            const [r, g, b, a] = data.subarray(i, i + 4);
            if (a < 120) continue;
            const index = (r >> 4) * 256 + (g >> 4) * 16 + (b >> 4);
            const bucket = buckets.get(index) ?? { channels: [0, 0, 0], count: 0 };
            bucket.channels[0] += r;
            bucket.channels[1] += g;
            bucket.channels[2] += b;
            bucket.count++;
            buckets.set(index, bucket);
        }
        const colors = [...buckets.values()].map(({ channels, count }) => {
            const rgb = channels.map((sum) => Math.round(sum / count));
            const chroma = Math.max(...rgb) - Math.min(...rgb);
            return { rgb, weight: count * (0.45 + chroma / 128) };
        }).sort((a, b) => b.weight - a.weight);
        const palette = [];
        for (const { rgb } of colors) {
            const distinct = palette.every((color) => Math.hypot(...rgb.map((channel, i) => channel - color[i])) >= 58);
            if (distinct) palette.push(rgb);
            if (palette.length === 3) break;
        }
        while (palette.length && palette.length < 3) palette.push(palette[palette.length - 1]);
        return palette.length ? palette : null;
    }
})()
