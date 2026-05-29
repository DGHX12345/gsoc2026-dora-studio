import logging

import pyarrow as pa
from dora import Node


def main():
    node = Node()

    for event in node:
        if event["type"] == "INPUT" and event["id"] == "boxes":
            boxes = event["value"].to_pylist()[0]
            cmd_vel = "linear=0.20 angular=0.00" if "0 boxes" in boxes else "linear=0.08 angular=0.25"
            node.send_output("cmd_vel", pa.array([cmd_vel]))
            logging.info("planner generated %s from %s", cmd_vel, boxes)
        elif event["type"] == "STOP":
            break


if __name__ == "__main__":
    main()
