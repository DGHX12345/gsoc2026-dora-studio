import logging

import pyarrow as pa
from dora import Node


def main():
    node = Node()
    frame_id = 0

    for event in node:
        if event["type"] == "INPUT" and event["id"] == "tick":
            frame = f"frame-{frame_id:04d}: simulated RGB image"
            node.send_output("frame", pa.array([frame]))
            logging.info("camera published %s", frame)
            frame_id += 1
        elif event["type"] == "STOP":
            break


if __name__ == "__main__":
    main()
