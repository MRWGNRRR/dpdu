use crate::types::pdu_com_param::single::com::{
    CpCanFillerByte, CpCanFillerByteHandling, CpSendRemoteFrame,
};
use crate::types::pdu_com_param::single::err_hdl::CpRepeatReqCountTrans;
use crate::types::pdu_com_param::single::unique::{
    CpCanPhysReqExtAddr, CpCanPhysReqFormat, CpCanPhysReqId, CpCanRespUudtExtAddr,
    CpCanRespUudtFormat, CpCanRespUudtId, CpEcuLayerShortName,
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

/// Raw CAN transport stack (ISO 11898-2).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawCanTransportStack {
    #[serde(rename = "CP_CanFillerByte")]
    pub can_filler_byte: CpCanFillerByte,

    #[serde(rename = "CP_CanFillerByteHandling")]
    pub can_filler_byte_handling: CpCanFillerByteHandling,

    #[serde(rename = "CP_CanPhysReqExtAddr")]
    pub can_phys_req_ext_addr: CpCanPhysReqExtAddr,

    #[serde(rename = "CP_CanPhysReqFormat")]
    pub can_phys_req_format: CpCanPhysReqFormat,

    #[serde(rename = "CP_CanPhysReqId")]
    pub can_phys_req_id: CpCanPhysReqId,

    #[serde(rename = "CP_CanRespUUDTExtAddr")]
    pub can_resp_uudt_ext_addr: CpCanRespUudtExtAddr,

    #[serde(rename = "CP_CanRespUUDTFormat")]
    pub can_resp_uudt_format: CpCanRespUudtFormat,

    #[serde(rename = "CP_CanRespUUDTId")]
    pub can_resp_uudt_id: CpCanRespUudtId,

    #[serde(rename = "CP_RepeatReqCountTrans")]
    pub repeat_req_count_trans: CpRepeatReqCountTrans,

    #[serde(rename = "CP_SendRemoteFrame")]
    pub send_remote_frame: CpSendRemoteFrame,

    #[serde(rename = "CP_EcuLayerShortName")]
    pub ecu_layer_short_name: CpEcuLayerShortName,
}

impl RawCanTransportStack {
    pub fn configure_for_receive_11_bit(&mut self, id: u16) -> &mut Self {
        self.can_resp_uudt_format = CpCanRespUudtFormat::NORMAL_UNSEGMENTED_11_BIT;
        self.can_resp_uudt_id = CpCanRespUudtId(id as _);
        self.can_resp_uudt_ext_addr = CpCanRespUudtExtAddr::ZERO;
        self
    }

    pub fn configure_for_receive_29_bit(&mut self, id: u32) -> &mut Self {
        self.can_resp_uudt_format = CpCanRespUudtFormat::NORMAL_UNSEGMENTED_29_BIT;
        self.can_resp_uudt_id = CpCanRespUudtId::NOT_USED;
        self.can_resp_uudt_ext_addr = CpCanRespUudtExtAddr(id);
        self
    }

    pub fn set_can_filler_byte(&mut self, byte: impl Into<CpCanFillerByte>) -> &mut Self {
        self.can_filler_byte = byte.into();
        self
    }

    pub fn set_can_filler_byte_handling(
        &mut self,
        status: impl Into<CpCanFillerByteHandling>,
    ) -> &mut Self {
        self.can_filler_byte_handling = status.into();
        self
    }

    pub fn set_can_phys_req_ext_addr(
        &mut self,
        value: impl Into<CpCanPhysReqExtAddr>,
    ) -> &mut Self {
        self.can_phys_req_ext_addr = value.into();
        self
    }

    pub fn set_can_phys_req_format(&mut self, format: impl Into<CpCanPhysReqFormat>) -> &mut Self {
        self.can_phys_req_format = format.into();
        self
    }

    pub fn set_can_phys_req_id(&mut self, id: impl Into<CpCanPhysReqId>) -> &mut Self {
        self.can_phys_req_id = id.into();
        self
    }

    pub fn set_can_resp_uudt_ext_addr(
        &mut self,
        value: impl Into<CpCanRespUudtExtAddr>,
    ) -> &mut Self {
        self.can_resp_uudt_ext_addr = value.into();
        self
    }

    pub fn set_can_resp_uudt_format(
        &mut self,
        format: impl Into<CpCanRespUudtFormat>,
    ) -> &mut Self {
        self.can_resp_uudt_format = format.into();
        self
    }

    pub fn set_can_resp_uudt_id(&mut self, id: impl Into<CpCanRespUudtId>) -> &mut Self {
        self.can_resp_uudt_id = id.into();
        self
    }

    pub fn set_repeat_req_count_trans(
        &mut self,
        value: impl Into<CpRepeatReqCountTrans>,
    ) -> &mut Self {
        self.repeat_req_count_trans = value.into();
        self
    }

    pub fn set_send_remote_frame(&mut self, value: impl Into<CpSendRemoteFrame>) -> &mut Self {
        self.send_remote_frame = value.into();
        self
    }

    pub fn set_ecu_layer_short_name(&mut self, value: impl Into<CpEcuLayerShortName>) -> &mut Self {
        self.ecu_layer_short_name = value.into();
        self
    }
}

impl ComParamDefinitionStack for RawCanTransportStack {
    fn configure_from_serde_json_map(&mut self, map: &HashMap<String, Value>) {
        let mut value = serde_json::to_value(&self)
            .expect("internal error: cannot serialize RawCanTransportStack"); // infallible

        let obj = value
            .as_object_mut()
            .expect("internal error: cannot represent RawCanTransportStack as map"); // infallible

        for (k, v) in map {
            if !obj.contains_key(k) {
                continue;
            }
            obj.insert(k.clone(), v.clone());
        }

        let new_self: RawCanTransportStack = serde_json::from_value(value)
            .expect("internal error: cannot deserialize RawCanTransportStack"); // infallible

        *self = new_self;
    }

    fn build_set(&self) -> ComParamDefinitionSet<ComParamDefinition> {
        ComParamDefinitionSet(hash_set! {
            // Com.
            self.can_filler_byte.into(),
            self.can_filler_byte_handling.into(),
            self.send_remote_frame.into(),

            // Error handling.
            self.repeat_req_count_trans.into()
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
            }
        })
    }
}
