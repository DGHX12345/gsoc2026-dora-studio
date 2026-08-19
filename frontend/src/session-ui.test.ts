import assert from 'node:assert/strict';

import {
  canStartDataflow,
  canStartSession,
  canStopDataflow,
  canStopSession,
  canSwitchItem,
  formatBytes,
  formatRecordingTime,
  recordingAction,
  sessionUiState,
  versionBadge,
  type DoraVersionItem,
  type SessionStatus,
} from './session-ui';

type TestCase = {
  name: string;
  run: () => void;
};

const cases: TestCase[] = [];

function test(name: string, run: () => void) {
  cases.push({ name, run });
}

const session = (partial: Partial<SessionStatus> = {}): SessionStatus => ({
  status: 'stopped',
  running: false,
  coordinatorConnected: false,
  coordinatorStatus: 'unavailable',
  pid: null,
  version: 'dora 1.0.0-rc.4',
  lifecycleSupported: true,
  dataflowCount: 0,
  message: '',
  ...partial,
});

test('sessionUiState reflects busy transitions', () => {
  const stopped = session();
  assert.equal(sessionUiState(stopped, 'starting'), 'starting');
  assert.equal(sessionUiState(stopped, 'stopping'), 'stopping');
  assert.equal(sessionUiState(stopped, 'idle'), 'stopped');
});

test('sessionUiState distinguishes running, error, and unavailable', () => {
  assert.equal(sessionUiState(session({ running: true, status: 'running' }), 'idle'), 'running');
  assert.equal(sessionUiState(session({ status: 'failed' }), 'idle'), 'error');
  assert.equal(
    sessionUiState(session({ lifecycleSupported: false, version: 'dora 0.5.0' }), 'idle'),
    'unavailable',
  );
});

test('start is only allowed when idle, supported, and not running', () => {
  const stopped = session();
  assert.equal(canStartSession(stopped, 'idle'), true);
  assert.equal(canStartSession(stopped, 'starting'), false);
  assert.equal(canStartSession(session({ running: true }), 'idle'), false);
  assert.equal(canStartSession(session({ lifecycleSupported: false }), 'idle'), false);
});

test('stop is only allowed when idle, supported, and running', () => {
  const running = session({ running: true, status: 'running' });
  assert.equal(canStopSession(running, 'idle'), true);
  assert.equal(canStopSession(running, 'stopping'), false);
  assert.equal(canStopSession(session(), 'idle'), false);
  assert.equal(canStopSession(session({ running: true, lifecycleSupported: false }), 'idle'), false);
});

test('dataflow lifecycle buttons follow the version gate', () => {
  const supported = session();
  assert.equal(canStartDataflow('stopped', supported), true);
  assert.equal(canStartDataflow('running', supported), false);
  assert.equal(canStopDataflow('running', supported), true);
  assert.equal(canStopDataflow('stopped', supported), false);

  const unsupported = session({ lifecycleSupported: false });
  assert.equal(canStartDataflow('stopped', unsupported), false);
  assert.equal(canStopDataflow('running', unsupported), false);
});

test('a failed dataflow can still be stopped for cleanup', () => {
  const supported = session();
  assert.equal(canStopDataflow('failed', supported), true);
  assert.equal(canStopDataflow('failed', session({ lifecycleSupported: false })), false);
});

test('recording action toggles between record, recording, and disabled', () => {
  assert.equal(recordingAction('idle', true), 'record');
  assert.equal(recordingAction('recording', true), 'recording');
  assert.equal(recordingAction('recording', false), 'disabled');
  assert.equal(recordingAction('idle', false), 'disabled');
});

test('formatBytes renders human sizes', () => {
  assert.equal(formatBytes(0), '0 B');
  assert.equal(formatBytes(796), '796 B');
  assert.equal(formatBytes(2048), '2.0 KB');
  assert.equal(formatBytes(3 * 1024 * 1024), '3.0 MB');
  assert.equal(formatBytes(2 * 1024 * 1024 * 1024), '2.0 GB');
});

test('formatRecordingTime renders local clock time', () => {
  const date = new Date(2026, 7, 18, 14, 5, 9); // Aug 18 2026 14:05:09 local
  const formatted = formatRecordingTime(date.getTime());
  assert.match(formatted, /^\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}$/);
  assert.ok(formatted.startsWith('2026-08-18'), `starts with date, got ${formatted}`);
  assert.ok(formatted.endsWith('14:05:09'), `ends with local time, got ${formatted}`);
});

const versionItem = (partial: Partial<DoraVersionItem> = {}): DoraVersionItem => ({
  path: '/opt/dora',
  version: 'dora 1.0.0',
  compatible: true,
  active: false,
  ...partial,
});

test('versionBadge follows the active item and env override', () => {
  const items = [
    versionItem({ path: '/a', version: 'dora 1.0.0', compatible: true, active: true }),
    versionItem({ path: '/b', version: 'dora 0.5.0', compatible: false }),
  ];
  assert.equal(versionBadge(items, false), 'compatible');
  assert.equal(versionBadge(items, true), 'overridden');
  assert.equal(
    versionBadge(
      [versionItem({ version: 'dora 0.5.0', compatible: false, active: true })],
      false,
    ),
    'degraded',
  );
  assert.equal(versionBadge([versionItem()], false), 'degraded');
});

test('switching is blocked for the active item and under env override', () => {
  const inactive = versionItem();
  const active = versionItem({ active: true });
  assert.equal(canSwitchItem(inactive, false), true);
  assert.equal(canSwitchItem(active, false), false);
  assert.equal(canSwitchItem(inactive, true), false);
});

let failures = 0;
for (const { name, run } of cases) {
  try {
    run();
    console.log(`ok - ${name}`);
  } catch (error) {
    failures += 1;
    console.error(`FAIL - ${name}`);
    console.error(error instanceof Error ? error.message : error);
  }
}

if (failures > 0) {
  console.error(`${failures} session-ui test(s) failed`);
  process.exitCode = 1;
} else {
  console.log(`session-ui: ${cases.length} tests passed`);
}
