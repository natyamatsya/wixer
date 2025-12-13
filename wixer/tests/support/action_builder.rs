pub struct ActionBuilder {
    action: wixer::simulation::Action,
}

impl ActionBuilder {
    pub fn new_custom(id: impl Into<String>, ca_type: wixer::simulation::CustomActionType) -> Self {
        Self {
            action: wixer::simulation::Action::new_custom(id, ca_type),
        }
    }

    pub fn dll_binary(
        id: impl Into<String>,
        dll_entry: impl Into<String>,
        binary_ref: impl Into<String>,
    ) -> Self {
        Self::new_custom(
            id,
            wixer::simulation::CustomActionType::DllBinary {
                dll_entry: dll_entry.into(),
                binary_ref: binary_ref.into(),
            },
        )
    }

    pub fn condition(mut self, condition: impl Into<String>) -> Self {
        self.action.condition = Some(condition.into());
        self
    }

    pub fn before(mut self, before_action: impl Into<String>) -> Self {
        self.action.position = Some(wixer::simulation::SequencePosition::Before(
            before_action.into(),
        ));
        self
    }

    pub fn after(mut self, after_action: impl Into<String>) -> Self {
        self.action.position = Some(wixer::simulation::SequencePosition::After(after_action.into()));
        self
    }

    pub fn build(self) -> wixer::simulation::Action {
        self.action
    }
}
