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

/** Merge two recommendation lists: union by toolId, dedupe matchedPorts by
 * (nodeId, outputId), preserving first-list order then appending new tools. */
export function mergeRecommendations(
  a: ToolRecommendation[],
  b: ToolRecommendation[],
): ToolRecommendation[] {
  const merged = new Map<string, ToolRecommendation>();
  const seenPorts = new Map<string, Set<string>>();

  const addRecommendation = (recommendation: ToolRecommendation) => {
    const existing = merged.get(recommendation.toolId);
    if (!existing) {
      merged.set(recommendation.toolId, {
        toolId: recommendation.toolId,
        matchedPorts: [],
      });
    }
    const target = merged.get(recommendation.toolId)!;
    let seen = seenPorts.get(recommendation.toolId);
    if (!seen) {
      seen = new Set<string>();
      seenPorts.set(recommendation.toolId, seen);
    }
    for (const port of recommendation.matchedPorts) {
      const key = `${port.nodeId}\u0000${port.outputId}`;
      if (!seen.has(key)) {
        seen.add(key);
        target.matchedPorts.push(port);
      }
    }
  };

  for (const recommendation of a) addRecommendation(recommendation);
  for (const recommendation of b) addRecommendation(recommendation);

  return [...merged.values()];
}
