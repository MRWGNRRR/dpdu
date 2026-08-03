pub mod single;
pub mod stack;
pub mod table;

use crate::AsyncRuntimeTarget;
use crate::api::{ApiError, PduApi};
use crate::error::{GeneralError, GeneralResult};
use crate::types::PduObjectId;
use crate::utils::{PhantomPtr, PhantomRef};
use dpdu_api_types::{
    ParamByteFieldData, ParamLongFieldData, ParamStructAccessTiming, ParamStructFieldData,
    ParamStructSessionTiming, PduCpst, PduError, PduObjt, PduPc, PduPt,
};
use std::ffi::c_void;
use std::fmt::{Debug, Display, Formatter};
use std::hash::{Hash, Hasher};
use std::sync::OnceLock;
use tokio::task::spawn_blocking;
use tracing::warn;

/// With the current design, this structure cannot be created directly. It can only be constructed
/// via the [`from_*`] methods. This is done to prevent panics when calling [PduApi::set_com_param()].
///
/// Thus, a [ComParam] is always identified either by an ID or a short name.
#[derive(Clone)]
pub struct PduComParam {
    pub(crate) short_name: OnceLock<String>,

    pub(crate) id: PduObjectId,

    pub class: PduPc,

    pub variant: CpVariant,
}

impl Hash for PduComParam {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl PartialEq for PduComParam {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for PduComParam {}

impl Display for PduComParam {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self.short_name.get() {
            Some(v) => write!(f, "{v} - #{}", self.id),
            None => write!(f, "#{}", self.id),
        }
    }
}

impl PduComParam {
    pub fn from_id(id: PduObjectId, class: PduPc, variant: impl Into<CpVariant>) -> Self {
        Self {
            short_name: OnceLock::new(),
            id,
            class,
            variant: variant.into(),
        }
    }

    pub fn blocking_from_short_name(
        api: &PduApi,
        short_name: impl Into<String>,
        class: PduPc,
        variant: impl Into<CpVariant>,
    ) -> GeneralResult<PduComParam> {
        let sn = short_name.into();

        let _id = match api.pdu_get_object_id(PduObjt::ComParam, &sn) {
            Ok(v) => v,
            Err(ApiError::PduError(PduError::InvalidParameters)) => {
                warn!("Unsupported comparam: {sn}");
                return Err(PduError::ComParamNotSupported)?;
            }
            Err(err) => {
                return Err(err)?;
            }
        };

        let Some(id) = api.pdu_get_object_id(PduObjt::ComParam, &sn)? else {
            warn!("Unsupported comparam: {sn}");
            return Err(PduError::ComParamNotSupported)?;
        };

        let com_param = Self {
            short_name: sn.into(),
            id,
            class,
            variant: variant.into(),
        };

        Ok(com_param)
    }

    /// Recommended way to construct the current structure.
    pub async fn from_short_name<'a>(
        runtime: impl Into<AsyncRuntimeTarget<'a>>,
        short_name: impl Into<String>,
        class: PduPc,
        variant: impl Into<CpVariant>,
    ) -> GeneralResult<PduComParam> {
        let sn = short_name.into();
        let variant = variant.into();
        let id = match runtime.into() {
            AsyncRuntimeTarget::Worker(worker) => {
                worker.pdu_get_object_id(PduObjt::ComParam, &sn).await
            }
            AsyncRuntimeTarget::Api(api) => {
                let api = api.clone_arc();
                let name = sn.to_owned();

                let task = move || Self::blocking_from_short_name(&api, &name, class, variant);

                return spawn_blocking(task).await.expect(
                    "internal error: PduComParam::blocking_from_short_name() task panicked",
                );
            }
        };

        let id = match id {
            Ok(v) => v,
            Err(GeneralError::ApiError(ApiError::PduError(PduError::InvalidParameters))) => {
                warn!("Unsupported comparam: {sn}");
                return Err(PduError::ComParamNotSupported)?;
            }
            Err(err) => {
                return Err(err)?;
            }
        };

        let Some(id) = id else {
            warn!("Unsupported comparam: {sn}");
            return Err(ApiError::PduError(PduError::ComParamNotSupported))?;
        };

        let com_param = Self {
            short_name: sn.into(),
            id,
            class,
            variant: variant.into(),
        };

        Ok(com_param)
    }

