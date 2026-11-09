use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

const SETTINGS_FILE_NAME: &str = "ui-settings.toml";
const APPLICATION_CONFIG_DIRECTORY: &str = "native-system-monitor";

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ByteUnitSystem {
    Si,
    #[default]
    Iec,
}

impl ByteUnitSystem {
    pub fn from_selection(value: &str) -> Option<Self> {
        match value {
            "si" => Some(Self::Si),
            "iec" => Some(Self::Iec),
            _ => None,
        }
    }

    pub fn code(self) -> u8 {
        match self {
            Self::Si => 1,
            Self::Iec => 2,
        }
    }

    pub fn from_code(code: u8) -> Self {
        match code {
            1 => Self::Si,
            _ => Self::Iec,
        }
    }

    pub fn base(self) -> f64 {
        match self {
            Self::Si => 1_000.0,
            Self::Iec => 1_024.0,
        }
    }

    pub fn unit_labels(self) -> [&'static str; 3] {
        match self {
            Self::Si => ["kB", "MB", "GB"],
            Self::Iec => ["KiB", "MiB", "GiB"],
        }
    }

    pub fn process_memory_unit(self) -> &'static str {
        self.unit_labels()[1]
    }

    pub fn ui_value(self) -> crate::UiByteUnitSystem {
        match self {
            Self::Si => crate::UiByteUnitSystem::Si,
            Self::Iec => crate::UiByteUnitSystem::Iec,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum DensityPreference {
    #[default]
    Comfortable,
    Compact,
}

impl DensityPreference {
    pub fn from_selection(value: &str) -> Option<Self> {
        match value {
            "comfortable" => Some(Self::Comfortable),
            "compact" => Some(Self::Compact),
            _ => None,
        }
    }

    pub fn ui_value(self) -> crate::UiDensity {
        match self {
            Self::Comfortable => crate::UiDensity::Comfortable,
            Self::Compact => crate::UiDensity::Compact,
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct UiSettings {
    pub byte_unit_system: ByteUnitSystem,
    pub density: DensityPreference,
}

impl UiSettings {
    pub fn load(path: &Path) -> Result<Self> {
        let contents = match fs::read_to_string(path) {
            Ok(contents) => contents,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self::default());
            }
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("read desktop preferences from {}", path.display()));
            }
        };

        toml::from_str(&contents)
            .with_context(|| format!("parse desktop preferences from {}", path.display()))
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!("create desktop preferences directory {}", parent.display())
            })?;
        }

        let contents = toml::to_string_pretty(self).context("serialize desktop preferences")?;
        fs::write(path, contents)
            .with_context(|| format!("write desktop preferences to {}", path.display()))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ConfigPlatform {
    #[cfg(any(target_os = "linux", test))]
    Linux,
    #[cfg(any(target_os = "macos", test))]
    MacOs,
    #[cfg(any(target_os = "windows", test))]
    Windows,
}

fn config_file_path(
    platform: ConfigPlatform,
    xdg_config_home: Option<&Path>,
    home: Option<&Path>,
    appdata: Option<&Path>,
) -> Option<PathBuf> {
    let _ = (xdg_config_home, home, appdata);
    let config_directory = match platform {
        #[cfg(any(target_os = "linux", test))]
        ConfigPlatform::Linux => xdg_config_home
            .map(Path::to_path_buf)
            .or_else(|| home.map(|path| path.join(".config")))?,
        #[cfg(any(target_os = "macos", test))]
        ConfigPlatform::MacOs => home?.join("Library").join("Application Support"),
        #[cfg(any(target_os = "windows", test))]
        ConfigPlatform::Windows => appdata?.to_path_buf(),
    };

    Some(
        config_directory
            .join(APPLICATION_CONFIG_DIRECTORY)
            .join(SETTINGS_FILE_NAME),
    )
}

pub fn default_settings_path() -> Option<PathBuf> {
    let xdg_config_home = std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from);
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let appdata = std::env::var_os("APPDATA").map(PathBuf::from);

    #[cfg(target_os = "windows")]
    let platform = ConfigPlatform::Windows;
    #[cfg(target_os = "macos")]
    let platform = ConfigPlatform::MacOs;
    #[cfg(all(unix, not(target_os = "macos")))]
    let platform = ConfigPlatform::Linux;

    config_file_path(
        platform,
        xdg_config_home.as_deref(),
        home.as_deref(),
        appdata.as_deref(),
    )
}

