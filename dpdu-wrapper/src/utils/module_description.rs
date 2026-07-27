#![allow(dead_code)]

use self::raw_xml_types::Container;
use crate::utils::get_bomless_file_reader;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;
use tracing::{debug, error, info};

#[derive(Debug, Clone, thiserror::Error)]
pub enum PduModuleDescriptionError {
    #[error("io error: {0}")]
    IoError(String),

    #[error("xml parser error: {0}")]
    XmlParseError(#[from] quick_xml::de::DeError),
}

impl From<std::io::Error> for PduModuleDescriptionError {
    fn from(value: std::io::Error) -> Self {
        Self::IoError(value.to_string())
    }
}

#[derive(Debug)]
pub struct PduModuleDescription {
    /// Path to Module Description file.
    pub path: PathBuf,

    /// Value of the <DESCRIPTION> tag from the Module Description file.
    pub description: Option<String>,

    /// Value of the <SUPPLIER_NAME> tag from the Module Description file.
    pub supplier_name: Option<String>,

    pub pin_types: MdfPinTypeWrapper,

    pub module_types: MdfModuleTypeWrapper,

    pub resources: MdfResourceWrapper,

    pub protocols: MdfProtocolWrapper,

    pub bus_types: MdfBusTypeWrapper,

    pub com_params: MdfComParamWrapper,

    pub io_controls: MdfIoControlWrapper,

    pub error_codes: MdfErrorCodeWrapper,
}

impl PduModuleDescription {
    pub fn parse_from_xml_file<P>(path: P) -> Result<Self, PduModuleDescriptionError>
    where
        P: AsRef<Path>,
    {
        let path = path.as_ref();

        info!(path = %path.display(), "Parsing the D-PDU API mdf file...");
        let start = Instant::now();

        let mut reader = get_bomless_file_reader(path).inspect_err(|err| {
            error!(
                path = %path.display(),
                "Failed to obtain a BOM-less file reader for the D-PDU API mdf file: {err:?}"
            )
        })?;

        let container: Container = quick_xml::de::from_reader(&mut reader).inspect_err(|err| {
            error!(
                path = %path.display(),
                "Failed to parse the D-PDU API mdf file using quick-xml: {err:?}"
            )
        })?;

        let pin_types = Self::parse_pin_types(&container);
        let io_controls = Self::parse_io_controls(&container);
        let com_params = Self::parse_com_params(&container);
        let bus_types = Self::parse_bus_types(&container, &com_params);
        let protocols = Self::parse_protocols(&container, &com_params);
        let resources = Self::parse_resources(&container, &pin_types, &bus_types, &protocols);
        let module_types = Self::parse_module_types(&container, &io_controls, &resources);

        info!(
            path = %path.display(),
            elapsed_ms = start.elapsed().as_millis(),
            "Parsing the D-PDU API mdf file is complete."
        );

        Ok(Self {
            path: path.to_path_buf(),
            description: Self::parse_description(&container),
            supplier_name: Self::parse_supplier_name(&container),
            pin_types,
            module_types,
            resources,
            protocols,
            bus_types,
            com_params,
            io_controls,
            error_codes: Self::parse_error_codes(&container),
        })
    }

    fn parse_description(container: &Container) -> Option<String> {
        use raw_xml_types::ContainerElement as Element;
        for elem in container.elements.iter() {
            match elem {
                Element::Description(s) => {
                    return Some(s.clone());
                }
                _ => {}
            }
        }
        None
    }

    fn parse_supplier_name(container: &Container) -> Option<String> {
        use raw_xml_types::ContainerElement as Element;
        for elem in container.elements.iter() {
            match elem {
                Element::SupplierName(s) => {
                    return Some(s.clone());
                }
                _ => {}
            }
        }
        None
    }

    fn parse_pin_types(container: &Container) -> MdfPinTypeWrapper {
        use raw_xml_types::ContainerElement as Element;

        let mut map: HashMap<u32, Arc<MdfPinType>> = Default::default();

        for elem in container.elements.iter() {
            match elem {
                Element::PinType(pt) => {
                    map.insert(
                        pt.id,
                        Arc::new(MdfPinType {
                            id: pt.id,
                            eid: pt.eid.clone(),
                            short_name: pt.short_name.clone(),
                            description: pt.description.clone(),
                        }),
                    );
                }
                _ => {}
            }
        }

        map.into()
    }

    fn parse_io_controls(container: &Container) -> MdfIoControlWrapper {
        use raw_xml_types::ContainerElement as Element;

        let mut map: HashMap<u32, Arc<MdfIoControl>> = Default::default();

        for elem in container.elements.iter() {
            match elem {
                Element::IoControl(ic) => {
                    map.insert(
                        ic.id,
                        Arc::new(MdfIoControl {
                            id: ic.id,
                            eid: ic.eid.clone(),
                            short_name: ic.short_name.clone(),
                            description: ic.description.clone(),
                        }),
                    );
                }
                _ => {}
            }
        }

        map.into()
    }

