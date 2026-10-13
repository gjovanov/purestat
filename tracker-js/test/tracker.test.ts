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

function browser(opts: { href?: string; webdriver?: boolean; storage?: () => unknown } = {}) {
  const url = new URL(opts.href ?? 'https://example.com/a');
  const sent: Sent[] = [];
  const on: Record<string, Array<() => void>> = {};
  const clock = { now: 1_000 };
  const location = {
    href: url.href,
    pathname: url.pathname,
    hostname: url.hostname,
    protocol: url.protocol,
  };
  const document = {
    currentScript: {
      src: 'https://purestat.example/js/purestat.js',
      getAttribute: (name: string) => (name === 'data-domain' ? 'example.com' : null),
    },
    referrer: '',
    visibilityState: 'visible',
    addEventListener: (type: string, f: () => void) => (on[type] ||= []).push(f),
  };
  const window: Record<string, unknown> = {
    location,
    innerWidth: 1280,
    addEventListener: (type: string, f: () => void) => (on[type] ||= []).push(f),
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
    emit(type: string) {
      for (const f of on[type] || []) f();
    },
    setVisible(visible: boolean) {
      document.visibilityState = visible ? 'visible' : 'hidden';
      this.emit('visibilitychange');
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
  const cases = [
    browser({ href: 'http://localhost:5173/' }),
    browser({ href: 'http://192.168.1.20/' }),
    browser({ href: 'file:///C:/site/index.html' }),
    browser({ webdriver: true }),
    browser({ storage: () => ({ getItem: (k: string) => (k === 'purestat_ignore' ? 'true' : null) }) }),
  ];
  for (const b of cases) {
    (b.window.purestat as (name: string) => void)('signup');
    b.at(9_000);
    b.emit('pagehide');
    expect(b.sent).toEqual([]);
  }
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
