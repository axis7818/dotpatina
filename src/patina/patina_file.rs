//! [PatinaFile] and related functionality

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// A [PatinaFile] describes a template file and its target output path.
#[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PatinaFile {
    /// An optional list of tags for this patina file. This allows subsets of files to be specified from the command line
    #[serde(default)]
    pub tags: Vec<String>,

    /// The path to the template file
    pub template: PathBuf,

    /// The path to the garget output file
    pub target: PathBuf,

    /// When true, the template file's permissions are copied to the target file after writing.
    /// Useful for executable scripts.
    #[serde(default)]
    pub preserve_permissions: bool,
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    impl PatinaFile {
        pub fn new<P: AsRef<Path>>(template: P, target: P) -> PatinaFile {
            let template = template.as_ref().to_path_buf();
            let target = target.as_ref().to_path_buf();
            PatinaFile {
                template,
                target,
                tags: vec![],
                preserve_permissions: false,
            }
        }

        pub fn new_with_tags<P: AsRef<Path>>(
            template: P,
            target: P,
            tags: Vec<&str>,
        ) -> PatinaFile {
            let mut result = PatinaFile::new(template, target);
            result.tags = tags.iter().map(|s| s.to_string()).collect();
            result
        }
    }

    #[test]
    fn test_patina_file_new() {
        let patina_file = PatinaFile::new("template.txt", "target.txt");

        assert_eq!(PathBuf::from("template.txt"), patina_file.template);
        assert_eq!(PathBuf::from("target.txt"), patina_file.target);
        assert!(patina_file.tags.is_empty());
    }

    #[test]
    fn test_patina_file_with_tags() {
        let patina_file =
            PatinaFile::new_with_tags("template.txt", "target.txt", vec!["aaa", "bbb", "ccc"]);

        assert_eq!(PathBuf::from("template.txt"), patina_file.template);
        assert_eq!(PathBuf::from("target.txt"), patina_file.target);
        assert_eq!(patina_file.tags.len(), 3);
        assert_eq!(patina_file.tags[0], "aaa");
        assert_eq!(patina_file.tags[1], "bbb");
        assert_eq!(patina_file.tags[2], "ccc");
    }
}