    fn parse_com_params(container: &Container) -> MdfComParamWrapper {
        use raw_xml_types::ContainerElement as Element;

        let mut map: HashMap<u32, Arc<MdfComParam>> = Default::default();

        for elem in container.elements.iter() {
            match elem {
                Element::ComParam(cp) => {
                    let data_type: MdfComParamDataType = match cp.data_type.parse() {
                        Ok(v) => v,
                        Err(_) => {
                            debug!(
                                "ComParam #{} type has an invalid value: {:?}",
                                cp.id, cp.data_type
                            );
                            continue;
                        }
                    };
                    let class: MdfComParamClass = match cp.class.parse() {
                        Ok(v) => v,
                        Err(_) => {
                            debug!(
                                "ComParam #{} class has an invalid value: {:?}",
                                cp.id, cp.class
                            );
                            continue;
                        }
                    };
                    let layer: MdfComParamLayer = match cp.layer.parse() {
                        Ok(v) => v,
                        Err(_) => {
                            debug!(
                                "ComParam #{} layer has an invalid value: {:?}",
                                cp.id, cp.layer
                            );
                            continue;
                        }
                    };
                    let default_value = match MdfComParamValue::from_data_type_and_str(
                        cp.id,
                        &data_type,
                        &cp.default_value,
                    ) {
                        Ok(v) => v,
                        Err(_) => MdfComParamValue::Uncategorized(cp.default_value.clone()),
                    };
                    let min_value = match cp.min_value {
                        Some(ref value) => Some(
                            match MdfComParamValue::from_data_type_and_str(
                                cp.id,
                                &data_type,
                                value.trim(),
                            ) {
                                Ok(v) => v,
                                Err(_) => MdfComParamValue::Uncategorized(value.trim().to_owned()),
                            },
                        ),
                        None => None,
                    };
                    let max_value = match cp.max_value {
                        Some(ref value) => Some(
                            match MdfComParamValue::from_data_type_and_str(
                                cp.id,
                                &data_type,
                                value.trim(),
                            ) {
                                Ok(v) => v,
                                Err(_) => MdfComParamValue::Uncategorized(value.trim().to_owned()),
                            },
                        ),
                        None => None,
                    };

                    map.insert(
                        cp.id,
                        Arc::new(MdfComParam {
                            id: cp.id,
                            eid: cp.eid.clone(),
                            short_name: cp.short_name.clone(),
                            description: cp.description.clone(),
                            data_type,
                            default_value,
                            class,
                            layer,
                            min_value,
                            max_value,
                        }),
                    );
                }
                _ => {}
            }
        }

        map.into()
    }

    fn parse_bus_types(
        container: &Container,
        com_params: &MdfComParamWrapper,
    ) -> MdfBusTypeWrapper {
        use raw_xml_types::ContainerElement as Element;

        let mut map: HashMap<u32, Arc<MdfBusType>> = Default::default();

        for elem in container.elements.iter() {
            match elem {
                Element::BusType(bt) => {
                    let mut bus_com_params: HashMap<u32, Arc<MdfComParam>> = Default::default();

                    if let Some(ref params) = bt.com_params {
                        'p: for param in params.iter() {
                            let Some(com_param) = com_params.get_by_eid(&param.com_param_ref.to)
                            else {
                                debug!(
                                    "BusType #{} refers to an unknown COMPARAM: {}",
                                    bt.id, param.com_param_ref.to
                                );
                                continue 'p;
                            };
                            bus_com_params.insert(com_param.id, com_param.clone());
                        }
                    }

                    map.insert(
                        bt.id,
                        Arc::new(MdfBusType {
                            id: bt.id,
                            eid: bt.eid.clone(),
                            short_name: bt.short_name.clone(),
                            description: bt.description.clone(),
                            com_params: bus_com_params.into(),
                        }),
                    );
                }
                _ => {}
            }
        }

        map.into()
    }

    fn parse_protocols(
        container: &Container,
        com_params: &MdfComParamWrapper,
    ) -> MdfProtocolWrapper {
        use raw_xml_types::ContainerElement as Element;

        let mut map: HashMap<u32, Arc<MdfProtocol>> = Default::default();

        for elem in container.elements.iter() {
            match elem {
                Element::Protocol(p) => {
                    let mut protocol_com_params: HashMap<u32, Arc<MdfComParam>> =
                        Default::default();

                    if let Some(ref params) = p.com_params {
                        'p: for param in params.iter() {
                            let Some(com_param) = com_params.get_by_eid(&param.com_param_ref.to)
                            else {
                                debug!(
                                    "Protocol #{} refers to an unknown ComParam: {}",
                                    p.id, param.com_param_ref.to
                                );
                                continue 'p;
                            };
                            protocol_com_params.insert(com_param.id, com_param.clone());
                        }
                    }

                    map.insert(
                        p.id,
                        Arc::new(MdfProtocol {
                            id: p.id,
                            eid: p.eid.clone(),
                            short_name: p.short_name.clone(),
                            description: p.description.clone(),
                            com_params: protocol_com_params.into(),
                        }),
                    );
                }
                _ => {}
            }
        }

