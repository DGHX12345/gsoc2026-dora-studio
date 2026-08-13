// Built-in tool registration. Idempotent: VisualizationView remounts on
// every page switch, so re-registering must not throw.

import WaypointEchoPanel from './demo/WaypointEchoPanel.vue';
import { WaypointEchoTool } from './demo/WaypointEchoTool';
import { DvizPathTool } from './dviz/DvizPathTool';
import { toolRegistry } from './registry';

export function registerBuiltinTools() {
  if (!toolRegistry.get('waypoint-echo')) {
    const echo = new WaypointEchoTool();
    echo.panelComponent = WaypointEchoPanel;
    toolRegistry.register(echo);
  }
  // The D4 panel task binds panelComponent; core rendering needs none.
  if (!toolRegistry.get('dviz-path')) {
    toolRegistry.register(new DvizPathTool());
  }
}
