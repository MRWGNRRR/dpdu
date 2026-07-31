use crate::types::pdu_com_param::table::ComParamDefinition;
use dpdu_api_types::PduPc;
use serde::{Deserialize, Serialize};

/// CP_CANFDTxMaxDataLength.
///
/// Defines the maximum payload size in a single frame when transmitting data
/// using the CAN FD format.
#[repr(transparent)]
#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub struct CpCanFdTxMaxDataLength(pub u32);

impl From<CpCanFdTxMaxDataLength> for ComParamDefinition {
    fn from(value: CpCanFdTxMaxDataLength) -> Self {
        ComParamDefinition {
            class: PduPc::Com,
            short_name: "CP_CANFDTxMaxDataLength".to_string(),
            variant: value.0.into(),
        }
    }
}

impl CpCanFdTxMaxDataLength {
    pub const ZERO: Self = Self(0);
}

impl From<CpCanFdTxMaxDataLength> for u32 {
    fn from(value: CpCanFdTxMaxDataLength) -> Self {
        value.0
    }
}

impl From<u32> for CpCanFdTxMaxDataLength {
    fn from(value: u32) -> Self {
        Self(value)
    }
}
