use crate::types::pdu_com_logical_link::{CllCreateFlags, CllCreateType, PduCllData};
use crate::types::pdu_com_param::PduComParam;
use crate::types::pdu_com_param::table::PduComParamTable;
use crate::types::pdu_com_primitive::{PduPrimitiveParams};
use crate::types::pdu_error::{PduErrorData, PduLastErrorTarget};
use crate::types::pdu_event::{PduEvent, PduEventTarget};
use crate::types::pdu_io_ctl::{PduIoCtlCommand, PduIoCtlData, PduIoCtlTarget};
use crate::types::pdu_lock_resource::PduLockResourceMask;
use crate::types::pdu_module::{
    PduConflictingModules, PduModuleData, PduModuleList, PduModulesResourcesIds,
};
use crate::types::pdu_object::PduObjectIdSource;
use crate::types::pdu_resource::{
    BusSource, PduResource, PduResourceStatus, ProtocolSource, TargetPin,
};
use crate::types::pdu_status::{PduStatusData, PduStatusTarget};
use crate::types::pdu_version::PduVersionData;
use crate::types::{
    PduCllHandle, PduCopHandle, PduModuleHandle, PduObjectId, PduUniqueCllTag, PduUniqueCopTag,
};
use crate::utils::UnsafePtr;
use dpdu_api_types::{EventCallbackFn, IoFilterData, PduCopt, PduObjt, PduQueueMode};
use dpdu_wrapper_support::declare_worker_rpc;
use std::ffi::c_void;

