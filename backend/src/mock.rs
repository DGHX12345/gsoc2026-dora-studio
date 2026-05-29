use crate::models::{
    DataflowGraph, DataflowSummary, Diagnostic, GraphEdge, GraphNode, LogEntry, NodeMetrics,
    SystemStatus,
};

pub fn system_status() -> SystemStatus {
    SystemStatus {
        coordinator: "connected",
        daemon: "healthy",
        version: "dora 0.x mock",
        running_dataflows: 2,
        active_nodes: 7,
        error_count: 1,
    }
}

pub fn dataflows() -> Vec<DataflowSummary> {
    vec![
        DataflowSummary {
            id: "robot-perception-demo",
            name: "robot-perception-demo.yml",
            status: "running",
            node_count: 5,
            edge_count: 5,
        },
        DataflowSummary {
            id: "camera-logger",
            name: "camera-logger.yml",
            status: "stopped",
            node_count: 3,
            edge_count: 2,
        },
    ]
}

pub fn nodes() -> Vec<NodeMetrics> {
    vec![
        NodeMetrics {
            id: "camera",
            label: "camera",
            kind: "Python 数据源",
            status: "running",
            cpu: 18,
            memory: 164,
            restarts: 0,
            pending: 2,
        },
        NodeMetrics {
            id: "detector",
            label: "detector",
            kind: "Rust 算子",
            status: "degraded",
            cpu: 61,
            memory: 512,
            restarts: 1,
            pending: 18,
        },
        NodeMetrics {
            id: "planner",
            label: "planner",
            kind: "Python 节点",
            status: "running",
            cpu: 22,
            memory: 210,
            restarts: 0,
            pending: 4,
        },
        NodeMetrics {
            id: "logger",
            label: "logger",
            kind: "数据记录器",
            status: "running",
            cpu: 12,
            memory: 340,
            restarts: 0,
            pending: 6,
        },
        NodeMetrics {
            id: "robot_bridge",
            label: "robot_bridge",
            kind: "ROS2 桥接",
            status: "stopped",
            cpu: 0,
            memory: 0,
            restarts: 0,
            pending: 0,
        },
    ]
}

pub fn logs() -> Vec<LogEntry> {
    vec![
        LogEntry {
            time: "10:21:03".to_string(),
            node: "camera".to_string(),
            level: "info".to_string(),
            message: "已发布第 1842 帧，频率 30hz".to_string(),
        },
        LogEntry {
            time: "10:21:04".to_string(),
            node: "detector".to_string(),
            level: "warn".to_string(),
            message: "输入队列达到 18 条 pending message".to_string(),
        },
        LogEntry {
            time: "10:21:05".to_string(),
            node: "planner".to_string(),
            level: "info".to_string(),
            message: "根据 3 个检测结果生成 cmd_vel".to_string(),
        },
        LogEntry {
            time: "10:21:06".to_string(),
            node: "robot_bridge".to_string(),
            level: "error".to_string(),
            message: "桥接节点已停止，输出 cmd_vel 被丢弃".to_string(),
        },
        LogEntry {
            time: "10:21:07".to_string(),
            node: "logger".to_string(),
            level: "info".to_string(),
            message: "写入数据集分片 mock-session-0007".to_string(),
        },
    ]
}

pub fn graph() -> DataflowGraph {
    DataflowGraph {
        nodes: vec![
            GraphNode {
                id: "camera",
                label: "camera",
                kind: "Python 数据源",
                status: "running",
                x: 70,
                y: 130,
                inputs: vec!["tick: dora/timer/hz/30"],
                outputs: vec!["frame"],
                cpu: 18,
                memory: 164,
                restarts: 0,
                pending: 2,
                note: "从模拟机器人相机采集图像帧，是后续感知链路的数据入口。",
            },
            GraphNode {
                id: "detector",
                label: "detector",
                kind: "Python 算子",
                status: "degraded",
                x: 330,
                y: 130,
                inputs: vec!["frame: camera/frame"],
                outputs: vec!["boxes", "debug_image"],
                cpu: 61,
                memory: 512,
                restarts: 1,
                pending: 18,
                note: "执行目标检测；当前输入队列增长，用于展示退化节点的调试入口。",
            },
            GraphNode {
                id: "planner",
                label: "planner",
                kind: "Python 节点",
                status: "running",
                x: 590,
                y: 80,
                inputs: vec!["boxes: detector/boxes"],
                outputs: vec!["cmd_vel"],
                cpu: 22,
                memory: 210,
                restarts: 0,
                pending: 4,
                note: "将检测结果转换为机器人速度控制指令。",
            },
            GraphNode {
                id: "logger",
                label: "logger",
                kind: "数据记录器",
                status: "running",
                x: 590,
                y: 230,
                inputs: vec!["frame: camera/frame", "boxes: detector/boxes", "logs: dora/logs/error"],
                outputs: vec!["dataset_chunk"],
                cpu: 12,
                memory: 340,
                restarts: 0,
                pending: 6,
                note: "预留未来数据采集、回放和训练数据集导出的边界。",
            },
            GraphNode {
                id: "robot_bridge",
                label: "robot_bridge",
                kind: "机器人适配层",
                status: "stopped",
                x: 850,
                y: 80,
                inputs: vec!["cmd_vel: planner/cmd_vel"],
                outputs: vec![],
                cpu: 0,
                memory: 0,
                restarts: 0,
                pending: 0,
                note: "桥接节点在 mock 运行中停止，用于展示非活跃节点和错误定位。",
            },
        ],
        edges: vec![
            GraphEdge {
                id: "e1",
                from: "camera",
                to: "detector",
                label: "frame",
            },
            GraphEdge {
                id: "e2",
                from: "detector",
                to: "planner",
                label: "boxes",
            },
            GraphEdge {
                id: "e3",
                from: "camera",
                to: "logger",
                label: "frame",
            },
            GraphEdge {
                id: "e4",
                from: "detector",
                to: "logger",
                label: "boxes",
            },
            GraphEdge {
                id: "e5",
                from: "planner",
                to: "robot_bridge",
                label: "cmd_vel",
            },
        ],
        diagnostics: vec![
            Diagnostic {
                severity: "warning",
                message: "detector 的 pending queue 高于建议阈值。",
            },
            Diagnostic {
                severity: "info",
                message: "logger 被设计为未来数据集导出的边界。",
            },
            Diagnostic {
                severity: "error",
                message: "robot_bridge 已停止，cmd_vel 没有到达机器人适配层。",
            },
        ],
    }
}
