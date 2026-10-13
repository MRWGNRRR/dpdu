use crate::AsyncRuntimeTarget;
use crate::api::{ApiError, PduApi};
use crate::error::{GeneralError, GeneralResult};
use crate::types::pdu_com_param::{CpVariant, PduComParam};
use crate::types::pdu_object::PduObjectIdSource;
use crate::types::{PduCllHandle, PduModuleHandle, PduObjectId, PduUniqueRespIdentifier};
use crate::utils::ecu_name_to_unique_resp_id;
use async_trait::async_trait;
use dashmap::DashMap;
use dpdu_api_types::{PduError, PduPc};
use std::hash::{Hash, Hasher};
use std::ops::{Deref, DerefMut};
use tokio::task::spawn_blocking;

pub type PduComParamSet = ComParamDefinitionSet<PduComParam>;

pub type PduComParamTable = ComParamDefinitionTable<PduComParam>;

pub type PduComParamDefinitionSet = ComParamDefinitionSet<ComParamDefinition>;

pub type PduComParamDefinitionTable = ComParamDefinitionTable<ComParamDefinition>;

#[derive(Clone)]
pub enum SetTarget {
    Definitions(PduComParamDefinitionSet),
    ComParams(PduComParamSet),
}

impl From<PduComParamDefinitionSet> for SetTarget {
    fn from(value: PduComParamDefinitionSet) -> Self {
        SetTarget::Definitions(value)
    }
}

impl From<PduComParamSet> for SetTarget {
    fn from(value: PduComParamSet) -> Self {
        SetTarget::ComParams(value)
    }
}

#[derive(Clone)]
pub enum MapTarget {
    Definitions(PduComParamDefinitionTable),
    ComParams(PduComParamTable),
}

impl From<PduComParamDefinitionTable> for MapTarget {
    fn from(value: PduComParamDefinitionTable) -> Self {
        MapTarget::Definitions(value)
    }
}

impl From<PduComParamTable> for MapTarget {
    fn from(value: PduComParamTable) -> Self {
        MapTarget::ComParams(value)
    }
}

#[derive(Clone)]
pub struct ComParamDefinitionSet<T>(pub DashMap<T::Id, T>)
where
    T: IntoPduComParam;

impl<T> Default for ComParamDefinitionSet<T>
where
    T: IntoPduComParam,
{
    fn default() -> Self {
        Self(DashMap::default())
    }
}

impl<T> Deref for ComParamDefinitionSet<T>
where
    T: IntoPduComParam,
{
    type Target = DashMap<T::Id, T>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> ComParamDefinitionSet<T>
where
    T: IntoPduComParam,
{
    pub(crate) fn with_capacity(cap: usize) -> Self {
        ComParamDefinitionSet(DashMap::with_capacity(cap))
    }

    pub fn new() -> Self {
        ComParamDefinitionSet(DashMap::new())
    }

    pub fn merge(&self, other: Self) -> &Self
    where
        T: Clone,
    {
        for entry in other.0.iter() {
            self.insert(entry.key().clone(), entry.value().clone());
        }
        self
    }

    pub fn add(&self, object: T) -> &Self {
        self.insert(object.id().to_owned(), object);
        self
    }
}

impl<T> ComParamDefinitionSet<T>
where
    T: IntoPduComParam + 'static,
{
    pub fn update(&self, param: T) {
        if let Some(mut cp) = self.0.get_mut(param.id()) {
            if *cp == param {
                cp.set_variant(param.get_variant().clone());
            }
        }
    }

    fn do_blocking_refresh_full(
        set: &DashMap<T::Id, T>,
        api: &PduApi,
        h_mod: PduModuleHandle,
        h_cll: PduCllHandle,
    ) -> GeneralResult<()> {
        for mut cp in set.iter_mut() {
            match api.pdu_get_com_param(h_mod, h_cll, cp.get_pdu_object_id_source()) {
                Ok(v) => cp.set_variant(v.variant),
                Err(ApiError::PduError(PduError::ComParamNotSupported)) => {
                    continue;
                }
                Err(err) => {
                    return Err(GeneralError::ApiError(err));
                }
            };
        }
        Ok(())
    }

    /// Refreshes the values of the communication parameters in the current set.
    pub fn blocking_refresh_full(
        &self,
        api: &PduApi,
        h_mod: PduModuleHandle,
        h_cll: PduCllHandle,
    ) -> GeneralResult<()> {
        Self::do_blocking_refresh_full(&self.0, api, h_mod, h_cll)
    }

    /// The asynchronous version of the [`Self::blocking_refresh_full`] function.
    pub async fn refresh_full<'a>(
        &self,
        runtime: impl Into<AsyncRuntimeTarget<'a>> + Send,
        h_mod: PduModuleHandle,
        h_cll: PduCllHandle,
    ) -> GeneralResult<()> {
        match runtime.into() {
            AsyncRuntimeTarget::Worker(worker) => {
                for mut cp in self.0.iter_mut() {
                    match worker
                        .pdu_get_com_param_dedicated(h_mod, h_cll, cp.get_pdu_object_id_source())
                        .await
                    {
                        Ok(v) => cp.set_variant(v.variant),
                        Err(GeneralError::ApiError(ApiError::PduError(
                            PduError::ComParamNotSupported,
                        ))) => {
                            continue;
                        }
                        Err(err) => {
                            return Err(err);
                        }
                    }
                }
            }
            AsyncRuntimeTarget::Api(api) => {
                let api = api.clone_arc();
                let mut self_map = self.0.clone();
                let task =
                    move || Self::do_blocking_refresh_full(&mut self_map, &api, h_mod, h_cll);
                spawn_blocking(task).await.expect(
                    "internal error: PduComParamSet::do_blocking_refresh() task panicked",
                )?;
            }
        }

        Ok(())
    }
}

