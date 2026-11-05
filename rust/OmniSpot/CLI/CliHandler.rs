use super::cli_builder::{CliBuilder, CommandHandler};
use super::cli_options::CliOptions;
use std::fmt::Write;

pub(crate) enum CliOutcome {
    Continue,
    Help(String),
    Command(CommandHandler),
}

pub(crate) struct CliHandler<'a> {
    pub builder: &'a CliBuilder,
}

impl CliHandler<'_> {
    pub fn handle(&self, args: &[String], options: &mut CliOptions) -> Result<CliOutcome, String> {
        let mut matched = Vec::new();
        let mut i = 0;
        while i < args.len() {
            let arg = args[i].as_str();
            if arg == "--help" {
                return Ok(CliOutcome::Help(self.help()));
            }
            if let Some(option) = self.builder.flag_options.iter().find(|o| o.name == arg) {
                (option.handler)(options);
                i += 1;
                continue;
            }
            if let Some(option) = self.builder.valued_options.iter().find(|o| o.name == arg) {
                let value = args
                    .get(i + 1)
                    .ok_or_else(|| format!("Option {arg} requires a value."))?;
                (option.handler)(options, value);
                i += 2;
                continue;
            }
            if let Some(command) = self.builder.commands.iter().find(|c| c.name == arg) {
                matched.push(command);
            }
            i += 1;
        }
        if matched.len() > 1 {
            let names = matched
                .iter()
                .map(|c| c.name)
                .collect::<Vec<_>>()
                .join(", ");
            return Err(format!(
                "Multiple commands specified: {names}\nOnly one command is allowed at a time."
            ));
        }
        Ok(match matched.first() {
            Some(command) => CliOutcome::Command(command.handler),
            None => CliOutcome::Continue,
        })
    }

    fn help(&self) -> String {
        let mut message = String::from("OmniSpot.exe [command] [options]\n\n");
        if !self.builder.commands.is_empty() {
            message.push_str("Commands:\n");
            for command in &self.builder.commands {
                writeln!(message, "  {:<20} {}", command.name, command.description).unwrap();
            }
            writeln!(message, "  {:<20} Show this help message", "--help").unwrap();
            message.push('\n');
        }
        if !self.builder.flag_options.is_empty() || !self.builder.valued_options.is_empty() {
            message.push_str("Options:\n");
            for option in &self.builder.flag_options {
                writeln!(message, "  {:<20} {}", option.name, option.description).unwrap();
            }
            for option in &self.builder.valued_options {
                writeln!(message, "  {:<20} {}", option.name, option.description).unwrap();
            }
        }
        message
    }
}

#[cfg(test)]
mod tests {
    use super::{CliHandler, CliOutcome};
    use crate::cli::cli_builder::CliBuilder;
    use crate::cli::cli_options::CliOptions;

    fn noop() -> Result<(), String> {
        Ok(())
    }

    #[test]
    fn parses_options_and_rejects_multiple_commands() {
        let mut builder = CliBuilder::default();
        builder
            .add_command("--one", "one", noop)
            .add_command("--two", "two", noop)
            .add_flag_option("--debug", "debug", |options| {
                options.enable_debug_logging = true
            });
        let handler = CliHandler { builder: &builder };
        let mut options = CliOptions::default();
        assert!(matches!(
            handler.handle(&["--debug".into()], &mut options).unwrap(),
            CliOutcome::Continue
        ));
        assert!(options.enable_debug_logging);
        assert!(
            handler
                .handle(&["--one".into(), "--two".into()], &mut options)
                .is_err()
        );
        assert!(matches!(
            handler.handle(&["--help".into()], &mut options).unwrap(),
            CliOutcome::Help(_)
        ));
    }

    #[test]
    fn valued_option_requires_value() {
        let mut builder = CliBuilder::default();
        builder.add_valued_option("--value", "value", |_, _| {});
        let handler = CliHandler { builder: &builder };
        assert_eq!(
            handler
                .handle(&["--value".into()], &mut CliOptions::default())
                .err()
                .unwrap(),
            "Option --value requires a value."
        );
    }
}
