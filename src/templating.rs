//! Structures and functions for processing Patina templates.
//! Templating uses the [Handlebars](https://handlebarsjs.com/guide/) templating language.

use std::fs;

use handlebars::Handlebars;
use log::info;

use crate::patina::patina_file::PatinaFile;
use crate::patina::Patina;
use crate::utils::{Error, Result};

/// [PatinaFileRender] is an object that holds a reference to a [PatinaFile] and a
/// [String] of the final render.
#[derive(Debug)]
pub struct PatinaFileRender<'pf> {
    /// A reference to the [PatinaFile]
    pub patina_file: &'pf PatinaFile,

    /// Whether or not the file has changes.
    /// - [None]: if the file has not been diffed with the target yet
    /// - [true]: if the diff detected any changes
    /// - [false]: if the diff did not detect any changes
    pub any_changes: Option<bool>,

    /// The full render string for this file
    pub render_str: String,
}

/// Renders all the [PatinaFile]s in a [Patina].
pub fn render_patina(
    patina: &Patina,
    tags: Option<Vec<String>>,
) -> Result<Vec<PatinaFileRender<'_>>> {
    let mut hb = Handlebars::new();
    hb.register_escape_fn(handlebars::no_escape);
    hb.set_strict_mode(true);

    patina
        .files_for_tags(tags)
        .map(|pf| {
            let render = render_patina_file(&hb, patina, pf)?;
            Ok(PatinaFileRender {
                patina_file: pf,
                render_str: render,
                any_changes: None,
            })
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

        let render = render_patina(&patina, None);

        assert!(render.is_ok());
        let render = render.unwrap();
        assert_eq!(render.len(), 1);
        let render = &render[0];

        assert_eq!(render.patina_file, &patina.files[0]);

        assert_eq!(
            render.render_str,
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

        let render = render_patina(&patina, None);

        assert!(render.is_ok());
        let render = render.unwrap();

        assert_eq!(render.len(), 3);
        assert_eq!(render[0].render_str, "This is template_a.");
        assert_eq!(render[1].render_str, "This is template_b.");
        assert_eq!(render[2].render_str, "This is template_c.");
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

        let render = render_patina(&patina, None);
        assert!(render.is_err());
        let render = render.unwrap_err();
        assert!(render.is_render_template());
        let err = render.as_render_template().unwrap();
        assert_eq!(
            err.reason().to_string(),
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

        let render = render_patina(&patina, None);

        assert!(render.is_err());
        assert!(render.unwrap_err().is_render_template());
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

        let render = render_patina(&patina, None);
        assert!(render.is_ok());
        assert_eq!(
            render.unwrap()[0].render_str,
            "This file has {{ escaped }} handlebars\n"
        );
    }
}
