(function () {
  'use strict';
  var d = document,
    w = window,
    l = w.location;
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

  function send(name: string, props?: Record<string, string>) {
    var payload: Payload = {
      domain: domain,
      name: name,
      url: l.href,
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

  // Track pageviews (SPA-aware)
  var lastPage: string | undefined;
  var pageEntryTime: number = 0;

  function sendEngagement() {
    if (pageEntryTime > 0) {
      var duration = Math.round((Date.now() - pageEntryTime) / 1000);
      if (duration > 0) {
        send('engagement', { duration: duration.toString() });
      }
    }
  }

  function trackPageview() {
    if (lastPage === l.pathname) return;
    // Send engagement for the previous page before tracking the new one
    if (lastPage !== undefined) {
      sendEngagement();
    }
    lastPage = l.pathname;
    pageEntryTime = Date.now();
    send('pageview');
  }

  // Listen for SPA navigation
  var pushState = history.pushState;
  history.pushState = function () {
    pushState.apply(this, arguments as any);
    trackPageview();
  };
  w.addEventListener('popstate', trackPageview);

  // Leave detection
  var engagementSent = false;

  function onLeave() {
    if (engagementSent) return;
    engagementSent = true;
    sendEngagement();
  }

  d.addEventListener('visibilitychange', function () {
    if (d.visibilityState === 'hidden') {
      onLeave();
    } else {
      engagementSent = false;
    }
  });

  w.addEventListener('beforeunload', onLeave);

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
