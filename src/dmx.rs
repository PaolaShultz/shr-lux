//! Validated DMX addressing and uDMX range encoding. No hardware writes.
pub const CHANNELS: usize = 512;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Address(u16);

impl Address {
    pub fn new(value: u16) -> Option<Self> {
        (1..=CHANNELS as u16)
            .contains(&value)
            .then_some(Self(value))
    }

    pub fn index(self) -> u16 {
        self.0 - 1
    }
}

/// USB vendor OUT request 2. Values are DMX slots, without a start code.
#[derive(Debug, PartialEq, Eq)]
pub struct RangeRequest<'a> {
    pub request_type: u8,
    pub request: u8,
    pub value: u16,
    pub index: u16,
    pub data: &'a [u8],
}

pub fn encode_range(start: Address, values: &[u8]) -> Option<RangeRequest<'_>> {
    if values.is_empty() || values.len() > CHANNELS - usize::from(start.index()) {
        return None;
    }
    Some(RangeRequest {
        request_type: 0x40,
        request: 2,
        value: values.len() as u16,
        index: start.index(),
        data: values,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn address_boundaries() {
        assert!(Address::new(0).is_none());
        assert!(Address::new(513).is_none());
        assert_eq!(Address::new(1).unwrap().index(), 0);
        assert_eq!(Address::new(512).unwrap().index(), 511);
    }

    #[test]
    fn valid_wire_range_has_no_start_code() {
        let data = [17, 255];
        let request = encode_range(Address::new(511).unwrap(), &data).unwrap();
        assert_eq!(
            request,
            RangeRequest {
                request_type: 0x40,
                request: 2,
                value: 2,
                index: 510,
                data: &data,
            }
        );
        assert_eq!(
            encode_range(Address::new(1).unwrap(), &[0; 512])
                .unwrap()
                .value,
            512
        );
    }

    #[test]
    fn rejects_empty_and_overflowing_ranges() {
        assert!(encode_range(Address::new(1).unwrap(), &[]).is_none());
        assert!(encode_range(Address::new(512).unwrap(), &[0, 1]).is_none());
        assert!(encode_range(Address::new(1).unwrap(), &[0; 513]).is_none());
    }
}
