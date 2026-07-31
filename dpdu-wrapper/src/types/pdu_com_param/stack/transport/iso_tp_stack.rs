use crate::types::pdu_com_param::single::com::{
    CpBlockSize, CpBlockSizeOverride, CpCanDataSizeOffset, CpCanFdTxMaxDataLength, CpCanFillerByte,
    CpCanFillerByteHandling, CpCanFirstConsecutiveFrameValue, CpCanFuncReqExtAddr,
    CpCanFuncReqFormat, CpCanFuncReqId, CpCanMaxNumWaitFrames, CpMaxFirstFrameDataLength,
    CpRequestAddrMode, CpSendRemoteFrame,
};
use crate::types::pdu_com_param::single::err_hdl::CpRepeatReqCountTrans;
use crate::types::pdu_com_param::single::timing::{
    CpAr, CpAs, CpBr, CpBs, CpCr, CpCs, CpStMin, CpStMinOverride,
};
use crate::types::pdu_com_param::single::unique::{
    CpCanPhysReqExtAddr, CpCanPhysReqFormat, CpCanPhysReqId, CpCanRespUsdtExtAddr,
    CpCanRespUsdtFormat, CpCanRespUsdtId, CpCanRespUudtExtAddr, CpCanRespUudtFormat,
    CpCanRespUudtId, CpEcuLayerShortName,
};
use crate::types::pdu_com_param::stack::ComParamDefinitionStack;
use crate::types::pdu_com_param::table::{
    ComParamDefinition, ComParamDefinitionSet, ComParamDefinitionTable,
};
use crate::utils::ecu_name_to_unique_resp_id;
use map_macro::{hash_map_e, hash_set};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// ISO-TP transport stack (ISO 15765-2).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IsoTpStack {
    #[serde(rename = "CP_Ar")]
    pub ar: CpAr,

    #[serde(rename = "CP_As")]
    pub r#as: CpAs,

    #[serde(rename = "CP_BlockSize")]
    pub block_size: CpBlockSize,

    #[serde(rename = "CP_BlockSizeOverride")]
    pub block_size_override: CpBlockSizeOverride,

    #[serde(rename = "CpBr")]
    pub br: CpBr,

    #[serde(rename = "CpBs")]
    pub bs: CpBs,

    #[serde(rename = "CP_CanDataSizeOffset")]
    pub can_data_size_offset: CpCanDataSizeOffset,

    #[serde(rename = "CP_CanFillerByte")]
    pub can_filler_byte: CpCanFillerByte,

    #[serde(rename = "CP_CanFillerByteHandling")]
    pub can_filler_byte_handling: CpCanFillerByteHandling,

    #[serde(rename = "CP_CanFirstConsecutiveFrameValue")]
    pub can_first_consecutive_frame_value: CpCanFirstConsecutiveFrameValue,

    #[serde(rename = "CP_CANFDTxMaxDataLength")]
    pub canfd_tx_max_data_length: CpCanFdTxMaxDataLength,

    #[serde(rename = "CP_CanFuncReqExtAddr")]
    pub can_func_req_ext_addr: CpCanFuncReqExtAddr,

    #[serde(rename = "CP_CanFuncReqFormat")]
    pub can_func_req_format: CpCanFuncReqFormat,

    #[serde(rename = "CP_CanFuncReqId")]
    pub can_func_req_id: CpCanFuncReqId,

    #[serde(rename = "CP_CanMaxNumWaitFrames")]
    pub can_max_num_wait_frames: CpCanMaxNumWaitFrames,

    #[serde(rename = "CP_CanPhysReqExtAddr")]
    pub can_phys_req_ext_addr: CpCanPhysReqExtAddr,

    #[serde(rename = "CP_CanPhysReqFormat")]
    pub can_phys_req_format: CpCanPhysReqFormat,

    #[serde(rename = "CP_CanPhysReqId")]
    pub can_phys_req_id: CpCanPhysReqId,

    #[serde(rename = "CP_CanRespUSDTExtAddr")]
    pub can_resp_usdt_ext_addr: CpCanRespUsdtExtAddr,

    #[serde(rename = "CP_CanRespUSDTFormat")]
    pub can_resp_usdt_format: CpCanRespUsdtFormat,

    #[serde(rename = "CP_CanRespUSDTId")]
    pub can_resp_usdt_id: CpCanRespUsdtId,

    #[serde(rename = "CP_CanRespUUDTExtAddr")]
    pub can_resp_uudt_ext_addr: CpCanRespUudtExtAddr,

    #[serde(rename = "CP_CanRespUUDTFormat")]
    pub can_resp_uudt_format: CpCanRespUudtFormat,

    #[serde(rename = "CP_CanRespUUDTId")]
    pub can_resp_uudt_id: CpCanRespUudtId,

    #[serde(rename = "CP_Cr")]
    pub cr: CpCr,

    #[serde(rename = "CP_Cs")]
    pub cs: CpCs,

    #[serde(rename = "CP_EcuLayerShortName")]
    pub ecu_layer_short_name: CpEcuLayerShortName,

    #[serde(rename = "CP_MaxFirstFrameDataLength")]
    pub max_first_frame_data_length: CpMaxFirstFrameDataLength,

    #[serde(rename = "CP_RepeatReqCountTrans")]
    pub repeat_req_count_trans: CpRepeatReqCountTrans,

    #[serde(rename = "CP_RequestAddrMode")]
    pub request_addr_mode: CpRequestAddrMode,

    #[serde(rename = "CP_SendRemoteFrame")]
    pub send_remote_frame: CpSendRemoteFrame,

    #[serde(rename = "CP_StMin")]
    pub st_min: CpStMin,

    #[serde(rename = "CP_StMinOverride")]
    pub st_min_override: CpStMinOverride,
}

