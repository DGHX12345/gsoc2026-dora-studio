// Aggregated tool-slot test runner: each imported module self-executes its
// test list and sets process.exitCode on failure.

import '../session-ui.test';
import './tf.test';
import './matching.test';
import './registry.test';
import './feed.test';
import './dviz/DvizPathTool.test';
import './dviz/format.test';
import './dviz/parse.test';
import './moveit/parse.test';
import './moveit/joint-config.test';
import './moveit/MoveItTool.test';
import './moveit/urdf/xml.test';
import './moveit/urdf/urdf.test';
import './moveit/urdf/robot.test';
import './moveit/urdf/meshes.test';
import './moveit/collision.test';
import './moveit/co-visualization.test';
