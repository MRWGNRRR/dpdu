use crate::api::{ApiResult, PduApi};
use crate::types::pdu_module::{PduModuleData, PduModuleList};
use crate::utils::c_str;
use crate::vendor_specific::wrap_pdu_call;
use dpdu_api_types::PduError;
use std::ptr::NonNull;
use std::{ptr, slice};
use tracing::{error, trace};

impl PduApi {
    pub fn pdu_get_module_ids(&self) -> ApiResult<PduModuleList> {
        impl_defer_clear_suppress_options!(self, get_module_ids);

        const FUNC: &'static str = "PDUGetModuleIds";
        self.log_api_call(FUNC);

        let mut module_list_item_ptr = ptr::null_mut();

        let get_module_ids_fn = self.symbols.get_module_ids;
        let result = wrap_pdu_call(FUNC, || get_module_ids_fn(&mut module_list_item_ptr));

        if !result.is_success() {
            self.log_api_call_fail(
                FUNC,
                result,
                None,
                resolve_level_of_log_api_call_fail!(self, result, get_module_ids),
            );
            if !module_list_item_ptr.is_null() {
                self.pdu_destroy_item(module_list_item_ptr as _)?;
            }
            return Err(result)?;
        }

        trace!(
            func = FUNC,
            item_ptr = format!("0x{:x}", module_list_item_ptr as usize),
            item_type = ?NonNull::new(module_list_item_ptr).map(|wptr| unsafe { (&*wptr.as_ptr()).item_type }),
            "D-PDU API Call Return"
        );

        if module_list_item_ptr.is_null() {
            error!(
                func = FUNC,
                "Module list pointer is null. Emulation of PduError::FctFailed..."
            );
            return Err(PduError::FctFailed)?;
        }

        let module_list_item = unsafe { &*module_list_item_ptr };

        let ptr = module_list_item.p_module_data;
        let len = module_list_item.num_entries as _;

        trace!(
            func = FUNC,
            modules_ptr = format!("{:#x}", ptr as usize),
            modules_len = len,
            "D-PDU API Call Return"
        );

        let modules = if ptr.is_null() || len == 0 {
            &[]
        } else {
            unsafe { slice::from_raw_parts(ptr, len) }
        };

        let module_list = modules
            .into_iter()
            .map(|v| {
                let vendor_module_name = c_str(v.vendor_module_name as _);
                let vendor_additional_info = c_str(v.vendor_additional_info as _);

                trace!(
                    func = FUNC,
                    module_handle = v.h_mod,
                    module_type_id = v.module_type_id,
                    module_name = vendor_module_name,
                    module_add_info = vendor_additional_info,
                    "D-PDU API Call Return"
                );

                PduModuleData {
                    h_mod: v.h_mod,
                    module_type_id: v.module_type_id,
                    vendor_module_name,
                    vendor_additional_info,
                    status: v.status,
                }
            })
            .collect::<Vec<_>>();

        self.pdu_destroy_item(module_list_item_ptr as _)?;

        Ok(module_list)
    }
}
