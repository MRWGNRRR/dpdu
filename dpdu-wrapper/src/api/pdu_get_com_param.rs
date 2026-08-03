use crate::api::{ApiResult, PduApi};
use crate::types::pdu_com_param::{
    ByteFieldComParam, CpVariant, LongFieldComParam, PduComParam, StructComParam,
    StructFieldComParam,
};
use crate::types::pdu_object::PduObjectIdSource;
use crate::types::{PduCllHandle, PduModuleHandle};
use crate::vendor_specific::wrap_pdu_call;
use dpdu_api_types::{
    ParamByteFieldData, ParamItem, ParamLongFieldData, ParamStructFieldData, PduError, PduObjt,
    PduPt,
};
use std::ptr::NonNull;
use std::sync::OnceLock;
use std::{ptr, slice};
use tracing::{Level, error, trace};

impl PduApi {
    pub fn pdu_get_com_param(
        &self,
        h_mod: PduModuleHandle,
        h_cll: PduCllHandle,
        object_id: PduObjectIdSource,
    ) -> ApiResult<PduComParam> {
        impl_defer_clear_suppress_options!(self, get_com_param);

        const FUNC: &'static str = "PDUGetComParam";
        self.log_api_call(FUNC);

        trace!(func = FUNC, h_mod, h_cll, %object_id, "D-PDU API Call Args");

        let id = match &object_id {
            PduObjectIdSource::Id(v) => *v,
            PduObjectIdSource::ShortName(v) => {
                let Some(id) = self.pdu_get_object_id(PduObjt::ComParam, &v)? else {
                    let result = PduError::ComParamNotSupported;

                    // This is not a critical error.
                    // Therefore, we will not log it separately.
                    self.log_api_call_fail(
                        FUNC,
                        result,
                        Some(format!("unsupported com param: {v}")),
                        Some(Level::WARN),
                    );

                    return Err(result)?;
                };
                id
            }
        };

        let mut item_ptr: *mut ParamItem = ptr::null_mut();
        let get_com_param_fn = self.symbols.get_com_param;
        let result = wrap_pdu_call(FUNC, || get_com_param_fn(h_mod, h_cll, id, &mut item_ptr));

        trace!(
            func = FUNC,
            item_ptr = format!("0x{:x}", item_ptr as usize),
            item_type = ?NonNull::new(item_ptr).map(|wptr| unsafe { (&*wptr.as_ptr()).item_type }),
            "D-PDU API Call Return"
        );

        if !result.is_success() {
            return match result {
                PduError::ComParamNotSupported | PduError::InvalidParameters => {
                    // This is not a critical error.
                    // Therefore, we will not log it separately.
                    self.log_api_call_fail(
                        FUNC,
                        result,
                        Some(format!("unsupported com param: {object_id}")),
                        Some(Level::WARN),
                    );
                    Err(PduError::ComParamNotSupported)?
                }
                _ => {
                    self.log_api_call_fail(
                        FUNC,
                        result,
                        None,
                        resolve_level_of_log_api_call_fail!(self, result, get_com_param),
                    );
                    Err(result)?
                }
            };
        }

        if item_ptr.is_null() {
            error!(
                func = FUNC,
                "Item pointer is null. Emulation of PduError::FctFailed..."
            );
            return Err(PduError::FctFailed)?;
        }

        let cp = unsafe {
            use ptr::read;

            let item = &*item_ptr;
            let data_ptr = item.p_com_param_data;
            let short_name = OnceLock::new();

            match &object_id {
                PduObjectIdSource::ShortName(v) => {
                    let _ = short_name.set(v.clone());
                }
                _ => {
                    let sn_opt = self
                        .module_description
                        .as_ref()
                        .and_then(|mdf_desc| mdf_desc.com_params.get_by_id(id))
                        .and_then(|mdf_cp| mdf_cp.short_name.clone());

                    if let Some(sn) = sn_opt {
                        let _ = short_name.set(sn);
                    }
                }
            };

            PduComParam {
                short_name,
                id,
                class: item.com_param_class,
                variant: match item.com_param_data_type {
                    PduPt::Unum8 => CpVariant::Unum8(read(data_ptr as _)),
                    PduPt::Snum8 => CpVariant::Snum8(read(data_ptr as _)),
                    PduPt::Unum16 => CpVariant::Unum16(read(data_ptr as _)),
                    PduPt::Snum16 => CpVariant::Snum16(read(data_ptr as _)),
                    PduPt::Unum32 => CpVariant::Unum32(read(data_ptr as _)),
                    PduPt::Snum32 => CpVariant::Snum32(read(data_ptr as _)),
                    PduPt::ByteField => CpVariant::ByteField({
                        let data = &*(data_ptr as *const ParamByteFieldData);
                        let ptr = data.p_data_array;
                        let len = data.param_act_len as _;
                        let slice = if ptr.is_null() || len == 0 {
                            &[]
                        } else {
                            slice::from_raw_parts(ptr, len)
                        };
                        ByteFieldComParam::new(slice.to_vec(), Some(data.param_max_len as _))
                    }),
                    PduPt::StructField => CpVariant::StructField({
                        let data = &*(data_ptr as *const ParamStructFieldData);
                        let ptr = data.p_struct_array as *mut StructComParam;
                        let len = data.param_act_entries as _;
                        let slice = if ptr.is_null() || len == 0 {
                            &[]
                        } else {
                            slice::from_raw_parts(ptr, len)
                        };
                        StructFieldComParam::new(
                            data.com_param_struct_type,
                            slice.to_vec(),
                            Some(data.param_max_entries as _),
                        )
                    }),
                    PduPt::LongField => CpVariant::LongField({
                        let data = &*(data_ptr as *const ParamLongFieldData);
                        let ptr = data.p_data_array;
                        let len = data.param_act_len as _;
                        let slice = if ptr.is_null() || len == 0 {
                            &[]
                        } else {
                            slice::from_raw_parts(ptr, len)
                        };
                        LongFieldComParam::new(slice.to_vec(), Some(data.param_max_len as _))
                    }),
                },
            }
        };

        self.pdu_destroy_item(item_ptr as _)?;

        Ok(cp)
    }
}
