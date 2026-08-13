import assert from 'node:assert/strict';

import { findRecommendations, matchToolPorts, patternMatches } from './matching';
import type { PortPattern } from './types';

type TestCase = {
  name: string;
  run: () => void;
};

const port = (node: string, output: string) => ({ nodeId: node, outputId: output });

const tests: TestCase[] = [
  {
    name: 'patternMatches handles anchored regular expressions',
    run: () => {
      assert.equal(patternMatches(/^waypoints$/i, 'waypoints'), true);
      assert.equal(patternMatches(/^waypoints$/i, 'WayPoints'), true);
      assert.equal(patternMatches(/^waypoints$/i, 'path'), false);
    },
  },
  {
    name: 'patternMatches handles unanchored regular expressions',
    run: () => {
      assert.equal(patternMatches(/waypoints/, 'my_waypoints_out'), true);
      assert.equal(patternMatches(/waypoints/, 'trajectory'), false);
    },
  },
  {
    name: 'patternMatches treats string patterns as anchored case-insensitive globs',
    run: () => {
      assert.equal(patternMatches('planner*', 'planner_1'), true);
      assert.equal(patternMatches('planner*', 'my_planner'), false);
      assert.equal(patternMatches('PLANNER*', 'planner_1'), true);
      assert.equal(patternMatches('*', 'anything_at_all'), true);
      assert.equal(patternMatches('planner', 'planner'), true);
      assert.equal(patternMatches('planner', 'planner_extra'), false);
    },
  },
  {
    name: 'patternMatches treats regex metacharacters literally in string patterns',
    run: () => {
      assert.equal(patternMatches('planner.v1', 'planner.v1'), true);
      assert.equal(patternMatches('planner.v1', 'plannerXv1'), false);
    },
  },
  {
    name: 'matchToolPorts requires both node and output patterns to match',
    run: () => {
      const ports: PortPattern[] = [
        { nodeIdPattern: /planner.*/, outputIdPattern: /^waypoints$/i },
        { nodeIdPattern: /.*/, outputIdPattern: /^trajectory$/i },
      ];

      assert.equal(matchToolPorts(ports, 'planner_1', 'waypoints'), true);
      assert.equal(matchToolPorts(ports, 'planner_1', 'trajectory'), true);
      assert.equal(matchToolPorts(ports, 'camera', 'waypoints'), false);
      assert.equal(matchToolPorts(ports, 'planner_1', 'image'), false);
    },
  },
  {
    name: 'matchToolPorts returns false for an empty subscription list',
    run: () => {
      assert.equal(matchToolPorts([], 'planner', 'waypoints'), false);
    },
  },
  {
    name: 'findRecommendations lists every tool with a matching port',
    run: () => {
      const tools = [
        {
          id: 'dviz-path',
          subscribePorts: [
            { nodeIdPattern: /.*/, outputIdPattern: /^(waypoints|path)$/i },
          ],
        },
        {
          id: 'moveit',
          subscribePorts: [
            { nodeIdPattern: /.*/, outputIdPattern: /^trajectory$/i },
          ],
        },
      ];
      const ports = [port('planner', 'waypoints'), port('planner', 'trajectory')];

      const recommendations = findRecommendations(tools, ports);

      assert.equal(recommendations.length, 2);
      assert.equal(recommendations[0].toolId, 'dviz-path');
      assert.deepEqual(recommendations[0].matchedPorts, [port('planner', 'waypoints')]);
      assert.equal(recommendations[1].toolId, 'moveit');
      assert.deepEqual(recommendations[1].matchedPorts, [port('planner', 'trajectory')]);
    },
  },
  {
    name: 'findRecommendations omits tools with no matching ports',
    run: () => {
      const tools = [
        {
          id: 'dviz-path',
          subscribePorts: [
            { nodeIdPattern: /.*/, outputIdPattern: /^waypoints$/i },
          ],
        },
      ];
      const ports = [port('camera', 'image')];

      assert.deepEqual(findRecommendations(tools, ports), []);
    },
  },
  {
    name: 'findRecommendations handles empty inputs',
    run: () => {
      assert.deepEqual(findRecommendations([], []), []);
      assert.deepEqual(
        findRecommendations(
          [{ id: 't', subscribePorts: [{ nodeIdPattern: '*', outputIdPattern: '*' }] }],
          [],
        ),
        [],
      );
    },
  },
  {
    name: 'findRecommendations collects all matching ports for a tool',
    run: () => {
      const tools = [
        {
          id: 'multi',
          subscribePorts: [
            { nodeIdPattern: /.*/, outputIdPattern: /^waypoints$/i },
            { nodeIdPattern: /.*/, outputIdPattern: /^trajectory$/i },
          ],
        },
      ];
      const ports = [
        port('planner', 'waypoints'),
        port('planner', 'trajectory'),
        port('camera', 'image'),
      ];

      const recommendations = findRecommendations(tools, ports);

      assert.equal(recommendations.length, 1);
      assert.deepEqual(recommendations[0].matchedPorts, [
        port('planner', 'waypoints'),
        port('planner', 'trajectory'),
      ]);
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