        map.into()
    }

    fn parse_resources(
        container: &Container,
        pin_types: &MdfPinTypeWrapper,
        bus_types: &MdfBusTypeWrapper,
        protocols: &MdfProtocolWrapper,
    ) -> MdfResourceWrapper {
        use raw_xml_types::ContainerElement as Element;

        let mut map: HashMap<u32, Arc<MdfResource>> = Default::default();

        'e: for elem in container.elements.iter() {
            match elem {
                Element::Resource(r) => {
                    let mut module_pins: HashMap<u8, MdfResourceModulePin> = Default::default();

                    if let Some(ref pins_on_module) = r.pins_on_module {
                        'p: for pin_on_module in pins_on_module.iter() {
                            let Some(pin_type) =
                                pin_types.get_by_eid(&pin_on_module.pin_type_ref.to)
                            else {
                                debug!(
                                    "Resource #{} refers to an unknown PinType: {}",
                                    r.id, pin_on_module.pin_type_ref.to
                                );
                                continue 'p;
                            };

                            module_pins.insert(
                                pin_on_module.pin_number,
                                MdfResourceModulePin {
                                    num: pin_on_module.pin_number,
                                    pin_type: pin_type.clone(),
                                },
                            );
                        }
                    }

                    let Some(bus_type) = bus_types.get_by_eid(&r.bus_type_ref.to) else {
                        debug!(
                            "Resource #{} refers to an unknown BusType: {}",
                            r.id, r.bus_type_ref.to
                        );
                        continue 'e;
                    };
                    let Some(protocol) = protocols.get_by_eid(&r.protocol_ref.to) else {
                        debug!(
                            "Resource #{} refers to an unknown Protocol: {}",
                            r.id, r.bus_type_ref.to
                        );
                        continue 'e;
                    };

                    map.insert(
                        r.id,
                        Arc::new(MdfResource {
                            id: r.id,
                            eid: r.eid.clone(),
                            short_name: r.short_name.clone(),
                            description: r.description.clone(),
                            module_pins: module_pins.into(),
                            bus_type: bus_type.clone(),
                            protocol: protocol.clone(),
                        }),
                    );
                }
                _ => {}
            }
        }

        map.into()
    }

    fn parse_module_types(
        container: &Container,
        io_controls: &MdfIoControlWrapper,
        resources: &MdfResourceWrapper,
    ) -> MdfModuleTypeWrapper {
        use raw_xml_types::ContainerElement as Element;

        let mut map: HashMap<u32, MdfModuleType> = Default::default();

        for elem in container.elements.iter() {
            match elem {
                Element::ModuleType(mt) => {
                    let mut module_type_resources: HashMap<u32, Arc<MdfResource>> =
                        Default::default();
                    let mut module_type_io_controls: HashMap<u32, Arc<MdfIoControl>> =
                        Default::default();

                    if let Some(ref resource_refs) = mt.resource_refs {
                        'r: for resource_ref in resource_refs.iter() {
                            let Some(resource) = resources.get_by_eid(&resource_ref.to) else {
                                debug!(
                                    "ModuleType #{} refers to an unknown Resource: {}",
                                    mt.id, resource_ref.to
                                );
                                continue 'r;
                            };
                            module_type_resources.insert(resource.id, resource.clone());
                        }
                    }

                    if let Some(ref io_control_refs) = mt.io_control_refs {
                        'i: for io_control_ref in io_control_refs.iter() {
                            let Some(io_control) = io_controls.get_by_eid(&io_control_ref.to)
                            else {
                                debug!(
                                    "ModuleType #{} refers to an unknown IoControl: {}",
                                    mt.id, io_control_ref.to
                                );
                                continue 'i;
                            };
                            module_type_io_controls.insert(io_control.id, io_control.clone());
                        }
                    }

                    map.insert(
                        mt.id,
                        MdfModuleType {
                            id: mt.id,
                            short_name: mt.short_name.clone(),
                            description: mt.description.clone(),
                            resources: module_type_resources.into(),
                            io_controls: module_type_io_controls.into(),
                        },
                    );
                }
                _ => {}
            }
        }

        map.into()
    }

    fn parse_error_codes(container: &Container) -> MdfErrorCodeWrapper {
        use raw_xml_types::ContainerElement as Element;

        let mut map: HashMap<u32, MdfErrorCode> = Default::default();

        for elem in container.elements.iter() {
            match elem {
                Element::ErrorCode(ec) => {
                    map.insert(
                        ec.id,
                        MdfErrorCode {
                            id: ec.id,
                            short_name: ec.short_name.clone(),
                            description: ec.description.clone(),
                        },
                    );
                }
                _ => {}
            }
        }

        map.into()
    }
}

#[derive(Debug)]
pub struct MdfPinType {
    pub id: u32,
    pub eid: String,
    pub short_name: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Default)]
pub struct MdfPinTypeWrapper {
    pub inner: HashMap<u32, Arc<MdfPinType>>,
}

impl From<HashMap<u32, Arc<MdfPinType>>> for MdfPinTypeWrapper {
    fn from(inner: HashMap<u32, Arc<MdfPinType>>) -> Self {
        MdfPinTypeWrapper { inner }
    }
}

impl MdfPinTypeWrapper {
    pub fn get_by_id(&self, id: u32) -> Option<&Arc<MdfPinType>> {
        self.inner.get(&id)
    }

    pub fn get_by_eid<S: Into<String>>(&self, eid: S) -> Option<&Arc<MdfPinType>> {
        let eid = eid.into();

        for (_, pin_type) in self.inner.iter() {
            if &pin_type.eid == &eid {
                return Some(pin_type);
            }
        }

        None
    }

