//! A client of [lynk](lynk:lynk.self): runs its command line and reads what it
//! prints, offering in Rust what [`resources/lynk.wit`](lynk:filesystem.file)
//! offers as a component.

pub mod span;

mod run;

// The output of the lint is a list of problems, so the module is named
// after what it returns rather than after the command.
#[path = "lint.rs"]
pub mod problem;

/// A repository, and the commands run on it.
pub mod workspace {
    use std::path::PathBuf;

    use crate::problem::Problem;
    use crate::run;
    use crate::span::Span;

    pub struct Workspace {
        root: PathBuf,
    }

    impl Workspace {
        /// The repository whose root directory is [`root`](lynk:rust.argument).
        pub fn new(root: impl Into<PathBuf>) -> Self {
            Self { root: root.into() }
        }

        /// Every link failing [`lynk lint`](lynk:lynk.lint), and why.
        pub fn lint(&self) -> Result<Vec<Problem>, String> {
            let output = run::lynk(&self.root, &["lint"])?;
            Ok(output.stdout.lines().filter_map(Problem::parse).collect())
        }

        /// The spans a link points at, as [`lynk where`](lynk:lynk.where) prints them: the
        /// link is its `<provider.selector>` and its arguments, unquoted.
        pub fn locate(&self, selector: &str, arguments: &[&str]) -> Result<Vec<Span>, String> {
            self.spans("where", selector, arguments)
        }

        /// The places linking to what a link points at, as [`lynk ref`](lynk:lynk.ref)
        /// prints them.
        pub fn references(&self, selector: &str, arguments: &[&str]) -> Result<Vec<Span>, String> {
            self.spans("ref", selector, arguments)
        }

        fn spans(&self, command: &str, selector: &str, arguments: &[&str]) -> Result<Vec<Span>, String> {
            let mut command_line = vec![command, selector];
            command_line.extend_from_slice(arguments);
            let output = run::lynk(&self.root, &command_line)?.into_result()?;
            // Each line is a span, then a tab and the text it starts with.
            Ok(output
                .stdout
                .lines()
                .filter_map(|line| Span::parse(line.split('\t').next()?))
                .collect())
        }

        /// The parts of the documentation matching
        /// [`words`](lynk:rust.argument), best first, as
        /// [`lynk find`](lynk:lynk.find) lists them.
        pub fn find(&self, words: &[&str]) -> Result<Vec<Span>, String> {
            let mut command_line = vec!["find"];
            command_line.extend_from_slice(words);
            let output = run::lynk(&self.root, &command_line)?.into_result()?;
            // Each line is a score, the span and its title, separated by tabs.
            Ok(output
                .stdout
                .lines()
                .filter_map(|line| Span::parse(line.split('\t').nth(1)?))
                .collect())
        }
    }
}
