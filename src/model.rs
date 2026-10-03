use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Debug, Serialize)]
pub struct Device {
    pub id: String,
    pub name: String,
    pub battery: Option<u8>,
    pub fresh: bool,
    pub source: String,
    pub detail: String,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct Snapshot {
    pub devices: Vec<Device>,
    pub status: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub warning_percent: u8,
    pub critical_percent: u8,
    pub poll_seconds: u64,
    pub warning_reminder_minutes: u64,
    pub critical_reminder_minutes: u64,
    pub display_seconds: u64,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            warning_percent: 20,
            critical_percent: 10,
            poll_seconds: 60,
            warning_reminder_minutes: 240,
            critical_reminder_minutes: 60,
            display_seconds: 12,
        }
    }
}
impl Config {
    pub fn validate(&self) -> Result<(), String> {
        if self.critical_percent == 0
            || self.critical_percent >= self.warning_percent
            || self.warning_percent > 90
        {
            return Err("Thresholds must satisfy 0 < critical < warning <= 90.".into());
        }
        if !(30..=3600).contains(&self.poll_seconds)
            || !(5..=60).contains(&self.display_seconds)
            || !(1..=1440).contains(&self.warning_reminder_minutes)
            || !(1..=1440).contains(&self.critical_reminder_minutes)
        {
            return Err(
                "Use poll_seconds 30-3600, display_seconds 5-60 and reminder minutes 1-1440."
                    .into(),
            );
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AlertState {
    pub tier: u8,
    pub last_alert: u64,
}
#[derive(Default, Serialize, Deserialize)]
pub struct Policy {
    pub devices: HashMap<String, AlertState>,
}
impl Policy {
    /// Only a successful live read can issue or rearm an alert. Unknown and cached
    /// readings never become zero, and a disconnect never resets reminder state.
    pub fn evaluate(&mut self, device: &Device, config: &Config, now: u64) -> bool {
        let Some(battery) = device.battery.filter(|v| *v <= 100) else {
            return false;
        };
        if !device.fresh {
            return false;
        }
        let state = self.devices.entry(device.id.clone()).or_default();
        if battery > config.warning_percent.saturating_add(5) {
            state.tier = 0;
            return false;
        }
        let tier = if battery <= config.critical_percent {
            2
        } else if battery <= config.warning_percent {
            1
        } else {
            0
        };
        if tier == 0 {
            return false;
        }
        let minutes = if tier == 2 {
            config.critical_reminder_minutes
        } else {
            config.warning_reminder_minutes
        };
        if tier > state.tier || now.saturating_sub(state.last_alert) >= minutes * 60 {
            state.tier = tier;
            state.last_alert = now;
            return true;
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn device(battery: Option<u8>, fresh: bool) -> Device {
        Device {
            id: "mouse".into(),
            name: "Mouse".into(),
            battery,
            fresh,
            source: "test".into(),
            detail: String::new(),
        }
    }
    #[test]
    fn alerts_at_threshold_escalates_and_limits_reminders() {
        let mut p = Policy::default();
        let c = Config::default();
        assert!(!p.evaluate(&device(Some(21), true), &c, 100));
        assert!(p.evaluate(&device(Some(20), true), &c, 100));
        assert!(!p.evaluate(&device(Some(19), true), &c, 200));
        assert!(p.evaluate(&device(Some(10), true), &c, 300));
        assert!(!p.evaluate(&device(Some(9), true), &c, 3899));
        assert!(p.evaluate(&device(Some(9), true), &c, 3900));
    }
    #[test]
    fn cached_unknown_and_disconnect_do_not_alert_or_rearm() {
        let mut p = Policy::default();
        let c = Config::default();
        assert!(!p.evaluate(&device(None, true), &c, 100));
        assert!(!p.evaluate(&device(Some(0), false), &c, 100));
        assert!(p.evaluate(&device(Some(15), true), &c, 100));
        p.evaluate(&device(Some(100), false), &c, 110);
        p.evaluate(&device(None, false), &c, 120);
        assert!(!p.evaluate(&device(Some(15), true), &c, 130));
        assert!(p.evaluate(&device(Some(0), true), &c, 140));
    }
    #[test]
    fn recharge_rearms_with_hysteresis_and_state_survives_restart() {
        let c = Config::default();
        let mut p = Policy::default();
        assert!(p.evaluate(&device(Some(20), true), &c, 100));
        p.evaluate(&device(Some(23), true), &c, 200);
        assert!(!p.evaluate(&device(Some(20), true), &c, 300));
        let json = serde_json::to_string(&p).unwrap();
        let mut p: Policy = serde_json::from_str(&json).unwrap();
        assert!(!p.evaluate(&device(Some(20), true), &c, 400));
        p.evaluate(&device(Some(26), true), &c, 500);
        assert!(p.evaluate(&device(Some(20), true), &c, 600));
    }
}
