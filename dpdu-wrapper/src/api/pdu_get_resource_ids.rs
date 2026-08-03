use crate::api::{ApiResult, PduApi, target_pins_to_pin_data};
use crate::types::PduModuleHandle;
use crate::types::pdu_module::PduModulesResourcesIds;
use crate::types::pdu_resource::{BusSource, ProtocolSource, TargetPin};
use crate::utils::take_slice_ptr;
use crate::vendor_specific::wrap_pdu_call;
use dpdu_api_types::{PDU_HANDLE_UNDEF, PduError, PduIt, RscData};
use std::ptr::NonNull;
use std::{ptr, slice};
use tracing::{error, trace};

impl PduApi {
    pub fn pdu_get_resource_ids(
        &self,
        h_mod: Option<PduModuleHandle>,
        bus: &BusSource,
        protocol: &ProtocolSource,
        pins: &[TargetPin],
    ) -> ApiResult<PduModulesResourcesIds> {
        impl_defer_clear_suppress_options!(self, get_resource_ids);

        const FUNC: &'static str = "PDUGetResourceIds";
        self.log_api_call(FUNC);

        let h_mod = h_mod.unwrap_or(PDU_HANDLE_UNDEF);

        trace!(
            func = FUNC,
            h_mod,
            %bus,
            %protocol,
            "D-PDU API Call Args",
        );

        let bus_id = bus.resolve_bus_id(FUNC, self)?;
        let protocol_id = protocol.resolve_protocol_id(FUNC, self)?;
        let pin_data = target_pins_to_pin_data(self, FUNC, pins)?;

        let resource_data = RscData {
            bus_type_id: bus_id,
            protocol_id,
            num_pin_data: pin_data.len() as _,
            p_dlc_pin_data: take_slice_ptr(&pin_data),
        };

        let mut rsc_data_ptr = ptr::null_mut();

        let get_resource_ids_fn = self.symbols.get_resource_ids;
        let result = wrap_pdu_call(FUNC, || {
            get_resource_ids_fn(h_mod, &resource_data as *const _ as _, &mut rsc_data_ptr)
        });

        if !result.is_success() {
            self.log_api_call_fail(
                FUNC,
                result,
                None,
                resolve_level_of_log_api_call_fail!(self, result, get_resource_ids),
            );
            return Err(result)?;
        }

        trace!(
            func = FUNC,
            item_ptr = format!("0x{:x}", rsc_data_ptr as usize),
            item_type = ?NonNull::new(rsc_data_ptr).map(|wptr| unsafe { (&*wptr.as_ptr()).item_type }),
            "D-PDU API Call Return"
        );

        if rsc_data_ptr.is_null() {
            error!(
                func = FUNC,
                "Item data pointer is null. Emulation of PduError::FctFailed..."
            );
            return Err(PduError::FctFailed)?;
        }

        let rsc_data = unsafe { &*rsc_data_ptr };

        if !matches!(rsc_data.item_type, PduIt::RscConflict) {
            error!(
                func = FUNC,
                "Invalid item type received: PduIt::{}. Emulation of PduError::FctFailed...",
                rsc_data.item_type.as_str(),
            );

            self.pdu_destroy_item(rsc_data_ptr as _)?;
            return Err(PduError::FctFailed)?;
        }

        let mut map = PduModulesResourcesIds::with_capacity(rsc_data.num_modules as _);

        let rsc_items_ptr = rsc_data.p_id_item_data;
        let rsc_items_len = rsc_data.num_modules as usize;

        let rsc_items = if rsc_items_ptr.is_null() || rsc_items_len == 0 {
            &[]
        } else {
            unsafe { slice::from_raw_parts(rsc_items_ptr, rsc_items_len) }
        };

        for rsc_item in rsc_items {
            let rsc_ids_ptr = rsc_item.p_resource_id_array;
            let rsc_ids_len = rsc_item.num_ids as usize;

            let resource_ids = if rsc_ids_ptr.is_null() || rsc_ids_len == 0 {
                &[]
            } else {
                unsafe { slice::from_raw_parts(rsc_ids_ptr, rsc_ids_len) }
            };

            trace!(
                func = FUNC,
                rsc_item_h_mod = rsc_item.h_mod,
                rsc_item_resource_ids = ?resource_ids,
                "D-PDU API Call Return"
            );

            map.insert(rsc_item.h_mod, resource_ids.to_vec());
        }

        Ok(map)
    }
}
