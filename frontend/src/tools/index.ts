// Built-in tool registration. Idempotent: VisualizationView remounts on
// every page switch, so re-registering must not throw.

import WaypointEchoPanel from './demo/WaypointEchoPanel.vue';
import { WaypointEchoTool } from './demo/WaypointEchoTool';
import { toolRegistry } from './registry';

export function registerBuiltinTools() {
  if (!toolRegistry.get('waypoint-echo')) {
    const echo = new WaypointEchoTool();
    echo.panelComponent = WaypointEchoPanel;
    toolRegistry.register(echo);
  }
}
