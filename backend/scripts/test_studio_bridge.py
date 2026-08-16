"""Unit tests for studio_bridge.py payload logic.

Run with the M15 venv (has pyarrow):
    /home/dora/.venvs/dora-studio/bin/python -m unittest \
        backend/scripts/test_studio_bridge.py
"""

import base64
import http.server
import json
import socket
import threading
import time
import unittest
from datetime import datetime, timezone

import pyarrow as pa

import studio_bridge


class TestResolveSource(unittest.TestCase):
    def test_resolves_input_id_to_source_pair(self):
        inputs = {"trajectory": "planner/trajectory"}
        self.assertEqual(
            studio_bridge.resolve_source(inputs, "trajectory"),
            ("planner", "trajectory"),
        )

    def test_returns_none_for_unknown_input_id(self):
        self.assertIsNone(studio_bridge.resolve_source({}, "trajectory"))

    def test_returns_none_for_malformed_source(self):
        self.assertIsNone(
            studio_bridge.resolve_source(
                {"trajectory": "dora/timer/millis/50"}, "trajectory"
            )
        )


class TestSerializeEvent(unittest.TestCase):
    def test_numeric_event_serializes_full_payload(self):
        event = {
            "type": "INPUT",
            "id": "trajectory",
            "kind": "data",
            "value": pa.array([1.5, 2.5, 3.5]),
            "metadata": {
                "timestamp": "2026-08-14 10:48:01.644921+00:00",
                "num_waypoints": 3,
                "num_joints": 1,
            },
        }
        out = studio_bridge.serialize_event(
            event, "trajectory", ("planner", "trajectory")
        )
        self.assertEqual(out["node_id"], "planner")
        self.assertEqual(out["output_id"], "trajectory")
        self.assertEqual(
            out["timestamp"], 1786704481644921000
        )
        self.assertEqual(out["payload"]["values"], [1.5, 2.5, 3.5])
        self.assertEqual(out["payload"]["metadata"]["num_waypoints"], 3)

    def test_control_events_are_skipped(self):
        self.assertIsNone(
            studio_bridge.serialize_event(
                {"type": "STOP", "id": "ALL_INPUTS_CLOSED", "kind": "control"},
                "trajectory",
                ("planner", "trajectory"),
            )
        )

    def test_datetime_object_timestamp_is_supported(self):
        dt = datetime(2026, 8, 14, 10, 48, 1, 644921, tzinfo=timezone.utc)
        out = studio_bridge.serialize_event(
            {
                "type": "INPUT",
                "id": "x",
                "kind": "data",
                "value": pa.array([1]),
                "metadata": {"timestamp": dt},
            },
            "x",
            ("a", "x"),
        )
        self.assertEqual(out["timestamp"], 1786704481644921000)

    def test_missing_timestamp_falls_back_to_wall_clock(self):
        before = time.time_ns()
        out = studio_bridge.serialize_event(
            {
                "type": "INPUT",
                "id": "x",
                "kind": "data",
                "value": pa.array([1]),
                "metadata": {},
            },
            "x",
            ("a", "x"),
        )
        after = time.time_ns()
        self.assertGreaterEqual(out["timestamp"], before)
        self.assertLessEqual(out["timestamp"], after)

    def test_sim_time_float_timestamp_falls_back_to_wall_clock(self):
        # dora-mujoco sends data.time (sim clock, not wall clock) as the
        # timestamp metadata — it must not crash the bridge nor be used
        # as a frame timestamp.
        before = time.time_ns()
        out = studio_bridge.serialize_event(
            {
                "type": "INPUT",
                "id": "joint_velocities",
                "kind": "data",
                "value": pa.array([0.1, 0.2]),
                "metadata": {"timestamp": 0.123},
            },
            "joint_velocities",
            ("mujoco_sim", "joint_velocities"),
        )
        after = time.time_ns()
        self.assertGreaterEqual(out["timestamp"], before)
        self.assertLessEqual(out["timestamp"], after)

    def test_unparseable_timestamp_falls_back_to_wall_clock(self):
        before = time.time_ns()
        out = studio_bridge.serialize_event(
            {
                "type": "INPUT",
                "id": "x",
                "kind": "data",
                "value": pa.array([1]),
                "metadata": {"timestamp": "not-a-timestamp"},
            },
            "x",
            ("a", "x"),
        )
        after = time.time_ns()
        self.assertGreaterEqual(out["timestamp"], before)
        self.assertLessEqual(out["timestamp"], after)

    def test_uint8_json_bytes_are_parsed_as_json(self):
        raw = json.dumps({"success": True, "message": "ok"}).encode()
        event = {
            "type": "INPUT",
            "id": "plan_status",
            "kind": "data",
            "value": pa.array(list(raw), type=pa.uint8()),
            "metadata": {"timestamp": "2026-08-14 10:48:01.644921+00:00"},
        }
        out = studio_bridge.serialize_event(
            event, "plan_status", ("planner", "plan_status")
        )
        self.assertEqual(out["payload"]["json"], {"success": True, "message": "ok"})

    def test_uint8_non_json_bytes_are_base64(self):
        raw = b"\x00\x01\xffbinary"
        event = {
            "type": "INPUT",
            "id": "blob",
            "kind": "data",
            "value": pa.array(list(raw), type=pa.uint8()),
            "metadata": {"timestamp": "2026-08-14 10:48:01.644921+00:00"},
        }
        out = studio_bridge.serialize_event(
            event, "blob", ("camera", "blob")
        )
        self.assertEqual(
            out["payload"]["bytes_base64"], base64.b64encode(raw).decode()
        )

    def test_string_array_keeps_values(self):
        event = {
            "type": "INPUT",
            "id": "status",
            "kind": "data",
            "value": pa.array(["idle", "moving"]),
            "metadata": {"timestamp": "2026-08-14 10:48:01.644921+00:00"},
        }
        out = studio_bridge.serialize_event(
            event, "status", ("executor", "status")
        )
        self.assertEqual(out["payload"]["values"], ["idle", "moving"])