    pub fn get_by_short_name<S: Into<String>>(&self, short_name: S) -> Option<&Arc<MdfPinType>> {
        let name = short_name.into();

        for (_, pin_type) in self.inner.iter() {
            if pin_type.short_name.as_ref() == Some(&name) {
                return Some(pin_type);
            }
        }

        None
    }
}

#[derive(Debug, Default)]
pub struct MdfModuleTypeWrapper {
    pub inner: HashMap<u32, MdfModuleType>,
}

impl From<HashMap<u32, MdfModuleType>> for MdfModuleTypeWrapper {
    fn from(inner: HashMap<u32, MdfModuleType>) -> Self {
        MdfModuleTypeWrapper { inner }
    }
}

impl MdfModuleTypeWrapper {
    pub fn get_by_id(&self, id: u32) -> Option<&MdfModuleType> {
        self.inner.get(&id)
    }

    pub fn get_by_short_name<S: Into<String>>(&self, short_name: S) -> Option<&MdfModuleType> {
        let name = short_name.into();

        for (_, module_type) in self.inner.iter() {
            if module_type.short_name.as_ref() == Some(&name) {
                return Some(module_type);
            }
        }

        None
    }
}

#[derive(Debug)]
pub struct MdfModuleType {
    pub id: u32,
    pub short_name: Option<String>,
    pub description: Option<String>,
    pub resources: MdfResourceWrapper,
    pub io_controls: MdfIoControlWrapper,
}

#[derive(Debug, Default)]
pub struct MdfResourceWrapper {
    pub inner: HashMap<u32, Arc<MdfResource>>,
}

impl From<HashMap<u32, Arc<MdfResource>>> for MdfResourceWrapper {
    fn from(inner: HashMap<u32, Arc<MdfResource>>) -> Self {
        MdfResourceWrapper { inner }
    }
}

impl MdfResourceWrapper {
    pub fn get_by_id(&self, id: u32) -> Option<&Arc<MdfResource>> {
        self.inner.get(&id)
    }

    pub fn get_by_eid<S: Into<String>>(&self, eid: S) -> Option<&Arc<MdfResource>> {
        let eid = eid.into();

        for (_, resource) in self.inner.iter() {
            if resource.eid == eid {
                return Some(resource);
            }
        }

        None
    }

    pub fn get_by_short_name<S: Into<String>>(&self, short_name: S) -> Option<&Arc<MdfResource>> {
        let name = short_name.into();

        for (_, resource) in self.inner.iter() {
            if resource.short_name.as_ref() == Some(&name) {
                return Some(resource);
            }
        }

        None
    }
}

#[derive(Debug)]
pub struct MdfResource {
    pub id: u32,
    pub eid: String,
    pub short_name: Option<String>,
    pub description: Option<String>,
    pub module_pins: HashMap<u8, MdfResourceModulePin>,
    pub bus_type: Arc<MdfBusType>,
    pub protocol: Arc<MdfProtocol>,
}

#[derive(Debug)]
pub struct MdfResourceModulePin {
    pub num: u8,
    pub pin_type: Arc<MdfPinType>,
}

#[derive(Debug)]
pub struct MdfBusType {
    pub id: u32,
    pub eid: String,
    pub short_name: Option<String>,
    pub description: Option<String>,
    pub com_params: MdfComParamWrapper,
}

#[derive(Debug, Default)]
pub struct MdfBusTypeWrapper {
    pub inner: HashMap<u32, Arc<MdfBusType>>,
}

impl From<HashMap<u32, Arc<MdfBusType>>> for MdfBusTypeWrapper {
    fn from(value: HashMap<u32, Arc<MdfBusType>>) -> Self {
        Self { inner: value }
    }
}

impl MdfBusTypeWrapper {
    pub fn get_by_id(&self, id: u32) -> Option<&Arc<MdfBusType>> {
        self.inner.get(&id)
    }

    pub fn get_by_short_name<S: Into<String>>(&self, short_name: S) -> Option<&Arc<MdfBusType>> {
        let name = short_name.into();

        for (_, bus_type) in self.inner.iter() {
            if bus_type.short_name.as_ref() == Some(&name) {
                return Some(bus_type);
            }
        }

        None
    }

    pub fn get_by_eid<S: Into<String>>(&self, eid: S) -> Option<&Arc<MdfBusType>> {
        let eid = eid.into();

        for (_, bus_type) in self.inner.iter() {
            if bus_type.eid == eid {
                return Some(bus_type);
            }
        }

        None
    }
}

#[derive(Debug, Default)]
pub struct MdfProtocolWrapper {
    pub inner: HashMap<u32, Arc<MdfProtocol>>,
}

impl From<HashMap<u32, Arc<MdfProtocol>>> for MdfProtocolWrapper {
    fn from(value: HashMap<u32, Arc<MdfProtocol>>) -> Self {
        Self { inner: value }
    }
}

impl MdfProtocolWrapper {
    pub fn get_by_id(&self, id: u32) -> Option<&Arc<MdfProtocol>> {
        self.inner.get(&id)
    }

    pub fn get_by_short_name<S: Into<String>>(&self, short_name: S) -> Option<&Arc<MdfProtocol>> {
        let name = short_name.into();

        for (_, protocol) in self.inner.iter() {
            if protocol.short_name.as_ref() == Some(&name) {
                return Some(protocol);
            }
        }

        None
    }

