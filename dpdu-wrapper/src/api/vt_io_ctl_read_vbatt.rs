use crate::api::{ApiError, ApiResult, PduApi};
use crate::types::PduModuleHandle;
use crate::types::pdu_io_ctl::{PduIoCtlCommand, PduIoCtlData, PduIoCtlTarget};
use dpdu_api_types::{PduError, PduObjt};
use tracing::error;

impl PduApi {
    /// Reads the vehicle battery voltage using the `PDU_IOCTL_READ_VBATT` IO control command.
    ///
    /// The returned voltage is converted from millivolts to volts.
    ///
    /// # Returns
    ///
    /// - `Ok(Some(voltage))` - Battery voltage in volts.
    /// - `Ok(None)` - The module does not support the `PDU_IOCTL_READ_VBATT` command.
    /// - `Err(...)` - An error occurred while executing the IOCTL request.
    pub fn vt_io_ctl_read_vbatt(&self, h_mod: PduModuleHandle) -> ApiResult<Option<f32>> {
        let ioctl_target = PduIoCtlTarget::Module(h_mod);
        let ioctl_id = match self.pdu_get_object_id(PduObjt::IoCtrl, "PDU_IOCTL_READ_VBATT")? {
            Some(v) => PduIoCtlCommand::Id(v),
            None => return Ok(None),
        };

        match self.pdu_io_ctl(&ioctl_target, &ioctl_id, None) {
            Ok(Some(PduIoCtlData::U32(v))) => Ok(Some(v as f32 / 1000.0)),
            Ok(Some(_)) => {
                error!(
                    h_mod,
                    "IoCtl output data is wrong. Emulation of PduError::FctFailed..."
                );
                Err(PduError::FctFailed)?
            }
            Ok(None) => {
                error!(
                    h_mod,
                    "IoCtl output data is null. Emulation of PduError::FctFailed..."
                );
                Err(PduError::FctFailed)?
            }
            Err(ApiError::PduError(PduError::IdNotSupported)) => return Ok(None),
            Err(err) => Err(err),
        }
    }
}
