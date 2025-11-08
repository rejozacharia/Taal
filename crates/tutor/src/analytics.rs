use serde::{Deserialize, Serialize};
use taal_domain::PracticeStatistics;

use crate::scoring::PerformanceReport;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionAnalytics {
    pub report: PerformanceReport,
}

impl SessionAnalytics {
    pub fn new(report: PerformanceReport) -> Self {
        Self { report }
    }

    pub fn update_statistics(&self, stats: &mut PracticeStatistics) {
        stats.average_accuracy = (stats.average_accuracy + self.report.accuracy) / 2.0;
        if self.report.accuracy > 0.9 {
            stats.highest_streak += 1;
        } else {
            stats.highest_streak = stats.highest_streak.saturating_sub(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn analytics_updates_stats() {
        let mut stats = PracticeStatistics::new();
        let analytics = SessionAnalytics::new(PerformanceReport {
            accuracy: 0.95,
            early_hits: 1,
            late_hits: 0,
        });
        analytics.update_statistics(&mut stats);
        assert!(stats.average_accuracy > 0.0);
        assert_eq!(stats.highest_streak, 1);
    }
}
