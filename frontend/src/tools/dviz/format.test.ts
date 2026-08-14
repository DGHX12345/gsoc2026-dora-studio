// Flat dviz formatting helper tests (M12 D4). Self-executes on import — see tests.ts.

import assert from 'node:assert/strict';

import { formatLength, formatPosition } from './format';

type TestCase = {
  name: string;
  run: () => void;
};

const tests: TestCase[] = [
  {
    name: 'formatLength renders 0 with two decimals',
    run: () => {
      assert.equal(formatLength(0), '0.00 m');
    },
  },
  {
    name: 'formatLength rounds to two decimals',
    run: () => {
      assert.equal(formatLength(0.457), '0.46 m');
    },
  },
  {
    name: 'formatLength keeps two decimals on whole meters',
    run: () => {
      assert.equal(formatLength(12), '12.00 m');
    },
  },
  {
    name: 'formatLength handles large values without exponent notation',
    run: () => {
      assert.equal(formatLength(1234.5), '1234.50 m');
    },
  },
  {
    name: 'formatPosition rounds each axis to two decimals',
    run: () => {
      assert.equal(formatPosition(0.124, -0.456, 0.016), '(0.12, -0.46, 0.02)');
    },
  },
  {
    name: 'formatPosition renders negative values with a leading minus',
    run: () => {
      assert.equal(formatPosition(-1, -2.5, -12), '(-1.00, -2.50, -12.00)');
    },
  },
  {
    name: 'formatPosition normalizes negative zero to 0.00',
    run: () => {
      assert.equal(formatPosition(-0.001, -0, 0.004), '(0.00, 0.00, 0.00)');
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
