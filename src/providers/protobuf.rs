//! The messages of protobuf files and their fields, read with `protox-parse`
//! one file at a time; its design record's
//! [Decision](nexus:markdown.section
//! (design/reads-protobuf-with-protox-parse.md)) says why.

use std::path::{Path, PathBuf};

use prost_types::{DescriptorProto, FileDescriptorProto};

use crate::formats::protobuf::NAME;
use crate::link::{Member, Shape};
use crate::location::{LineIndex, Span};
use crate::providers::{MemberError, Place, Provider, Query, check_complete_map, normalize};
use crate::workspace::Workspace;

/// The field numbers of `FileDescriptorProto.message_type` and
/// `DescriptorProto.nested_type`, which source code info locations are
/// paths of.
const MESSAGE_TYPE: i32 = 4;
const NESTED_TYPE: i32 = 3;

pub struct Protobuf;

impl Provider for Protobuf {
    fn name(&self) -> &'static str {
        NAME
    }

    fn selectors(&self) -> &'static [&'static str] {
        &["message", "map-field"]
    }

    fn shape(&self, selector: &str) -> Shape {
        Shape {
            map: selector == "map-field",
            takes_arguments: true,
        }
    }

    fn query(&self, selector: &str, arguments: &[&str], _place: Option<&Place>) -> Result<Box<dyn Query>, String> {
        let [file, name] = arguments else {
            return Err(format!("`{NAME}.{selector}` takes a file and a message"));
        };
        let file = normalize(file);
        let name = (*name).to_owned();
        Ok(Box::new(match selector {
            "message" => ProtobufQuery::Message { file, name },
            _ => ProtobufQuery::MapField { file, name },
        }))
    }
}

/// A message of a protobuf file by its name, `<outer>.<message>` for a
/// nested one.
#[derive(Debug)]
enum ProtobufQuery {
    Message {
        file: PathBuf,
        name: String,
    },
    /// The fields of a message, as complete map.
    MapField {
        file: PathBuf,
        name: String,
    },
}

impl Query for ProtobufQuery {
    fn resolve(&self, workspace: &Workspace) -> Vec<Span> {
        let (Self::Message { file, name } | Self::MapField { file, name }) = self;
        find_message(&workspace.root, file, name)
            .map(|(span, _)| span)
            .into_iter()
            .collect()
    }

    /// A message that is not found has no fields to check, its link points
    /// at nothing.
    fn check_members(&self, members: &[Member], workspace: &Workspace) -> Vec<MemberError> {
        match self {
            Self::MapField { file, name } => find_message(&workspace.root, file, name)
                .map(|(_, fields)| check_complete_map(members, &fields))
                .unwrap_or_default(),
            Self::Message { .. } => Vec::new(),
        }
    }
}

/// The message called [`name`](nexus:rust.argument) in the protobuf file
/// at [`file`](nexus:rust.argument), relative to
/// [`root`](nexus:rust.argument): its span, from its keyword to its closing
/// brace, and the names of its fields, those of its `oneof`s included. A
/// nested message is named after the messages it is nested in,
/// `<outer>.<message>`, without the package.
fn find_message(root: &Path, file: &Path, name: &str) -> Option<(Span, Vec<String>)> {
    let text = std::fs::read_to_string(root.join(file)).ok()?;
    let descriptor = protox_parse::parse(&file.to_string_lossy(), &text).ok()?;
    let (path, message) = messages(&descriptor)
        .into_iter()
        .find_map(|(message_name, path, message)| (message_name == name).then_some((path, message)))?;
    let location = descriptor
        .source_code_info
        .iter()
        .flat_map(|info| &info.location)
        .find(|location| location.path == path)?;
    let line_index = LineIndex::new(&text);
    let range = byte_range(&line_index, &location.span).filter(|range| range.start <= range.end && range.end <= text.len())?;
    let fields = message.field.iter().map(|field| field.name().to_owned()).collect();
    Some((line_index.span(file, range), fields))
}

/// Every message of a file, nested ones included, with the name a link
/// gives it and the path of its source code info location.
fn messages(file: &FileDescriptorProto) -> Vec<(String, Vec<i32>, &DescriptorProto)> {
    let mut found = Vec::new();
    collect_messages(&file.message_type, "", &[MESSAGE_TYPE], &mut found);
    found
}

