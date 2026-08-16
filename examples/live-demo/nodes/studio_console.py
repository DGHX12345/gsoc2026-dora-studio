"""studio_console — command channel node (M15 B6).

The reverse of studio_bridge: polls the Studio backend command queue
(GET /api/live/command?since_seq=N) and forwards console commands into
the dataflow as dora outputs:

  plan    -> target_point [x, y, z]   (planner replans to this goal)
  execute -> execute_command {"command": "execute"}
  stop    -> execute_command {"command": "stop"}
  auto    -> resume_command {"command": "auto"}  (planner resumes orbit)
  scene   -> scene_command {"action", "object"}  (costmap_source edits)

Commands are consumed once (seq watermark). Backend URL comes from
STUDIO_BACKEND_URL (default http://127.0.0.1:3001/api/live/ingest);
the command path is derived by replacing /ingest with /command.
"""

import json
import time
import urllib.request

import pyarrow as pa
from dora import Node

DEFAULT_BACKEND_URL = "http://127.0.0.1:3001/api/live/ingest"
POLL_INTERVAL_S = 0.5
TARGET_Z = 0.30


def command_to_outputs(command):
    """Map one backend command to (output_id, payload) dora outputs."""
    kind = command.get("kind")
    if kind == "plan":
        target = command.get("target")
        if not isinstance(target, list) or len(target) < 2:
            return []
        x, y = float(target[0]), float(target[1])
        z = float(target[2]) if len(target) >= 3 and isinstance(target[2], (int, float)) else TARGET_Z
        return [
            ("target_point", [x, y, z]),
            ("mode_command", {"command": "plan"}),
        ]
    if kind in ("execute", "stop"):
        return [("execute_command", {"command": kind})]
    if kind == "auto":
        return [("resume_command", {"command": "auto"})]
    if kind == "scene":
        action = command.get("action")
        obj = command.get("object")
        if action is None or obj is None:
            return []
        return [("scene_command", {"action": action, "object": obj})]
    return []


def advance_watermark(since_seq, next_seq):
    """Backend restart detection: when the backend's next seq is at or
    below our watermark, the queue reset — restart polling from 0."""
    if since_seq >= next_seq:
        return 0
    return since_seq


def command_url(backend_url):
    if backend_url.endswith("/api/live/ingest"):
        return backend_url[: -len("/api/live/ingest")] + "/api/live/command"
    return backend_url


def main():
    node = Node()
    url = command_url(node.node_config().get("env", {}).get(
        "STUDIO_BACKEND_URL", DEFAULT_BACKEND_URL
    ))
    since_seq = 0
    while True:
        event = node.try_recv()
        if event is not None and event.get("type") == "STOP":
            break

        try:
            req = urllib.request.Request(f"{url}?since_seq={since_seq}")
            with urllib.request.urlopen(req, timeout=2.0) as resp:
                body = json.loads(resp.read().decode("utf-8"))
            next_seq = body.get("next_seq", 0)
            since_seq = advance_watermark(since_seq, next_seq)
            for command in body.get("commands", []):
                seq = command.get("seq", since_seq)
                for output_id, payload in command_to_outputs(command):
                    if output_id in (
                        "scene_command",
                        "execute_command",
                        "resume_command",
                        "mode_command",
                    ):
                        encoded = json.dumps(payload).encode()
                        node.send_output(
                            output_id,
                            pa.array(list(encoded), type=pa.uint8()),
                        )
                    else:
                        node.send_output(output_id, pa.array(payload))
                since_seq = max(since_seq, seq)
        except (urllib.error.URLError, OSError, json.JSONDecodeError):
            pass  # backend not up yet — keep polling

        time.sleep(POLL_INTERVAL_S)


if __name__ == "__main__":
    main()
