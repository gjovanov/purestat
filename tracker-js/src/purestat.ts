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
      var blob = new Blob([body], { type: 'application/json' });
      navigator.sendBeacon(apiUrl, blob);
    } else {
      var xhr = new XMLHttpRequest();
      xhr.open('POST', apiUrl, true);
      xhr.setRequestHeader('Content-Type', 'application/json');
      xhr.send(body);
    }
  }

  // Track pageviews (SPA-aware)
  var lastPage: string | undefined;
  function trackPageview() {
    if (lastPage === l.pathname) return;
    lastPage = l.pathname;
    send('pageview');
  }

  // Listen for SPA navigation
  var pushState = history.pushState;
  history.pushState = function () {
    pushState.apply(this, arguments as any);
    trackPageview();
  };
  w.addEventListener('popstate', trackPageview);

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
