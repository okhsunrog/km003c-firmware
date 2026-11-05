//! Exact fixed-SPR requests. Never turn insufficient current into a mismatch RDO.
use usbpd::protocol_layer::message::data::{
    request::{FixedVariableSupply, PowerSource},
    source_capabilities::PowerDataObject,
};

pub fn fixed_request(
    pdos: &[PowerDataObject],
    voltage_mv: u32,
    current_ma: u16,
) -> Option<PowerSource> {
    if voltage_mv == 0 || current_ma == 0 || !current_ma.is_multiple_of(10) {
        return None;
    }
    pdos.iter().take(7).enumerate().find_map(|(index, pdo)| {
        let PowerDataObject::FixedSupply(fixed) = pdo else {
            return None;
        };
        if u32::from(fixed.raw_voltage()) * 50 != voltage_mv
            || u32::from(fixed.raw_max_current()) * 10 < u32::from(current_ma)
        {
            return None;
        }
        Some(PowerSource::FixedVariableSupply(
            FixedVariableSupply(0)
                .with_object_position((index + 1) as u8)
                .with_raw_operating_current(current_ma / 10)
                .with_raw_max_operating_current(current_ma / 10)
                .with_no_usb_suspend(true)
                .with_usb_communications_capable(false),
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use usbpd::protocol_layer::message::data::source_capabilities::FixedSupply;
    fn fixed(mv: u16, ma: u16) -> PowerDataObject {
        PowerDataObject::FixedSupply(
            FixedSupply(0)
                .with_raw_voltage(mv / 50)
                .with_raw_max_current(ma / 10),
        )
    }
    #[test]
    fn exact_current_and_original_position_are_preserved() {
        let pdos = [fixed(5000, 3000), fixed(9000, 3000), fixed(15000, 3000)];
        let Some(PowerSource::FixedVariableSupply(r)) = fixed_request(&pdos, 15000, 2500) else {
            panic!()
        };
        assert_eq!(r.object_position(), 3);
        assert_eq!(r.raw_operating_current(), 250);
        assert_eq!(r.raw_max_operating_current(), 250);
        assert!(!r.capability_mismatch());
    }
    #[test]
    fn insufficient_or_unadvertised_profiles_never_produce_a_request() {
        let pdos = [fixed(5000, 3000), fixed(15000, 1500)];
        assert!(fixed_request(&pdos, 15000, 3000).is_none());
        assert!(fixed_request(&pdos, 20000, 100).is_none());
        assert!(fixed_request(&pdos, 15000, 101).is_none());
        assert!(fixed_request(&pdos, 15000, 0).is_none());
    }
}
