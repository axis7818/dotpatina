//! Structures and functions for processing Patina templates.
//! Templating uses the [Handlebars](https://handlebarsjs.com/guide/) templating language.

use std::fs;

use handlebars::Handlebars;
use log::info;

use crate::patina::patina_file::PatinaFile;
use crate::patina::Patina;
use crate::utils::{Error, Result};

/// [PatinaFileRender] is an object that holds a reference to a [PatinaFile] and the result of
/// rendering it.
#[derive(Debug)]
pub struct PatinaFileRender<'pf> {
    /// A reference to the [PatinaFile]
    pub patina_file: &'pf PatinaFile,

    /// Whether or not the file has any changes (content or permissions).
    /// - [None]: if the file has not been diffed with the target yet
    /// - [true]: if the diff detected any changes
    /// - [false]: if the diff did not detect any changes
    pub any_changes: Option<bool>,

    /// Whether or not the file content has changes specifically.
    /// - [None]: if the file has not been diffed with the target yet
    /// - [true]: if the content diff detected changes
    /// - [false]: if the content diff did not detect changes
    pub content_changes: Option<bool>,

    /// The permission change (old_mode, new_mode) if permissions differ between template and target.
    /// Always [None] on non-unix platforms or when [PatinaFile::preserve_permissions] is false.
    pub permission_change: Option<(u32, u32)>,

    /// The rendered content, or the error that occurred during rendering.
    pub render_result: Result<String>,
}

/// Renders all the [PatinaFile]s in a [Patina].
///
/// All files are attempted regardless of individual failures. Check
/// [PatinaFileRender::render_result] on each entry to determine whether it succeeded.
pub fn render_patina(patina: &Patina, tags: Option<Vec<String>>) -> Vec<PatinaFileRender<'_>> {
    let mut hb = Handlebars::new();
    hb.register_escape_fn(handlebars::no_escape);
    hb.set_strict_mode(true);

    patina
        .files_for_tags(tags)
        .map(|pf| PatinaFileRender {
            patina_file: pf,
            render_result: render_patina_file(&hb, patina, pf),
            any_changes: None,
            content_changes: None,
            permission_change: None,
        })
        .collect()
}

