import { describe, expect, test } from 'bun:test';
import {
  FILE_TYPES,
  fileTypes,
  ignoreFlagSet,
  isLocalHost,
  linkEvent,
  optedIn,
  pageviewId,
  seenPercent,
  shouldSkip,
  visibleClock,
} from '../src/rules';

describe('seenPercent', () => {
  test('the bottom of the screen, as a share of the page', () => {
    expect(seenPercent(0, 1000, 4000)).toBe(25);
    expect(seenPercent(2000, 1000, 4000)).toBe(75);
    expect(seenPercent(3000, 1000, 4000)).toBe(100);
    // Overscroll (iOS bounces) never passes the bottom or the top.
    expect(seenPercent(3500, 1000, 4000)).toBe(100);
    expect(seenPercent(-200, 1000, 4000)).toBe(25);
  });

  test('a page that does not scroll has no depth', () => {
    expect(seenPercent(0, 1000, 1000)).toBe(0);
    expect(seenPercent(0, 1000, 600)).toBe(0);
    expect(seenPercent(0, 0, 600)).toBe(0);
  });
});

describe('script attributes', () => {
  test('an opt-in is present and not "false"', () => {
    expect(optedIn('')).toBe(true);
    expect(optedIn('true')).toBe(true);
    expect(optedIn('pdf,zip')).toBe(true);
    expect(optedIn(null)).toBe(false);
    expect(optedIn('false')).toBe(false);
  });

  test('file types: the ones listed, or the default list', () => {
    expect(fileTypes('')).toEqual(FILE_TYPES.split(','));
    expect(fileTypes('true')).toEqual(FILE_TYPES.split(','));
    expect(fileTypes('PDF, .zip,epub')).toEqual(['pdf', 'zip', 'epub']);
    expect(FILE_TYPES.split(',')).toContain('pdf');
  });
});

describe('linkEvent', () => {
  const types = fileTypes('');

  test('a link to another host is an outbound click, sent without its query or fragment', () => {
    expect(
      linkEvent('https://user:pw@github.com/gjovanov/roomler-ai?token=secret#readme', 'roomler.ai', true, null)
    ).toEqual(['outbound-click', 'https://github.com/gjovanov/roomler-ai']);
    expect(linkEvent('https://docs.roomler.ai/', 'roomler.ai', true, null)).toEqual([
      'outbound-click',
      'https://docs.roomler.ai/',
    ]);
  });

  test('a link on the page host, or outbound clicks not asked for, is nothing', () => {
    expect(linkEvent('https://roomler.ai/pricing', 'roomler.ai', true, types)).toBeNull();
    expect(linkEvent('https://github.com/', 'roomler.ai', false, types)).toBeNull();
  });

  test('a file of a counted type is a download, wherever it is', () => {
    expect(linkEvent('https://roomler.ai/files/Guide.PDF?v=2', 'roomler.ai', false, types)).toEqual([
      'file-download',
      'https://roomler.ai/files/Guide.PDF',
    ]);
    expect(linkEvent('https://cdn.example.net/app.dmg', 'roomler.ai', true, types)).toEqual([
      'file-download',
      'https://cdn.example.net/app.dmg',
    ]);
    expect(linkEvent('https://roomler.ai/blog/v1.2', 'roomler.ai', false, types)).toBeNull();
    expect(linkEvent('https://roomler.ai/a.epub', 'roomler.ai', false, fileTypes('epub'))).toEqual([
      'file-download',
      'https://roomler.ai/a.epub',
    ]);
    expect(linkEvent('https://roomler.ai/a.pdf', 'roomler.ai', false, null)).toBeNull();
  });

  test('anything but a web address is nothing', () => {
    expect(linkEvent('mailto:hi@example.com', 'roomler.ai', true, types)).toBeNull();
    expect(linkEvent('javascript:void(0)', 'roomler.ai', true, types)).toBeNull();
    expect(linkEvent('not a url', 'roomler.ai', true, types)).toBeNull();
  });
});

