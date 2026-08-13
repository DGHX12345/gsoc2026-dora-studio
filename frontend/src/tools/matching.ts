// Port pattern matching + tool recommendation engine (D4).
//
// String patterns are anchored, case-insensitive globs where `*` matches any
// run of characters; other regex metacharacters are treated literally.
// RegExp patterns are used as-is.

import type { PortPattern } from './types';

export function patternMatches(pattern: string | RegExp, value: string): boolean {
  if (pattern instanceof RegExp) {
    pattern.lastIndex = 0;
    return pattern.test(value);
  }
  return globToRegExp(pattern).test(value);
}

function globToRegExp(glob: string): RegExp {
  const escaped = glob
    .replace(/[.+?^${}()|[\]\\]/g, '\\$&')
    .replace(/\*/g, '.*');
  return new RegExp(`^${escaped}$`, 'i');
}

export function matchToolPorts(
  subscribePorts: ReadonlyArray<PortPattern>,
  nodeId: string,
  outputId: string,
): boolean {
  return subscribePorts.some(
    (spec) =>
      patternMatches(spec.nodeIdPattern, nodeId) &&
      patternMatches(spec.outputIdPattern, outputId),
  );
}

export interface MatchablePort {
  nodeId: string;
  outputId: string;
}

export interface ToolRecommendation {
  toolId: string;
  matchedPorts: MatchablePort[];
}

export function findRecommendations(
  tools: ReadonlyArray<{ id: string; subscribePorts: PortPattern[] }>,
  ports: ReadonlyArray<MatchablePort>,
): ToolRecommendation[] {
  const recommendations: ToolRecommendation[] = [];

  for (const tool of tools) {
    const matchedPorts = ports.filter((p) =>
      matchToolPorts(tool.subscribePorts, p.nodeId, p.outputId),
    );
    if (matchedPorts.length > 0) {
      recommendations.push({ toolId: tool.id, matchedPorts });
    }
  }

  return recommendations;
}