    pub fn get_short_name(&self) -> Option<&String> {
        self.short_name.get()
    }

    pub(crate) fn set_short_name(&self, name: impl Into<String>) {
        let _ = self.short_name.set(name.into());
    }

    pub(crate) fn try_init_short_name(&self, api: &PduApi) -> bool {
        let Some(desc) = api.module_description.as_ref() else {
            return false;
        };

        let opt = Some(desc)
            .and_then(|mdf_desc| mdf_desc.com_params.get_by_id(self.id))
            .and_then(|mdf_cp| mdf_cp.short_name.clone());

        if let Some(short_name) = opt {
            self.set_short_name(short_name);
            return true;
        }

        false
    }

    pub fn get_id(&self) -> &PduObjectId {
        &self.id
    }

    pub(crate) fn get_debug_name(&self) -> String {
        self.short_name
            .get()
            .map(|v| v.clone())
            .unwrap_or_else(|| format!("#{}", self.id))
    }
}

pub type ByteFieldComParam = FieldComParam<u8>;

impl ByteFieldComParam {
    pub fn new(mut data: Vec<u8>, capacity: Option<usize>) -> Self {
        let len = data.len();

        let capacity = capacity
            .and_then(|v| if v > len { Some(v) } else { None })
            .unwrap_or(len);

        data.reserve(capacity - len);

        FieldComParam {
            capacity: data.capacity(),
            owned_data: data,
            struct_type: None,
        }
    }

    pub(crate) fn get_pdu_data(&self) -> PhantomRef<'_, ParamByteFieldData> {
        PhantomRef::new(ParamByteFieldData {
            param_max_len: normalize_capacity(self.capacity) as _,
            param_act_len: self.owned_data.len() as _,
            p_data_array: self.owned_data.as_ptr() as _,
        })
    }
}

pub type StructFieldComParam = FieldComParam<StructComParam>;

impl StructFieldComParam {
    pub fn new(
        struct_type: PduCpst,
        mut data: Vec<StructComParam>,
        capacity: Option<usize>,
    ) -> Self {
        let len = data.len();

        let capacity = capacity
            .and_then(|v| if v > len { Some(v) } else { None })
            .unwrap_or(len);

        data.reserve(capacity - len);

        FieldComParam {
            capacity: data.capacity(),
            owned_data: data,
            struct_type: Some(struct_type),
        }
    }

    pub(crate) fn get_pdu_data(&self) -> PhantomRef<'_, ParamStructFieldData> {
        PhantomRef::new(ParamStructFieldData {
            com_param_struct_type: self.struct_type.expect("struct type is set"),
            param_max_entries: normalize_capacity(self.capacity) as _,
            param_act_entries: self.owned_data.len() as _,
            p_struct_array: self.owned_data.as_ptr() as _,
        })
    }
}

pub type LongFieldComParam = FieldComParam<u32>;

impl LongFieldComParam {
    pub fn new(mut data: Vec<u32>, capacity: Option<usize>) -> Self {
        let len = data.len();

        let capacity = capacity
            .and_then(|v| if v > len { Some(v) } else { None })
            .unwrap_or(len);

        data.reserve(capacity - len);

        FieldComParam {
            capacity: data.capacity(),
            owned_data: data,
            struct_type: None,
        }
    }

    pub(crate) fn get_pdu_data(&self) -> PhantomRef<'_, ParamLongFieldData> {
        PhantomRef::new(ParamLongFieldData {
            param_max_len: normalize_capacity(self.capacity) as _,
            param_act_len: self.owned_data.len() as _,
            p_data_array: self.owned_data.as_ptr() as _,
        })
    }
}

