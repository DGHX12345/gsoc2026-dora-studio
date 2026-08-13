//! Unified monitoring controller — opt-in diagnostic mode (M11.5 D1).
//!
//! Both collectors start stopped; the controller is the single switch the
//! API layer uses to start/stop monitoring targets.

use crate::metrics::MetricsCollector;
use crate::otel::OtelCollector;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MonitorTarget {
    NodeMetrics,
    OtelSpans,
}

pub struct MonitoringController {
    pub metrics: MetricsCollector,
    pub otel: OtelCollector,
}

impl MonitoringController {
    pub fn new(metrics: MetricsCollector, otel: OtelCollector) -> Self {
        Self { metrics, otel }
    }

    pub fn set_enabled(&self, target: MonitorTarget, enabled: bool) {
        match target {
            MonitorTarget::NodeMetrics => {
                if enabled {
                    self.metrics.start();
                } else {
                    self.metrics.stop();
                }
            }
            MonitorTarget::OtelSpans => {
                if enabled {
                    self.otel.start();
                } else {
                    self.otel.stop();
                }
            }
        }
    }

    pub async fn status(&self) -> serde_json::Value {
        serde_json::json!({
            "nodeMetrics": self.metrics.status().await,
            "otelSpans": self.otel.status().await,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::MetricsCollector;
    use crate::otel::OtelCollector;
    use std::time::Duration;

    #[tokio::test]
    async fn set_enabled_starts_and_stops_the_targeted_collector() {
        let metrics = MetricsCollector::new(Duration::from_millis(5));
        let otel = OtelCollector::new("http://localhost:1".into());
        let controller = MonitoringController::new(metrics.clone(), otel.clone());

        assert!(!metrics.is_running());
        assert!(!otel.is_running());

        controller.set_enabled(MonitorTarget::NodeMetrics, true);
        assert!(metrics.is_running());
        assert!(!otel.is_running());

        controller.set_enabled(MonitorTarget::OtelSpans, true);
        assert!(otel.is_running());

        controller.set_enabled(MonitorTarget::NodeMetrics, false);
        assert!(!metrics.is_running());
        assert!(otel.is_running());

        // idempotent: enabling an enabled target stays enabled
        controller.set_enabled(MonitorTarget::OtelSpans, true);
        assert!(otel.is_running());
        controller.set_enabled(MonitorTarget::OtelSpans, false);
        assert!(!otel.is_running());
    }

    #[tokio::test]
    async fn status_reports_both_targets_with_enabled_flag() {
        let metrics = MetricsCollector::new(Duration::from_millis(5));
        let otel = OtelCollector::new("http://localhost:1".into());
        let controller = MonitoringController::new(metrics.clone(), otel.clone());

        controller.set_enabled(MonitorTarget::NodeMetrics, true);
        controller.set_enabled(MonitorTarget::OtelSpans, true);

        let status = controller.status().await;
        assert_eq!(status["nodeMetrics"]["enabled"], true);
        assert_eq!(status["otelSpans"]["enabled"], true);
        assert!(status["nodeMetrics"]["sampleCount"].is_u64());
        assert!(status["otelSpans"]["spanCount"].is_u64());

        controller.set_enabled(MonitorTarget::NodeMetrics, false);
        controller.set_enabled(MonitorTarget::OtelSpans, false);
        let status = controller.status().await;
        assert_eq!(status["nodeMetrics"]["enabled"], false);
        assert_eq!(status["otelSpans"]["enabled"], false);
    }
}