#[derive(Clone, Default)]
pub struct ComParamDefinitionTable<T>(
    pub DashMap<PduUniqueRespIdentifier, ComParamDefinitionSet<T>>,
)
where
    T: IntoPduComParam;

impl<T> Deref for ComParamDefinitionTable<T>
where
    T: IntoPduComParam,
{
    type Target = DashMap<PduUniqueRespIdentifier, ComParamDefinitionSet<T>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> ComParamDefinitionTable<T>
where
    T: IntoPduComParam,
{
    pub(crate) fn with_capacity(cap: usize) -> Self {
        ComParamDefinitionTable(DashMap::with_capacity(cap))
    }

    pub fn new() -> Self {
        ComParamDefinitionTable(DashMap::new())
    }

    pub fn add(&self, id: PduUniqueRespIdentifier, com_param: T) -> &Self {
        self.entry(id)
            .or_default()
            .insert(com_param.id().to_owned(), com_param);
        self
    }

    pub fn add_with_ecu_name(&self, name: &str, com_param: T) -> &Self {
        self.entry(ecu_name_to_unique_resp_id(name))
            .or_default()
            .insert(com_param.id().to_owned(), com_param);
        self
    }

    pub fn add_set<I>(&self, id: PduUniqueRespIdentifier, set: I) -> &Self
    where
        I: IntoIterator<Item = T>,
    {
        let map = self.0.entry(id).or_default();

        for param in set {
            map.insert(param.id().clone(), param);
        }

        self
    }

    pub fn add_set_with_ecu_name<I>(&self, name: &str, set: I) -> &Self
    where
        I: IntoIterator<Item = T>,
    {
        self.add_set(ecu_name_to_unique_resp_id(name), set)
    }

    pub fn merge(&self, another: Self) -> &Self {
        for entry in another.0.iter() {
            let id = *entry.key();

            for def in entry.value().iter() {
                self.add(id, def.value().clone());
            }
        }

        self
    }
}

mod sealed {
    pub trait Sealed {}
}

#[async_trait]
pub trait IntoPduComParam: sealed::Sealed + Eq + Hash + Clone + Send + Sync {
    /// A type used to identify a communication parameter.
    type Id: Clone + Hash + Eq + Send + Sync;

    fn blocking_build(&self, api: &PduApi) -> GeneralResult<PduComParam>;

    async fn build<'a>(
        &self,
        runtime: impl Into<AsyncRuntimeTarget<'a>> + Send,
    ) -> GeneralResult<PduComParam>;

    fn get_pdu_object_id_source(&self) -> PduObjectIdSource;

    fn set_variant(&mut self, variant: CpVariant);

    fn get_variant(&self) -> &CpVariant;

    fn to_definition(&self) -> Result<ComParamDefinition, MissingCommunicationParameterName>;

    fn id(&self) -> &Self::Id;
}

