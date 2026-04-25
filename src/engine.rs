//! The engine module contains the core logic for dotpatina operations.

use std::{
    fs,
    path::{Path, PathBuf},
};

use colored::Colorize;
use interface::PatinaInterface;
use log::info;
use similar::TextDiff;

pub mod interface;

use crate::templating::PatinaFileRender;
use crate::{
    diff::DiffAnalysis,
    patina::Patina,
    templating,
    utils::{Error, Result},
};

/// The PatinaEngine is the main driver of logic for dotpatina operations
pub struct PatinaEngine<'a, PI>
where
    PI: PatinaInterface,
{
    /// A reference to the PatinaInterface that defines how to interact with the user via input & output
    pi: &'a PI,

    /// The path to the patina file on disk
    patina_path: PathBuf,

    /// The set of tags to filter on
    tags: Option<Vec<String>>,

    /// A list of variables path files
    variables_files: Vec<PathBuf>,
}

impl<'a, PI> PatinaEngine<'a, PI>
where
    PI: PatinaInterface,
{
    /// Create a new PatinaEngine
    pub fn new(
        pi: &'a PI,
        patina_path: &Path,
        tags: Vec<String>,
        variables_files: Vec<PathBuf>,
    ) -> PatinaEngine<'a, PI> {
        let tags = match &*tags {
            [] => None,
            _ => Some(tags),
        };
        PatinaEngine {
            pi,
            patina_path: patina_path.to_path_buf(),
            tags,
            variables_files,
        }
    }

    /// Renders a Patina
    pub fn render_patina(&self) -> Result<()> {
        let mut patina = Patina::from_toml_file(&self.patina_path)?;
        patina.load_vars_files(self.variables_files.clone())?;

        info!("got patina: {:#?}", patina);
        let render = templating::render_patina(&patina, self.tags.clone())?;

        self.pi
            .output(format!("Rendered {} files\n\n", render.len()));
        for r in render.iter() {
            self.pi.output_file_header(&r.patina_file.template);
            self.pi.output(format!("{}\n", r.render_str));
        }

        Ok(())
    }

    /// Applies all the Patina files
    pub fn apply_patina(&self, use_trash: bool) -> Result<()> {
        let mut patina = Patina::from_toml_file(&self.patina_path)?;
        patina.load_vars_files(self.variables_files.clone())?;

        info!("got patina: {:#?}", patina);
        let mut render = templating::render_patina(&patina, self.tags.clone())?;

        let any_changes = self.generate_and_display_diffs(&patina, &mut render);

        // If there are no changes, quit
        if !any_changes {
            self.pi.output("No file changes detected in the patina\n");
            return Ok(());
        }

        // Get user confirmation to continue
        if self.pi.is_input_enabled() && !self.pi.confirm_apply()? {
            self.pi.output("Not applying patina.");
            return Ok(());
        }

        // Write out all files
        self.pi.output("\nApplying patina files\n");
        let num_trashed = self.apply_renders(&patina, render, use_trash)?;

        self.pi.output("Done");
        if num_trashed > 0 {
            self.pi.output(
                " (original files moved to trash)"
                    .bright_black()
                    .to_string(),
            );
        }
        self.pi.output("\n");
        Ok(())
    }

    fn generate_and_display_diffs(
        &self,
        patina: &Patina,
        render: &mut Vec<PatinaFileRender>,
    ) -> bool {
        let mut any_changes = false;

        let mut files_with_changes: Vec<(PathBuf, String)> = vec![];
        let mut files_without_changes: Vec<(PathBuf, String)> = vec![];

        // Generate and display diffs
        for r in render.iter_mut() {
            let target_path = patina.get_patina_path(&r.patina_file.target);

            let target_file_str = fs::read_to_string(&target_path).unwrap_or_default();
            let diff = TextDiff::from_lines(&target_file_str, &r.render_str);

            r.any_changes = Some(diff.any_changes());
            if r.any_changes.unwrap() {
                any_changes = true
            }

            if r.any_changes.unwrap() {
                files_with_changes.push((target_path, diff.to_string()));
            } else {
                files_without_changes.push((target_path, diff.to_string()));
            }
        }

        let any_unchanged_files = !files_without_changes.is_empty();
        if any_unchanged_files {
            self.pi.output("\nFiles without changes:\n");
            for (target_path, diff_str) in files_without_changes {
                self.pi.output(format!(
                    "  {} {}",
                    target_path.display().to_string().yellow(),
                    diff_str.blue()
                ));
            }
            self.pi.output("\n");
        }

        if !files_with_changes.is_empty() {
            if !any_unchanged_files {
                self.pi.output("\n");
            }
            for (target_path, diff_str) in files_with_changes {
                self.pi.output_file_header(&target_path);
                self.pi.output(diff_str);
                self.pi.output("\n");
            }
        }

        any_changes
    }

    fn apply_renders(
        &self,
        patina: &Patina,
        render: Vec<PatinaFileRender>,
        use_trash: bool,
    ) -> Result<usize> {
        let mut num_trashed = 0;
        for r in render.iter() {
            let target_path = patina.get_patina_path(&r.patina_file.target);
            self.pi.output(format!("   {}", target_path.display()));

            if r.any_changes == Some(false) {
                self.pi.output(format!(
                    " {} {}\n",
                    "✓".green(),
                    "(no change)".bright_black()
                ));
                continue;
            }

            // If the target file exists and there are changes, trash it
            if use_trash && target_path.is_file() && r.any_changes == Some(true) {
                if let Err(e) = trash::delete(&target_path) {
                    return Err(Error::MoveFileToTrash(e));
                }
                num_trashed += 1;
            }

            // Create parent directories and write file
            if let Some(target_parent) = target_path.parent() {
                if let Err(e) = fs::create_dir_all(target_parent) {
                    return Err(Error::FileWrite(target_path, e));
                }
            }
            if let Err(e) = fs::write(&target_path, &r.render_str) {
                return Err(Error::FileWrite(target_path.clone(), e));
            }

            #[cfg(unix)]
            if r.patina_file.preserve_permissions {
                let template_path = patina.get_patina_path(&r.patina_file.template);
                let permissions = fs::metadata(&template_path)
                    .map_err(|e| Error::FileRead(template_path, e))?
                    .permissions();
                fs::set_permissions(&target_path, permissions)
                    .map_err(|e| Error::FileWrite(target_path.clone(), e))?;
            }

            self.pi.output(" ✓\n".green().to_string());
        }

        Ok(num_trashed)
    }
}

