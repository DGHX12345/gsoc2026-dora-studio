import logging

import pyarrow as pa
from dora import Node


def main():
    node = Node()
    latest = {}
    chunk_id = 0

    for event in node:
        if event["type"] == "INPUT":
            latest[event["id"]] = event["value"].to_pylist()[0]
            if {"frame", "boxes", "cmd_vel"}.issubset(latest):
                chunk = f"chunk-{chunk_id:04d}: frame={latest['frame']} | boxes={latest['boxes']} | cmd_vel={latest['cmd_vel']}"
                node.send_output("dataset_chunk", pa.array([chunk]))
                logging.info("logger recorded %s", chunk)
                chunk_id += 1
        elif event["type"] == "STOP":
            break


if __name__ == "__main__":
    main()