// TODO : impl Debug
#[derive(Clone)]
pub enum CpVariant {
    Unum8(u8),
    Snum8(i8),
    Unum16(u16),
    Snum16(i16),
    Unum32(u32),
    Snum32(i32),
    ByteField(ByteFieldComParam),
    StructField(StructFieldComParam),
    LongField(LongFieldComParam),
}

impl From<u8> for CpVariant {
    fn from(value: u8) -> Self {
        Self::Unum8(value)
    }
}

impl From<i8> for CpVariant {
    fn from(value: i8) -> Self {
        Self::Snum8(value)
    }
}

impl From<u16> for CpVariant {
    fn from(value: u16) -> Self {
        Self::Unum16(value)
    }
}

impl From<i16> for CpVariant {
    fn from(value: i16) -> Self {
        Self::Snum16(value)
    }
}

impl From<u32> for CpVariant {
    fn from(value: u32) -> Self {
        Self::Unum32(value)
    }
}

impl From<i32> for CpVariant {
    fn from(value: i32) -> Self {
        Self::Snum32(value)
    }
}

impl From<Vec<u8>> for CpVariant {
    fn from(value: Vec<u8>) -> Self {
        Self::ByteField(ByteFieldComParam::new(value, None))
    }
}

impl From<(Vec<u8>, usize)> for CpVariant {
    fn from(value: (Vec<u8>, usize)) -> Self {
        Self::ByteField(ByteFieldComParam::new(value.0, Some(value.1)))
    }
}

impl From<Vec<ParamStructAccessTiming>> for CpVariant {
    fn from(value: Vec<ParamStructAccessTiming>) -> Self {
        let vec = value.into_iter().map(|v| StructComParam::from(v)).collect();

        Self::StructField(StructFieldComParam::new(PduCpst::AccessTiming, vec, None))
    }
}

impl From<(Vec<ParamStructAccessTiming>, usize)> for CpVariant {
    fn from(value: (Vec<ParamStructAccessTiming>, usize)) -> Self {
        let vec = value
            .0
            .into_iter()
            .map(|v| StructComParam::from(v))
            .collect();

        Self::StructField(StructFieldComParam::new(
            PduCpst::AccessTiming,
            vec,
            Some(value.1),
        ))
    }
}

impl From<Vec<ParamStructSessionTiming>> for CpVariant {
    fn from(value: Vec<ParamStructSessionTiming>) -> Self {
        let vec = value.into_iter().map(|v| StructComParam::from(v)).collect();

        Self::StructField(StructFieldComParam::new(PduCpst::SessionTiming, vec, None))
    }
}

impl From<(Vec<ParamStructSessionTiming>, usize)> for CpVariant {
    fn from(value: (Vec<ParamStructSessionTiming>, usize)) -> Self {
        let vec = value
            .0
            .into_iter()
            .map(|v| StructComParam::from(v))
            .collect();

        Self::StructField(StructFieldComParam::new(
            PduCpst::SessionTiming,
            vec,
            Some(value.1),
        ))
    }
}

impl From<Vec<u32>> for CpVariant {
    fn from(value: Vec<u32>) -> Self {
        Self::LongField(LongFieldComParam::new(value, None))
    }
}

impl From<(Vec<u32>, usize)> for CpVariant {
    fn from(value: (Vec<u32>, usize)) -> Self {
        Self::LongField(LongFieldComParam::new(value.0, Some(value.1)))
    }
}

impl CpVariant {
    pub fn as_unum8(&self) -> Option<&u8> {
        match self {
            Self::Unum8(v) => Some(&v),
            _ => None,
        }
    }

    pub fn as_snum8(&self) -> Option<&i8> {
        match self {
            Self::Snum8(v) => Some(&v),
            _ => None,
        }
    }

    pub fn as_unum16(&self) -> Option<&u16> {
        match self {
            Self::Unum16(v) => Some(&v),
            _ => None,
        }
    }

