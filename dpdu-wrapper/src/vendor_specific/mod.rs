use dpdu_api_types::PduError;

mod detours;
mod softing;
mod vxdiag;

pub fn wrap_pdu_call<F>(func: &str, mut f: F) -> PduError
where
    F: FnMut() -> PduError,
{
    register_detours();
    register_detour_callbacks();
    do_miscellaneous();

    let result = f();

    if vxdiag::has_device_not_connected() {
        return match func {
            "PDUModuleDisconnect"
            | "PDUGetTimestamp"
            | "PDUIoCtl"
            | "PDUGetVersion"
            | "PDUGetLastError"
            | "PDUCreateComLogicalLink"
            | "PDUDestroyComLogicalLink"
            | "PDUDisconnect"
            | "PDULockResource"
            | "PDUUnlockResource"
            | "PDUGetComParam"
            | "PDUSetComParam"
            | "PDUStartComPrimitive"
            | "PDUCancelComPrimitive"
            | "PDUGetEventItem"
            | "PDURegisterEventCallback"
            | "PDUGetUniqueRespIdTable"
            | "PDUSetUniqueRespIdTable" => PduError::ModuleNotConnected,
            _ => PduError::FctFailed,
        };
    }

    result
}

fn register_detours() {
    detours::msg_box_a::hook_message_box_a();
}

fn register_detour_callbacks() {
    vxdiag::register_callback();
}

fn do_miscellaneous() {
    softing::restore_protocol_dll();
}
