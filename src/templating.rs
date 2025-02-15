use handlebars::Handlebars;
use log::info;

use crate::patina::{Patina, PatinaFile};
use crate::utils::{Error, Result};

/// Renders all of the files in a Patina, each to a string in the result vector.
pub fn render_patina(patina: &Patina) -> Result<Vec<String>> {
    let hb = Handlebars::new();
    patina
        .files
        .iter()
        .map(|pf| render_patina_file(&hb, patina, pf))
        .collect()
}

fn render_patina_file(
    hb: &Handlebars,
    patina: &Patina,
    patina_file: &PatinaFile,
) -> Result<String> {
    info!("rendering patina file: {}", patina_file.template.display());

    let template_str = patina_file.load_template_file_as_string()?;

    match hb.render_template(&template_str, &patina.vars) {
        Ok(render) => Ok(render),
        Err(e) => Err(Error::RenderTemplate(e)),
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use serde_json::json;

    use super::*;

    #[test]
    fn test_render_patina() {
        let patina = Patina {
            name: String::from("sample-patina"),
            description: String::from("This is a sample Patina"),
            vars: Some(json!({
                "name": {
                    "first": "Patina",
                    "last": "User"
                }
            })),
            files: vec![PatinaFile {
                template: PathBuf::from("tests/fixtures/template.txt.hbs"),
                target: PathBuf::from("tests/fixtures/template.txt"),
            }],
        };

        let render = render_patina(&patina);

        assert!(render.is_ok());
        let render = render.unwrap();
        assert_eq!(render.len(), 1);
        let render = &render[0];

        assert_eq!(
            render,
            r#"Hello, Patina User!
This is an example Patina template file.
Templates use the Handebars templating language. For more information, see <https://handlebarsjs.com/guide/>.
"#
        );
    }

    #[test]
    fn test_render_patina_multiple_templates() {
        let patina = Patina {
            name: String::from("multi-template-patina"),
            description: String::from("This is a patina with multiple templates"),
            vars: Some(json!({
                "A": "template_a",
                "B": "template_b",
                "C": "template_c",
            })),
            files: vec![
                PatinaFile {
                    template: PathBuf::from("tests/fixtures/template_a.txt.hbs"),
                    target: PathBuf::from("output_a.txt"),
                },
                PatinaFile {
                    template: PathBuf::from("tests/fixtures/template_b.txt.hbs"),
                    target: PathBuf::from("output_b.txt"),
                },
                PatinaFile {
                    template: PathBuf::from("tests/fixtures/template_c.txt.hbs"),
                    target: PathBuf::from("output_c.txt"),
                },
            ],
        };

        let render = render_patina(&patina);

        assert!(render.is_ok());
        let render = render.unwrap();

        assert_eq!(render.len(), 3);
        assert_eq!(render[0], "This is template_a.\n");
        assert_eq!(render[1], "This is template_b.\n");
        assert_eq!(render[2], "This is template_c.\n");
    }

    #[test]
    fn test_render_patina_missing_variable() {
        let patina = Patina {
            name: String::from("sample-patina"),
            description: String::from("This is a sample Patina"),
            vars: Some(json!({})),
            files: vec![PatinaFile {
                template: PathBuf::from("tests/fixtures/template.txt.hbs"),
                target: PathBuf::from("tests/fixtures/template.txt"),
            }],
        };

        let render = render_patina(&patina);

        assert!(render.is_ok());
        let render = render.unwrap();
        assert_eq!(render.len(), 1);
        let render = &render[0];

        let expected = r#"Hello,  !
This is an example Patina template file.
Templates use the Handebars templating language. For more information, see <https://handlebarsjs.com/guide/>.
"#;
        assert_eq!(expected, render);
    }

    #[test]
    fn test_render_patina_invalid_template() {
        let patina = Patina {
            name: String::from("sample-patina"),
            description: String::from("This is a sample Patina"),
            vars: Some(json!({})),
            files: vec![PatinaFile {
                template: PathBuf::from("tests/fixtures/invalid_template.txt.hbs"),
                target: PathBuf::from("tests/fixtures/template.txt"),
            }],
        };

        let render = render_patina(&patina);

        assert!(render.is_err());
        assert!(render.unwrap_err().is_render_template());
    }
}
