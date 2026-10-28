use std::path::Path;

use margin_vaults::{normalise_slashes, profile_name_from_vault_path};

use crate::steps::Steps;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum WizardStep {
    Folder,
    Name,
    Passphrase,
}

impl WizardStep {
    const ALL: [Self; 3] = [Self::Folder, Self::Name, Self::Passphrase];
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Wizard {
    steps: Steps,
    folder: Option<String>,
}

impl Default for Wizard {
    fn default() -> Self {
        Self {
            steps: Steps::new(WizardStep::ALL.len()),
            folder: None,
        }
    }
}

impl Wizard {
    pub(crate) const fn steps(&self) -> Steps {
        self.steps
    }

    pub(crate) const fn step(&self) -> WizardStep {
        WizardStep::ALL[self.steps.current()]
    }

    pub(crate) fn folder(&self) -> Option<&str> {
        self.folder.as_deref()
    }

    pub(crate) fn choose_folder(&mut self, path: &Path) -> &str {
        self.folder
            .insert(normalise_slashes(&path.to_string_lossy()))
    }

    pub(crate) const fn can_advance(&self) -> bool {
        self.folder.is_some()
    }

    pub(crate) fn go_to(&mut self, step: usize) -> bool {
        (step == 0 || self.can_advance()) && self.steps.go_to(step)
    }

    pub(crate) fn next(&mut self) -> bool {
        self.go_to(self.steps.current() + 1)
    }

    pub(crate) const fn previous(&mut self) -> bool {
        self.steps.previous()
    }
}

pub(crate) fn profile_name(typed: &str, folder: &str) -> String {
    match typed.trim() {
        "" => profile_name_from_vault_path(folder),
        name => name.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_folder_is_required_before_leaving_the_first_step() {
        let mut wizard = Wizard::default();
        assert_eq!(wizard.step(), WizardStep::Folder);
        assert!(!wizard.next());
        assert!(!wizard.go_to(2));
        assert_eq!(wizard.step(), WizardStep::Folder);

        assert_eq!(
            wizard.choose_folder(Path::new("C:\\Users\\me\\Notes")),
            "C:/Users/me/Notes"
        );
        assert!(wizard.next());
        assert_eq!(wizard.step(), WizardStep::Name);
        assert!(wizard.next());
        assert_eq!(wizard.step(), WizardStep::Passphrase);
        assert!(!wizard.next());
    }

    #[test]
    fn going_back_keeps_the_chosen_folder() {
        let mut wizard = Wizard::default();
        wizard.choose_folder(Path::new("/home/me/Notes"));
        assert!(wizard.go_to(2));
        assert!(wizard.previous());
        assert!(wizard.previous());
        assert!(!wizard.previous());
        assert_eq!(wizard.step(), WizardStep::Folder);
        assert_eq!(wizard.folder(), Some("/home/me/Notes"));
        assert!(wizard.go_to(0));
    }

    #[test]
    fn names_the_profile_after_the_folder_when_left_blank() {
        assert_eq!(profile_name("  Work  ", "/home/me/Notes"), "Work");
        assert_eq!(profile_name("   ", "/home/me/Notes"), "Notes");
        assert_eq!(profile_name("", "/"), "Vault");
    }
}
