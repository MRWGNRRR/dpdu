use crate::api::{ApiResult, PduApi};
use crate::types::pdu_io_ctl::{PduIoCtlCommand, PduIoCtlData, PduIoCtlTarget};
use crate::types::{PduModuleHandle, PduObjectId};
use dpdu_api_types::{PDU_ID_UNDEF, PduError};
use tracing::error;

impl PduApi {
    pub fn vt_io_ctl_get_cable_id(&self, h_mod: PduModuleHandle) -> ApiResult<Option<PduObjectId>> {
        match self.pdu_io_ctl(
            &PduIoCtlTarget::Module(h_mod),
            &PduIoCtlCommand::from("PDU_IOCTL_GET_CABLE_ID"),
            None,
        )? {
            Some(PduIoCtlData::U32(v)) if v == PDU_ID_UNDEF => Ok(None),
            Some(PduIoCtlData::U32(v)) => Ok(Some(v)),
            Some(_) => {
                error!(
                    h_mod,
                    "IoCtl output data is wrong. Emulation of PduError::FctFailed..."
                );
                Err(PduError::FctFailed)?
            }
            None => {
                error!(
                    h_mod,
                    "IoCtl output data is null. Emulation of PduError::FctFailed..."
                );
                Err(PduError::FctFailed)?
            }
        }
    }
}