    pub fn get_by_eid<S: Into<String>>(&self, eid: S) -> Option<&Arc<MdfProtocol>> {
        let eid = eid.into();

        for (_, protocol) in self.inner.iter() {
            if protocol.eid == eid {
                return Some(protocol);
            }
        }

        None
    }
}

#[derive(Debug)]
pub struct MdfProtocol {
    pub id: u32,
    pub eid: String,
    pub short_name: Option<String>,
    pub description: Option<String>,
    pub com_params: MdfComParamWrapper,
}

#[derive(Debug, strum::EnumString)]
pub enum MdfComParamDataType {
    #[strum(serialize = "PDU_PT_UNUM8", serialize = "UNUM8")]
    UnsignedNum8, // UNUM8 aka u8

    #[strum(serialize = "PDU_PT_SNUM8", serialize = "SNUM8")]
    SignedNum8, // SNUM8 aka i8

    #[strum(serialize = "PDU_PT_UNUM16", serialize = "UNUM16")]
    UnsignedNum16, // UNUM16 aka u16

    #[strum(serialize = "PDU_PT_SNUM16", serialize = "SNUM16")]
    SignedNum16, // SNUM16 aka i16

    #[strum(serialize = "PDU_PT_UNUM32", serialize = "UNUM32")]
    UnsignedNum32, // UNUM32 aka u32

    #[strum(serialize = "PDU_PT_SNUM32", serialize = "SNUM32")]
    SignedNum32, // SNUM32 aka i32

    #[strum(serialize = "PDU_PT_BYTEFIELD")]
    ByteField,

    #[strum(serialize = "PDU_PT_STRUCTFIELD")]
    StructField,

    #[strum(serialize = "PDU_PT_LONGFIELD")]
    LongField,
}

#[derive(Debug, strum::EnumString)]
pub enum MdfComParamClass {
    #[strum(serialize = "PDU_PC_TIMING", serialize = "TIMING")]
    Timing,

    #[strum(serialize = "PDU_PC_INIT", serialize = "INIT")]
    Init,

    #[strum(serialize = "PDU_PC_COM", serialize = "COM")]
    Com,

    #[strum(serialize = "PDU_PC_ERRHDL", serialize = "ERRHDL")]
    ErrHdl,

    #[strum(serialize = "PDU_PC_BUSTYPE", serialize = "BUSTYPE")]
    BusType,

    #[strum(serialize = "PDU_PC_UNIQUE_ID", serialize = "UNIQUE_ID")]
    UniqueId,

    #[strum(serialize = "PDU_PC_TESTER_PRESENT", serialize = "TESTER_PRESENT")]
    TesterPresent,
}

#[derive(Debug, strum::EnumString)]
pub enum MdfComParamLayer {
    #[strum(
        serialize = "PDU_PL_APPLICATION",
        serialize = "APPLICATION",
        serialize = "APP"
    )]
    Application,

    #[strum(
        serialize = "PDU_PL_TRANSPORT",
        serialize = "TRANSPORT",
        serialize = "TRANS"
    )]
    Transport,

    #[strum(
        serialize = "PDU_PL_PHYSICAL",
        serialize = "PHYSICAL",
        serialize = "PHYS"
    )]
    Physical,
}

#[derive(Debug)]
pub enum MdfComParamValue {
    UnsignedNum8(u8),
    SignedNum8(i8),
    UnsignedNum16(u16),
    SignedNum16(i16),
    UnsignedNum32(u32),
    SignedNum32(i32),
    LongField {
        max_values: u8,
        actual_values: u8,
        data: Vec<u32>,
    },
    ByteField {
        max_values: u8,
        actual_values: u8,
        data: Vec<u8>,
    },
    StructField {
        raw: String, //max_values: u8,
                     //actual_values: u8,
                     //data: Vec<Vec<u32>>
    },
    Uncategorized(String),
}

#[derive(Debug, thiserror::Error)]
pub enum MdfComParamValueParseError {
    #[error("number parse error")]
    NumParseError,