declare_worker_rpc! {
    // Virtual functions.
    VtIoCtlReset
        => vt_io_ctl_reset() -> (),

    VtIoCtlClearTxQueue
        => vt_io_ctl_clear_tx_queue(h_mod: PduModuleHandle, h_cll: PduCllHandle) -> (),

    VtIoCtlSuspendTxQueue
        => vt_io_ctl_suspend_tx_queue(h_mod: PduModuleHandle, h_cll: PduCllHandle) -> (),

    VtIoCtlResumeTxQueue
        => vt_io_ctl_resume_tx_queue(h_mod: PduModuleHandle, h_cll: PduCllHandle) -> (),

    VtIoCtlClearRxQueue
        => vt_io_ctl_clear_rx_queue(h_mod: PduModuleHandle, h_cll: PduCllHandle) -> (),

    VtIoCtlReadVbatt
        => vt_io_ctl_read_vbatt(h_mod: PduModuleHandle) -> f32,

    VtIoCtlSetProgVoltage
        => vt_io_ctl_set_prog_voltage(h_mod: PduModuleHandle, voltage: f32, pin: u32) -> (),

    VtIoCtlReagProgVoltage
        => vt_io_ctl_read_prog_voltage(h_mod: PduModuleHandle) -> f32,

    VtIoCtlGeneric
        => vt_io_ctl_generic(h_mod: PduModuleHandle, data: Vec<u8>) -> (),

    VtIoCtlSetBufferSize
        => vt_io_ctl_set_buffer_size(h_mod: PduModuleHandle, h_cll: PduCllHandle, size: u32) -> (),

    VtIoCtlStartMsgFilter
        => vt_io_ctl_start_msg_filter(
            h_mod: PduModuleHandle,
            h_cll: PduCllHandle,
            data: IoFilterData
        ) -> (),

    VtIoCtlStopMsgFilter
        => vt_io_ctl_stop_msg_filter(
            h_mod: PduModuleHandle,
            h_cll: PduCllHandle,
            number: u32
        ) -> (),

    VtIoCtlClearMsgFilter
        => vt_io_ctl_clear_msg_filter(h_mod: PduModuleHandle, h_cll: PduCllHandle) -> (),

    VtIoCtlSetEventQueueProperties
        => vt_io_ctl_set_event_queue_properties(
            h_mod: PduModuleHandle,
            h_cll: PduCllHandle,
            size: u32,
            mode: PduQueueMode
        ) -> (),

    VtIoCtlGetCableId
        => vt_io_ctl_get_cable_id(h_mod: PduModuleHandle) -> Option<u32>,

    VtIoCtlSendBreak
        => vt_io_ctl_send_break(h_mod: PduModuleHandle, h_cll: PduCllHandle) -> (),

    VtIoCtlReadIgnitionSenseState
        => vt_io_ctl_read_ignition_sense_state(h_mod: PduModuleHandle, pin: Option<u32>) -> bool,

    VtModuleDestructor
        => !_virtual(h_mod: PduModuleHandle) -> (),

    VtCllDestructor
        => !_virtual(h_mod: PduModuleHandle, h_cll: PduCllHandle) -> (),

    VtCopDestructor
        => !_virtual(h_mod: PduModuleHandle, h_cll: PduCllHandle, h_cop: PduCopHandle) -> (),

    // Real D-PDU functions.
    PduCancelComPrimitive
        => pdu_cancel_com_primitive(
            h_mod: PduModuleHandle,
            h_cll: PduCllHandle,
            h_cop: PduCopHandle
        ) -> (),

    PduConnect
        => pdu_connect(h_mod: PduModuleHandle, h_cll: PduCllHandle) -> (),

    PduConstruct
        => pdu_construct() -> (),

    PduCreateComLogicalLink
        => pdu_create_com_logical_link(
            h_mod: PduModuleHandle,
            create_type: CllCreateType,
            create_flags: CllCreateFlags,
            tag: Option<PduUniqueCllTag>
        ) -> PduCllData,

    PduDestruct
        => pdu_destruct() -> (),

    PduDestroyComLogicalLink
        => pdu_destroy_com_logical_link(h_mod: PduModuleHandle, h_cll: PduCllHandle) -> (),

    PduDestroyItem
        => pdu_destroy_item(ptr: UnsafePtr<c_void>) -> (),

    PduDisconnect
        => pdu_disconnect(h_mod: PduModuleHandle, h_cll: PduCllHandle) -> (),

    PduGetComParam
        => pdu_get_com_param(
            h_mod: PduModuleHandle,
            h_cll: PduCllHandle,
            object_id: PduObjectIdSource
        ) -> PduComParam,

    PduGetConflictingResources
        => pdu_get_conflicting_resources(
            resource_id: PduObjectId,
            modules: Vec<PduModuleData>
        ) -> PduConflictingModules,

    PduGetEventItem
        => pdu_get_event_item(target: PduEventTarget) -> Option<PduEvent>,

    PduGetLastError
        => pdu_get_last_error(target: PduLastErrorTarget) -> PduErrorData,

    PduGetModuleIds
        => pdu_get_module_ids() -> PduModuleList,

    PduGetObjectId
        => pdu_get_object_id(object: PduObjt, short_name: Into<String>) -> Option<PduObjectId>,

    PduGetResourceIds
        => pdu_get_resource_ids(
            h_mod: Option<PduModuleHandle>,
            bus: BusSource,
            protocol: ProtocolSource,
            pins: Vec<TargetPin>
        ) -> PduModulesResourcesIds,

    PduGetResourceStatus
        => pdu_get_resource_status(resources: Vec<PduResource>) -> PduResourceStatus,

    PduGetStatus
        => pdu_get_status(target: PduStatusTarget) -> PduStatusData,

    PduGetTimestamp
        => pdu_get_timestamp(h_mod: PduModuleHandle) -> u32,

    PduGetUniqueRespIdTable
        => pdu_get_unique_resp_id_table(
            h_mod: PduModuleHandle,
            h_cll: PduCllHandle
        ) -> PduComParamTable,

    PduGetVersion
        => pdu_get_version(h_mod: PduModuleHandle) -> PduVersionData,

    PduIoCtl
        => pdu_io_ctl(
            target: PduIoCtlTarget,
            command: PduIoCtlCommand,
            data: Option<PduIoCtlData>
        ) -> Option<PduIoCtlData>,

    PduLockResource
        => pdu_lock_resource(
            h_mod: PduModuleHandle,
            h_cll: PduCllHandle,
            mask: PduLockResourceMask
        ) -> (),

    PduModuleConnect
        => pdu_module_connect(h_mod: PduModuleHandle) -> (),

    PduModuleDisconnect
        => pdu_module_disconnect(h_mod: Option<PduModuleHandle>) -> (),

    PduRegisterEventCallback
        => pdu_register_event_callback(
            target: PduEventTarget,
            callback: Option<EventCallbackFn>
        ) -> (),

    PduSetComParam
        => pdu_set_com_param(h_mod: PduModuleHandle, h_cll: PduCllHandle, cp: PduComParam) -> (),

    PduSetUniqueRespIdTable
        => pdu_set_unique_resp_id_table(
            h_mod: PduModuleHandle,
            h_cll: PduCllHandle,
            table: PduComParamTable
        ) -> (),

    PduStartComPrimitive => pdu_start_com_primitive(
        h_mod: PduModuleHandle,
        h_cll: PduCllHandle,
        cop_type: PduCopt,
        data: Vec<u8>,
        params: Option<PduPrimitiveParams>,
        tag: Option<PduUniqueCopTag>
    ) -> PduCopHandle,

    PduUnlockResource => pdu_unlock_resource(
        h_mod: PduModuleHandle,
        h_cll: PduCllHandle,
        mask: PduLockResourceMask
    ) -> (),
}
