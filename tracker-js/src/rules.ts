// What the tracker decides before it sends anything, the clock that
// measures how long a page was read, how far down it was seen, and what a
// click on a link is worth. Kept apart from the browser glue so they can be
// tested without a browser (test/rules.test.ts).

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

// Counts the time a page is read, and only that: a stretch starts when the
// page is shown (visible and focused) and ends when it is hidden or loses
// the focus.
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

// How far down a page the screen has reached, in percent of the page's
// height, or 0 for a page that does not scroll: such a page has no depth,
// and its reads come from time alone.
export function seenPercent(scrollTop: number, viewport: number, height: number): number {
  if (!(height > viewport) || !(viewport > 0)) return 0;
  return Math.min(100, Math.round(((Math.max(0, scrollTop) + viewport) / height) * 100));
}

// An opt-in script attribute: present, and not "false".
export function optedIn(value: string | null): boolean {
  return value != null && value !== 'false';
}

// The extensions data-file-downloads counts: the ones it lists
// ("pdf,zip"), or these when it lists none.
export var FILE_TYPES =
  'pdf,xlsx,docx,txt,rtf,csv,exe,key,pps,ppt,pptx,7z,pkg,rar,gz,zip,avi,mov,mp4,mpeg,wmv,midi,mp3,wav,wma,dmg,iso,msi,deb,rpm,apk,tar,bz2,xz';

export function fileTypes(value: string | null): string[] {
  var listed = value && value !== 'true' ? value : FILE_TYPES;
  return listed.toLowerCase().replace(/[\s.]/g, '').split(',');
}

// The event a click on a link is worth, if any: [name, address]. The
// address is the link's origin and path only: a query or a fragment can
// carry anything, credentials included, so neither is sent. A download
// wins over an outbound click; anything but http(s) is neither.
export function linkEvent(
  href: string,
  pageHost: string,
  outbound: boolean,
  types: string[] | null
): [string, string] | null {
  var u: URL;
  try {
    u = new URL(href);
  } catch (e) {
    return null;
  }
  if (u.protocol !== 'http:' && u.protocol !== 'https:') return null;
  var address = u.protocol + '//' + u.host + u.pathname;
  var ext = /\.([a-z0-9]+)$/i.exec(u.pathname);
  if (types && ext && types.indexOf(ext[1].toLowerCase()) >= 0) return ['file-download', address];
  if (outbound && u.hostname !== pageHost) return ['outbound-click', address];
  return null;
}
