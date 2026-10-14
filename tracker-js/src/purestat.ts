import {
  fileTypes,
  ignoreFlagSet,
  linkEvent,
  optedIn,
  pageviewId,
  seenPercent,
  shouldSkip,
  visibleClock,
} from './rules';

(function () {
  'use strict';
  var d = document,
    w = window,
    l = w.location;
  var storage = function () {
    return w.localStorage;
  };

  if (
    shouldSkip({
      hostname: l.hostname,
      protocol: l.protocol,
      webdriver: !!navigator.webdriver,
      ignored: ignoreFlagSet(storage),
    })
  ) {
    // Nothing is sent from here, and a page's own calls still work.
    (w as any).purestat = function () {};
    return;
  }

  var script = d.currentScript as HTMLScriptElement;
  var attr = function (name: string) {
    return script.getAttribute(name);
  };
  var apiUrl = attr('data-api') || new URL(script.src).origin + '/api/event';
  var domain = attr('data-domain') || l.hostname;
  // Clicks on links are reported only when the page asks for them.
  var outbound = optedIn(attr('data-outbound-links'));
  var downloads = attr('data-file-downloads');
  var types = optedIn(downloads) ? fileTypes(downloads) : null;

  interface Payload {
    domain: string;
    name: string;
    url: string;
    referrer: string;
    screen_width: number;
    props?: Record<string, string>;
  }

  function send(name: string, props?: Record<string, string>, url?: string) {
    if (ignoreFlagSet(storage)) return;
    var payload: Payload = {
      domain: domain,
      name: name,
      url: url || l.href,
      referrer: d.referrer || '',
      screen_width: w.innerWidth,
    };
    if (props) payload.props = props;

    var body = JSON.stringify(payload);
    if (navigator.sendBeacon) {
      navigator.sendBeacon(apiUrl, body);
    } else {
      var xhr = new XMLHttpRequest();
      xhr.open('POST', apiUrl, true);
      xhr.setRequestHeader('Content-Type', 'application/json');
      xhr.send(body);
    }
  }

  function isVisible() {
    return d.visibilityState !== 'hidden';
  }

  // Read time is time the page is visible and has the focus (a frame of
  // the page holding it counts). A browser without hasFocus counts visible.
  function isActive() {
    return isVisible() && (!d.hasFocus || d.hasFocus());
  }

  // The page being measured: its path, its address, its pageview id, the
  // read seconds and the depth already reported for it, and the deepest
  // point of it seen so far.
  var lastPage: string | undefined;
  var pageUrl = '';
  var pv = '';
  var reported = 0;
  var reportedDepth = 0;
  var depth = 0;
  var clock = visibleClock(Date.now, isActive());

  function measure() {
    var el = d.documentElement;
    if (!el) return;
    var seen = seenPercent(
      w.pageYOffset || el.scrollTop,
      w.innerHeight,
      Math.max(el.scrollHeight, d.body ? d.body.scrollHeight : 0)
    );
    if (seen > depth) depth = seen;
  }

  // Reports the page's read time and depth so far, at the page's own
  // address. A pageview's engaged time is its longest report and its depth
  // its deepest; nothing is sent until one of them has grown.
  function sendEngagement() {
    measure();
    var seconds = clock.seconds();
    if (pv && (seconds > reported || depth > reportedDepth)) {
      reported = seconds;
      reportedDepth = depth;
      var props: Record<string, string> = { duration: String(seconds), pv: pv };
      if (depth) props.scroll = String(depth);
      send('engagement', props, pageUrl);
    }
  }

  // Track pageviews (SPA-aware)
  function trackPageview() {
    if (lastPage === l.pathname) return;
    if (lastPage !== undefined) sendEngagement();
    lastPage = l.pathname;
    pageUrl = l.href;
    pv = pageviewId(Math.random);
    reported = 0;
    reportedDepth = 0;
    depth = 0;
    clock.reset(isActive());
    send('pageview');
  }

  // Listen for SPA navigation
  var pushState = history.pushState;
  history.pushState = function () {
    pushState.apply(this, arguments as any);
    trackPageview();
  };
  w.addEventListener('popstate', trackPageview);

  // The clock stops while the page is hidden or without the focus, and the
  // time so far is reported when it hides or goes away.
  function sync() {
    if (isActive()) clock.show();
    else clock.hide();
  }
  d.addEventListener('visibilitychange', function () {
    sync();
    if (!isVisible()) sendEngagement();
  });
  w.addEventListener('focus', sync);
  // The focus may have moved into a frame of the page: ask once it has.
  w.addEventListener('blur', function () {
    setTimeout(sync, 0);
  });
  w.addEventListener('pagehide', function () {
    clock.hide();
    sendEngagement();
  });
  w.addEventListener('scroll', measure, { passive: true });

  // Outbound links and file downloads, opted in by data-outbound-links and
  // data-file-downloads: a click, or a middle click that opens a tab.
  function onLink(e: MouseEvent) {
    if (e.type === 'auxclick' && e.button !== 1) return;
    var el = e.target as HTMLAnchorElement | null;
    while (el && !(el.tagName === 'A' && el.href)) el = el.parentNode as HTMLAnchorElement | null;
    var event = el && linkEvent(el.href, l.hostname, outbound, types);
    if (event) send(event[0], { url: event[1] });
  }
  if (outbound || types) {
    d.addEventListener('click', onLink);
    d.addEventListener('auxclick', onLink);
  }

  // Initial pageview
  trackPageview();

  // Public API for custom events
  (w as any).purestat = function (
    name: string,
    opts?: { props?: Record<string, string> }
  ) {
    send(name, opts && opts.props);
  };
})();
