// Built-in tool registration. Idempotent: VisualizationView remounts on
// every page switch, so re-registering must not throw.

import WaypointEchoPanel from './demo/WaypointEchoPanel.vue';
import { WaypointEchoTool } from './demo/WaypointEchoTool';
import DvizPathPanel from './dviz/DvizPathPanel.vue';
import { DvizPathTool } from './dviz/DvizPathTool';
import { toolRegistry } from './registry';

export function registerBuiltinTools() {
  if (!toolRegistry.get('waypoint-echo')) {
    const echo = new WaypointEchoTool();
    echo.panelComponent = WaypointEchoPanel;
    toolRegistry.register(echo);
  }
  if (!toolRegistry.get('dviz-path')) {
    const dviz = new DvizPathTool();
    dviz.panelComponent = DvizPathPanel;
    toolRegistry.register(dviz);
  }
}