    #[error("byte field parse error: {0}")]
    ByteFieldParseError(&'static str),

    #[error("long field parse error: {0}")]
    LongFieldParseError(&'static str),

    #[error("empty value")]
    EmptyValue,
}

impl MdfComParamValue {
    fn from_data_type_and_str(
        com_param_id: u32,
        com_param_data_type: &MdfComParamDataType,
        str: &str,
    ) -> Result<MdfComParamValue, MdfComParamValueParseError> {
        if str.is_empty() {
            debug!("ComParam #{com_param_id}: empty value");
            return Err(MdfComParamValueParseError::NumParseError);
        }

        let simple_error_mapper = |_kind| {
            debug!("ComParam #{com_param_id}: parse int value error: {str}");
            MdfComParamValueParseError::NumParseError
        };

        fn is_hex_string_repr(str: &str) -> bool {
            str.contains('x') || str.contains('X')
        }

        macro_rules! str_to_integer {
            ($str:expr, $ty:ty, $e_variant:expr) => {
                {
                    let (normalized_str, radix) = match is_hex_string_repr($str) {
                        true => ($str.chars().skip(2).collect::<String>(), 16),
                        false => ($str.to_owned(), 10)
                    };

                    <$ty>::from_str_radix(normalized_str.trim(), radix)
                        .map(|v| $e_variant(v))
                        .map_err(simple_error_mapper)
                }
            };
        }

        match com_param_data_type {
            MdfComParamDataType::UnsignedNum8 => {
                str_to_integer!(str, u8, MdfComParamValue::UnsignedNum8)
            }
            MdfComParamDataType::SignedNum8 => {
                str_to_integer!(str, i8, MdfComParamValue::SignedNum8)
            }
            MdfComParamDataType::UnsignedNum16 => {
                str_to_integer!(str, u16, MdfComParamValue::UnsignedNum16)
            }
            MdfComParamDataType::SignedNum16 => {
                str_to_integer!(str, i16, MdfComParamValue::SignedNum16)
            }
            MdfComParamDataType::UnsignedNum32 => {
                str_to_integer!(str, u32, MdfComParamValue::UnsignedNum32)
            }
            MdfComParamDataType::SignedNum32 => {
                str_to_integer!(str, i32, MdfComParamValue::SignedNum32)
            }
            MdfComParamDataType::ByteField => {
                // chapter F.2.2.3
                let entries = str.split(' ').collect::<Vec<&str>>();

                if entries.len() < 2 {
                    debug!("ComParam #{com_param_id}: too short byte field value: {str}");
                    return Err(MdfComParamValueParseError::ByteFieldParseError(
                        "Too short value",
                    ));
                }

                let mut max_bytes = 0;
                let mut actual_bytes = 0;
                let mut data: Vec<u8> = vec![];

                for (i, entry) in entries.iter().enumerate() {
                    let radix = if is_hex_string_repr(entry) { 16 } else { 10 };

                    if i == 0 {
                        let Ok(v) = u8::from_str_radix(entry, radix) else {
                            debug!(
                                "ComParam #{com_param_id}: malformed byte field max length: {str}"
                            );
                            return Err(MdfComParamValueParseError::ByteFieldParseError(
                                "Error of parsing the max length of byte array",
                            ));
                        };
                        max_bytes = v;
                    } else if i == 1 {
                        let Ok(v) = u8::from_str_radix(entry, radix) else {
                            debug!(
                                "ComParam #{com_param_id}: malformed byte field actual length: {str}"
                            );
                            return Err(MdfComParamValueParseError::ByteFieldParseError(
                                "Error of parsing the actual length of byte array",
                            ));
                        };
                        actual_bytes = v;
                    } else {
                        let Ok(v) = u8::from_str_radix(entry, radix) else {
                            debug!("ComParam #{com_param_id} malformed byte at pos {i}: {str}");
                            return Err(MdfComParamValueParseError::ByteFieldParseError(
                                "Malformed byte array",
                            ));
                        };
                        data.push(v);
                    }
                }

                Ok(MdfComParamValue::ByteField {
                    max_values: max_bytes,
                    actual_values: actual_bytes,
                    data,
                })
            }
            MdfComParamDataType::LongField => {
                // chapter F.2.2.4
                let entries = str.split(' ').collect::<Vec<&str>>();

                if entries.len() < 2 {
                    debug!("ComParam #{com_param_id}: too short long field value: {str}");
                    return Err(MdfComParamValueParseError::LongFieldParseError(
                        "Too short value",
                    ));
                }

                let mut max_values = 0;
                let mut actual_values = 0;
                let mut data: Vec<u32> = vec![];

                for (i, entry) in entries.iter().enumerate() {
                    let radix = if is_hex_string_repr(entry) { 16 } else { 10 };

                    if i == 0 {
                        let Ok(v) = u8::from_str_radix(entry, radix) else {
                            debug!(
                                "ComParam #{com_param_id}: malformed long array max length: {str}"
                            );
                            return Err(MdfComParamValueParseError::LongFieldParseError(
                                "Error of parsing the max length of long array",
                            ));
                        };
                        max_values = v;
                    } else if i == 1 {
                        let Ok(v) = u8::from_str_radix(entry, radix) else {
                            debug!(
                                "ComParam #{com_param_id}: malformed long array actual length: {str}"
                            );
                            return Err(MdfComParamValueParseError::LongFieldParseError(
                                "Error of parsing the max length of long array",
                            ));
                        };
                        actual_values = v;
                    } else {
                        let Ok(v) = u32::from_str_radix(entry, radix) else {
                            debug!("ComParam #{com_param_id} malformed long at pos {i}: {str}");
                            return Err(MdfComParamValueParseError::ByteFieldParseError(
                                "Malformed long array",
                            ));
                        };
                        data.push(v);
                    }
                }

                Ok(MdfComParamValue::LongField {
                    max_values,
                    actual_values,
                    data,
                })
            }
            MdfComParamDataType::StructField => Ok(MdfComParamValue::StructField {
                raw: str.to_owned(),
            }),
        }
    }
}

#[derive(Debug)]
pub struct MdfComParam {
    pub id: u32,
    pub eid: String,
    pub short_name: Option<String>,
    pub description: Option<String>,
    pub data_type: MdfComParamDataType,
    pub default_value: MdfComParamValue,
    pub class: MdfComParamClass,
    pub layer: MdfComParamLayer,
    pub min_value: Option<MdfComParamValue>,
    pub max_value: Option<MdfComParamValue>,
}

#[derive(Debug, Default)]
pub struct MdfComParamWrapper {
    pub inner: HashMap<u32, Arc<MdfComParam>>,
}

impl From<HashMap<u32, Arc<MdfComParam>>> for MdfComParamWrapper {
    fn from(inner: HashMap<u32, Arc<MdfComParam>>) -> Self {
        MdfComParamWrapper { inner }
    }
}

impl MdfComParamWrapper {
    pub fn get_by_id(&self, id: u32) -> Option<&Arc<MdfComParam>> {
        self.inner.get(&id)
    }

    pub fn get_by_eid<S: Into<String>>(&self, eid: S) -> Option<&Arc<MdfComParam>> {
        let eid = eid.into();

        for (_, com_param) in self.inner.iter() {
            if com_param.eid == eid {
                return Some(com_param);
            }
        }

        None
    }

    pub fn get_by_short_name<S: Into<String>>(&self, short_name: S) -> Option<&Arc<MdfComParam>> {
        let name = short_name.into();

        for (_, com_param) in self.inner.iter() {
            if com_param.short_name.as_ref() == Some(&name) {
                return Some(com_param);
            }
        }

        None
    }
}

#[derive(Debug)]
pub struct MdfIoControl {
    pub id: u32,
    pub eid: String,
    pub short_name: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Default)]
pub struct MdfIoControlWrapper {
    pub inner: HashMap<u32, Arc<MdfIoControl>>,
}

impl From<HashMap<u32, Arc<MdfIoControl>>> for MdfIoControlWrapper {
    fn from(inner: HashMap<u32, Arc<MdfIoControl>>) -> Self {
        MdfIoControlWrapper { inner }
    }
}

impl MdfIoControlWrapper {
    pub fn get_by_id(&self, id: u32) -> Option<&Arc<MdfIoControl>> {
        self.inner.get(&id)
    }

