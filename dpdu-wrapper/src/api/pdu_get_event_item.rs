use crate::api::{ApiResult, PduApi};
use crate::types::pdu_event::{
    PduErrorEvent, PduEvent, PduEventData, PduEventTarget, PduInfoEvent, PduResultEvent,
    PduStatusEvent,
};
use crate::vendor_specific::wrap_pdu_call;
use dpdu_api_types::{
    ErrorData, EventItem, InfoData, PDU_HANDLE_UNDEF, PduError, PduIt, PduStatus, ResultData,
};
use std::cell::OnceCell;
use std::num::NonZeroUsize;
use std::ptr::NonNull;
use std::{ptr, slice};
use tracing::{error, trace};

impl PduApi {
    pub fn pdu_get_event_item(&self, target: &PduEventTarget) -> ApiResult<Option<PduEvent>> {
        impl_defer_clear_suppress_options!(self, get_event_item);

        const FUNC: &'static str = "PDUGetEventItem";
        self.log_api_call(FUNC);

        let h_mod = target.get_module_handle().unwrap_or(PDU_HANDLE_UNDEF);
        let h_cll = target.get_cll_handle().unwrap_or(PDU_HANDLE_UNDEF);

        trace!(func = FUNC, h_mod, h_cll, "D-PDU API Call Args");

        let mut item_ptr: *mut EventItem = ptr::null_mut();

        let get_event_item_fn = self.symbols.get_event_item;
        let result = wrap_pdu_call(FUNC, || get_event_item_fn(h_mod, h_cll, &mut item_ptr));
        let debug_ret = || {
            trace!(
                func = FUNC,
                item_ptr = format!("0x{:x}", item_ptr as usize),
                item_type = ?NonNull::new(item_ptr).map(|wptr| unsafe { (&*wptr.as_ptr()).item_type }),
                "D-PDU API Call Return"
            );
        };

        match result {
            PduError::StatusNoError => {}
            PduError::EventQueueEmpty => {
                debug_ret();
                return Ok(None);
            }
            v => {
                self.log_api_call_fail(
                    FUNC,
                    result,
                    None,
                    resolve_level_of_log_api_call_fail!(self, result, get_event_item),
                );
                return Err(v)?;
            }
        }

        debug_ret();

        if item_ptr.is_null() {
            return Ok(None);
        }

        let item = unsafe { &*item_ptr };

        if item.p_data.is_null() {
            error!(
                func = FUNC,
                "Item data pointer is null. Emulation of PduError::FctFailed..."
            );
            return Err(PduError::FctFailed)?;
        }

        let data: PduEventData = match item.item_type {
            PduIt::Status => PduStatusEvent(unsafe { *(item.p_data as *const PduStatus) }).into(),
            PduIt::Result => {
                let data = unsafe { &*(item.p_data as *const ResultData) };

                let mut extra_header = OnceCell::new();
                let mut extra_footer = OnceCell::new();

                if !data.p_extra_info.is_null() {
                    let extra_info = unsafe { &*data.p_extra_info };
                    if !extra_info.p_header_bytes.is_null() {
                        let ptr = extra_info.p_header_bytes;
                        let len = extra_info.num_header_bytes;
                        if !ptr.is_null() && len > 0 {
                            extra_header
                                .set(unsafe { slice::from_raw_parts(ptr, len as _) }.to_vec())
                                .unwrap();
                        }
                    }
                    if !extra_info.p_footer_bytes.is_null() {
                        let ptr = extra_info.p_footer_bytes;
                        let len = extra_info.num_footer_bytes;
                        if !ptr.is_null() && len > 0 {
                            extra_footer
                                .set(unsafe { slice::from_raw_parts(ptr, len as _) }.to_vec())
                                .unwrap();
                        }
                    }
                }

                PduResultEvent {
                    rx_flags: unsafe {
                        let ptr = data.rx_flag.p_flag_data;
                        let len = data.rx_flag.num_flag_bytes as usize;
                        let slice = if ptr.is_null() || len == 0 {
                            &[]
                        } else {
                            slice::from_raw_parts(ptr, len)
                        };
                        slice.to_vec().into()
                    },
                    unique_resp_identifier: data.unique_resp_identifier,
                    acceptance_id: data.acceptance_id,
                    timestamp_flags: unsafe {
                        let ptr = data.timestamp_flags.p_flag_data;
                        let len = data.timestamp_flags.num_flag_bytes as usize;
                        let slice = if ptr.is_null() || len == 0 {
                            &[]
                        } else {
                            slice::from_raw_parts(ptr, len)
                        };
                        slice.to_vec().into()
                    },
                    tx_msg_done_timestamp: data.tx_msg_done_timestamp,
                    start_msg_timestamp: data.start_msg_timestamp,
                    data: unsafe {
                        let ptr = data.p_data_bytes;
                        let len = data.num_data_bytes as usize;
                        let slice = if ptr.is_null() || len == 0 {
                            &[]
                        } else {
                            slice::from_raw_parts(ptr, len)
                        };
                        slice.to_vec().into()
                    },
                    extra_info_header: extra_header.take(),
                    extra_info_footer: extra_footer.take(),
                }
                .into()
            }
            PduIt::Error => {
                let data = unsafe { &*(item.p_data as *const ErrorData) };
                PduErrorEvent {
                    code: data.error_code_id,
                    extra_code: data.extra_error_info_id,
                }
                .into()
            }
            PduIt::Info => {
                let data = unsafe { &*(item.p_data as *const InfoData) };
                PduInfoEvent {
                    code: data.info_code,
                    extra_code: data.extra_info_data,
                }
                .into()
            }
            typ => {
                self.pdu_destroy_item(item_ptr as _)?;
                error!(
                    func = FUNC,
                    "Unexpected PduEventItemType = {}. Emulation of PduError::FctFailed...",
                    typ.as_str()
                );
                return Err(PduError::FctFailed)?;
            }
        };

        let h_cop = (item.h_cop != PDU_HANDLE_UNDEF).then(|| item.h_cop);
        let cop_tag = NonZeroUsize::new(item.p_cop_tag as _);
        let timestamp = item.timestamp;

        self.pdu_destroy_item(item_ptr as _)?;

        Ok(Some(PduEvent {
            target: target.clone(),
            h_cop,
            cop_tag,
            timestamp,
            data,
        }))
    }
}
