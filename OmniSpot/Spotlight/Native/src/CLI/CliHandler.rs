use super::cli_builder::CliBuilder;
use super::cli_options::CliOptions;

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum CliResult {
    NoArgs,
    Handled,
    HelpRequested,
    Error(String),
}

pub(crate) struct CliHandler {
    builder: CliBuilder,
}

impl CliHandler {
    pub fn new(builder: CliBuilder) -> Self {
        Self { builder }
    }

    pub fn handle(&self, args: &[String], options: &mut CliOptions) -> CliResult {
        if args.is_empty() {
            return CliResult::NoArgs;
        }

        let mut matched_commands = Vec::new();
        let mut i = 0;

        while i < args.len() {
            let arg = &args[i];

            if arg == "--help" || arg == "-h" {
                return CliResult::HelpRequested;
            }

            if let Some(flag) = self.builder.flag_options.iter().find(|o| o.name == *arg) {
                (flag.handler)(options);
                i += 1;
                continue;
            }

            if let Some(valued) = self.builder.valued_options.iter().find(|o| o.name == *arg) {
                if i + 1 >= args.len() {
                    return CliResult::Error(format!("Option {} requires a value.", arg));
                }
                (valued.handler)(options, &args[i + 1]);
                i += 2;
                continue;
            }

            if let Some(cmd) = self.builder.commands.iter().find(|c| c.name == *arg) {
                matched_commands.push(cmd);
                i += 1;
                continue;
            }

            return CliResult::Error(format!("Unknown option or command: {}", arg));
        }

        for cmd in matched_commands {
            (cmd.handler)();
        }

        CliResult::Handled
    }
}

#[cfg(test)]
#[path = "../tests/CLI/CliTests.rs"]
mod tests;
