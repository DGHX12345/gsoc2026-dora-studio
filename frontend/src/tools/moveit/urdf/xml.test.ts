// Minimal XML parser tests (M13 D4). Self-executes on import — see tests.ts.
// URDF parsing must work in node (no DOMParser), hence this tiny parser.

import assert from 'node:assert/strict';

import { parseXml } from './xml';

type TestCase = {
  name: string;
  run: () => void;
};

const tests: TestCase[] = [
  {
    name: 'parses a single element with attributes',
    run: () => {
      const el = parseXml('<robot name="reBot_B601_DM_with_gripper"/>');
      assert.equal(el.name, 'robot');
      assert.deepEqual(el.attributes, { name: 'reBot_B601_DM_with_gripper' });
      assert.equal(el.children.length, 0);
    },
  },
  {
    name: 'parses nested elements and preserves document order',
    run: () => {
      const el = parseXml('<a x="1"><b/><c y="2"><d/></c></a>');
      assert.equal(el.name, 'a');
      assert.deepEqual(el.children.map((c) => c.name), ['b', 'c']);
      assert.equal(el.children[1].attributes.y, '2');
      assert.equal(el.children[1].children[0].name, 'd');
    },
  },
  {
    name: 'skips the prolog and comments anywhere',
    run: () => {
      const el = parseXml(
        '<?xml version="1.0" encoding="utf-8"?>\n<!-- a comment --><root><!-- inner --><child/></root>',
      );
      assert.equal(el.name, 'root');
      assert.equal(el.children.length, 1);
      assert.equal(el.children[0].name, 'child');
    },
  },
  {
    name: 'supports single-quoted attribute values',
    run: () => {
      const el = parseXml("<joint name='joint1' type='revolute'/>");
      assert.equal(el.attributes.name, 'joint1');
      assert.equal(el.attributes.type, 'revolute');
    },
  },
  {
    name: 'unescapes XML entities in attribute values',
    run: () => {
      const el = parseXml('<mesh filename="a&amp;b&lt;c&gt;d&quot;e"/>');
      assert.equal(el.attributes.filename, 'a&b<c>d"e');
    },
  },
  {
    name: 'tolerates whitespace and newlines between tokens',
    run: () => {
      const el = parseXml('<joint\n  name = "joint1"\n  type = "revolute"\n>\n</joint>');
      assert.equal(el.name, 'joint');
      assert.equal(el.attributes.name, 'joint1');
      assert.equal(el.attributes.type, 'revolute');
    },
  },
  {
    name: 'throws on an unclosed element',
    run: () => {
      assert.throws(() => parseXml('<robot><link></robot>'), /mismatched closing tag/);
    },
  },
  {
    name: 'throws on trailing content after the root element',
    run: () => {
      assert.throws(() => parseXml('<a/><b/>'), /trailing content/);
    },
  },
  {
    name: 'throws on malformed XML',
    run: () => {
      assert.throws(() => parseXml('<a attr=unquoted/>'));
      assert.throws(() => parseXml('not xml at all'));
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
