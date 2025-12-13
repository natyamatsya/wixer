use super::action_builder::ActionBuilder;

pub struct SimBuilder {
    sim: wixer::simulation::Simulator,
}

impl SimBuilder {
    pub fn new() -> Self {
        Self {
            sim: wixer::simulation::Simulator::new(),
        }
    }

    pub fn custom_action(mut self, action: wixer::simulation::Action) -> Self {
        self.sim.add_custom_action(action);
        self
    }

    pub fn action(self, builder: ActionBuilder) -> Self {
        self.custom_action(builder.build())
    }

    pub fn build(self) -> wixer::simulation::Simulator {
        self.sim
    }
}
