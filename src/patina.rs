//! Core [Patina] data structure and functions.

use std::path::{Path, PathBuf};

use patina_file::PatinaFile;
use serde::{Deserialize, Serialize};

use crate::utils::{normalize_path, Error, Result};

pub mod patina_file;
mod vars;

/// A [Patina] describes a set of variables and templates that can be rendered to files.
#[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Patina {
    /// The name of the Patina
    pub name: String,

    /// A short description of the Patina
    #[serde(default)]
    pub description: String,

    /// A map of variables that can be used in the templates
    #[serde(default)]
    pub vars: Option<serde_json::Value>,

    /// A list of files referencing templates and their target output paths
    #[serde(default)]
    pub files: Vec<PatinaFile>,

    /// The path to this patina
    #[serde(skip)]
    pub base_path: Option<PathBuf>,
}

impl Patina {
    /// Load a Patina from a TOML file
    pub fn from_toml_file(toml_file_path: &PathBuf) -> Result<Patina> {
        let toml_str = match std::fs::read_to_string(toml_file_path) {
            Ok(toml_str) => toml_str,
            Err(e) => return Err(Error::FileRead(toml_file_path.clone(), e)),
        };

        let mut patina: Patina = match toml::from_str(&toml_str) {
            Ok(patina) => patina,
            Err(e) => return Err(Error::TomlParse(e)),
        };

        patina.base_path = Some(toml_file_path.parent().unwrap().to_path_buf());

        Ok(patina)
    }

    /// Get a path within the context of this Patina
    pub fn get_patina_path<P: AsRef<Path>>(&self, path: P) -> PathBuf {
        let path = path.as_ref();
        let path = normalize_path(path).unwrap_or(path.to_path_buf());

        if path.is_absolute() {
            return path.to_path_buf();
        }

        let mut result = self
            .base_path
            .as_ref()
            .unwrap_or(&PathBuf::from("."))
            .clone();
        result.push(path);

        normalize_path(&result).unwrap_or(result)
    }

