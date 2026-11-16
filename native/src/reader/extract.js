// Adapted from Zigzag Web/Reader.swift, commit 79437afc94a1104f4442d52415054532e694f082.
// Copyright 2026 Zigao Wang. Apache-2.0. Runs only in the host-created isolated world.
window.__roveReader = {
  // Readability's defaults pass over short articles (three good paragraphs); Safari offers Reader for those too.
  isArticle: () => { try { return isProbablyReaderable(document, { minScore: 10, minContentLength: 100 }); } catch (e) { return false; } },
  // Done ahead, while the page is idle, so opening Reader is instant. Kept for this address only.
  prepare: () => {
    const r = window.__roveReader;
    if (r.cached && r.cached.url === location.href) return;
    const run = () => { try { r.cached = { url: location.href, json: r.parse() }; } catch (e) {} };
    (window.requestIdleCallback || ((f) => setTimeout(f, 300)))(run, { timeout: 3000 });
  },
  take: () => {
    const r = window.__roveReader;
    if (r.cached && r.cached.url === location.href && r.cached.json) return r.cached.json;
    return r.parse();
  },
  parse: () => {
    try {
      const copy = document.cloneNode(true);
      // Wikipedia and other MediaWiki sites: the fact box, notes about other pages, edit links and footnote
      // markers are page furniture, and would otherwise open the article.
      const wiki = !!copy.querySelector('.mw-parser-output');
      if (wiki) {
        copy.querySelectorAll('.infobox, .navbox, .navbox-styles, .vertical-navbox, .sidebar, .metadata, .ambox, .hatnote, ' +
          '.shortdescription, .mw-editsection, sup.reference, .mw-jump-link, #toc, .toc, .noprint, .mw-empty-elt, ' +
          '.side-box, .sistersitebox, .portalbox, .catlinks, .authority-control, .navigation-not-searchable').forEach(el => el.remove());
        // The little ⓘ after a pronunciation plays a sound on the page; in reading it's just a mark.
        copy.querySelectorAll('a, span, sup').forEach(el => { if (el.childElementCount === 0 && el.textContent.trim() === 'ⓘ') el.remove(); });
        // A picture floats beside the paragraph after it on the page; in one column it follows that paragraph,
        // so an article opens with its words, not a stack of pictures. Last first, so a run keeps its order.
        [...copy.querySelectorAll('.mw-parser-output figure')].reverse().forEach(figure => {
          let next = figure.nextElementSibling;
          while (next && next.tagName === 'FIGURE') next = next.nextElementSibling;
          if (next && next.tagName === 'P' && next.textContent.trim()) next.after(figure);
        });
      }
      // Icons and spacers aren't pictures.
      copy.querySelectorAll('img').forEach(img => {
        const w = parseInt(img.getAttribute('width') || '0', 10), h = parseInt(img.getAttribute('height') || '0', 10);
        if ((w && w < 32 && h && h < 32) || (w && w <= 2) || (h && h <= 2)) img.remove();
      });
      // What the site calls itself, when it says so briefly; otherwise its address. Never a company's legal name.
      const siteName = () => {
        const host = location.hostname.replace(/^(www|m)\./, '');
        if (/(^|\.)wikipedia\.org$/.test(host)) return 'Wikipedia';
        const own = (meta('og:site_name') || '').trim();
        return own && own.length <= 28 && !/\b(inc|llc|ltd|foundation|corporation)\b/i.test(own) ? own : host;
      };
      const meta = n => (document.querySelector(`meta[property="${n}"], meta[name="${n}"]`) || {}).content || '';
      const article = new Readability(copy).parse();
      if (!article) return null;
      const body = document.createElement('div');
      body.innerHTML = article.content || '';
      // A heading's own link marker (¶, #, §) is for sharing the spot on the page; in reading it's noise.
      body.querySelectorAll(':is(h1, h2, h3, h4, h5, h6) a, a.headerlink').forEach(a => {
        if (a.classList.contains('headerlink') || /^[¶#§]?$/.test(a.textContent.trim()) && !a.querySelector('img')) a.remove();
      });
      // A picture in a line of text (a formula, an emoji) stays in the line; every other one is a block.
      body.querySelectorAll('img').forEach(img => {
        const block = img.closest('p, li, td, th, dd');
        if (block && !img.closest('figure') && block.textContent.trim().length > 0) img.classList.add('inline');
      });
      // A wiki's date is when the page was started, not when what you read was written.
      const time = wiki ? null : document.querySelector('article time[datetime], main time[datetime], header time[datetime]');
      const published = wiki ? '' : article.publishedTime || meta('article:published_time') || (time ? time.getAttribute('datetime') : '') || '';
      return JSON.stringify({ title: article.title || document.title, byline: (article.byline || '').trim(),
        site: siteName(), siteTitle: (meta('og:site_name') || meta('application-name') || '').trim(),
        content: body.innerHTML, length: article.length || 0,
        lang: article.lang || document.documentElement.lang || '',
        sample: (article.textContent || '').replace(/\s+/g, ' ').slice(0, 600),
        published: (() => {
          // Keep the publisher's calendar day; do not shift it to the
          // machine's timezone. Invalid and future dates stay absent.
          const day = published.match(/^(\d{4}-\d{2}-\d{2})/);
          const date = new Date(day ? day[1] + 'T00:00:00Z' : published);
          return Number.isFinite(date.getTime()) && date.getUTCFullYear() > 1990 && date.getTime() < Date.now() + 86400000
            ? new Intl.DateTimeFormat(navigator.language, { dateStyle: 'long', timeZone: 'UTC' }).format(date) : '';
        })(),
        url: location.href });
    } catch (e) { return null; }
  }
};
