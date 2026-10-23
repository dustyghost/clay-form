use super::*;

/// A configurable temporary-mode chord. Empty means disabled.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct HoldBinding {
    pub key: Option<String>,
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub command: bool,
}
impl HoldBinding {
    pub fn visibility_default() -> Self {
        Self {
            ctrl: true,
            shift: true,
            ..Self::default()
        }
    }
    pub fn active(&self, input: &egui::InputState) -> bool {
        input.focused
            && self.enabled()
            && (!self.ctrl || input.modifiers.ctrl || input.modifiers.command)
            && (!self.shift || input.modifiers.shift)
            && (!self.alt || input.modifiers.alt)
            && (!self.command || input.modifiers.mac_cmd)
            && self
                .key
                .as_ref()
                .is_none_or(|key| Key::from_name(key).is_some_and(|key| input.key_down(key)))
    }
    pub fn enabled(&self) -> bool {
        self.key.is_some() || self.ctrl || self.shift || self.alt || self.command
    }
    pub fn label(&self) -> String {
        if !self.enabled() {
            return "Unbound".into();
        }
        let mut label = Shortcut {
            key: self.key.clone().unwrap_or_default(),
            ctrl: self.ctrl,
            shift: self.shift,
            alt: self.alt,
            command: self.command,
        }
        .label();
        while label.ends_with('+') {
            label.pop();
        }
        label
    }
    pub fn validate(&self) -> Result<(), String> {
        if self
            .key
            .as_ref()
            .is_some_and(|key| key == "Escape" || Key::from_name(key).is_none())
        {
            return Err(
                "Temporary modes require a valid key; Escape is reserved for cancellation.".into(),
            );
        }
        Ok(())
    }
    /// Ignore only this mode's held modifiers when matching its menu invocation.
    pub fn matches_shortcut(&self, event: &Shortcut, binding: &Shortcut) -> bool {
        event.key == binding.key
            && (event.ctrl == binding.ctrl || (self.ctrl && event.ctrl && !binding.ctrl))
            && (event.shift == binding.shift || (self.shift && event.shift && !binding.shift))
            && (event.alt == binding.alt || (self.alt && event.alt && !binding.alt))
            && (event.command == binding.command
                || ((self.command || self.ctrl) && event.command && !binding.command))
    }
}
