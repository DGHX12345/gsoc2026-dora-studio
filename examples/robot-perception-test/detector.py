import logging
import random

import pyarrow as pa
from dora import Node


def main():
    node = Node()
    frame_count = 0

    for event in node:
        if event["type"] == "INPUT" and event["id"] == "frame":
            frame = event["value"].to_pylist()[0]
            frame_count += 1
            count = random.randint(0, 3)
            boxes = f"{frame} -> {count} boxes"
            debug_image = f"debug overlay for {frame}"
            node.send_output("boxes", pa.array([boxes]))
            node.send_output("debug_image", pa.array([debug_image]))
            logging.info("detector produced %s", boxes)
            if frame_count % 5 == 0:
                logging.warning("detector pending queue is high in test frame %s", frame_count)
        elif event["type"] == "STOP":
            break


if __name__ == "__main__":
    main()
