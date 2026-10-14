// Runs the BUILT tracker (dist/purestat.js, what sites load) against a fake
// browser, and reads what it sends. Build first: `bun run build`.
import { expect, test } from 'bun:test';
import { readFileSync } from 'fs';
import { join } from 'path';

const bundle = readFileSync(join(import.meta.dir, '..', 'dist', 'purestat.js'), 'utf8');

interface Sent {
  name: string;
  url: string;
  domain: string;
  props?: Record<string, string>;
}

function browser(
  opts: {
    href?: string;
    webdriver?: boolean;
    storage?: () => unknown;
    /** The script tag's attributes, beside data-domain. */
    attrs?: Record<string, string>;
    /** The page's height; the window is 1000 px tall. */
    pageHeight?: number;
  } = {}
) {
  const url = new URL(opts.href ?? 'https://example.com/a');
  const sent: Sent[] = [];
  const on: Record<string, Array<(e?: unknown) => void>> = {};
  const clock = { now: 1_000 };
  const state = { focused: true };
  const attrs: Record<string, string> = { 'data-domain': 'example.com', ...opts.attrs };
  const location = {
    href: url.href,
    pathname: url.pathname,
    hostname: url.hostname,
    protocol: url.protocol,
  };
  const listen = (type: string, f: (e?: unknown) => void) => (on[type] ||= []).push(f);
  const documentElement = { scrollTop: 0, scrollHeight: opts.pageHeight ?? 1000 };
  const document = {
    currentScript: {
      src: 'https://purestat.example/js/purestat.js',
      getAttribute: (name: string) => (name in attrs ? attrs[name] : null),
    },
    referrer: '',
    visibilityState: 'visible',
    documentElement,
    body: { scrollHeight: 0 },
    hasFocus: () => state.focused,
    addEventListener: listen,
  };
  const window: Record<string, unknown> = {
    location,
    innerWidth: 1280,
    innerHeight: 1000,
    pageYOffset: 0,
    addEventListener: listen,
  };
  Object.defineProperty(window, 'localStorage', {
    get: opts.storage ?? (() => ({ getItem: () => null })),
  });
  const navigator = {
    webdriver: opts.webdriver ?? false,
    sendBeacon: (_url: string, body: string) => {
      sent.push(JSON.parse(body));
      return true;
    },
  };
  const history = { pushState: () => {} };
  const FakeDate = { now: () => clock.now };

  new Function('window', 'document', 'navigator', 'history', 'Date', bundle)(
    window,
    document,
    navigator,
    history,
    FakeDate
  );

  return {
    sent,
    window,
    at(ms: number) {
      clock.now = ms;
    },
    emit(type: string, event?: unknown) {
      for (const f of on[type] || []) f(event);
    },
    setVisible(visible: boolean) {
      document.visibilityState = visible ? 'visible' : 'hidden';
      this.emit('visibilitychange');
    },
    /** The window loses the focus; `toFrame` keeps it in a frame of the page. */
    async blur(toFrame = false) {
      state.focused = toFrame;
      this.emit('blur');
      await new Promise((resolve) => setTimeout(resolve, 0));
    },
    focus() {
      state.focused = true;
      this.emit('focus');
    },
    scrollTo(y: number) {
      window.pageYOffset = y;
      this.emit('scroll');
    },
    /** A click on a link (inside a span), or a middle or right click. */
    click(href: string, how: { type?: string; button?: number } = {}) {
      const link = { tagName: 'A', href, parentNode: null };
      const target = { tagName: 'SPAN', parentNode: link };
      this.emit(how.type ?? 'click', { type: how.type ?? 'click', button: how.button ?? 0, target });
    },
    navigate(path: string) {
      const next = new URL(path, location.href);
      location.href = next.href;
      location.pathname = next.pathname;
      history.pushState();
    },
  };
}

test('engaged time is visible time, reported at the address of the page it measures', () => {
  const b = browser();
  b.at(11_000);
  b.setVisible(false); // 10 s visible
  b.at(111_000);
  b.setVisible(true); // 100 s hidden
  b.at(116_000);
  b.navigate('/b'); // 5 s more visible on /a
  b.at(118_000);
  b.emit('pagehide'); // 2 s on /b

  const [first, ...rest] = b.sent;
  expect(first).toMatchObject({ name: 'pageview', url: 'https://example.com/a', domain: 'example.com' });
  expect(first.props).toBeUndefined();

  const reports = rest.map((e) => [e.name, e.url, e.props?.duration]);
  expect(reports).toEqual([
    ['engagement', 'https://example.com/a', '10'],
    ['engagement', 'https://example.com/a', '15'],
    ['pageview', 'https://example.com/b', undefined],
    ['engagement', 'https://example.com/b', '2'],
  ]);

  // One id per pageview, on each of its reports.
  const [a1, a2, , b1] = rest;
  expect(a1.props?.pv).toMatch(/^[0-9a-z]{16}$/);
  expect(a2.props?.pv).toBe(a1.props?.pv);
  expect(b1.props?.pv).not.toBe(a1.props?.pv);
});

test('a report is sent only when the visible time grew', () => {
  const b = browser();
  b.at(5_000);
  b.setVisible(false);
  b.emit('pagehide'); // nothing new since the hide
  expect(b.sent.map((e) => [e.name, e.props?.duration])).toEqual([
    ['pageview', undefined],
    ['engagement', '4'],
  ]);
});

