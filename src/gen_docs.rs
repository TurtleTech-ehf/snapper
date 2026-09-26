use clap::CommandFactory;
use snapper_fmt::cli::Cli;

fn main() {
    let cmd = Cli::command();
    let markdown = clap_markdown::help_markdown_command(&cmd);
    print!("{markdown}");
}