fn collect_messages<'a>(
    messages: &'a [DescriptorProto],
    prefix: &str,
    path: &[i32],
    found: &mut Vec<(String, Vec<i32>, &'a DescriptorProto)>,
) {
    for (index, message) in messages.iter().enumerate() {
        let name = format!("{prefix}{}", message.name());
        let mut message_path = path.to_vec();
        message_path.push(i32::try_from(index).unwrap_or(i32::MAX));
        let nested_path: Vec<i32> = message_path.iter().copied().chain([NESTED_TYPE]).collect();
        collect_messages(&message.nested_type, &format!("{name}."), &nested_path, found);
        found.push((name, message_path, message));
    }
}

/// The byte range of a source code info span: a start line and column, an
/// end line if it differs, and an end column, all counted from 0, columns
/// in bytes, the end excluded.
fn byte_range(line_index: &LineIndex, span: &[i32]) -> Option<std::ops::Range<usize>> {
    let span: Vec<usize> = span.iter().map(|&number| usize::try_from(number).ok()).collect::<Option<_>>()?;
    let (start_line, start_column, end_line, end_column) = match span[..] {
        [line, start_column, end_column] => (line, start_column, line, end_column),
        [start_line, start_column, end_line, end_column] => (start_line, start_column, end_line, end_column),
        _ => return None,
    };
    let line_starts: Vec<usize> = line_index.lines().map(|line| line.start).collect();
    Some(line_starts.get(start_line)? + start_column..line_starts.get(end_line)? + end_column)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::tests::{assert_query, read_first_link};

    fn message_spans(root: &Path, name: &str) -> Vec<String> {
        find_message(root, Path::new("proto/window.proto"), name)
            .iter()
            .map(|(span, _)| span.to_string())
            .collect()
    }

    fn message_fields(root: &Path, name: &str) -> Option<Vec<String>> {
        find_message(root, Path::new("proto/window.proto"), name).map(|(_, fields)| fields)
    }

    fn write_file(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("nexus-protobuf-{name}-{}", std::process::id()));
        let path = root.join("proto/window.proto");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(
            path,
            concat!(
                "syntax = \"proto3\";\npackage example.window;\n\n",
                "message OpenWindow {\n  string title = 1;\n  message Size {\n    uint32 width = 1;\n  }\n}\n\n",
                "message Ok { uint64 view_id = 1; }\n\n",
                "message Event {\n  oneof kind {\n    string key = 1;\n    Ok opened = 2;\n  }\n  map<string, string> labels = 3;\n}\n",
            ),
        )
        .unwrap();
        root
    }

    #[test]
    fn finds_top_level_and_nested_messages() {
        let root = write_file("messages");
        assert_eq!(message_spans(&root, "OpenWindow"), vec!["proto/window.proto:4:1-9:1"]);
        assert_eq!(message_spans(&root, "OpenWindow.Size"), vec!["proto/window.proto:6:3-8:3"]);
        assert_eq!(message_spans(&root, "Ok"), vec!["proto/window.proto:11:1-11:34"]);
        assert!(message_spans(&root, "Size").is_empty());
        assert!(message_spans(&root, "example.window.Ok").is_empty());
        assert!(message_spans(&root, "Missing").is_empty());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn finds_the_fields_of_messages() {
        let root = write_file("fields");
        let owned = |names: &[&str]| Some(names.iter().map(|&name| name.to_owned()).collect());
        assert_eq!(message_fields(&root, "OpenWindow"), owned(&["title"]));
        assert_eq!(message_fields(&root, "OpenWindow.Size"), owned(&["width"]));
        assert_eq!(message_fields(&root, "Event"), owned(&["key", "opened", "labels"]));
        assert_eq!(message_fields(&root, "Missing"), None);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_map_of_fields_takes_a_file_a_message_and_a_body() {
        let (query, members) = read_first_link(
            "test.md",
            "<!-- nexus protobuf.map-field proto/window.proto Ok -->\n- view_id: its view\n<!-- /nexus -->\n",
        )
        .unwrap();
        assert_query(query.as_ref(), &ProtobufQuery::MapField {
            file: PathBuf::from("proto/window.proto"),
            name: "Ok".to_owned(),
        });
        assert_eq!(members[0].name, "view_id");
        assert!(Protobuf.query("map-field", &["proto/window.proto"], None).is_err());
    }
}
