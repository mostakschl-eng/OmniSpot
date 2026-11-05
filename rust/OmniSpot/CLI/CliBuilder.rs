use super::cli_options::CliOptions;

pub(crate) type CommandHandler = fn() -> Result<(), String>;

pub(crate) struct CommandRegistration {
    pub name: &'static str,
    pub description: &'static str,
    pub handler: CommandHandler,
}

pub(crate) struct FlagOptionRegistration {
    pub name: &'static str,
    pub description: &'static str,
    pub handler: fn(&mut CliOptions),
}

pub(crate) struct ValuedOptionRegistration {
    pub name: &'static str,
    pub description: &'static str,
    pub handler: fn(&mut CliOptions, &str),
}

#[derive(Default)]
pub(crate) struct CliBuilder {
    pub commands: Vec<CommandRegistration>,
    pub flag_options: Vec<FlagOptionRegistration>,
    pub valued_options: Vec<ValuedOptionRegistration>,
}

impl CliBuilder {
    pub fn add_command(
        &mut self,
        name: &'static str,
        description: &'static str,
        handler: CommandHandler,
    ) -> &mut Self {
        self.commands.push(CommandRegistration {
            name,
            description,
            handler,
        });
        self
    }

    pub fn add_flag_option(
        &mut self,
        name: &'static str,
        description: &'static str,
        handler: fn(&mut CliOptions),
    ) -> &mut Self {
        self.flag_options.push(FlagOptionRegistration {
            name,
            description,
            handler,
        });
        self
    }

    pub fn add_valued_option(
        &mut self,
        name: &'static str,
        description: &'static str,
        handler: fn(&mut CliOptions, &str),
    ) -> &mut Self {
        self.valued_options.push(ValuedOptionRegistration {
            name,
            description,
            handler,
        });
        self
    }
}
