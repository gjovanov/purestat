// What the tracker decides before it sends anything, and the clock that
// measures how long a page was visible. Kept apart from the browser glue so
// they can be tested without a browser (test/rules.test.ts).

// A host whose pages are never a visitor's: empty, localhost, a .local name,
// or a loopback, private, link-local, carrier-grade NAT or unspecified IP.
// The server drops the same hosts at ingest. `location.hostname` is always
// canonical (IPv4 dotted, IPv6 compressed in brackets), which is all this
// reads; an IPv6 literal that starts with `::` (loopback, unspecified,
// IPv4-mapped) counts as local, since no public site is browsed at one.
export function isLocalHost(hostname: string): boolean {
  var h = (hostname || '').toLowerCase().replace(/\.$/, '');
  if (!h || /(^|\.)local(host)?$/.test(h) || /^\[(::|f[cd][0-9a-f]{2}:|fe[89ab][0-9a-f]:)/.test(h)) {
    return true;
  }
  var v4 = /^(\d+)\.(\d+)\.\d+\.\d+$/.exec(h);
  if (!v4) return false;
  var a = +v4[1],
    b = +v4[2];
  return (
    a == 127 ||
    a == 10 ||
    a == 0 ||
    (a == 172 && b > 15 && b < 32) ||
    (a == 192 && b == 168) ||
    (a == 169 && b == 254) ||
    (a == 100 && b > 63 && b < 128)
  );
}

// Whether this browser was told never to be counted: the operator sets
// localStorage.purestat_ignore = 'true' once in each browser they test with.
// Storage that is blocked or missing reads as not set.
export function ignoreFlagSet(storage: () => Storage | null | undefined): boolean {
  try {
    var s = storage();
    return !!s && s.getItem('purestat_ignore') === 'true';
  } catch (e) {
    return false;
  }
}

export interface Environment {
  hostname: string;
  protocol: string;
  webdriver: boolean;
  ignored: boolean;
}

// Send nothing from a file, a local or private host, a browser driven by
// automation (navigator.webdriver), or a browser with the ignore flag.
export function shouldSkip(env: Environment): boolean {
  return env.protocol === 'file:' || env.webdriver || env.ignored || isLocalHost(env.hostname);
}

export interface VisibleClock {
  show(): void;
  hide(): void;
  // Whole seconds the page was visible since the last reset.
  seconds(): number;
  reset(visible: boolean): void;
}

// Counts the time a page is visible, and only that: a stretch starts when
// the page is shown and ends when it is hidden.
export function visibleClock(now: () => number, visible: boolean): VisibleClock {
  var total = 0;
  var since: number | null = visible ? now() : null;
  return {
    show: function () {
      if (since === null) since = now();
    },
    hide: function () {
      if (since !== null) {
        total += now() - since;
        since = null;
      }
    },
    seconds: function () {
      return Math.round((total + (since !== null ? now() - since : 0)) / 1000);
    },
    reset: function (isVisible: boolean) {
      total = 0;
      since = isVisible ? now() : null;
    },
  };
}

// A random id for one pageview, held in memory only: every engagement report
// of the pageview carries it, so the server can keep the longest one.
export function pageviewId(random: () => number): string {
  var id = '';
  while (id.length < 16) id += Math.floor(random() * 36).toString(36);
  return id;
}
