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
        let results = templating::render_patina(&patina, self.tags.clone());

        self.pi
            .output(format!("Rendered {} files\n\n", results.len()));

        let mut any_errors = false;
        for r in &results {
            self.pi.output_file_header(&r.patina_file.template);
            match &r.render_result {
                Ok(render_str) => self.pi.output(format!("{}\n", render_str)),
                Err(e) => {
                    any_errors = true;
                    self.pi.output(format!("{}\n\n", e.to_string().red()));
                }
            }
        }

        if any_errors {
            Err(Error::Message(
                "Some templates failed to render".to_string(),
            ))
        } else {
            Ok(())
        }
    }

    /// Applies all the Patina files
    pub fn apply_patina(&self, use_trash: bool) -> Result<()> {
        let mut patina = Patina::from_toml_file(&self.patina_path)?;
        patina.load_vars_files(self.variables_files.clone())?;

        info!("got patina: {:#?}", patina);
        let results = templating::render_patina(&patina, self.tags.clone());

        let mut any_render_errors = false;
        for r in &results {
            if let Err(e) = &r.render_result {
                any_render_errors = true;
                self.pi.output_file_header(&r.patina_file.template);
                self.pi.output(format!("{}\n", e.to_string().red()));
            }
        }
        if any_render_errors {
            return Err(Error::Message(
                "Some templates failed to render".to_string(),
            ));
        }

        let mut render = results;
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
            let render_str = r.render_result.as_ref().unwrap();
            let diff = TextDiff::from_lines(&target_file_str, render_str);

            let content_changed = diff.any_changes();
            r.content_changes = Some(content_changed);

            #[cfg(unix)]
            if r.patina_file.preserve_permissions && target_path.is_file() {
                use std::os::unix::fs::PermissionsExt;
                let template_path = patina.get_patina_path(&r.patina_file.template);
                let template_mode = fs::metadata(&template_path)
                    .ok()
                    .map(|m| m.permissions().mode() & 0o7777);
                let target_mode = fs::metadata(&target_path)
                    .ok()
                    .map(|m| m.permissions().mode() & 0o7777);
                if let (Some(tmpl_mode), Some(tgt_mode)) = (template_mode, target_mode) {
                    if tmpl_mode != tgt_mode {
                        r.permission_change = Some((tgt_mode, tmpl_mode));
                    }
                }
            }

            r.any_changes = Some(content_changed || r.permission_change.is_some());
            if r.any_changes.unwrap() {
                any_changes = true;
            }

            if r.any_changes.unwrap() {
                let mut diff_str = if content_changed {
                    diff.to_string()
                } else {
                    String::new()
                };
                if let Some((old_mode, new_mode)) = r.permission_change {
                    diff_str.push_str(&format!(
                        "{}{}\n",
                        "~ permissions: ".yellow(),
                        format!("{:04o} → {:04o}", old_mode, new_mode).blue()
                    ));
                }
                files_with_changes.push((target_path, diff_str));
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

            let content_changed = r.content_changes == Some(true);

            if content_changed {
                // If the target file exists and content changed, trash it
                if use_trash && target_path.is_file() {
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
                if let Err(e) = fs::write(&target_path, r.render_result.as_ref().unwrap()) {
                    return Err(Error::FileWrite(target_path.clone(), e));
                }
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
        assert!(render.unwrap_err().is_message());
    }

    #[test]
    fn test_render_patina_partial_failure() {
        let tmp_dir = TmpTestDir::new();
        let patina_path = tmp_dir.write_file(
            "partial_failure_patina.toml",
            r#"
                name = "partial-failure-patina"
                description = "Some templates fail"

                [vars]
                A = "value_a"
                C = "value_c"

                [[files]]
                template = "template_a.txt.hbs"
                target = "output_a.txt"

                [[files]]
                template = "template_b.txt.hbs"
                target = "output_b.txt"

                [[files]]
                template = "template_c.txt.hbs"
                target = "output_c.txt"
            "#,
        );
        tmp_dir.write_file("template_a.txt.hbs", "This is {{ A }}.");
        tmp_dir.write_file("template_b.txt.hbs", "This is {{ missing_var }}.");
        tmp_dir.write_file("template_c.txt.hbs", "This is {{ C }}.");

        let pi = TestPatinaInterface::new();
        let engine = PatinaEngine::new(&pi, &patina_path, vec![], vec![]);

        let render = engine.render_patina();
        assert!(render.is_err());

        let output = pi.get_all_output();
        assert!(output.contains("Rendered 3 files"));
        assert!(output.contains("This is value_a."));
        assert!(output.contains("This is value_c."));
        assert!(output.contains("template_b.txt.hbs"));
    }

    #[test]
    fn test_render_patina_error_has_trailing_newline() {
        let tmp_dir = TmpTestDir::new();
        let patina_path = tmp_dir.write_file(
            "error_spacing_patina.toml",
            r#"
                name = "error-spacing-patina"
                description = "Verify spacing after render errors"

                [[files]]
                template = "bad.txt.hbs"
                target = "output.txt"

                [[files]]
                template = "bad2.txt.hbs"
                target = "output2.txt"

                [[files]]
                template = "bad3.txt.hbs"
                target = "output3.txt"
            "#,
        );
        tmp_dir.write_file("bad.txt.hbs", "{{ missing_var }}");
        tmp_dir.write_file("bad2.txt.hbs", "{{ missing_var }}");
        tmp_dir.write_file("bad3.txt.hbs", "{{ missing_var }}");

        let pi = TestPatinaInterface::new();
        let engine = PatinaEngine::new(&pi, &patina_path, vec![], vec![]);

        assert!(engine.render_patina().is_err());

        assert_eq!(
            pi.get_all_output(),
            r#"Rendered 3 files

bad.txt.hbs
Template error: Error rendering "bad.txt.hbs" line 1, col 1: Failed to access variable in strict mode Some("missing_var")

bad2.txt.hbs
Template error: Error rendering "bad2.txt.hbs" line 1, col 1: Failed to access variable in strict mode Some("missing_var")

bad3.txt.hbs
Template error: Error rendering "bad3.txt.hbs" line 1, col 1: Failed to access variable in strict mode Some("missing_var")

"#
        );
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

    #[cfg(unix)]
    #[test]
    fn test_apply_patina_permission_only_change() {
        use std::os::unix::fs::PermissionsExt;

        let tmp_dir = TmpTestDir::new();
        let content = "#!/usr/bin/env bash\necho hello\n";
        let patina_path = tmp_dir.write_file(
            "perm_only_patina.toml",
            r#"name = "perm-only"
description = "Test permission-only change detection"

[[files]]
template = "script.sh"
target = "output.sh"
preserve_permissions = true
"#,
        );

        let template_path = tmp_dir.write_file("script.sh", content);
        fs::set_permissions(&template_path, fs::Permissions::from_mode(0o755)).unwrap();

        // Existing target with same content but different permissions
        let target_path = tmp_dir.write_file("output.sh", content);
        fs::set_permissions(&target_path, fs::Permissions::from_mode(0o644)).unwrap();

        let pi = TestPatinaInterface::new();
        let engine = PatinaEngine::new(&pi, &patina_path, vec![], vec![]);
        let apply = engine.apply_patina(false);
        assert!(apply.is_ok());

        let output = pi.get_all_output();
        assert!(!output.contains("No file changes detected"));
        assert!(output.contains("~ permissions: 0644 → 0755"));
        assert!(output.contains("Applying patina files"));

        // Content should be unchanged
        assert_eq!(fs::read_to_string(&target_path).unwrap(), content);
        // Permissions should be updated
        let output_mode = fs::metadata(&target_path).unwrap().permissions().mode();
        assert_eq!(output_mode & 0o7777, 0o755);
    }

    #[cfg(unix)]
    #[test]
    fn test_apply_patina_no_change_when_permissions_match() {
        use std::os::unix::fs::PermissionsExt;

        let tmp_dir = TmpTestDir::new();
        let content = "#!/usr/bin/env bash\necho hello\n";
        let patina_path = tmp_dir.write_file(
            "perm_match_patina.toml",
            r#"name = "perm-match"
description = "Test no change when content and permissions match"

[[files]]
template = "script.sh"
target = "output.sh"
preserve_permissions = true
"#,
        );

        let template_path = tmp_dir.write_file("script.sh", content);
        fs::set_permissions(&template_path, fs::Permissions::from_mode(0o755)).unwrap();

        let target_path = tmp_dir.write_file("output.sh", content);
        fs::set_permissions(&target_path, fs::Permissions::from_mode(0o755)).unwrap();

        let pi = TestPatinaInterface::new();
        let engine = PatinaEngine::new(&pi, &patina_path, vec![], vec![]);
        let apply = engine.apply_patina(false);
        assert!(apply.is_ok());

        let output = pi.get_all_output();
        assert!(output.contains("No file changes detected"));
        assert!(!output.contains("~ permissions:"));
    }

    #[cfg(unix)]
    #[test]
    fn test_apply_patina_content_and_permission_change() {
        use std::os::unix::fs::PermissionsExt;

        let tmp_dir = TmpTestDir::new();
        let patina_path = tmp_dir.write_file(
            "content_and_perm_patina.toml",
            r#"name = "content-and-perm"
description = "Test content and permission change together"

[[files]]
template = "script.sh"
target = "output.sh"
preserve_permissions = true
"#,
        );

        let template_path = tmp_dir.write_file("script.sh", "#!/usr/bin/env bash\necho updated\n");
        fs::set_permissions(&template_path, fs::Permissions::from_mode(0o755)).unwrap();

        let target_path = tmp_dir.write_file("output.sh", "#!/usr/bin/env bash\necho original\n");
        fs::set_permissions(&target_path, fs::Permissions::from_mode(0o644)).unwrap();

        let pi = TestPatinaInterface::new();
        let engine = PatinaEngine::new(&pi, &patina_path, vec![], vec![]);
        let apply = engine.apply_patina(false);
        assert!(apply.is_ok());

        let output = pi.get_all_output();
        assert!(!output.contains("No file changes detected"));
        assert!(output.contains("~ permissions: 0644 → 0755"));
        assert!(output.contains("echo updated"));
        assert!(output.contains("Applying patina files"));

        assert_eq!(
            fs::read_to_string(&target_path).unwrap(),
            "#!/usr/bin/env bash\necho updated\n"
        );
        let output_mode = fs::metadata(&target_path).unwrap().permissions().mode();
        assert_eq!(output_mode & 0o7777, 0o755);
    }

    #[cfg(unix)]
    #[test]
    fn test_apply_patina_permission_difference_ignored_without_preserve() {
        use std::os::unix::fs::PermissionsExt;

        let tmp_dir = TmpTestDir::new();
        let content = "#!/usr/bin/env bash\necho hello\n";
        let patina_path = tmp_dir.write_file(
            "no_preserve_patina.toml",
            r#"name = "no-preserve"
description = "Test that permission differences are ignored without preserve_permissions"

[[files]]
template = "script.sh"
target = "output.sh"
"#,
        );

        let template_path = tmp_dir.write_file("script.sh", content);
        fs::set_permissions(&template_path, fs::Permissions::from_mode(0o755)).unwrap();

        let target_path = tmp_dir.write_file("output.sh", content);
        fs::set_permissions(&target_path, fs::Permissions::from_mode(0o644)).unwrap();

        let pi = TestPatinaInterface::new();
        let engine = PatinaEngine::new(&pi, &patina_path, vec![], vec![]);
        let apply = engine.apply_patina(false);
        assert!(apply.is_ok());

        let output = pi.get_all_output();
        assert!(output.contains("No file changes detected"));
        assert!(!output.contains("~ permissions:"));

        // Permissions should remain unchanged since preserve_permissions is false
        let output_mode = fs::metadata(&target_path).unwrap().permissions().mode();
        assert_eq!(output_mode & 0o7777, 0o644);
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