    pub fn get_by_eid<S: Into<String>>(&self, eid: S) -> Option<&Arc<MdfIoControl>> {
        let eid = eid.into();

        for (_, io_control) in self.inner.iter() {
            if io_control.eid == eid {
                return Some(io_control);
            }
        }

        None
    }

    pub fn get_by_short_name<S: Into<String>>(&self, short_name: S) -> Option<&Arc<MdfIoControl>> {
        let name = short_name.into();

        for (_, com_param) in self.inner.iter() {
            if com_param.short_name.as_ref() == Some(&name) {
                return Some(com_param);
            }
        }

        None
    }
}

#[derive(Debug)]
pub struct MdfErrorCode {
    id: u32,
    short_name: Option<String>,
    description: Option<String>,
}

#[derive(Debug, Default)]
pub struct MdfErrorCodeWrapper {
    pub inner: HashMap<u32, MdfErrorCode>,
}

impl From<HashMap<u32, MdfErrorCode>> for MdfErrorCodeWrapper {
    fn from(inner: HashMap<u32, MdfErrorCode>) -> Self {
        MdfErrorCodeWrapper { inner }
    }
}

impl MdfErrorCodeWrapper {
    pub fn get_by_id(&self, id: u32) -> Option<&MdfErrorCode> {
        self.inner.get(&id)
    }

    pub fn get_by_short_name<S: Into<String>>(&self, short_name: S) -> Option<&MdfErrorCode> {
        let short_name = short_name.into();

        for (_, error_code) in self.inner.iter() {
            if error_code.short_name.as_ref() == Some(&short_name) {
                return Some(error_code);
            }
        }

        None
    }
}

mod raw_xml_types {
    #![allow(missing_docs)]

    #[derive(Debug, serde::Deserialize)]
    #[serde(rename = "MVCI_MODULE_DESCRIPTION")]
    pub struct Container {
        #[serde(rename = "$value")]
        pub elements: Vec<ContainerElement>,
    }

    #[derive(Debug, serde::Deserialize)]
    pub struct PinType {
        #[serde(rename = "ID")]
        pub id: u32,

        #[serde(rename = "@EID")]
        pub eid: String,

        #[serde(rename = "SHORT_NAME")]
        pub short_name: Option<String>,

        #[serde(rename = "DESCRIPTION")]
        pub description: Option<String>,
    }

    #[derive(Debug, serde::Deserialize)]
    pub struct ModuleType {
        #[serde(rename = "ID")]
        pub id: u32,

        #[serde(rename = "SHORT_NAME")]
        pub short_name: Option<String>,

        #[serde(rename = "DESCRIPTION")]
        pub description: Option<String>,

        #[serde(rename = "IO_CTRL")]
        pub io_control_refs: Option<Vec<IoControlRef>>,

        #[serde(rename = "RESOURCE")]
        pub resource_refs: Option<Vec<ResourceRef>>,
    }

    #[derive(Debug, serde::Deserialize)]
    pub struct IoControlRef {
        #[serde(rename = "@IDREF")]
        pub to: String,
    }

    #[derive(Debug, serde::Deserialize)]
    pub struct ResourceRef {
        #[serde(rename = "@IDREF")]
        pub to: String,
    }

    #[derive(Debug, serde::Deserialize)]
    pub struct Resource {
        #[serde(rename = "ID")]
        pub id: u32,