describe('isLocalHost', () => {
  test('local, loopback and private hosts', () => {
    for (const host of [
      '',
      'localhost',
      'LOCALHOST',
      'localhost.',
      'app.localhost',
      'mybox.local',
      '127.0.0.1',
      '0.0.0.0',
      '10.1.2.3',
      '172.16.0.9',
      '172.31.255.1',
      '192.168.1.20',
      '169.254.10.1',
      '100.64.0.7',
      '100.127.1.1',
      '[::1]',
      '[::]',
      '[fd00::2]',
      '[fc12:3456::1]',
      '[fe80::1]',
      '[febf::1]',
      '[::ffff:a00:1]',
      '[::ffff:7f00:1]',
    ]) {
      expect([host, isLocalHost(host)]).toEqual([host, true]);
    }
  });

  test('public hosts', () => {
    for (const host of [
      'roomler.ai',
      'www.roomler.ai',
      'local.example.com',
      'localhost.example.com',
      '172.32.0.1',
      '100.128.0.1',
      '8.8.8.8',
      '[2001:db8::1]',
      '[fc::1]',
      '[fec0::1]',
      'mylocal',
      'notlocalhost.com',
    ]) {
      expect([host, isLocalHost(host)]).toEqual([host, false]);
    }
  });
});

describe('shouldSkip', () => {
  const page = { hostname: 'roomler.ai', protocol: 'https:', webdriver: false, ignored: false };

  test('a visitor on a real host is counted', () => {
    expect(shouldSkip(page)).toBe(false);
  });

  test('a file, a local host, automation and the ignore flag are not', () => {
    expect(shouldSkip({ ...page, protocol: 'file:', hostname: '' })).toBe(true);
    expect(shouldSkip({ ...page, hostname: 'localhost' })).toBe(true);
    expect(shouldSkip({ ...page, hostname: '192.168.1.5' })).toBe(true);
    expect(shouldSkip({ ...page, webdriver: true })).toBe(true);
    expect(shouldSkip({ ...page, ignored: true })).toBe(true);
  });
});

describe('ignoreFlagSet', () => {
  const store = (value: string | null) => () => ({ getItem: () => value }) as unknown as Storage;

  test('only the string "true" sets it', () => {
    expect(ignoreFlagSet(store('true'))).toBe(true);
    expect(ignoreFlagSet(store('1'))).toBe(false);
    expect(ignoreFlagSet(store(null))).toBe(false);
    expect(ignoreFlagSet(() => null)).toBe(false);
  });

  test('storage that throws reads as not set', () => {
    expect(
      ignoreFlagSet(() => {
        throw new Error('SecurityError');
      })
    ).toBe(false);
  });
});

describe('visibleClock', () => {
  test('counts visible time only', () => {
    let now = 0;
    const clock = visibleClock(() => now, true);
    now = 10_000;
    clock.hide();
    expect(clock.seconds()).toBe(10);
    now = 110_000; // 100 s hidden
    expect(clock.seconds()).toBe(10);
    clock.show();
    now = 115_000;
    expect(clock.seconds()).toBe(15);
  });

  test('a page that loads hidden starts counting when shown', () => {
    let now = 0;
    const clock = visibleClock(() => now, false);
    now = 60_000;
    expect(clock.seconds()).toBe(0);
    clock.show();
    now = 64_000;
    expect(clock.seconds()).toBe(4);
  });

  test('show and hide twice in a row change nothing', () => {
    let now = 0;
    const clock = visibleClock(() => now, true);
    now = 1_000;
    clock.show();
    now = 2_000;
    clock.hide();
    now = 5_000;
    clock.hide();
    expect(clock.seconds()).toBe(2);
  });

  test('reset starts over', () => {
    let now = 0;
    const clock = visibleClock(() => now, true);
    now = 30_000;
    clock.reset(true);
    now = 31_000;
    expect(clock.seconds()).toBe(1);
  });
});

test('a pageview id is 16 base-36 characters', () => {
  const id = pageviewId(Math.random);
  expect(id).toMatch(/^[0-9a-z]{16}$/);
  expect(pageviewId(Math.random)).not.toBe(id);
});
