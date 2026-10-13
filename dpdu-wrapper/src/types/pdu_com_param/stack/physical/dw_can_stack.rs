use crate::com_param_definition_set;
use crate::types::pdu_com_param::single::bus_type::{
    CpBaudrate, CpBitSamplePoint, CpCanBaudrateRecord, CpCanFdBaudrate, CpCanFdBitSamplePoint,
    CpCanFdSyncJumpWidth, CpListenOnly, CpSamplesPerBit, CpSyncJumpWidth, CpTerminationType,
};
use crate::types::pdu_com_param::stack::ComParamDefinitionStack;
use crate::types::pdu_com_param::table::{
    ComParamDefinition, ComParamDefinitionSet, ComParamDefinitionTable,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// DW can stack (ISO 11898-2).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DwCanStack {
    #[serde(rename = "CP_Baudrate")]
    pub baudrate: CpBaudrate,

    #[serde(rename = "CP_BitSamplePoint")]
    pub bit_sample_point: CpBitSamplePoint,

    #[serde(rename = "CP_CanBaudrateRecord")]
    pub can_baudrate_record: CpCanBaudrateRecord,

    #[serde(rename = "CP_CANFDBaudrate")]
    pub canfd_baudrate: CpCanFdBaudrate,

    #[serde(rename = "CP_CANFDBitSamplePoint")]
    pub canfd_bit_sample_point: CpCanFdBitSamplePoint,

    #[serde(rename = "CP_CANFDSyncJumpWidth")]
    pub canfd_sync_jump_width: CpCanFdSyncJumpWidth,

    #[serde(rename = "CP_ListenOnly")]
    pub listen_only: CpListenOnly,

    #[serde(rename = "CP_SamplesPerBit")]
    pub samples_per_bit: CpSamplesPerBit,

    #[serde(rename = "CP_SyncJumpWidth")]
    pub sync_jump_width: CpSyncJumpWidth,

    #[serde(rename = "CP_TerminationType")]
    pub termination_type: CpTerminationType,
}

impl DwCanStack {
    pub fn set_baudrate(&mut self, rate: impl Into<CpBaudrate>) -> &mut Self {
        self.baudrate = rate.into();
        self
    }

    pub fn set_canfd_baudrate(&mut self, rate: impl Into<CpCanFdBaudrate>) -> &mut Self {
        self.canfd_baudrate = rate.into();
        self
    }

    pub fn set_canfd_bit_sample_point(
        &mut self,
        point: impl Into<CpCanFdBitSamplePoint>,
    ) -> &mut Self {
        self.canfd_bit_sample_point = point.into();
        self
    }

    pub fn set_canfd_sync_jump_width(
        &mut self,
        width: impl Into<CpCanFdSyncJumpWidth>,
    ) -> &mut Self {
        self.canfd_sync_jump_width = width.into();
        self
    }

    pub fn set_bit_sample_point(&mut self, point: impl Into<CpBitSamplePoint>) -> &mut Self {
        self.bit_sample_point = point.into();
        self
    }

    pub fn set_can_baudrate_record(&mut self, record: impl Into<CpCanBaudrateRecord>) -> &mut Self {
        self.can_baudrate_record = record.into();
        self
    }

    pub fn set_listen_only(&mut self, status: impl Into<CpListenOnly>) -> &mut Self {
        self.listen_only = status.into();
        self
    }

    pub fn set_samples_per_bit(&mut self, samples: impl Into<CpSamplesPerBit>) -> &mut Self {
        self.samples_per_bit = samples.into();
        self
    }

    pub fn set_sync_jump_width(&mut self, width: impl Into<CpSyncJumpWidth>) -> &mut Self {
        self.sync_jump_width = width.into();
        self
    }

    pub fn set_termination_type(&mut self, typ: impl Into<CpTerminationType>) -> &mut Self {
        self.termination_type = typ.into();
        self
    }
}

impl ComParamDefinitionStack for DwCanStack {
    fn configure_from_serde_json_map(&mut self, map: &HashMap<String, Value>) {
        let mut value =
            serde_json::to_value(&self).expect("internal error: cannot serialize DwCanStack"); // infallible

        let obj = value
            .as_object_mut()
            .expect("internal error: cannot represent DwCanStack as map"); // infallible

        for (k, v) in map {
            if !obj.contains_key(k) {
                continue;
            }
            obj.insert(k.clone(), v.clone());
        }

        let new_self: DwCanStack =
            serde_json::from_value(value).expect("internal error: cannot deserialize DwCanStack"); // infallible

        *self = new_self;
    }

    fn build_set(&self) -> ComParamDefinitionSet<ComParamDefinition> {
        com_param_definition_set! {
            // Bus type.
            self.baudrate,
            self.bit_sample_point,
            self.canfd_baudrate,
            self.canfd_bit_sample_point,
            self.canfd_sync_jump_width,
            self.can_baudrate_record.clone(),
            self.listen_only,
            self.samples_per_bit,
            self.sync_jump_width,
            self.termination_type
        }
    }

    fn build_table(&self) -> ComParamDefinitionTable<ComParamDefinition> {
        ComParamDefinitionTable::new()
    }
}
