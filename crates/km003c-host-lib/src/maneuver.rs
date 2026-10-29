//! A validated timeline of explicit PD commands; capture supplies final Stop.
use anyhow::{Result, ensure};
use km003c_interface::PdControl;
use serde::{Deserialize, Serialize};
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Maneuver {
    pub name: String,
    pub duration_ms: u64,
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
        ensure!(!self.name.trim().is_empty(), "maneuver needs a name");
        ensure!(
            (1..=600_000).contains(&self.duration_ms),
            "duration must be 1..600000 ms"
        );
        let mut previous = None;
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
            previous = Some(step.at_ms);
        }
        Ok(())
    }
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
    fn reject_invalid_timeline_before_touching_hardware() {
        let mut m = Maneuver {
            name: "test".into(),
            duration_ms: 1000,
            steps: vec![Step {
                at_ms: 0,
                command: PdControl::Sink {
                    volts: 21,
                    seconds: 1,
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
