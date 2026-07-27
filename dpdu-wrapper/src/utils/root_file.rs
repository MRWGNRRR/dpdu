use self::raw_xml_types::Container;
use cfg_if::cfg_if;
use regex::Regex;
use semver::Version;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use std::time::Instant;
use tracing::{error, info, warn};

use crate::utils::{get_bomless_file_reader, get_winreg_arch_flags, HKEY_LM_REG_KEY};
pub use semver::VersionReq;

#[derive(Debug, thiserror::Error)]
pub enum PduRootFileError {
    #[error("io error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("xml parse error: {0}")]
    XmlParseError(#[from] quick_xml::de::DeError),

    #[error("failed to guess a path to the D-PDU API Root file")]
    GuessError,
}

/// Parsed structure of the D-PDU API root file.
#[derive(Debug, Clone, Default)]
pub struct PduRootFile {
    pub path: Option<PathBuf>,
    pub mvci_list: Vec<Mvci>,
}

impl PduRootFile {
    /// Creates new and empty [`PduRootFile`].
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the latest verion of requested MVCI.
    ///
    /// If you want to find version ^1.20, call this function as follows:
    ///
    /// ```
    /// root_file.get_mvci_by_short_name_and_version_req("EDIC_D_PDU_API", VersionReq::parse("^1.20").unwrap())
    /// ```
    ///
    /// Or to find the latest version:
    /// ```
    /// root_file.get_mvci_by_short_name_and_version_req("EDIC_D_PDU_API", VersionReq::STAR)
    /// ```
    ///
    /// Currently this applies only to the following MVCIs:
    ///   - EDIC_D_PDU_API_1_20_037
    ///   - XS_D_PDU_API_FOR_DTS_8_16_015
    ///   - Or any MVCI whose name contains a version in the `major_minor_patch` format
    pub fn get_mvci_by_short_name_and_version_req(
        &self,
        short_name: &str,
        version_req: VersionReq,
    ) -> Option<&Mvci> {
        static REGEX: LazyLock<Regex> =
            LazyLock::new(|| Regex::new(r#"(?<major>\d+)_(?<minor>\d+)_(?<patch>\d+)"#).unwrap());

        self.mvci_list
            .iter()
            .filter_map(|mvci| {
                let mvci_short_name = mvci.short_name.as_ref()?;
                if !mvci_short_name.starts_with(short_name) {
                    return None;
                }

                if let Some(caps) = REGEX.captures(mvci_short_name) {
                    let major = (&caps["major"]).parse::<u64>().ok()?;
                    let minor = (&caps["minor"]).parse::<u64>().ok()?;
                    let patch = (&caps["patch"]).parse::<u64>().ok()?;

                    let version = Version::new(major, minor, patch);
                    if version_req.matches(&version) {
                        Some((version, mvci))
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .max_by(|a, b| a.0.cmp(&b.0))
            .map(|(_, mvci)| mvci)
    }

    /// Returns [`Mvci`] by short name.
    pub fn get_mvci_by_short_name(&self, short_name: &str) -> Option<&Mvci> {
        self.mvci_list
            .iter()
            .find(|v| v.short_name.as_deref() == Some(short_name))
    }

    /// Attempts to guess a path to the D-PDU API root file and parse it.
    ///
    /// Otherwise, returns [`PduRootFileError::GuessError`].
    pub fn guess_and_parse() -> Result<Option<Self>, PduRootFileError> {
        info!("An attempt to guess a path to the D-PDU API root file...");

        let path: Option<PathBuf> = Self::lookup_root_file_path_in_windows_registry()
            .map(|opt| opt.or_else(Self::lookup_root_file_path_in_typical_places))?;

        if let Some(path) = path {
            info!(
                "D-PDU API root file path has been guessed: {}",
                path.display()
            );
            return Self::parse_from_xml_file(path).map(|v| Some(v));
        } else {
            error!("Unable to guess info about the D-PDU API root file");
        }

        Ok(None)
    }

    /// Searches for a path to the D-PDU API root file in typical places.
    pub fn lookup_root_file_path_in_typical_places() -> Option<PathBuf> {
        info!("Searching for the D-PDU API root file in the typical places...");

        cfg_if! {
            if #[cfg(target_arch = "x86_64")] {
                const PATHS: [&'static str; 1] = [
                    "C:\\Program Files\\D-PDU API\\pdu_api_root_x64.xml"
                ];
            } else if #[cfg(target_arch = "x86")] {
                const PATHS: [&'static str; 1] = [
                    "C:\\Program Files (x86)\\D-PDU API\\pdu_api_root.xml"
                ];
            } else {
                compile_error!("Unsupported target architecture");
            }
        }

        for path in PATHS {
            let path = PathBuf::from(path);
            if path.exists() && path.is_file() {
                info!(
                    "Found the D-PDU API root file in a typical place: {}",
                    path.display()
                );
                return Some(path);
            }
        }

        error!("The D-PDU API root file was not found in any typical place");
        None
    }

    /// Searches for a path to the D-PDU API root file via the Windows registry.
    pub fn lookup_root_file_path_in_windows_registry() -> Result<Option<PathBuf>, PduRootFileError>
    {
        use winreg::RegKey;
        use winreg::enums::{self, HKEY_LOCAL_MACHINE};

        const WINREG_PATH: &'static str = "SOFTWARE\\D-PDU API";
        const WINREG_KEY: &'static str = "Root File";

        info!("Reading a path of the D-PDU API root file through the Windows registry...");

        let path = HKEY_LM_REG_KEY.open_subkey_with_flags(WINREG_PATH, get_winreg_arch_flags())
            .inspect_err(|err| error!(path = WINREG_PATH, "Registry path cannot be opened: {err:?}"))
            .map(|sub_key| sub_key.get_value::<String, _>(WINREG_KEY))
            .inspect_err(|err| error!(path = WINREG_PATH, key = WINREG_KEY, "Registry value cannot be read: {err:?}"))?
            .map(|path| Some(PathBuf::from(path)))?
            .inspect(|path| {
                info!("D-PDU API root file path obtained from the Windows registry: {}", path.display())
            })
            .filter(|path| {
                let res = path.exists() && path.is_file();
                if !res {
                    error!(path = %path.display(), "D-PDU API root file path has been read, but it is invalid");
                }
                res
            });

        // This isn't a mistake — it's by design.
        if path.is_none() {
            error!(
                "Unable to retrieve info about the D-PDU API root file via the Windows Registry"
            );
        }

        Ok(path)
    }

    /// Parses the D-PDU API root file at the specified path.
    pub fn parse_from_xml_file<P>(path: P) -> Result<Self, PduRootFileError>
    where
        P: AsRef<Path>,
    {
        let path = path.as_ref();

        info!(path = %path.display(), "Parsing the D-PDU API root file...");
        let start = Instant::now();

        let mut reader = get_bomless_file_reader(path).inspect_err(|err| {
            error!(
                path = %path.display(),
                "Failed to obtain a BOM-less file reader for the D-PDU API root file: {err:?}"
            )
        })?;

        let container: Container = quick_xml::de::from_reader(&mut reader).inspect_err(|err| {
            error!(
                path = %path.display(),
                "Failed to parse the D-PDU API root file using quick-xml: {err:?}"
            )
        })?;

        // Auxiliary set used to eliminate duplicate libraries.
        let mut library_set = HashSet::with_capacity(container.elements.len());

        // Vector of normalized <MVCI_PDU_API> XML elements.
        let mut mvci_list = Vec::with_capacity(container.elements.len());

        for element in container.elements.iter() {
            let Some(library_file) = element.library_file.as_ref().map(|v| {
                // Some D-PDU API library paths may start with:
                //   - file:/
                //   - file://
                //   - file:///
                PathBuf::from(
                    v.path
                        .clone()
                        .trim_start_matches("file:") // remove the "file:" prefix
                        .trim_start_matches("/"), // and any leading slashes that follow it
                )
            }) else {
                continue;
            };

            // Library is already present in `library_set`.
            if library_set.contains(&library_file) {
                continue;
            }

            // Such a library does not exist.
            if !library_file.exists() || !library_file.is_file() {
                warn!(
                    root_file = %path.display(),
                    library_file = %library_file.display(),
                    "D-PDU API library file does not exist",
                );
                continue;
            }

            // If the library cannot be safely loaded into the current process address space.
            if !can_load_library(library_file.as_path()) {
                continue;
            }

            let mdf = element
                .module_description_file
                .as_ref()
                .map(|v| PathBuf::from(v.path.trim_start_matches("file:").trim_start_matches("/")));

            let cbf = element
                .cable_description_file
                .as_ref()
                .map(|v| PathBuf::from(v.path.trim_start_matches("file:").trim_start_matches("/")));

            library_set.insert(library_file.clone());

            mvci_list.push(Mvci {
                short_name: element.short_name.clone(),
                description: element.description.clone(),
                supplier_name: element.supplier_name.clone(),
                library_file,
                module_description_file: mdf,
                cable_description_file: cbf,
            });
        }

        info!(
            path = %path.display(),
            elapsed_ms = start.elapsed().as_millis(),
            "Parsing the D-PDU API root file is complete."
        );

        Ok(Self {
            path: Some(path.to_path_buf()),
            mvci_list,
        })
    }
}

/// [`MVCI_PDU_API`] structure from the D-PDU API root file.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Mvci {
    pub short_name: Option<String>,

    pub description: Option<String>,

    pub supplier_name: Option<String>,

    pub library_file: PathBuf,

    pub module_description_file: Option<PathBuf>,

    pub cable_description_file: Option<PathBuf>,
}

/// Checks if the specified library can be safely loaded into process memory.
///
/// The check is based on:
///
///   - the existence of PE/NT library headers
///   - matching the target architectures of the current application and the library
fn can_load_library(path: &Path) -> bool {
    use exe::{ImageFileMachine, NTHeaders, PE, VecPE};

    let pe = match VecPE::from_disk_file(path) {
        Ok(v) => v,
        Err(err) => {
            error!(path = %path.display(),"DLL cannot be safely loaded: {err}");
            return false;
        }
    };

    let nt_headers = match pe.get_valid_nt_headers() {
        Ok(v) => v,
        Err(err) => {
            error!(path = %path.display(),"DLL cannot be safely loaded: {err}");
            return false;
        }
    };

    cfg_if! {
        if #[cfg(target_arch = "x86_64")] {
            let target_arch = ImageFileMachine::AMD64 as u16;
        } else if #[cfg(target_arch = "x86")] {
            let target_arch = ImageFileMachine::I386 as u16;
        } else {
            compile_error!("Unsupported target architecture");
        }
    }

    match nt_headers {
        NTHeaders::NTHeaders32(h) if h.file_header.machine == target_arch => {}
        NTHeaders::NTHeaders64(h) if h.file_header.machine == target_arch => {}
        _ => {
            error!(
                path = %path.display(),
                "DLL cannot be safely loaded: system arch != library arch",
            );
            return false;
        }
    }

    true
}

mod raw_xml_types {
    #![allow(missing_docs)]

    #[derive(Debug, serde::Deserialize)]
    #[serde(rename = "MVCI_PDU_API_ROOT")]
    pub struct Container {
        #[serde(rename = "$value")]
        pub elements: Vec<Item>,
    }

    #[derive(Debug, serde::Deserialize)]
    #[serde(rename = "MVCI_PDU_API")]
    pub struct Item {
        #[serde(rename = "SHORT_NAME")]
        pub short_name: Option<String>,

        #[serde(rename = "DESCRIPTION")]
        pub description: Option<String>,

        #[serde(rename = "SUPPLIER_NAME")]
        pub supplier_name: Option<String>,

        #[serde(rename = "LIBRARY_FILE")]
        pub library_file: Option<FileRef>,

        #[serde(rename = "MODULE_DESCRIPTION_FILE")]
        pub module_description_file: Option<FileRef>,

        #[serde(rename = "CABLE_DESCRIPTION_FILE")]
        pub cable_description_file: Option<FileRef>,
    }

    #[derive(Debug, serde::Deserialize)]
    pub struct FileRef {
        #[serde(rename = "@URI")]
        pub path: String,
    }
}