#[cfg(test)]
mod tests {
    use crate::{engine::interface::test::TestPatinaInterface, tests::test_utils::TmpTestDir};

    use super::*;

    #[test]
    fn test_render_patina() {
        let tmp_dir = TmpTestDir::new();
        let patina_path = tmp_dir.write_file(
            "template_patina.toml",
            r#"
                name = "template-patina"
                description = "This is a Patina for a test template file"

                [vars]
                name.first = "Patina"
                name.last = "User"

                [[files]]
                template = "template.txt.hbs"
                target = "template.txt"
            "#,
        );
        tmp_dir.write_file("template.txt.hbs", r#"Hello, {{ name.first }} {{ name.last }}!
This is an example Patina template file.
Templates use the Handebars templating language. For more information, see <https://handlebarsjs.com/guide/>.
"#);

        colored::control::set_override(false);
        let pi = TestPatinaInterface::new();
        let engine = PatinaEngine::new(&pi, &patina_path, vec![], vec![]);

        let render = engine.render_patina();

        assert!(render.is_ok());

        assert_eq!(
            pi.get_all_output(),
            r#"Rendered 1 files

template.txt.hbs
Hello, Patina User!
This is an example Patina template file.
Templates use the Handebars templating language. For more information, see <https://handlebarsjs.com/guide/>.

"#
        );
    }

    #[test]
    fn test_render_patina_failed_file_load() {
        let patina_path = PathBuf::from("this/path/does/not/exist.toml");
        let pi = TestPatinaInterface::new();
        let engine = PatinaEngine::new(&pi, &patina_path, vec![], vec![]);

        let render = engine.render_patina();
        assert!(render.is_err());
        assert!(render.unwrap_err().is_file_read());
    }

    #[test]
    fn test_render_patina_render_fails() {
        let tmp_dir = TmpTestDir::new();
        let patina_path = tmp_dir.write_file(
            "missing_template_patina.toml",
            r#"
                name = "missing-template-patina"
                description = "This is a Patina that references a template file that does not exist"

                [vars]
                name = "Patina"

                [[files]]
                template = "this/template/does/not/exist.txt"
                target = "./output.txt"
            "#,
        );

        let pi = TestPatinaInterface::new();
        let engine = PatinaEngine::new(&pi, &patina_path, vec![], vec![]);

        let render = engine.render_patina();
        assert!(render.is_err());
        assert!(render.unwrap_err().is_file_read());
    }

    #[test]
    fn test_apply_patina() {
        let tmp_dir = TmpTestDir::new();
        let patina_path = tmp_dir.write_file(
            "template_patina.toml",
            r#"name = "template-patina"
description = "This is a Patina for a test template file"

[vars]
name.first = "Patina"
name.last = "User"

[[files]]
template = "template.txt.hbs"
target = "template.txt"
        "#,
        );
        tmp_dir.write_file("template.txt.hbs", r#"Hello, {{ name.first }} {{ name.last }}!
This is an example Patina template file.
Templates use the Handebars templating language. For more information, see <https://handlebarsjs.com/guide/>.
"#);

        let pi = TestPatinaInterface::new();
        let engine = PatinaEngine::new(&pi, &patina_path, vec![], vec![]);

        let apply = engine.apply_patina(false);

        assert!(apply.is_ok());

        assert!(pi.get_all_output().contains(r#"+   1 | Hello, Patina User!
+   2 | This is an example Patina template file.
+   3 | Templates use the Handebars templating language. For more information, see <https://handlebarsjs.com/guide/>.


Applying patina files"#));

        let applied_file_path = tmp_dir.get_file_path("template.txt");
        let applied_file = fs::read_to_string(applied_file_path);
        assert!(applied_file.is_ok());
        assert_eq!(
            applied_file.unwrap(),
            r#"Hello, Patina User!
This is an example Patina template file.
Templates use the Handebars templating language. For more information, see <https://handlebarsjs.com/guide/>.
"#
        );
    }

    #[test]
    fn test_apply_patina_abort_without_user_confirmation() {
        let tmp_dir = TmpTestDir::new();
        let patina_path = tmp_dir.write_file(
            "template_patina.toml",
            r#"
                name = "template-patina"
                description = "This is a Patina for a test template file"

                [vars]
                name.first = "Patina"
                name.last = "User"

                [[files]]
                template = "template.txt.hbs"
                target = "template.txt"
            "#,
        );
        tmp_dir.write_file("template.txt.hbs", r#"Hello, {{ name.first }} {{ name.last }}!
This is an example Patina template file.
Templates use the Handebars templating language. For more information, see <https://handlebarsjs.com/guide/>.
"#);

        let mut pi = TestPatinaInterface::new();
        pi.confirm_apply = false;
        let engine = PatinaEngine::new(&pi, &patina_path, vec![], vec![]);

        let apply = engine.apply_patina(false);

        assert!(apply.is_ok());
        assert!(pi.get_all_output().contains("Not applying patina."))
    }

    #[cfg(unix)]
    #[test]
    fn test_apply_patina_preserve_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let tmp_dir = TmpTestDir::new();
        let patina_path = tmp_dir.write_file(
            "preserve_perms_patina.toml",
            r#"name = "preserve-perms"
description = "Test preserve_permissions"

[[files]]
template = "script.sh"
target = "output.sh"
preserve_permissions = true
"#,
        );
        let template_path = tmp_dir.write_file("script.sh", "#!/usr/bin/env bash\necho hello\n");
        let template_mode = 0o755;
        fs::set_permissions(&template_path, fs::Permissions::from_mode(template_mode)).unwrap();

        let pi = TestPatinaInterface::new();
        let engine = PatinaEngine::new(&pi, &patina_path, vec![], vec![]);
        let apply = engine.apply_patina(false);
        assert!(apply.is_ok());

        let output_path = tmp_dir.get_file_path("output.sh");
        let output_mode = fs::metadata(&output_path).unwrap().permissions().mode();
        assert_eq!(output_mode & 0o7777, template_mode);
    }

    #[test]
    fn test_apply_patina_does_nothing_if_there_are_no_changes() {
        let tmp_dir = TmpTestDir::new();
        let patina_path = tmp_dir.write_file(
            "no_files_patina.toml",
            r#"
                name = "no files"
                description = "this patina has no files"
            "#,
        );

        let pi = TestPatinaInterface::new();
        let engine = PatinaEngine::new(&pi, &patina_path, vec![], vec![]);
        let apply = engine.apply_patina(false);

        assert!(apply.is_ok());
        assert!(pi
            .get_all_output()
            .contains("No file changes detected in the patina"));
    }
}