    pub fn as_snum16(&self) -> Option<&i16> {
        match self {
            Self::Snum16(v) => Some(&v),
            _ => None,
        }
    }

    pub fn as_unum32(&self) -> Option<&u32> {
        match self {
            Self::Unum32(v) => Some(&v),
            _ => None,
        }
    }

    pub fn as_snum32(&self) -> Option<&i32> {
        match self {
            Self::Snum32(v) => Some(&v),
            _ => None,
        }
    }

    pub fn as_bytefield(&self) -> Option<&ByteFieldComParam> {
        match self {
            Self::ByteField(v) => Some(&v),
            _ => None,
        }
    }

    pub fn as_structfield(&self) -> Option<&StructFieldComParam> {
        match self {
            Self::StructField(v) => Some(&v),
            _ => None,
        }
    }

    pub fn as_longfield(&self) -> Option<&LongFieldComParam> {
        match self {
            Self::LongField(v) => Some(&v),
            _ => None,
        }
    }

    pub fn get_pdu_type(&self) -> PduPt {
        match self {
            Self::Unum8(..) => PduPt::Unum8,
            Self::Snum8(..) => PduPt::Snum8,

            Self::Unum16(..) => PduPt::Unum16,
            Self::Snum16(..) => PduPt::Snum16,

            Self::Unum32(..) => PduPt::Unum32,
            Self::Snum32(..) => PduPt::Snum32,

            Self::ByteField(..) => PduPt::ByteField,
            Self::StructField(..) => PduPt::StructField,
            Self::LongField(..) => PduPt::LongField,
        }
    }

    pub(crate) fn get_pdu_ptr(&self) -> PhantomPtr<'_> {
        let ptr: *const c_void = match self {
            Self::Unum8(d) => d as *const u8 as _,
            Self::Snum8(d) => d as *const i8 as _,

            Self::Unum16(d) => d as *const u16 as _,
            Self::Snum16(d) => d as *const i16 as _,

            Self::Unum32(d) => d as *const u32 as _,
            Self::Snum32(d) => d as *const i32 as _,

            Self::ByteField(d) => d.get_pdu_data().as_ptr() as _,
            Self::StructField(d) => d.get_pdu_data().as_ptr() as _,
            Self::LongField(d) => d.get_pdu_data().as_ptr() as _,
        };

        PhantomPtr::new(ptr)
    }
}

#[derive(Clone, Default)]
pub struct FieldComParam<T> {
    pub capacity: usize,
    pub owned_data: Vec<T>,
    pub struct_type: Option<PduCpst>,
}

impl<T> Debug for FieldComParam<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut s = f.debug_struct("FieldComParam");

        s /*.field("pdu_data", &self.pdu_data)*/
            .field("struct_type", &self.struct_type);

        s.finish()
    }
}

#[derive(Copy, Clone)]
pub union StructComParam {
    pub session_timing: ParamStructSessionTiming,
    pub access_timing: ParamStructAccessTiming,
}

impl From<ParamStructAccessTiming> for StructComParam {
    fn from(value: ParamStructAccessTiming) -> Self {
        StructComParam {
            access_timing: value,
        }
    }
}

impl From<ParamStructSessionTiming> for StructComParam {
    fn from(value: ParamStructSessionTiming) -> Self {
        StructComParam {
            session_timing: value,
        }
    }
}

/// Normalizes a structure capacity value for D-PDU API compatibility.
///
/// Some D-PDU API drivers (notably Softing) do not correctly handle
/// STRUCTFIELD ComParams with a maximum number of structures equal to zero.
/// Even when the actual number of structures is zero, such drivers may use
/// the maximum value during internal processing and cause memory corruption.
///
/// To avoid triggering this driver bug, zero capacity is replaced with one.
/// The actual number of structures remains unchanged, so this only affects
/// the advertised maximum capacity.
fn normalize_capacity(cap: usize) -> usize {
    (cap == 0).then(|| 1).unwrap_or(cap)
}
