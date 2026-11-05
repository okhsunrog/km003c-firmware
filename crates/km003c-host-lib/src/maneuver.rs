//! A validated timeline of explicit PD commands; capture supplies final Stop.
use anyhow::{Result, ensure};
use km003c_interface::PdControl;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Maneuver {
    pub name: String,
    pub duration_ms: u64,
    #[serde(default = "default_interval")]
    pub measurement_interval_ms: u16,
    pub steps: Vec<Step>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Step {
    pub at_ms: u64,
    pub command: PdControl,
}

impl Maneuver {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            km003c_interface::valid_stream_interval(self.measurement_interval_ms),
            "measurement interval must be 20..5000 ms"
        );
        ensure!(!self.name.trim().is_empty(), "maneuver needs a name");
        ensure!(
            (1..=600_000).contains(&self.duration_ms),
            "duration must be 1..600000 ms"
        );
        let mut previous = None;
        let mut sink_until = None;
        for step in &self.steps {
            ensure!(
                step.at_ms < self.duration_ms,
                "step is outside capture duration"
            );
            ensure!(
                previous.is_none_or(|p| step.at_ms > p),
                "step times must increase strictly"
            );
            crate::rpc::validate_control(step.command)?;
            match step.command {
                PdControl::Sink { seconds, .. } => {
                    sink_until = Some(step.at_ms + u64::from(seconds) * 1000)
                }
                PdControl::SetProfile { .. } => ensure!(
                    sink_until.is_some_and(|end| step.at_ms < end),
                    "profile change needs an active sink step within its original lease"
                ),
                PdControl::Stop | PdControl::Monitor { .. } => sink_until = None,
            }
            previous = Some(step.at_ms);
        }
        Ok(())
    }
}

fn default_interval() -> u16 {
    km003c_interface::DEFAULT_STREAM_INTERVAL_MS
}

pub fn load(path: &std::path::Path) -> Result<Maneuver> {
    let plan: Maneuver = serde_json::from_slice(&std::fs::read(path)?)?;
    plan.validate()?;
    Ok(plan)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_plan_defaults_and_profile_lifetime() {
        let mut m: Maneuver = serde_json::from_str(
            r#"{
            "name":"profiles", "duration_ms":4000,
            "steps":[
                {"at_ms":0,"command":{"Sink":{"volts":15,"seconds":2}}},
                {"at_ms":1000,"command":{"SetProfile":{"volts":9,"current_ma":100}}}
            ]
        }"#,
        )
        .unwrap();
        assert_eq!(m.measurement_interval_ms, 250);
        assert!(matches!(
            m.steps[0].command,
            PdControl::Sink {
                current_ma: 100,
                ..
            }
        ));
        assert!(m.validate().is_ok());
        // A profile change never renews the original sink lease.
        m.steps[1].at_ms = 2000;
        assert!(m.validate().is_err());
        m.steps[1].at_ms = 1000;
        m.steps.insert(
            1,
            Step {
                at_ms: 500,
                command: PdControl::Stop,
            },
        );
        assert!(m.validate().is_err());
        m.steps[1].command = PdControl::Monitor { input: None };
        assert!(m.validate().is_err());
    }

    #[test]
    fn reject_invalid_timeline_before_touching_hardware() {
        let mut m = Maneuver {
            name: "test".into(),
            duration_ms: 1000,
            measurement_interval_ms: 250,
            steps: vec![Step {
                at_ms: 0,
                command: PdControl::Sink {
                    volts: 21,
                    seconds: 1,
                    current_ma: 100,
                },
            }],
        };
        assert!(m.validate().is_err());
        m.steps[0].command = PdControl::Stop;
        assert!(m.validate().is_ok());
        m.steps.push(Step {
            at_ms: 0,
            command: PdControl::Stop,
        });
        assert!(m.validate().is_err());
    }
}
