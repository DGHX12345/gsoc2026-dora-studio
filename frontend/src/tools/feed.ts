// Feed conversion — turns .drec replay entries into tool batches (R1).
// JSON number arrays are exposed as Float32Array (the dviz/moveit wire
// format); JSON objects stay as `json`; anything else stays raw bytes.

import type { SeekEntryResponse } from '../api';
import type { ToolBatch } from './types';

export function entryToToolBatch(entry: SeekEntryResponse): ToolBatch | null {
  const bytes = entry.eventBytes;
  if (!bytes || bytes.length === 0) return null;

  const text = utf8Decode(bytes);
  if (text === null) return null;

  try {
    const json: unknown = JSON.parse(text);
    if (Array.isArray(json) && json.every((n) => typeof n === 'number')) {
      return {
        nodeId: entry.nodeId,
        outputId: entry.outputId,
        timestampNs: entry.timestampNanos,
        payload: { f32: Float32Array.from(json as number[]), json },
      };
    }
    return {
      nodeId: entry.nodeId,
      outputId: entry.outputId,
      timestampNs: entry.timestampNanos,
      payload: { json },
    };
  } catch {
    // Non-JSON payload (e.g. Arrow IPC bytes) — pass through as-is
    return {
      nodeId: entry.nodeId,
      outputId: entry.outputId,
      timestampNs: entry.timestampNanos,
      payload: { bytes: Uint8Array.from(bytes) },
    };
  }
}

function utf8Decode(bytes: number[]): string | null {
  try {
    return new TextDecoder('utf-8', { fatal: true }).decode(new Uint8Array(bytes));
  } catch {
    return null;
  }
}
