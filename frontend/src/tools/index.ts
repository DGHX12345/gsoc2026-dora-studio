// Built-in tool registration. Idempotent: VisualizationView remounts on
// every page switch, so re-registering must not throw.

import DvizPathPanel from './dviz/DvizPathPanel.vue';
import { DvizPathTool } from './dviz/DvizPathTool';
import MoveItPanel from './moveit/MoveItPanel.vue';
import { MoveItTool } from './moveit/MoveItTool';
import { toolRegistry } from './registry';

export function registerBuiltinTools() {
  if (!toolRegistry.get('dviz-path')) {
    const dviz = new DvizPathTool();
    dviz.panelComponent = DvizPathPanel;
    toolRegistry.register(dviz);
  }
  if (!toolRegistry.get('moveit-bridge')) {
    const moveit = new MoveItTool();
    moveit.panelComponent = MoveItPanel;
    toolRegistry.register(moveit);
  }
}
