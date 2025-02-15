use std::path::Path;

use colored::Colorize;
use similar::{ChangeTag, TextDiff};

use crate::utils::{Error, Result};

/// PatinaOutput specifies operations for interfacing with user operations
pub trait PatinaInterface {
    /// Output a single string
    fn output<S>(&self, s: S)
    where
        S: Into<String>;

    /// Prompts the user for confirmation to apply the patina
    fn confirm_apply(&self) -> Result<bool> {
        self.output("Do you want to continue? (y/n): ");
        let mut input = String::new();
        match std::io::stdin().read_line(&mut input) {
            Ok(_) => {
                if input.trim().to_lowercase() != "y" {
                    return Ok(false);
                }
            }
            Err(e) => return Err(Error::GetUserInput(e)),
        }

        Ok(true)
    }

    /// Output a patina render
    fn output_file_header(&self, template_path: &Path) {
        let template_path = template_path.display().to_string();
        self.output(
            format!("{}\n", "=".repeat(template_path.len() + 16))
                .yellow()
                .bold()
                .to_string(),
        );
        self.output(
            format!("> Patina file {} <\n", template_path)
                .yellow()
                .bold()
                .to_string(),
        );
        self.output(
            format!("{}\n", "=".repeat(template_path.len() + 16))
                .yellow()
                .bold()
                .to_string(),
        );
    }

    /// Output a diff view
    fn output_diff<'a>(&self, diff: &TextDiff<'a, 'a, 'a, str>) {
        for change in diff.iter_all_changes() {
            match change.tag() {
                ChangeTag::Insert => {
                    self.output(format!("+ {}", change).green().bold().to_string())
                }
                ChangeTag::Equal => self.output(format!("| {}", change).bold().to_string()),
                ChangeTag::Delete => self.output(format!("- {}", change).red().bold().to_string()),
            }
        }
    }
}

#[cfg(test)]
pub mod test {
    use std::cell::RefCell;

    use super::*;

    pub struct TestPatinaInterface {
        pub confirm_apply: bool,
        pub lines: RefCell<Vec<String>>,
    }

    impl TestPatinaInterface {
        pub fn new() -> TestPatinaInterface {
            colored::control::set_override(false);

            TestPatinaInterface {
                confirm_apply: true,
                lines: RefCell::new(vec![]),
            }
        }

        pub fn get_all_output(self) -> String {
            self.lines.into_inner().join("")
        }
    }

    impl PatinaInterface for TestPatinaInterface {
        fn output<S>(&self, s: S)
        where
            S: Into<String>,
        {
            self.lines.borrow_mut().push(s.into());
        }

        fn confirm_apply(&self) -> Result<bool> {
            Ok(self.confirm_apply)
        }
    }
}