impl IsoTpStack {
    pub fn set_ar(&mut self, value: impl Into<CpAr>) -> &mut Self {
        self.ar = value.into();
        self
    }

    pub fn set_as(&mut self, value: impl Into<CpAs>) -> &mut Self {
        self.r#as = value.into();
        self
    }

    pub fn set_block_size(&mut self, value: impl Into<CpBlockSize>) -> &mut Self {
        self.block_size = value.into();
        self
    }

    pub fn set_block_size_override(&mut self, value: impl Into<CpBlockSizeOverride>) -> &mut Self {
        self.block_size_override = value.into();
        self
    }

    pub fn set_br(&mut self, value: impl Into<CpBr>) -> &mut Self {
        self.br = value.into();
        self
    }

    pub fn set_bs(&mut self, value: impl Into<CpBs>) -> &mut Self {
        self.bs = value.into();
        self
    }

    pub fn set_can_data_size_offset(&mut self, value: impl Into<CpCanDataSizeOffset>) -> &mut Self {
        self.can_data_size_offset = value.into();
        self
    }

    pub fn set_can_filler_byte(&mut self, value: impl Into<CpCanFillerByte>) -> &mut Self {
        self.can_filler_byte = value.into();
        self
    }

    pub fn set_can_filler_byte_handling(
        &mut self,
        value: impl Into<CpCanFillerByteHandling>,
    ) -> &mut Self {
        self.can_filler_byte_handling = value.into();
        self
    }

    pub fn set_can_first_consecutive_frame_value(
        &mut self,
        value: impl Into<CpCanFirstConsecutiveFrameValue>,
    ) -> &mut Self {
        self.can_first_consecutive_frame_value = value.into();
        self
    }

    pub fn set_can_func_req_ext_addr(
        &mut self,
        value: impl Into<CpCanFuncReqExtAddr>,
    ) -> &mut Self {
        self.can_func_req_ext_addr = value.into();
        self
    }

    pub fn set_can_func_req_format(&mut self, value: impl Into<CpCanFuncReqFormat>) -> &mut Self {
        self.can_func_req_format = value.into();
        self
    }

    pub fn set_can_func_req_id(&mut self, value: impl Into<CpCanFuncReqId>) -> &mut Self {
        self.can_func_req_id = value.into();
        self
    }

    pub fn set_can_max_num_wait_frames(
        &mut self,
        value: impl Into<CpCanMaxNumWaitFrames>,
    ) -> &mut Self {
        self.can_max_num_wait_frames = value.into();
        self
    }

    pub fn set_can_phys_req_ext_addr(
        &mut self,
        value: impl Into<CpCanPhysReqExtAddr>,
    ) -> &mut Self {
        self.can_phys_req_ext_addr = value.into();
        self
    }

    pub fn set_can_phys_req_format(&mut self, value: impl Into<CpCanPhysReqFormat>) -> &mut Self {
        self.can_phys_req_format = value.into();
        self
    }

    pub fn set_can_phys_req_id(&mut self, value: impl Into<CpCanPhysReqId>) -> &mut Self {
        self.can_phys_req_id = value.into();
        self
    }

    pub fn set_can_resp_usdt_ext_addr(
        &mut self,
        value: impl Into<CpCanRespUsdtExtAddr>,
    ) -> &mut Self {
        self.can_resp_usdt_ext_addr = value.into();
        self
    }

    pub fn set_can_resp_usdt_format(&mut self, value: impl Into<CpCanRespUsdtFormat>) -> &mut Self {
        self.can_resp_usdt_format = value.into();
        self
    }

    pub fn set_can_resp_usdt_id(&mut self, value: impl Into<CpCanRespUsdtId>) -> &mut Self {
        self.can_resp_usdt_id = value.into();
        self
    }

    pub fn set_can_resp_uudt_ext_addr(
        &mut self,
        value: impl Into<CpCanRespUudtExtAddr>,
    ) -> &mut Self {
        self.can_resp_uudt_ext_addr = value.into();
        self
    }