    /// Get an iterator for all PatinaFiles that are tagged with any of the provided tags
    pub fn files_for_tags(&self, tags: Option<Vec<String>>) -> impl Iterator<Item = &PatinaFile> {
        self.files.iter().filter(move |f| match &tags {
            Some(tags) => f.tags.iter().any(|t| tags.contains(t)),
            None => true,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{tests::test_utils::TmpTestDir, utils::tests::get_home_dir};

    #[test]
    fn test_patina_deserialize() {
        let patina = r#"
            name = "simple-patina"
            description = "This is a simple Patina example"

            [vars]
            name = "Patina"

            [[files]]
            template = "./templates/hello.txt"
            target = "./output/hello.txt"
        "#;

        let patina = toml::from_str::<Patina>(patina);
        assert!(patina.is_ok());
        let patina = patina.unwrap();

        assert_eq!(patina.name, "simple-patina");
        assert_eq!(patina.description, "This is a simple Patina example");

        // assert vars
        assert!(patina.vars.is_some());
        let vars = patina.vars.unwrap();
        assert!(vars.is_object());
        assert_eq!(vars.as_object().unwrap().get("name").unwrap(), "Patina");

        let patina_files = patina.files;
        assert_eq!(patina_files.len(), 1);
        let patina_file = &patina_files[0];
        assert_eq!(patina_file.template, PathBuf::from("./templates/hello.txt"));
        assert_eq!(patina_file.target, PathBuf::from("./output/hello.txt"));
    }

    #[test]
    fn test_patina_deserialize_name_missing() {
        let patina = r#"
            description = "This is a simple Patina example"

            [vars]
            name = "Patina"

            [[files]]
            template = "./templates/hello.txt"
            target = "./output/hello.txt"
        "#;

        let patina = toml::from_str::<Patina>(patina);
        assert!(patina.is_err());
        assert_eq!(patina.unwrap_err().message(), "missing field `name`");
    }

    #[test]
    fn test_patina_deserialize_default_description() {
        let patina = r#"
            name = "simple-patina"

            [vars]
            name = "Patina"

            [[files]]
            template = "./templates/hello.txt"
            target = "./output/hello.txt"
        "#;

        let patina = toml::from_str::<Patina>(patina);
        assert!(patina.is_ok());
        let patina = patina.unwrap();

        assert!(patina.description.is_empty());
    }

    #[test]
    fn test_patina_deserialize_default_files() {
        let patina = r#"
            name = "simple-patina"
            description = "This is a simple Patina example"

            [vars]
            name = "Patina"
        "#;

        let patina = toml::from_str::<Patina>(patina);
        assert!(patina.is_ok());
        let patina = patina.unwrap();

        assert!(patina.files.is_empty());
    }

    #[test]
    fn test_patina_file_template_missing() {
        let patina = r#"
            name = "simple-patina"
            description = "This is a simple Patina example"

            [vars]
            name = "Patina"

            [[files]]
            target = "./output/hello.txt"
        "#;

        let patina = toml::from_str::<Patina>(patina);
        assert!(patina.is_err());
        assert_eq!(patina.unwrap_err().message(), "missing field `template`");
    }

    #[test]
    fn test_patina_file_target_missing() {
        let patina = r#"
            name = "simple-patina"
            description = "This is a simple Patina example"

            [vars]
            name = "Patina"

            [[files]]
            template = "./templates/hello.txt"
        "#;

        let patina = toml::from_str::<Patina>(patina);
        assert!(patina.is_err());
        assert_eq!(patina.unwrap_err().message(), "missing field `target`");
    }

    #[test]
    fn test_patina_from_toml_file() {
        let tmp_dir = TmpTestDir::new();
        let path = tmp_dir.write_file(
            "patina.toml",
            r#"
                name = "simple-patina"
                description = "This is a simple Patina example"

                [vars]
                name = "Patina"

                [[files]]
                template = "../../examples/simple/templates/hello.txt"
                target = "./output/hello.txt"
            "#,
        );

        let patina = Patina::from_toml_file(&path);
        assert!(patina.is_ok());
        let patina = patina.unwrap();
        assert_eq!(patina.name, "simple-patina");
    }

    #[test]
    fn test_patina_from_toml_file_missing_file() {
        let path = PathBuf::from("this/file/does/not/exist.toml");
        let patina = Patina::from_toml_file(&path);

        let err = match patina {
            Ok(_) => panic!("expected error"),
            Err(e) => e,
        };

        let (path, err) = match err.as_file_read() {
            Some(err) => err,
            None => panic!("expected FileRead error"),
        };
        assert_eq!(path, &PathBuf::from("this/file/does/not/exist.toml"));
        assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
    }

    #[test]
    fn test_patina_from_toml_file_invalid_format() {
        let tmp_dir = TmpTestDir::new();
        let path = tmp_dir.write_file(
            "invalid_patina.toml",
            r#"
                description = "This is a simple Patina example"

                [vars]
                name = "Patina"

                [[files]]
                template = "examples/simple/templates/hello.txt"
                target = "./output/hello.txt"
            "#,
        );

        let patina = Patina::from_toml_file(&path);
        assert!(patina.is_err());
        let err = patina.unwrap_err();
        assert!(matches!(err, Error::TomlParse(_)));
    }

    #[test]
    fn test_patina_get_patina_path_in_home_dir() {
        let patina = Patina {
            name: "test".to_string(),
            description: "".to_string(),
            base_path: Some(PathBuf::from("tests")),
            vars: None,
            files: vec![],
        };

        let result = patina.get_patina_path(PathBuf::from("~/.dotpatina/home-dir-test.txt"));
        assert_eq!(
            PathBuf::from(format!("{}/.dotpatina/home-dir-test.txt", get_home_dir())),
            result
        );
    }

    #[test]
    fn test_patina_get_patina_path_absolute_dir() {
        let patina = Patina {
            name: "test".to_string(),
            description: "".to_string(),
            base_path: Some(PathBuf::from("tests")),
            vars: None,
            files: vec![],
        };

        let result = patina.get_patina_path(PathBuf::from("/tmp/dotpatina/absolute-test.txt"));
        assert_eq!(PathBuf::from("/tmp/dotpatina/absolute-test.txt"), result);
    }

    #[test]
    fn test_patina_get_patina_path_combine_parts() {
        let patina = Patina {
            name: "test".to_string(),
            description: "".to_string(),
            base_path: Some(PathBuf::from("tests")),
            vars: None,
            files: vec![],
        };

        let result = patina.get_patina_path(PathBuf::from("some-dir/test.txt"));
        assert_eq!(PathBuf::from("tests/some-dir/test.txt"), result);
    }

    #[test]
    fn test_patina_files_for_tags() {
        let patina = Patina {
            name: "test".to_string(),
            description: "".to_string(),
            base_path: None,
            vars: None,
            files: vec![
                PatinaFile {
                    template: PathBuf::from("a.hbs"),
                    target: PathBuf::from("a.txt"),
                    tags: vec!["a".to_string()],
                    preserve_permissions: false,
                },
                PatinaFile {
                    template: PathBuf::from("b.hbs"),
                    target: PathBuf::from("b.txt"),
                    tags: vec!["b".to_string()],
                    preserve_permissions: false,
                },
                PatinaFile {
                    template: PathBuf::from("ab.hbs"),
                    target: PathBuf::from("ab.txt"),
                    tags: vec!["a".to_string(), "b".to_string()],
                    preserve_permissions: false,
                },
            ],
        };

        let patina_file_a = &patina.files[0];
        let patina_file_b = &patina.files[1];
        let patina_file_ab = &patina.files[2];

        let tags = None;
        let filter_none = patina.files_for_tags(tags).collect::<Vec<&PatinaFile>>();
        assert_eq!(filter_none.len(), 3);

        let tags = Some(vec!["a".to_string()]);
        let filter_a: Vec<&PatinaFile> = patina.files_for_tags(tags).collect();
        assert_eq!(filter_a.len(), 2);
        assert_eq!(filter_a[0], patina_file_a);
        assert_eq!(filter_a[1], patina_file_ab);

        let tags = Some(vec!["b".to_string()]);
        let filter_b: Vec<&PatinaFile> = patina.files_for_tags(tags).collect();
        assert_eq!(filter_b.len(), 2);
        assert_eq!(filter_b[0], patina_file_b);
        assert_eq!(filter_b[1], patina_file_ab);

        let tags = Some(vec!["a".to_string(), "b".to_string()]);
        let filter_ab: Vec<&PatinaFile> = patina.files_for_tags(tags).collect();
        assert_eq!(filter_ab.len(), 3);
        assert_eq!(filter_ab[0], patina_file_a);
        assert_eq!(filter_ab[1], patina_file_b);
        assert_eq!(filter_ab[2], patina_file_ab);
    }
}
