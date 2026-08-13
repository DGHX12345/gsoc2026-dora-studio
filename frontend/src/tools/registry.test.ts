import assert from 'node:assert/strict';
import type * as THREE from 'three';
import type { Component } from 'vue';

import { ToolRegistry, toolRegistry } from './registry';
import type { ToolBatch, ToolCategory, ToolContext, ViewportTool } from './types';

const scene = {} as unknown as THREE.Scene;
const camera = {} as unknown as THREE.Camera;
const panel = {} as unknown as Component;
const requestRender = () => {};
const context: ToolContext = { scene, camera, requestRender };

type TestCase = {
  name: string;
  run: () => void;
};

interface FakeToolOptions {
  id?: string;
  category?: ToolCategory;
  subscribePorts?: ViewportTool['subscribePorts'];
  withSeek?: boolean;
  throwOnAttach?: boolean;
  throwOnBatch?: boolean;
}

class FakeTool implements ViewportTool {
  readonly id: string;
  readonly displayName: string;
  readonly category: ToolCategory;
  readonly subscribePorts: ViewportTool['subscribePorts'];
  panelComponent?: Component;

  attachCalls = 0;
  attachContexts: ToolContext[] = [];
  detachCalls = 0;
  seekCalls: number[] = [];
  batches: ToolBatch[] = [];

  private readonly withSeek: boolean;
  private readonly throwOnAttach: boolean;
  private readonly throwOnBatch: boolean;

  constructor(options: FakeToolOptions = {}) {
    this.id = options.id ?? 'fake';
    this.displayName = `Fake ${this.id}`;
    this.category = options.category ?? 'visualization';
    this.subscribePorts = options.subscribePorts ?? [
      { nodeIdPattern: /.*/, outputIdPattern: /.*/ },
    ];
    this.withSeek = options.withSeek ?? false;
    this.throwOnAttach = options.throwOnAttach ?? false;
    this.throwOnBatch = options.throwOnBatch ?? false;
    if (this.withSeek || this.throwOnAttach || this.throwOnBatch) {
      this.panelComponent = panel;
    }
  }

  onAttach(context: ToolContext): void {
    this.attachCalls += 1;
    this.attachContexts.push(context);
    if (this.throwOnAttach) throw new Error('attach exploded');
  }

  onBatch(batch: ToolBatch): void {
    if (this.throwOnBatch) throw new Error('batch exploded');
    this.batches.push(batch);
  }

  onTimelineSeek(timestampNs: number): void {
    if (this.withSeek) this.seekCalls.push(timestampNs);
  }

  onDetach(): void {
    this.detachCalls += 1;
  }
}

const batch = (nodeId: string, outputId: string): ToolBatch => ({
  nodeId,
  outputId,
  timestampNs: 1_000_000,
  payload: { json: {} },
});

