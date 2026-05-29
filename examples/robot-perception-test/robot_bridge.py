import logging

from dora import Node


def main():
    node = Node()
    command_count = 0

    for event in node:
        if event["type"] == "INPUT" and event["id"] == "cmd_vel":
            cmd_vel = event["value"].to_pylist()[0]
            command_count += 1
            logging.info("robot_bridge would send command: %s", cmd_vel)
            print(f"[robot_bridge] {cmd_vel}")
            if command_count % 9 == 0:
                logging.error("robot_bridge simulated command drop at command %s", command_count)
        elif event["type"] == "STOP":
            break


if __name__ == "__main__":
    main()