class TestPostEvent(unittest.TestCase):
    def setUp(self):
        self.received = []
        received = self.received

        class Handler(http.server.BaseHTTPRequestHandler):
            def do_POST(self):
                if self.path != "/api/live/ingest":
                    self.send_response(404)
                    self.end_headers()
                    return
                length = int(self.headers["Content-Length"])
                received.append((self.path, json.loads(self.rfile.read(length))))
                self.send_response(200)
                self.end_headers()

            def log_message(self, *args):
                pass

        self.server = http.server.HTTPServer(("127.0.0.1", 0), Handler)
        self.port = self.server.server_address[1]
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)
        self.thread.start()
        for _ in range(100):
            try:
                with socket.create_connection(
                    ("127.0.0.1", self.port), timeout=0.1
                ):
                    break
            except OSError:
                time.sleep(0.01)

    def tearDown(self):
        self.server.shutdown()
        self.server.server_close()

    def test_posts_json_to_backend_url(self):
        url = f"http://127.0.0.1:{self.port}/api/live/ingest"
        body = {"node_id": "a", "output_id": "x", "timestamp": 1, "payload": {}}
        ok = studio_bridge.post_event(url, body)
        self.assertTrue(ok)
        self.assertEqual(len(self.received), 1)
        self.assertEqual(self.received[0][0], "/api/live/ingest")
        self.assertEqual(self.received[0][1], body)

    def test_returns_false_on_http_error(self):
        url = f"http://127.0.0.1:{self.port}/nope"
        ok = studio_bridge.post_event(
            url, {"node_id": "a", "output_id": "x", "timestamp": 1, "payload": {}}
        )
        self.assertFalse(ok)


if __name__ == "__main__":
    unittest.main()
