use super::cli_options::CliOptions;

pub(crate) struct FlagOptionRegistration {
    pub name: String,
    pub description: String,
    pub handler: Box<dyn Fn(&mut CliOptions)>,
}

pub(crate) struct ValuedOptionRegistration {
    pub name: String,
    pub description: String,
    pub handler: Box<dyn Fn(&mut CliOptions, &str)>,
}

pub(crate) struct CommandRegistration {
    pub name: String,
    pub description: String,
    pub handler: Box<dyn Fn()>,
}

#[derive(Default)]
pub(crate) struct CliBuilder {
    pub commands: Vec<CommandRegistration>,
    pub flag_options: Vec<FlagOptionRegistration>,
    pub valued_options: Vec<ValuedOptionRegistration>,
}

impl CliBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_command<F: Fn() + 'static>(
        mut self,
        name: impl Into<String>,
        description: impl Into<String>,
        handler: F,
    ) -> Self {
        self.commands.push(CommandRegistration {
            name: name.into(),
            description: description.into(),
            handler: Box::new(handler),
        });
        self
    }

    pub fn add_flag_option<F: Fn(&mut CliOptions) + 'static>(
        mut self,
        name: impl Into<String>,
        description: impl Into<String>,
        handler: F,
    ) -> Self {
        self.flag_options.push(FlagOptionRegistration {
            name: name.into(),
            description: description.into(),
            handler: Box::new(handler),
        });
        self
    }

    pub fn add_valued_option<F: Fn(&mut CliOptions, &str) + 'static>(
        mut self,
        name: impl Into<String>,
        description: impl Into<String>,
        handler: F,
    ) -> Self {
        self.valued_options.push(ValuedOptionRegistration {
            name: name.into(),
            description: description.into(),
            handler: Box::new(handler),
        });
        self
    }
}