impl sealed::Sealed for PduComParam {}

#[async_trait]
impl IntoPduComParam for PduComParam {
    type Id = PduObjectId;

    fn blocking_build(&self, _api: &PduApi) -> GeneralResult<PduComParam> {
        Ok(self.clone())
    }

    async fn build<'a>(
        &self,
        _runtime: impl Into<AsyncRuntimeTarget<'a>> + Send,
    ) -> GeneralResult<PduComParam> {
        Ok(self.clone())
    }

    fn get_pdu_object_id_source(&self) -> PduObjectIdSource {
        PduObjectIdSource::Id(self.id)
    }

    fn set_variant(&mut self, variant: CpVariant) {
        self.variant = variant;
    }

    fn get_variant(&self) -> &CpVariant {
        &self.variant
    }

    fn to_definition(&self) -> Result<ComParamDefinition, MissingCommunicationParameterName> {
        let Some(short_name) = self.short_name.get() else {
            return Err(MissingCommunicationParameterName);
        };

        Ok(ComParamDefinition {
            class: self.class,
            short_name: short_name.clone(),
            variant: self.variant.clone(),
        })
    }

    fn id(&self) -> &Self::Id {
        &self.id
    }
}

/// A communication parameter definition abstracted from a specific D-PDU API driver.
///
/// ## WARNING
///
/// The [`PartialEq`] and [`Hash`] implementations for this structure must consider only the
/// `short_name` field.
///
/// The [`Eq`] trait is implemented for compatibility with collections that require it.
/// Therefore, a positive equality comparison does not guarantee bit-by-bit equality.
#[derive(Clone)]
pub struct ComParamDefinition {
    pub class: PduPc,
    pub short_name: String,
    pub variant: CpVariant,
}

#[derive(thiserror::Error, Debug, Copy, Clone)]
#[error("the name of the communication parameter is missing")]
pub struct MissingCommunicationParameterName;

impl Eq for ComParamDefinition {}

impl PartialEq for ComParamDefinition {
    fn eq(&self, other: &Self) -> bool {
        self.short_name.eq(&other.short_name)
    }
}

impl Hash for ComParamDefinition {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.short_name.hash(state)
    }
}

impl ComParamDefinition {
    pub fn new(class: PduPc, short_name: impl Into<String>, variant: impl Into<CpVariant>) -> Self {
        let short_name = short_name.into();
        let variant = variant.into();
        Self {
            class,
            short_name,
            variant,
        }
    }

    pub fn unique(short_name: impl Into<String>, variant: impl Into<CpVariant>) -> Self {
        ComParamDefinition::new(PduPc::UniqueId, short_name, variant)
    }
}

impl sealed::Sealed for ComParamDefinition {}

#[async_trait]
impl IntoPduComParam for ComParamDefinition {
    type Id = String;

    fn blocking_build(&self, api: &PduApi) -> GeneralResult<PduComParam> {
        PduComParam::blocking_from_short_name(
            api,
            &self.short_name,
            self.class,
            self.variant.clone(),
        )
    }

    async fn build<'a>(
        &self,
        runtime: impl Into<AsyncRuntimeTarget<'a>> + Send,
    ) -> GeneralResult<PduComParam> {
        PduComParam::from_short_name(runtime, &self.short_name, self.class, self.variant.clone())
            .await
    }

    fn get_pdu_object_id_source(&self) -> PduObjectIdSource {
        PduObjectIdSource::ShortName(self.short_name.clone())
    }

    fn set_variant(&mut self, variant: CpVariant) {
        self.variant = variant;
    }

    fn get_variant(&self) -> &CpVariant {
        &self.variant
    }

    fn to_definition(&self) -> Result<ComParamDefinition, MissingCommunicationParameterName> {
        Ok(self.clone())
    }

    fn id(&self) -> &Self::Id {
        &self.short_name
    }
}
