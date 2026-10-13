import { ignoreFlagSet, pageviewId, shouldSkip, visibleClock } from './rules';

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
  var apiUrl =
    script.getAttribute('data-api') ||
    new URL(script.src).origin + '/api/event';
  var domain = script.getAttribute('data-domain') || l.hostname;

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

  // The page being measured: its path, its address, its pageview id, and
  // the visible seconds already reported for it.
  var lastPage: string | undefined;
  var pageUrl = '';
  var pv = '';
  var reported = 0;
  var clock = visibleClock(Date.now, isVisible());

  // Reports the page's visible time so far, at the page's own address. A
  // pageview's engaged time is its longest report; nothing is sent until the
  // time has grown.
  function sendEngagement() {
    var seconds = clock.seconds();
    if (pv && seconds > reported) {
      reported = seconds;
      send('engagement', { duration: String(seconds), pv: pv }, pageUrl);
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
    clock.reset(isVisible());
    send('pageview');
  }

  // Listen for SPA navigation
  var pushState = history.pushState;
  history.pushState = function () {
    pushState.apply(this, arguments as any);
    trackPageview();
  };
  w.addEventListener('popstate', trackPageview);

  // Hidden time is not engaged time: the clock stops while the page is
  // hidden, and the time so far is reported when it hides or goes away.
  d.addEventListener('visibilitychange', function () {
    if (isVisible()) {
      clock.show();
    } else {
      clock.hide();
      sendEngagement();
    }
  });
  w.addEventListener('pagehide', function () {
    clock.hide();
    sendEngagement();
  });

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
