// Built-in tool registration. Idempotent: VisualizationView remounts on
// every page switch, so re-registering must not throw.

import DvizPathPanel from './dviz/DvizPathPanel.vue';
import { DvizPathTool } from './dviz/DvizPathTool';
import { MoveItTool } from './moveit/MoveItTool';
import { toolRegistry } from './registry';

export function registerBuiltinTools() {
  if (!toolRegistry.get('dviz-path')) {
    const dviz = new DvizPathTool();
    dviz.panelComponent = DvizPathPanel;
    toolRegistry.register(dviz);
  }
  // panelComponent binds at D6 (MoveItPanel.vue)
  if (!toolRegistry.get('moveit-bridge')) {
    toolRegistry.register(new MoveItTool());
  }
}
