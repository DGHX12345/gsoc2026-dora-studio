import assert from 'node:assert/strict';

import type { SeekEntryResponse } from '../api';
import { entryToToolBatch } from './feed';

type TestCase = {
  name: string;
  run: () => void;
};

const entry = (eventBytes: number[] | undefined, overrides: Partial<SeekEntryResponse> = {}): SeekEntryResponse => ({
  byteOffset: 0,
  timestampNanos: 1_500_000,
  nodeId: 'planner',
  outputId: 'waypoints',
  eventBytes,
  ...overrides,
});

const utf8 = (text: string) => [...new TextEncoder().encode(text)];

const tests: TestCase[] = [
  {
    name: 'entryToToolBatch parses a JSON object payload',
    run: () => {
      const batch = entryToToolBatch(entry(utf8('{"waypoints":[[1,2],[3,4]]}')));

      assert.ok(batch);
      assert.equal(batch.nodeId, 'planner');
      assert.equal(batch.outputId, 'waypoints');
      assert.equal(batch.timestampNs, 1_500_000);
      assert.deepEqual(batch.payload.json, { waypoints: [[1, 2], [3, 4]] });
      assert.equal(batch.payload.f32, undefined);
    },
  },
  {
    name: 'entryToToolBatch exposes a JSON number array as Float32Array',
    run: () => {
      const batch = entryToToolBatch(entry(utf8('[0.5, 1.5, -2]')));

      assert.ok(batch);
      assert.ok(batch.payload.f32 instanceof Float32Array);
      assert.deepEqual([...batch.payload.f32!], [0.5, 1.5, -2]);
      assert.deepEqual(batch.payload.json, [0.5, 1.5, -2]);
    },
  },
  {
    name: 'entryToToolBatch keeps non-JSON payloads as raw bytes',
    run: () => {
      const bytes = [0x44, 0x4f, 0x52, 0x41, 0x41, 0x54, 0x54, 0x00, 0x01, 0x02];
      const batch = entryToToolBatch(entry(bytes));

      assert.ok(batch);
      assert.ok(batch.payload.bytes instanceof Uint8Array);
      assert.deepEqual([...batch.payload.bytes!], bytes);
      assert.equal(batch.payload.json, undefined);
      assert.equal(batch.payload.f32, undefined);
    },
  },
  {
    name: 'entryToToolBatch returns null for entries without payload bytes',
    run: () => {
      assert.equal(entryToToolBatch(entry(undefined)), null);
      assert.equal(entryToToolBatch(entry([])), null);
    },
  },
  {
    name: 'entryToToolBatch returns null for invalid UTF-8 payloads',
    run: () => {
      // 0xFF is never valid UTF-8
      assert.equal(entryToToolBatch(entry([0xff, 0xfe, 0xfd])), null);
    },
  },
];

let failures = 0;

for (const test of tests) {
  try {
    test.run();
    console.log(`ok - ${test.name}`);
  } catch (error) {
    failures += 1;
    console.error(`not ok - ${test.name}`);
    console.error(error);
  }
}

if (failures > 0) {
  process.exitCode = 1;
}
