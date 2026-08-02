use crate::types::pdu_com_param::table::ComParamDefinition;
use dpdu_api_types::PduPc;
use serde::{Deserialize, Serialize};

/// CP_RequestAddrMode.
///
/// Specifies the addressing mode that will be used when transmitting requests
/// or messages over the CAN network. ISO-TP supports different addressing
/// methods for identifying communication endpoints (for example, using normal
/// or extended addressing). CP_RequestAddrMode allows the system to select the
/// addressing method to apply for requests.
#[repr(transparent)]
#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub struct CpRequestAddrMode(pub u32);

impl From<CpRequestAddrMode> for ComParamDefinition {
    fn from(value: CpRequestAddrMode) -> Self {
        ComParamDefinition {
            class: PduPc::Com,
            short_name: "CP_RequestAddrMode".to_string(),
            variant: value.0.into(),
        }
    }
}

impl CpRequestAddrMode {
    pub const PHYSICAL: Self = CpRequestAddrMode(1);
    pub const FUNCTIONAL: Self = CpRequestAddrMode(2);
}

impl From<CpRequestAddrMode> for u32 {
    fn from(value: CpRequestAddrMode) -> Self {
        value.0
    }
}

impl From<u32> for CpRequestAddrMode {
    fn from(value: u32) -> Self {
        Self(value)
    }
}
