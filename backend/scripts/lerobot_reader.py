#!/usr/bin/env python3
"""dora-studio LeRobot reader bridge. JSON on stdout, errors as {"error": ...}."""
import json
import sys
from pathlib import Path

try:
    import pyarrow as pa
    import pyarrow.compute as pc
    import pyarrow.parquet as pq
except ImportError:
    pa = pc = pq = None


def fail(msg):
    print(json.dumps({"error": msg}))
    sys.exit(1)


def main():
    if len(sys.argv) < 2:
        fail("usage: lerobot_reader.py <scan|frames|gen-demo> ...")
    if pa is None or pq is None:
        fail("pyarrow not installed; install with: pip install pyarrow")
    cmd = sys.argv[1]
    if cmd == "scan":
        scan(Path(sys.argv[2]))
    elif cmd == "frames":
        frames(Path(sys.argv[2]), int(sys.argv[3]), int(sys.argv[4]), int(sys.argv[5]))
    elif cmd == "gen-demo":
        gen_demo(Path(sys.argv[2]), sys.argv[3])
    else:
        fail(f"unknown command: {cmd}")


def scan(root: Path):
    # v1: data/episode_*.parquet; v2: data/chunk-*/file-*.parquet
    v1 = sorted(root.glob("data/episode_*.parquet"))
    v2 = sorted(root.glob("data/chunk-*/file-*.parquet"))
    if v1:
        layout, files = "v1", v1
        episodes = []
        for f in files:
            idx = int(f.stem.split("_")[-1])
            meta = pq.ParquetFile(f).metadata
            t = pq.read_table(f, columns=["timestamp"]).to_pydict()["timestamp"]
            episodes.append({"index": idx, "rows": meta.num_rows,
                             "startTs": float(t[0]), "endTs": float(t[-1])})
    elif v2:
        layout, files = "v2", v2
        by_ep = {}
        for f in files:
            d = pq.read_table(f, columns=["episode_index", "timestamp"]).to_pydict()
            for ep, ts in zip(d["episode_index"], d["timestamp"]):
                by_ep.setdefault(int(ep), []).append(float(ts))
        episodes = [{"index": ep, "rows": len(ts), "startTs": ts[0], "endTs": ts[-1]}
                    for ep, ts in sorted(by_ep.items())]
    else:
        fail(f"no LeRobot parquet data found under {root}")
    # schema_arrow.names 返回顶层字段名（List 列取 action/observation.state，
    # 而不是内部的 element）
    columns = list(dict.fromkeys(pq.ParquetFile(files[0]).schema_arrow.names))
    tasks = {}
    tasks_file = root / "meta" / "tasks.parquet"
    if tasks_file.exists():
        t = pq.read_table(tasks_file).to_pydict()
        desc_col = next((c for c in t if c != "task_index"), None)
        for i, ti in enumerate(t.get("task_index", [])):
            tasks[int(ti)] = str(t[desc_col][i]) if desc_col else f"Task {int(ti)}"
    print(json.dumps({
        "name": root.name,
        "layout": layout,
        "columns": columns,
        "episodes": episodes,
        "tasks": tasks,
        "hasImageColumns": any(c.startswith("observation.images") for c in columns),
    }))


def frames(root: Path, episode: int, offset: int, limit: int):
    v1 = sorted(root.glob("data/episode_*.parquet"))
    if v1:
        f = root / f"data/episode_{episode:06d}.parquet"
        if not f.exists():
            fail(f"episode {episode} not found")
        t = pq.read_table(f)
    else:
        tables = []
        for f in sorted(root.glob("data/chunk-*/file-*.parquet")):
            t = pq.read_table(f)
            tables.append(t.filter(pc.equal(t.column("episode_index"), episode)))
        tables = [t for t in tables if t.num_rows > 0]
        if not tables:
            fail(f"episode {episode} not found")
        t = pa.concat_tables(tables) if len(tables) > 1 else tables[0]
    d = t.to_pydict()
    total = len(d["timestamp"])
    sl = slice(offset, offset + limit)
    ts = d["timestamp"]
    print(json.dumps({
        "frames": [{
            "frameIndex": int(d["frame_index"][i]) if d.get("frame_index") and d["frame_index"][i] is not None else i,
            "timestamp": float(ts[i]),
            "taskIndex": int(d["task_index"][i]) if d.get("task_index") and d["task_index"][i] is not None else None,
            "action": [float(x) for x in (d["action"][i] or [])],
            "state": [float(x) for x in (d["observation.state"][i] or [])],
        } for i in range(sl.start, min(sl.stop, total))],
        "total": total,
        "episodeStartTs": float(ts[0]),
    }))


def gen_demo(root: Path, layout: str):
    root.mkdir(parents=True, exist_ok=True)
    (root / "meta").mkdir(exist_ok=True)
    n_ep, n_frames = 3, 40
    rows = []
    for ep in range(n_ep):
        for i in range(n_frames):
            rows.append({
                "action": [0.1 * ep + 0.001 * i] * 7,
                "observation.state": [0.2 * ep + 0.001 * i] * 7,
                "timestamp": float(i) / 30.0,
                "frame_index": i,
                "episode_index": ep,
                "index": ep * n_frames + i,
                "task_index": ep,
            })
    schema = pa.schema([
        ("action", pa.list_(pa.float32())),
        ("observation.state", pa.list_(pa.float32())),
        ("timestamp", pa.float32()),
        ("frame_index", pa.int64()),
        ("episode_index", pa.int64()),
        ("index", pa.int64()),
        ("task_index", pa.int64()),
    ])
    table = pa.Table.from_pylist(rows, schema=schema)
    tasks = pa.Table.from_pylist(
        [{"task_index": ep, "task": f"Demo task {ep}"} for ep in range(n_ep)])
    if layout == "v1":
        (root / "data").mkdir(exist_ok=True)
        for ep in range(n_ep):
            pq.write_table(table.filter(pc.equal(table.column("episode_index"), ep)),
                           root / "data" / f"episode_{ep:06d}.parquet")
    else:
        (root / "data" / "chunk-000").mkdir(parents=True, exist_ok=True)
        pq.write_table(table, root / "data" / "chunk-000" / "file-000.parquet")
    pq.write_table(tasks, root / "meta" / "tasks.parquet")
    print(json.dumps({"ok": True, "layout": layout}))


if __name__ == "__main__":
    main()
