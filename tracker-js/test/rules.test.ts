import { describe, expect, test } from 'bun:test';
import { ignoreFlagSet, isLocalHost, pageviewId, shouldSkip, visibleClock } from '../src/rules';

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