        #[serde(rename = "@EID")]
        pub eid: String,

        #[serde(rename = "SHORT_NAME")]
        pub short_name: Option<String>,

        #[serde(rename = "DESCRIPTION")]
        pub description: Option<String>,

        #[serde(rename = "PIN_ON_MODULE")]
        pub pins_on_module: Option<Vec<PinOnModule>>,

        #[serde(rename = "BUSTYPE")]
        pub bus_type_ref: BusTypeRef,

        #[serde(rename = "PROTOCOL")]
        pub protocol_ref: ProtocolRef,
    }

    #[derive(Debug, serde::Deserialize)]
    pub struct BusTypeRef {
        #[serde(rename = "@IDREF")]
        pub to: String,
    }

    #[derive(Debug, serde::Deserialize)]
    pub struct ProtocolRef {
        #[serde(rename = "@IDREF")]
        pub to: String,
    }

    #[derive(Debug, serde::Deserialize)]
    pub struct PinOnModule {
        #[serde(rename = "PIN_ON_MODULE")]
        pub pin_number: u8,

        #[serde(rename = "PINTYPE")]
        pub pin_type_ref: PinTypeRef,
    }

    #[derive(Debug, serde::Deserialize)]
    pub struct PinTypeRef {
        #[serde(rename = "@IDREF")]
        pub to: String,
    }

    #[derive(Debug, serde::Deserialize)]
    pub struct Protocol {
        #[serde(rename = "ID")]
        pub id: u32,

        #[serde(rename = "@EID")]
        pub eid: String,

        #[serde(rename = "SHORT_NAME")]
        pub short_name: Option<String>,

        #[serde(rename = "DESCRIPTION")]
        pub description: Option<String>,

        #[serde(rename = "COMPARAM_REF")]
        pub com_params: Option<Vec<BusProtocolComParam>>,
    }

    #[derive(Debug, serde::Deserialize)]
    pub struct BusProtocolComParam {
        #[serde(rename = "DEFAULT_VALUE")]
        pub default_value: String,

        #[serde(rename = "COMPARAM")]
        pub com_param_ref: ComParamRef,
    }

    #[derive(Debug, serde::Deserialize)]
    pub struct ComParamRef {
        #[serde(rename = "@IDREF")]
        pub to: String,
    }

    #[derive(Debug, serde::Deserialize)]
    pub struct BusType {
        #[serde(rename = "ID")]
        pub id: u32,

        #[serde(rename = "@EID")]
        pub eid: String,

        #[serde(rename = "SHORT_NAME")]
        pub short_name: Option<String>,

        #[serde(rename = "DESCRIPTION")]
        pub description: Option<String>,

        #[serde(rename = "COMPARAM_REF")]
        pub com_params: Option<Vec<BusProtocolComParam>>,
    }

    #[derive(Debug, serde::Deserialize)]
    pub struct IoControl {
        #[serde(rename = "ID")]
        pub id: u32,

        #[serde(rename = "@EID")]
        pub eid: String,

        #[serde(rename = "SHORT_NAME")]
        pub short_name: Option<String>,

        #[serde(rename = "DESCRIPTION")]
        pub description: Option<String>,
    }

    #[derive(Debug, serde::Deserialize)]
    pub struct ComParam {
        #[serde(rename = "ID")]
        pub id: u32,

        #[serde(rename = "@EID")]
        pub eid: String,

        #[serde(rename = "SHORT_NAME")]
        pub short_name: Option<String>,

        #[serde(rename = "DESCRIPTION")]
        pub description: Option<String>,

        #[serde(rename = "DATA_TYPE")]
        pub data_type: String,

        #[serde(rename = "DEFAULT_VALUE")]
        pub default_value: String,

        #[serde(rename = "CLASS")]
        pub class: String,

        #[serde(rename = "LAYER")]
        pub layer: String,

        #[serde(rename = "MIN_VALUE")]
        pub min_value: Option<String>,

        #[serde(rename = "MAX_VALUE")]
        pub max_value: Option<String>,
    }

    #[derive(Debug, serde::Deserialize)]
    pub struct ErrorCode {
        #[serde(rename = "ID")]
        pub id: u32,

        #[serde(rename = "SHORT_NAME")]
        pub short_name: Option<String>,

        #[serde(rename = "DESCRIPTION")]
        pub description: Option<String>,
    }

    #[derive(Debug, serde::Deserialize)]
    pub enum ContainerElement {
        #[serde(rename = "DESCRIPTION")]
        Description(String),

        #[serde(rename = "SUPPLIER_NAME")]
        SupplierName(String),

        #[serde(rename = "PINTYPE")]
        PinType(PinType),

        #[serde(rename = "MODULETYPE")]
        ModuleType(ModuleType),

        #[serde(rename = "RESOURCE")]
        Resource(Resource),

        #[serde(rename = "PROTOCOL")]
        Protocol(Protocol),

        #[serde(rename = "BUSTYPE")]
        BusType(BusType),

        #[serde(rename = "IO_CTRL")]
        IoControl(IoControl),

        #[serde(rename = "COMPARAM")]
        ComParam(ComParam),

        #[serde(rename = "ERROR_CODE")]
        ErrorCode(ErrorCode),
    }
}