const tests: TestCase[] = [
  {
    name: 'register adds a tool and list returns it',
    run: () => {
      const registry = new ToolRegistry();
      const tool = new FakeTool({ id: 'a' });

      registry.register(tool);

      assert.equal(registry.get('a'), tool);
      assert.equal(registry.list().length, 1);
      assert.equal(registry.list()[0].id, 'a');
    },
  },
  {
    name: 'register rejects a duplicate tool id',
    run: () => {
      const registry = new ToolRegistry();
      registry.register(new FakeTool({ id: 'a' }));

      assert.throws(() => registry.register(new FakeTool({ id: 'a' })), /already registered/);
    },
  },
  {
    name: 'unregister removes a tool and is a no-op for unknown ids',
    run: () => {
      const registry = new ToolRegistry();
      registry.register(new FakeTool({ id: 'a' }));

      registry.unregister('a');
      assert.equal(registry.get('a'), undefined);
      assert.equal(registry.list().length, 0);
      registry.unregister('nope');
    },
  },
  {
    name: 'attachToScene calls onAttach and marks the tool attached',
    run: () => {
      const registry = new ToolRegistry();
      const tool = new FakeTool({ id: 'a' });
      registry.register(tool);

      registry.attachToScene('a', context);

      assert.equal(tool.attachCalls, 1);
      assert.equal(registry.statusOf('a'), 'attached');
    },
  },
  {
    name: 'attachToScene passes the scene, camera and render trigger to the tool',
    run: () => {
      const registry = new ToolRegistry();
      const tool = new FakeTool({ id: 'a' });
      registry.register(tool);

      registry.attachToScene('a', context);

      assert.equal(tool.attachContexts.length, 1);
      assert.equal(tool.attachContexts[0].scene, scene);
      assert.equal(tool.attachContexts[0].camera, camera);
      assert.equal(tool.attachContexts[0].requestRender, requestRender);
    },
  },
  {
    name: 'attachToScene throws for an unknown tool id',
    run: () => {
      const registry = new ToolRegistry();
      assert.throws(() => registry.attachToScene('nope', context), /unknown tool/);
    },
  },
  {
    name: 'attaching an attached tool does not call onAttach twice',
    run: () => {
      const registry = new ToolRegistry();
      const tool = new FakeTool({ id: 'a' });
      registry.register(tool);

      registry.attachToScene('a', context);
      registry.attachToScene('a', context);

      assert.equal(tool.attachCalls, 1);
    },
  },
  {
    name: 'detachFromScene calls onDetach and marks the tool detached',
    run: () => {
      const registry = new ToolRegistry();
      const tool = new FakeTool({ id: 'a' });
      registry.register(tool);
      registry.attachToScene('a', context);

      registry.detachFromScene('a');

      assert.equal(tool.detachCalls, 1);
      assert.equal(registry.statusOf('a'), 'detached');
    },
  },
  {
    name: 'detaching a detached tool does not call onDetach',
    run: () => {
      const registry = new ToolRegistry();
      const tool = new FakeTool({ id: 'a' });
      registry.register(tool);

      registry.detachFromScene('a');

      assert.equal(tool.detachCalls, 0);
    },
  },
  {
    name: 'a throwing onAttach marks the tool errored and the registry survives',
    run: () => {
      const registry = new ToolRegistry();
      const bad = new FakeTool({ id: 'bad', throwOnAttach: true });
      const good = new FakeTool({ id: 'good' });
      registry.register(bad);
      registry.register(good);

      registry.attachToScene('bad', context);
      registry.attachToScene('good', context);

      assert.equal(registry.statusOf('bad'), 'error');
      assert.equal(registry.statusOf('good'), 'attached');
    },
  },
  {
    name: 'broadcastBatch delivers to attached tools with a matching port',
    run: () => {
      const registry = new ToolRegistry();
      const tool = new FakeTool({
        id: 'path',
        subscribePorts: [{ nodeIdPattern: /.*/, outputIdPattern: /^waypoints$/i }],
      });
      registry.register(tool);
      registry.attachToScene('path', context);

      registry.broadcastBatch(batch('planner', 'waypoints'));

      assert.equal(tool.batches.length, 1);
      assert.equal(tool.batches[0].nodeId, 'planner');
    },
  },
  {
    name: 'broadcastBatch skips non-matching ports and detached tools',
    run: () => {
      const registry = new ToolRegistry();
      const tool = new FakeTool({
        id: 'path',
        subscribePorts: [{ nodeIdPattern: /.*/, outputIdPattern: /^waypoints$/i }],
      });
      const detached = new FakeTool({ id: 'off' });
      registry.register(tool);
      registry.register(detached);
      registry.attachToScene('path', context);

      registry.broadcastBatch(batch('planner', 'trajectory'));
      registry.broadcastBatch(batch('planner', 'waypoints'));

      assert.equal(tool.batches.length, 1);
      assert.equal(detached.batches.length, 0);
    },
  },
  {
    name: 'broadcastBatch delivers one batch to every subscribed tool',
    run: () => {
      const registry = new ToolRegistry();
      const first = new FakeTool({
        id: 'first',
        subscribePorts: [{ nodeIdPattern: /.*/, outputIdPattern: /^waypoints$/i }],
      });
      const second = new FakeTool({
        id: 'second',
        subscribePorts: [{ nodeIdPattern: /.*/, outputIdPattern: /^waypoints$/i }],
      });
      registry.register(first);
      registry.register(second);
      registry.attachToScene('first', context);
      registry.attachToScene('second', context);

      registry.broadcastBatch(batch('planner', 'waypoints'));

      assert.equal(first.batches.length, 1);
      assert.equal(second.batches.length, 1);
    },
  },
  {
    name: 'a throwing onBatch does not break delivery to other tools',
    run: () => {
      const registry = new ToolRegistry();
      const bad = new FakeTool({ id: 'bad', throwOnBatch: true });
      const good = new FakeTool({ id: 'good' });
      registry.register(bad);
      registry.register(good);
      registry.attachToScene('bad', context);
      registry.attachToScene('good', context);

      registry.broadcastBatch(batch('node', 'out'));

      assert.equal(good.batches.length, 1);
      assert.equal(registry.statusOf('bad'), 'error');
    },
  },
  {
    name: 'broadcastSeek calls onTimelineSeek only on attached tools that implement it',
    run: () => {
      const registry = new ToolRegistry();
      const withSeek = new FakeTool({ id: 'seeker', withSeek: true });
      const withoutSeek = new FakeTool({ id: 'plain' });
      const detachedSeeker = new FakeTool({ id: 'off', withSeek: true });
      registry.register(withSeek);
      registry.register(withoutSeek);
      registry.register(detachedSeeker);
      registry.attachToScene('seeker', context);
      registry.attachToScene('plain', context);

      registry.broadcastSeek(42_000_000);

      assert.deepEqual(withSeek.seekCalls, [42_000_000]);
      assert.deepEqual(withoutSeek.seekCalls, []);
      assert.deepEqual(detachedSeeker.seekCalls, []);
    },
  },
  {
    name: 'subscribers are notified when the registry state changes',
    run: () => {
      const registry = new ToolRegistry();
      const events: string[] = [];
      registry.subscribe(() => events.push('changed'));

      registry.register(new FakeTool({ id: 'a' }));
      registry.attachToScene('a', context);
      registry.detachFromScene('a');
      registry.unregister('a');

      assert.equal(events.length, 4);
    },
  },
  {
    name: 'a singleton toolRegistry is exported for the app',
    run: () => {
      assert.ok(toolRegistry instanceof ToolRegistry);
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