    pub fn set_can_resp_uudt_format(&mut self, value: impl Into<CpCanRespUudtFormat>) -> &mut Self {
        self.can_resp_uudt_format = value.into();
        self
    }

    pub fn set_can_resp_uudt_id(&mut self, value: impl Into<CpCanRespUudtId>) -> &mut Self {
        self.can_resp_uudt_id = value.into();
        self
    }

    pub fn set_canfd_tx_max_data_length(
        &mut self,
        value: impl Into<CpCanFdTxMaxDataLength>,
    ) -> &mut Self {
        self.canfd_tx_max_data_length = value.into();
        self
    }

    pub fn set_cr(&mut self, value: impl Into<CpCr>) -> &mut Self {
        self.cr = value.into();
        self
    }

    pub fn set_cs(&mut self, value: impl Into<CpCs>) -> &mut Self {
        self.cs = value.into();
        self
    }

    pub fn set_ecu_layer_short_name(&mut self, value: impl Into<CpEcuLayerShortName>) -> &mut Self {
        self.ecu_layer_short_name = value.into();
        self
    }

    pub fn set_max_first_frame_data_length(
        &mut self,
        value: impl Into<CpMaxFirstFrameDataLength>,
    ) -> &mut Self {
        self.max_first_frame_data_length = value.into();
        self
    }

    pub fn set_repeat_req_count_trans(
        &mut self,
        value: impl Into<CpRepeatReqCountTrans>,
    ) -> &mut Self {
        self.repeat_req_count_trans = value.into();
        self
    }

    pub fn set_request_addr_mode(&mut self, value: impl Into<CpRequestAddrMode>) -> &mut Self {
        self.request_addr_mode = value.into();
        self
    }

    pub fn set_send_remote_frame(&mut self, value: impl Into<CpSendRemoteFrame>) -> &mut Self {
        self.send_remote_frame = value.into();
        self
    }

    pub fn set_st_min(&mut self, value: impl Into<CpStMin>) -> &mut Self {
        self.st_min = value.into();
        self
    }

    pub fn set_st_min_override(&mut self, value: impl Into<CpStMinOverride>) -> &mut Self {
        self.st_min_override = value.into();
        self
    }
}

impl ComParamDefinitionStack for IsoTpStack {
    fn configure_from_serde_json_map(&mut self, map: &HashMap<String, Value>) {
        let mut value =
            serde_json::to_value(&self).expect("internal error: cannot serialize IsoTpStack"); // infallible

        let obj = value
            .as_object_mut()
            .expect("internal error: cannot represent IsoTpStack as map"); // infallible

        for (k, v) in map {
            if !obj.contains_key(k) {
                continue;
            }
            obj.insert(k.clone(), v.clone());
        }

        let new_self: IsoTpStack =
            serde_json::from_value(value).expect("internal error: cannot deserialize IsoTpStack"); // infallible

        *self = new_self;
    }

    fn build_set(&self) -> ComParamDefinitionSet<ComParamDefinition> {
        ComParamDefinitionSet(hash_set! {
            // Timing.
            self.ar.into(),
            self.r#as.into(),
            self.br.into(),
            self.bs.into(),
            self.cr.into(),
            self.cs.into(),
            self.st_min.into(),
            self.st_min_override.into(),

            // Com.
            self.block_size.into(),
            self.block_size_override.into(),
            self.can_data_size_offset.into(),
            self.can_filler_byte.into(),
            self.can_filler_byte_handling.into(),
            self.can_first_consecutive_frame_value.into(),
            self.can_func_req_ext_addr.into(),
            self.can_func_req_format.into(),
            self.can_func_req_id.into(),
            self.can_max_num_wait_frames.into(),

            self.canfd_tx_max_data_length.into(),
            self.max_first_frame_data_length.into(),
            self.request_addr_mode.into(),
            self.send_remote_frame.into(),

            // Error handling.
            self.repeat_req_count_trans.into(),
        })
    }

    fn build_table(&self) -> ComParamDefinitionTable<ComParamDefinition> {
        let id = if self.ecu_layer_short_name.0.is_empty() {
            0
        } else {
            ecu_name_to_unique_resp_id(&self.ecu_layer_short_name.0)
        };

        ComParamDefinitionTable(hash_map_e! {
            id => hash_set! {
                self.can_phys_req_ext_addr.into(),
                self.can_phys_req_format.into(),
                self.can_phys_req_id.into(),
                self.can_resp_uudt_ext_addr.into(),
                self.can_resp_uudt_format.into(),
                self.can_resp_uudt_id.into(),
                self.can_resp_usdt_ext_addr.into(),
                self.can_resp_usdt_format.into(),
                self.can_resp_usdt_id.into(),
            }
        })
    }
}
