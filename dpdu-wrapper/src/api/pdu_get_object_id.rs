use crate::api::{ApiResult, PduApi};
use crate::types::PduObjectId;
use crate::vendor_specific::wrap_pdu_call;
use dpdu_api_types::{PDU_ID_UNDEF, PduObjt};
use std::ffi::CString;
use std::mem::MaybeUninit;
use tracing::trace;

impl PduApi {
    pub fn pdu_get_object_id(
        &self,
        object: PduObjt,
        short_name: &str,
    ) -> ApiResult<Option<PduObjectId>> {
        impl_defer_clear_suppress_options!(self, get_object_id);

        const FUNC: &'static str = "PDUGetObjectId";
        self.log_api_call(FUNC);

        trace!(
            func = FUNC,
            object = object.as_str(),
            short_name,
            "D-PDU API Call Args"
        );

        if let Some(desc) = &self.module_description {
            // First, we will try to obtain the required object ID from the module description
            // file supplied with the D-PDU API driver in order to reduce
            // the number of D-PDU API calls.
            let object_id = match object {
                PduObjt::IoCtrl => desc.io_controls.get_by_short_name(short_name).map(|v| v.id),
                PduObjt::Resource => desc.resources.get_by_short_name(short_name).map(|v| v.id),
                PduObjt::Protocol => desc.protocols.get_by_short_name(short_name).map(|v| v.id),
                PduObjt::BusType => desc.bus_types.get_by_short_name(short_name).map(|v| v.id),
                PduObjt::PinType => desc.pin_types.get_by_short_name(short_name).map(|v| v.id),
                PduObjt::ComParam => desc.com_params.get_by_short_name(short_name).map(|v| v.id),
            };

            if let Some(id) = object_id {
                trace!(func = FUNC, id, "D-PDU API Call Return [virtual]");
                return Ok(Some(id));
            }
        }

        let short_name = CString::new(short_name).expect("CString::new() failed");
        let mut object_id: MaybeUninit<u32> = MaybeUninit::uninit();

        let get_object_id_fn = self.symbols.get_object_id;
        let result = wrap_pdu_call(FUNC, || {
            get_object_id_fn(object, short_name.as_ptr() as _, object_id.as_mut_ptr())
        });

        if !result.is_success() {
            self.log_api_call_fail(
                FUNC,
                result,
                None,
                resolve_level_of_log_api_call_fail!(self, result, get_object_id),
            );
            return Err(result)?;
        }

        // SAFETY:
        // PDUGetObjectId guarantees that `object_id` is initialized on success.
        let object_id = unsafe { object_id.assume_init() };

        trace!(func = FUNC, object_id, "D-PDU API Call Return");

        if object_id != PDU_ID_UNDEF {
            Ok(Some(object_id))
        } else {
            Ok(None)
        }
    }
}
