use crate::api::{ApiResult, PduApi};
use crate::types::pdu_com_param::{CpVariant, PduComParam};
use crate::types::{PduCllHandle, PduModuleHandle};
use crate::utils::PhantomRef;
use crate::vendor_specific::wrap_pdu_call;
use dpdu_api_types::{
    ParamByteFieldData, ParamItem, ParamLongFieldData, ParamStructFieldData, PduError, PduIt,
    PduPc, PduPt,
};
use std::mem::ManuallyDrop;
use tracing::{Level, trace};

impl PduApi {
    pub fn pdu_set_com_param(
        &self,
        h_mod: PduModuleHandle,
        h_cll: PduCllHandle,
        cp: &PduComParam,
    ) -> ApiResult<()> {
        impl_defer_clear_suppress_options!(self, set_com_param);

        const FUNC: &'static str = "PDUSetComParam";
        self.log_api_call(FUNC);

        if matches!(cp.class, PduPc::UniqueId) {
            // Chapter 9.4.27.1:
            //
            // NOTE ComParams that are of type PDU_PC_UNIQUE_ID can only be used with
            // the Unique Response ID Table.
            // They cannot be used in the functions PDUGetComParam() or PDUSetComParam().
            //
            // Therefore, to reduce the number of calls to the D-PDU API, we proactively
            // return the PduError::InvalidParameters error on our side.
            let result = PduError::InvalidParameters;
            self.log_api_call_fail(
                FUNC,
                result,
                Some("PDUSetComParam accepts only UniqueId classes".to_string()),
                None,
            );
            return Err(result)?;
        }

        cp.try_init_short_name(self);

        #[repr(C)]
        union TempUnion<'a> {
            u8: u8,
            s8: i8,
            u16: u16,
            s16: i16,
            u32: u32,
            s32: i32,
            long_field: ManuallyDrop<PhantomRef<'a, ParamLongFieldData>>,
            struct_field: ManuallyDrop<PhantomRef<'a, ParamStructFieldData>>,
            byte_field: ManuallyDrop<PhantomRef<'a, ParamByteFieldData>>,
        }

        struct AutoDrop<'a> {
            temp: TempUnion<'a>,
            pdu_type: PduPt,
        }

        impl<'a> Drop for AutoDrop<'a> {
            fn drop(&mut self) {
                match self.pdu_type {
                    PduPt::LongField => unsafe { ManuallyDrop::drop(&mut self.temp.long_field) },
                    PduPt::StructField => unsafe {
                        ManuallyDrop::drop(&mut self.temp.struct_field)
                    },
                    PduPt::ByteField => unsafe { ManuallyDrop::drop(&mut self.temp.byte_field) },
                    _ => {}
                }
            }
        }

        let data = AutoDrop {
            temp: match &cp.variant {
                CpVariant::Unum8(v) => TempUnion { u8: v.to_owned() },
                CpVariant::Snum8(v) => TempUnion { s8: v.to_owned() },
                CpVariant::Unum16(v) => TempUnion { u16: v.to_owned() },
                CpVariant::Snum16(v) => TempUnion { s16: v.to_owned() },
                CpVariant::Unum32(v) => TempUnion { u32: v.to_owned() },
                CpVariant::Snum32(v) => TempUnion { s32: v.to_owned() },
                CpVariant::LongField(v) => TempUnion {
                    long_field: ManuallyDrop::new(v.get_pdu_data()),
                },
                CpVariant::StructField(v) => TempUnion {
                    struct_field: ManuallyDrop::new(v.get_pdu_data()),
                },
                CpVariant::ByteField(v) => TempUnion {
                    byte_field: ManuallyDrop::new(v.get_pdu_data()),
                },
            },
            pdu_type: cp.variant.get_pdu_type(),
        };

        let item = ParamItem {
            item_type: PduIt::Param,
            com_param_id: cp.id,
            com_param_data_type: cp.variant.get_pdu_type(),
            com_param_class: cp.class,
            p_com_param_data: &data.temp as *const _ as _,
        };

        trace!(
            func = FUNC,
            h_mod,
            h_cll,
            com_param = cp.get_debug_name(),
            com_param_ptr = format!("0x{:x}", &item as *const _ as usize),
            "D-PDU API Call Args"
        );

        let set_com_param_fn = self.symbols.set_com_param;
        let result = wrap_pdu_call(FUNC, || {
            set_com_param_fn(h_mod, h_cll, &item as *const _ as _)
        });

        if !result.is_success() {
            return match result {
                PduError::ComParamNotSupported | PduError::InvalidParameters => {
                    // This is not a critical error.
                    // Therefore, we will not log it separately.
                    self.log_api_call_fail(
                        FUNC,
                        result,
                        Some(format!("unsupported com param: {cp}")),
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

        Ok(())
    }
}