/// Render a single [PatinaFile] to a string.
fn render_patina_file(
    hb: &Handlebars,
    patina: &Patina,
    patina_file: &PatinaFile,
) -> Result<String> {
    info!("rendering patina file: {}", patina_file.template.display());

    let template_path = patina.get_patina_path(&patina_file.template);
    let template_str = match fs::read_to_string(&template_path) {
        Ok(template_str) => template_str,
        Err(e) => return Err(Error::FileRead(template_path, e)),
    };

    if patina_file.disable_templating {
        return Ok(template_str);
    }

    match hb.render_template(&template_str, &patina.vars) {
        Ok(render) => Ok(render),
        Err(mut e) => {
            e.template_name = Some(patina_file.template.display().to_string());
            Err(Error::RenderTemplate(e))
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use serde_json::json;

    use crate::tests::test_utils::TmpTestDir;

    use super::*;

    #[test]
    fn test_render_patina() {
        let tmp_dir = TmpTestDir::new();
        let template_path = tmp_dir.write_file(
            "template.txt.hbs",
            r#"Hello, {{ name.first }} {{ name.last }}!
This is an example Patina template file.
Templates use the Handebars templating language. For more information, see <https://handlebarsjs.com/guide/>.
"#,
        );

        let patina = Patina {
            base_path: None,
            name: String::from("sample-patina"),
            description: String::from("This is a sample Patina"),
            vars: Some(json!({
                "name": {
                    "first": "Patina",
                    "last": "User"
                }
            })),
            files: vec![PatinaFile::new(
                template_path,
                PathBuf::from("tests/fixtures/template.txt"),
            )],
        };

        let results = render_patina(&patina, None);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].patina_file, &patina.files[0]);
        assert_eq!(
            results[0].render_result.as_ref().unwrap(),
            r#"Hello, Patina User!
This is an example Patina template file.
Templates use the Handebars templating language. For more information, see <https://handlebarsjs.com/guide/>.
"#
        );
    }

    #[test]
    fn test_render_patina_multiple_templates() {
        let tmp_dir = TmpTestDir::new();
        let template_a_path = tmp_dir.write_file("template_a.txt.hbs", "This is {{ A }}.");
        let template_b_path = tmp_dir.write_file("template_b.txt.hbs", "This is {{ B }}.");
        let template_c_path = tmp_dir.write_file("template_c.txt.hbs", "This is {{ C }}.");

        let patina = Patina {
            base_path: None,
            name: String::from("multi-template-patina"),
            description: String::from("This is a patina with multiple templates"),
            vars: Some(json!({
                "A": "template_a",
                "B": "template_b",
                "C": "template_c",
            })),
            files: vec![
                PatinaFile::new(template_a_path, PathBuf::from("output_a.txt")),
                PatinaFile::new(template_b_path, PathBuf::from("output_b.txt")),
                PatinaFile::new(template_c_path, PathBuf::from("output_c.txt")),
            ],
        };

        let results = render_patina(&patina, None);

        assert_eq!(results.len(), 3);
        assert_eq!(results[0].render_result.as_ref().unwrap(), "This is template_a.");
        assert_eq!(results[1].render_result.as_ref().unwrap(), "This is template_b.");
        assert_eq!(results[2].render_result.as_ref().unwrap(), "This is template_c.");
    }

    #[test]
    fn test_render_patina_missing_variable() {
        let tmp_dir = TmpTestDir::new();
        let template_path = tmp_dir.write_file("template.txt.hbs", r#"Hello, {{ name.first }} {{ name.last }}!
This is an example Patina template file.
Templates use the Handebars templating language. For more information, see <https://handlebarsjs.com/guide/>.
"#);

        let patina = Patina {
            base_path: None,
            name: String::from("sample-patina"),
            description: String::from("This is a sample Patina"),
            vars: Some(json!({})),
            files: vec![PatinaFile::new(
                template_path,
                PathBuf::from("tests/fixtures/template.txt"),
            )],
        };

        let results = render_patina(&patina, None);
        assert_eq!(results.len(), 1);
        let err = results[0].render_result.as_ref().unwrap_err();
        assert!(err.is_render_template());
        let render_err = err.as_render_template().unwrap();
        assert_eq!(
            render_err.reason().to_string(),
            "Failed to access variable in strict mode Some(\"name.first\")"
        );
    }

    #[test]
    fn test_render_patina_invalid_template() {
        let tmp_dir = TmpTestDir::new();
        let invalid_template_path = tmp_dir.write_file("invalid_template.txt.hbs", r#"
            Hello, {{ name }!

            This is an example Patina template file.

            Templates use the Handebars templating language. For more information, see <https://handlebarsjs.com/guide/>.
        "#);

        let patina = Patina {
            base_path: None,
            name: String::from("sample-patina"),
            description: String::from("This is a sample Patina"),
            vars: Some(json!({})),
            files: vec![PatinaFile::new(
                invalid_template_path,
                PathBuf::from("tests/fixtures/template.txt"),
            )],
        };

        let results = render_patina(&patina, None);
        assert_eq!(results.len(), 1);
        assert!(results[0].render_result.is_err());
        assert!(results[0].render_result.as_ref().unwrap_err().is_render_template());
    }

    #[test]
    fn test_render_patina_disable_templating() {
        let tmp_dir = TmpTestDir::new();
        let template_path = tmp_dir.write_file(
            "no-templating.txt",
            "Hello, {{name}}!\n",
        );

        let patina = Patina {
            base_path: None,
            name: String::from("no-templating-patina"),
            description: String::from("Templating is disabled"),
            vars: None,
            files: vec![PatinaFile {
                disable_templating: true,
                ..PatinaFile::new(template_path, PathBuf::from("output.txt"))
            }],
        };

        let results = render_patina(&patina, None);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].render_result.as_ref().unwrap(), "Hello, {{name}}!\n");
    }

    #[test]
    fn test_render_patina_escaped_handlebars() {
        let tmp_dir = TmpTestDir::new();
        let template_path = tmp_dir.write_file(
            "template_with_escaped_handlebars.hbs",
            r#"This file has \{{ escaped }} handlebars
"#,
        );

        let patina = Patina {
            name: "escaped_handlebars".to_string(),
            description: "this patina shows escaping handlebars".to_string(),
            base_path: None,
            vars: None,
            files: vec![PatinaFile::new(
                template_path,
                PathBuf::from("tests/fixtures/output.txt"),
            )],
        };

        let results = render_patina(&patina, None);
        assert_eq!(results.len(), 1);
        assert_eq!(
            results[0].render_result.as_ref().unwrap(),
            "This file has {{ escaped }} handlebars\n"
        );
    }

    #[test]
    fn test_render_patina_partial_failure() {
        let tmp_dir = TmpTestDir::new();
        let template_a_path = tmp_dir.write_file("template_a.txt.hbs", "This is {{ A }}.");
        let template_b_path =
            tmp_dir.write_file("template_b.txt.hbs", "This is {{ missing_var }}.");
        let template_c_path = tmp_dir.write_file("template_c.txt.hbs", "This is {{ C }}.");

        let patina = Patina {
            base_path: None,
            name: String::from("partial-failure-patina"),
            description: String::from("Some templates fail to render"),
            vars: Some(json!({
                "A": "template_a",
                "C": "template_c",
            })),
            files: vec![
                PatinaFile::new(template_a_path, PathBuf::from("output_a.txt")),
                PatinaFile::new(template_b_path, PathBuf::from("output_b.txt")),
                PatinaFile::new(template_c_path, PathBuf::from("output_c.txt")),
            ],
        };

        let results = render_patina(&patina, None);

        assert_eq!(results.len(), 3);
        assert!(results[0].render_result.is_ok());
        assert!(results[1].render_result.is_err());
        assert!(results[2].render_result.is_ok());
        assert_eq!(results[0].render_result.as_ref().unwrap(), "This is template_a.");
        assert_eq!(results[2].render_result.as_ref().unwrap(), "This is template_c.");
    }
}
