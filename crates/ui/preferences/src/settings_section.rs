use gpui_kit::assets::IconName;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum SettingsSection {
    #[default]
    Vault,
    Sync,
    Ai,
    Language,
    Appearance,
    ExportVault,
    ExportSettings,
    ImportSettings,
    Updates,
}

impl SettingsSection {
    pub(crate) const fn title_key(self) -> &'static str {
        match self {
            Self::Vault => "settings_vault_title",
            Self::Sync => "settings_s3_title",
            Self::Ai => "settings_ai_title",
            Self::Language => "settings_language",
            Self::Appearance => "appearance_title",
            Self::ExportVault => "settings_export_zip_title",
            Self::ExportSettings => "settings_export_title",
            Self::ImportSettings => "settings_import_title",
            Self::Updates => "settings_updates_title",
        }
    }

    pub(crate) const fn icon(self) -> IconName {
        match self {
            Self::Vault => IconName::KeyRound,
            Self::Sync => IconName::Cloud,
            Self::Ai => IconName::Sparkles,
            Self::Language => IconName::Globe,
            Self::Appearance => IconName::Palette,
            Self::ExportVault => IconName::Archive,
            Self::ExportSettings => IconName::Upload,
            Self::ImportSettings => IconName::Download,
            Self::Updates => IconName::ArrowDownToLine,
        }
    }
}
