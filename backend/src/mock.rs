use crate::models::{LogEntry, SystemStatus};

pub fn system_status() -> SystemStatus {
    SystemStatus {
        coordinator: "connected".to_string(),
        daemon: "healthy".to_string(),
        version: "dora 0.x mock".to_string(),
        running_dataflows: 2,
        active_nodes: 7,
        error_count: 1,
    }
}

pub fn logs() -> Vec<LogEntry> {
    vec![
        LogEntry {
            time: "10:21:03".to_string(),
            timestamp: "2026-06-05T10:21:03.000Z".to_string(),
            node: "camera".to_string(),
            level: "info".to_string(),
            message: "已发布第 1842 帧，频率 30hz".to_string(),
            raw_message: "2026-06-05T10:21:03.000Z stdout camera.py:44 已发布第 1842 帧，频率 30hz".to_string(),
            source: "stdout".to_string(),
            source_file: Some("camera.py".to_string()),
            source_line: Some("44".to_string()),
        },
        LogEntry {
            time: "10:21:04".to_string(),
            timestamp: "2026-06-05T10:21:04.000Z".to_string(),
            node: "detector".to_string(),
            level: "warn".to_string(),
            message: "输入队列达到 18 条 pending message".to_string(),
            raw_message: "2026-06-05T10:21:04.000Z stderr detector.rs:118 输入队列达到 18 条 pending message".to_string(),
            source: "stderr".to_string(),
            source_file: Some("detector.rs".to_string()),
            source_line: Some("118".to_string()),
        },
        LogEntry {
            time: "10:21:05".to_string(),
            timestamp: "2026-06-05T10:21:05.000Z".to_string(),
            node: "planner".to_string(),
            level: "info".to_string(),
            message: "根据 3 个检测结果生成 cmd_vel".to_string(),
            raw_message: "2026-06-05T10:21:05.000Z stdout planner.py:77 根据 3 个检测结果生成 cmd_vel".to_string(),
            source: "stdout".to_string(),
            source_file: Some("planner.py".to_string()),
            source_line: Some("77".to_string()),
        },
        LogEntry {
            time: "10:21:06".to_string(),
            timestamp: "2026-06-05T10:21:06.000Z".to_string(),
            node: "robot_bridge".to_string(),
            level: "error".to_string(),
            message: "桥接节点已停止，输出 cmd_vel 被丢弃".to_string(),
            raw_message: "2026-06-05T10:21:06.000Z stderr robot_bridge.py:31 桥接节点已停止，输出 cmd_vel 被丢弃".to_string(),
            source: "stderr".to_string(),
            source_file: Some("robot_bridge.py".to_string()),
            source_line: Some("31".to_string()),
        },
        LogEntry {
            time: "10:21:07".to_string(),
            timestamp: "2026-06-05T10:21:07.000Z".to_string(),
            node: "logger".to_string(),
            level: "info".to_string(),
            message: "写入数据集分片 mock-session-0007".to_string(),
            raw_message: "2026-06-05T10:21:07.000Z stdout logger.py:52 写入数据集分片 mock-session-0007".to_string(),
            source: "stdout".to_string(),
            source_file: Some("logger.py".to_string()),
            source_line: Some("52".to_string()),
        },
    ]
}
