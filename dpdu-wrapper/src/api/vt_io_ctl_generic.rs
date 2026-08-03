use crate::api::{ApiResult, PduApi};
use crate::types::PduModuleHandle;
use crate::types::pdu_io_ctl::{IoCtlByteArray, PduIoCtlCommand, PduIoCtlData, PduIoCtlTarget};

impl PduApi {
    pub fn vt_io_ctl_generic(&self, h_mod: PduModuleHandle, data: &[u8]) -> ApiResult<()> {
        let _ = self.pdu_io_ctl(
            &PduIoCtlTarget::Module(h_mod),
            &PduIoCtlCommand::from("PDU_IOCTL_GENERIC"),
            Some(&PduIoCtlData::ByteArray(IoCtlByteArray(data.to_vec()))),
        )?;
        Ok(())
    }
}