#[cfg(test)]
mod tests {
    use super::{ByteUnitSystem, ConfigPlatform, DensityPreference, UiSettings, config_file_path};
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn defaults_to_iec_and_comfortable_and_parses_supported_ui_choices() {
        assert_eq!(ByteUnitSystem::default(), ByteUnitSystem::Iec);
        assert_eq!(DensityPreference::default(), DensityPreference::Comfortable);
        assert_eq!(
            ByteUnitSystem::from_selection("si"),
            Some(ByteUnitSystem::Si)
        );
        assert_eq!(
            ByteUnitSystem::from_selection("iec"),
            Some(ByteUnitSystem::Iec)
        );
        assert_eq!(ByteUnitSystem::from_selection("SI"), None);
        assert_eq!(ByteUnitSystem::from_selection("other"), None);
        assert_eq!(
            DensityPreference::from_selection("comfortable"),
            Some(DensityPreference::Comfortable)
        );
        assert_eq!(
            DensityPreference::from_selection("compact"),
            Some(DensityPreference::Compact)
        );
        assert_eq!(DensityPreference::from_selection("extra-compact"), None);
        assert_eq!(ByteUnitSystem::from_code(1), ByteUnitSystem::Si);
        assert_eq!(ByteUnitSystem::from_code(99), ByteUnitSystem::Iec);
        assert_eq!(
            DensityPreference::Comfortable.ui_value(),
            crate::UiDensity::Comfortable
        );
        assert_eq!(
            DensityPreference::Compact.ui_value(),
            crate::UiDensity::Compact
        );
    }

    #[test]
    fn settings_round_trip_through_a_local_file() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock is after the Unix epoch")
            .as_nanos();
        let test_directory = std::env::temp_dir().join(format!(
            "native-monitor-ui-settings-{}-{unique}",
            std::process::id()
        ));
        let settings_path = test_directory.join("nested").join("ui-settings.toml");
        let settings = UiSettings {
            byte_unit_system: ByteUnitSystem::Si,
            density: DensityPreference::Compact,
        };

        settings
            .save(&settings_path)
            .expect("save the selected unit system");
        let serialized =
            std::fs::read_to_string(&settings_path).expect("read the serialized preferences file");
        assert!(serialized.contains("byte_unit_system = \"si\""));
        assert!(serialized.contains("density = \"compact\""));

        assert_eq!(
            UiSettings::load(&settings_path).expect("read back persisted preferences"),
            settings
        );
        std::fs::remove_dir_all(&test_directory).expect("remove test preferences directory");
    }

    #[test]
    fn missing_or_invalid_settings_fall_back_without_accepting_unknown_fields() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock is after the Unix epoch")
            .as_nanos();
        let test_directory = std::env::temp_dir().join(format!(
            "native-monitor-ui-settings-invalid-{}-{unique}",
            std::process::id()
        ));
        let settings_path = test_directory.join("ui-settings.toml");

        assert_eq!(
            UiSettings::load(&settings_path).expect("missing settings use defaults"),
            UiSettings::default()
        );
        let previous_settings: UiSettings =
            toml::from_str("byte_unit_system = \"si\"\n").expect("parse the previous schema");
        assert_eq!(previous_settings.byte_unit_system, ByteUnitSystem::Si);
        assert_eq!(previous_settings.density, DensityPreference::Comfortable);
        std::fs::create_dir_all(&test_directory).expect("create test preferences directory");
        std::fs::write(&settings_path, "unknown = true\n").expect("write malformed preferences");
        assert!(UiSettings::load(&settings_path).is_err());
        std::fs::remove_dir_all(&test_directory).expect("remove test preferences directory");
    }

    #[test]
    fn settings_path_uses_each_platforms_user_configuration_directory() {
        assert_eq!(
            config_file_path(
                ConfigPlatform::Linux,
                Some(Path::new("/user/config")),
                Some(Path::new("/user/home")),
                None,
            ),
            Some(PathBuf::from(
                "/user/config/native-system-monitor/ui-settings.toml"
            ))
        );
        assert_eq!(
            config_file_path(
                ConfigPlatform::Linux,
                None,
                Some(Path::new("/user/home")),
                None,
            ),
            Some(PathBuf::from(
                "/user/home/.config/native-system-monitor/ui-settings.toml"
            ))
        );
        assert_eq!(
            config_file_path(
                ConfigPlatform::MacOs,
                None,
                Some(Path::new("/user/home")),
                None,
            ),
            Some(PathBuf::from(
                "/user/home/Library/Application Support/native-system-monitor/ui-settings.toml"
            ))
        );
        assert_eq!(
            config_file_path(
                ConfigPlatform::Windows,
                None,
                None,
                Some(Path::new("C:/Users/Example/AppData/Roaming")),
            ),
            Some(PathBuf::from(
                "C:/Users/Example/AppData/Roaming/native-system-monitor/ui-settings.toml"
            ))
        );
        assert_eq!(
            config_file_path(ConfigPlatform::Linux, None, None, None),
            None
        );
    }
}
