"""studio_bridge — dora node forwarding subscribed ports to Studio backend.

Runs inside a dora dataflow. For every INPUT event it POSTs a JSON frame
to the Studio backend live ingest endpoint:

    POST {STUDIO_BACKEND_URL}  (default http://127.0.0.1:3001/api/live/ingest)
    {
      "node_id": "<source node>",
      "output_id": "<source output>",
      "timestamp": <epoch nanoseconds>,
      "payload": {"values": [...] | "json": {...} | "bytes_base64": "..."}
    }

Source node/output are resolved from the node's own dataflow config
(`node.node_config()["inputs"]` maps input_id -> "node_id/output_id").
Senders' per-send metadata (e.g. num_waypoints/num_joints) is forwarded
in payload.metadata. uint8 arrays are decoded as UTF-8 JSON when
possible (dora-moveit2 sends JSON-as-uint8), otherwise base64.
"""

import base64
import json
import logging
import os
import time
import urllib.error
import urllib.request
from datetime import datetime, timezone

import pyarrow as pa
from dora import Node

DEFAULT_BACKEND_URL = "http://127.0.0.1:3001/api/live/ingest"
POST_TIMEOUT_S = 2.0


def resolve_source(inputs, input_id):
    source = inputs.get(input_id)
    if not source or source.count("/") != 1:
        return None
    node_id, output_id = source.split("/", 1)
    if not node_id or not output_id:
        return None
    return (node_id, output_id)


def timestamp_to_ns(ts):
    """Metadata timestamp → epoch nanoseconds, or None when it is not a
    wall-clock time (mujoco sim-time floats, unparseable strings) — the
    caller then falls back to receive time."""
    if isinstance(ts, datetime):
        dt = ts
    elif isinstance(ts, (int, float)):
        return None
    else:
        try:
            dt = datetime.fromisoformat(ts)
        except (TypeError, ValueError):
            return None
    if dt.tzinfo is None:
        dt = dt.replace(tzinfo=timezone.utc)
    delta = dt - datetime(1970, 1, 1, tzinfo=timezone.utc)
    return (delta.days * 86400 + delta.seconds) * 1_000_000_000 + delta.microseconds * 1000


def payload_from_value(value):
    if pa.types.is_uint8(value.type):
        raw = bytes(value.to_pylist())
        try:
            return {"json": json.loads(raw.decode("utf-8"))}
        except (UnicodeDecodeError, json.JSONDecodeError):
            return {"bytes_base64": base64.b64encode(raw).decode()}
    return {"values": value.to_pylist()}


def serialize_event(event, input_id, source):
    if event.get("type") != "INPUT" or source is None:
        return None
    value = event.get("value")
    if value is None:
        return None
    metadata = event.get("metadata") or {}
    ts = metadata.get("timestamp")
    ts_ns = timestamp_to_ns(ts) if ts is not None else None
    if ts_ns is None:
        ts_ns = time.time_ns()
    payload = payload_from_value(value)
    payload["metadata"] = {k: v for k, v in metadata.items() if k != "timestamp"}
    return {
        "node_id": source[0],
        "output_id": source[1],
        "timestamp": ts_ns,
        "payload": payload,
    }


def post_event(url, body, timeout=POST_TIMEOUT_S):
    req = urllib.request.Request(
        url,
        data=json.dumps(body).encode(),
        headers={"Content-Type": "application/json"},
        method="POST",
    )
    try:
        with urllib.request.urlopen(req, timeout=timeout) as resp:
            return resp.status == 200
    except (urllib.error.URLError, OSError):
        return False


def main():
    logging.basicConfig(level=logging.INFO)
    url = os.environ.get("STUDIO_BACKEND_URL", DEFAULT_BACKEND_URL)
    node = Node()
    inputs = node.node_config().get("inputs", {})
    for event in node:
        if event.get("type") == "STOP":
            break
        if event.get("type") != "INPUT":
            continue
        input_id = event.get("id")
        source = resolve_source(inputs, input_id)
        if source is None:
            logging.warning("skipping input %r: unresolvable source", input_id)
            continue
        body = serialize_event(event, input_id, source)
        if body is None:
            continue
        if not post_event(url, body):
            logging.warning(
                "failed to post %s/%s to %s", body["node_id"], body["output_id"], url
            )


if __name__ == "__main__":
    main()