test('nothing is sent from a local host, under automation, or with the ignore flag', () => {
  // Every click event asked for, too: still nothing.
  const attrs = { 'data-outbound-links': '', 'data-file-downloads': '' };
  const cases = [
    browser({ href: 'http://localhost:5173/', attrs, pageHeight: 4000 }),
    browser({ href: 'http://192.168.1.20/', attrs, pageHeight: 4000 }),
    browser({ href: 'file:///C:/site/index.html', attrs, pageHeight: 4000 }),
    browser({ webdriver: true, attrs, pageHeight: 4000 }),
    browser({
      attrs,
      pageHeight: 4000,
      storage: () => ({ getItem: (k: string) => (k === 'purestat_ignore' ? 'true' : null) }),
    }),
  ];
  for (const b of cases) {
    (b.window.purestat as (name: string) => void)('signup');
    b.click('https://github.com/gjovanov');
    b.click('https://example.com/guide.pdf');
    b.scrollTo(3000);
    b.at(9_000);
    b.emit('pagehide');
    expect(b.sent).toEqual([]);
  }
});

test('read time stops while the window has lost the focus, but not for a frame of the page', async () => {
  const b = browser();
  b.at(11_000);
  await b.blur(); // 10 s read, then another window has the focus
  b.at(41_000);
  b.focus(); // 30 s unfocused
  b.at(46_000);
  await b.blur(true); // the focus moved into an embedded video: still reading
  b.at(56_000);
  b.emit('pagehide');
  expect(b.sent.filter((e) => e.name === 'engagement').map((e) => e.props?.duration)).toEqual(['25']);
});

test('scroll depth is the deepest point of the page seen, sent with the read time', () => {
  const b = browser({ pageHeight: 4000 });
  b.at(3_000);
  b.scrollTo(2000); // the screen's bottom at 3000 of 4000 px
  b.scrollTo(500);
  b.at(5_000);
  b.emit('pagehide');
  const [, report] = b.sent;
  expect(report).toMatchObject({ name: 'engagement', props: { duration: '4', scroll: '75' } });
});

test('a page that does not scroll reports read time only', () => {
  const b = browser({ pageHeight: 900 });
  b.scrollTo(0);
  b.at(5_000);
  b.emit('pagehide');
  expect(b.sent[1].props).toEqual({ duration: '4', pv: b.sent[1].props?.pv });
});

test('a deeper scroll is reported even when the read time has not grown', async () => {
  const b = browser({ pageHeight: 4000 });
  b.at(5_000);
  await b.blur(); // the clock stops at 4 s
  b.setVisible(false); // first report: 4 s, the first screen (25%)
  b.setVisible(true);
  b.scrollTo(3000); // a wheel over the unfocused window
  b.emit('pagehide');
  expect(
    b.sent.filter((e) => e.name === 'engagement').map((e) => [e.props?.duration, e.props?.scroll])
  ).toEqual([
    ['4', '25'],
    ['4', '100'],
  ]);
});

test('no click is reported unless the script asks for it', () => {
  const b = browser();
  b.click('https://github.com/gjovanov');
  b.click('https://example.com/guide.pdf');
  expect(b.sent.map((e) => e.name)).toEqual(['pageview']);
});

test('outbound clicks, with data-outbound-links, without query or fragment', () => {
  const b = browser({ attrs: { 'data-outbound-links': '' } });
  b.click('https://github.com/gjovanov/roomler-ai?tab=readme#install');
  b.click('https://example.com/pricing'); // the page's own host
  b.click('https://example.com/guide.pdf'); // a file, but downloads were not asked for
  b.click('https://www.youtube.com/watch', { type: 'auxclick', button: 1 }); // middle click
  b.click('https://www.youtube.com/watch', { type: 'auxclick', button: 2 }); // right click
  expect(b.sent.slice(1).map((e) => [e.name, e.props])).toEqual([
    ['outbound-click', { url: 'https://github.com/gjovanov/roomler-ai' }],
    ['outbound-click', { url: 'https://www.youtube.com/watch' }],
  ]);
});

test('file downloads, with data-file-downloads, of the default or the listed types', () => {
  const b = browser({ attrs: { 'data-file-downloads': '' } });
  b.click('https://example.com/files/guide.pdf?utm=x');
  b.click('https://cdn.example.net/roomler-setup.msi');
  b.click('https://github.com/gjovanov'); // outbound clicks were not asked for
  expect(b.sent.slice(1).map((e) => [e.name, e.props])).toEqual([
    ['file-download', { url: 'https://example.com/files/guide.pdf' }],
    ['file-download', { url: 'https://cdn.example.net/roomler-setup.msi' }],
  ]);

  const listed = browser({ attrs: { 'data-file-downloads': 'epub', 'data-outbound-links': '' } });
  listed.click('https://example.com/book.epub');
  listed.click('https://cdn.example.net/guide.pdf'); // not listed: an outbound click
  expect(listed.sent.slice(1).map((e) => [e.name, e.props?.url])).toEqual([
    ['file-download', 'https://example.com/book.epub'],
    ['outbound-click', 'https://cdn.example.net/guide.pdf'],
  ]);

  const off = browser({ attrs: { 'data-file-downloads': 'false', 'data-outbound-links': 'false' } });
  off.click('https://example.com/guide.pdf');
  off.click('https://github.com/');
  expect(off.sent.map((e) => e.name)).toEqual(['pageview']);
});

test('blocked storage does not stop a real visitor from being counted', () => {
  const b = browser({
    storage: () => {
      throw new Error('SecurityError');
    },
  });
  (b.window.purestat as (name: string, o?: object) => void)('signup', { props: { plan: 'pro' } });
  expect(b.sent.map((e) => [e.name, e.props])).toEqual([
    ['pageview', undefined],
    ['signup', { plan: 'pro' }],
  ]);
});
