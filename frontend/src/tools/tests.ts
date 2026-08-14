// Aggregated tool-slot test runner: each imported module self-executes its
// test list and sets process.exitCode on failure.

import './tf.test';
import './matching.test';
import './registry.test';
import './feed.test';
import './dviz/DvizPathTool.test';
import './dviz/format.test';
import './dviz/parse.test';
import './demo/WaypointEchoTool.test';
