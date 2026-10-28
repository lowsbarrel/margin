#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PropertiesEvent {
    Changed(String),
    OpenWikiLink(String),
    OpenUrl(String),
    TagSelected(String),
    FocusBody,
    FocusTitle,
}
