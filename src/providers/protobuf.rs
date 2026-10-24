//! The messages of protobuf files, read with `protox-parse` one file at a
//! time; its design record's
//! [Decision](nexus:markdown.section
//! (design/reads-protobuf-with-protox-parse.md)) says why.

use std::path::{Path, PathBuf};

use prost_types::{DescriptorProto, FileDescriptorProto};

use crate::formats::protobuf::NAME;
use crate::link::Shape;
use crate::location::{LineIndex, Span};
use crate::providers::{Place, Provider, Query, normalize};
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
        &["message"]
    }

    fn shape(&self, _selector: &str) -> Shape {
        Shape::LINK
    }

    fn query(&self, selector: &str, arguments: &[&str], _place: Option<&Place>) -> Result<Box<dyn Query>, String> {
        match arguments {
            [file, name] => Ok(Box::new(ProtobufMessage {
                file: normalize(file),
                name: (*name).to_owned(),
            })),
            _ => Err(format!("`{NAME}.{selector}` takes a file and a message")),
        }
    }
}

/// The message of a protobuf file with a name, `<outer>.<message>` for a
/// nested one.
#[derive(Debug)]
struct ProtobufMessage {
    file: PathBuf,
    name: String,
}

impl Query for ProtobufMessage {
    fn resolve(&self, workspace: &Workspace) -> Vec<Span> {
        find_message(&workspace.root, &self.file, &self.name)
    }
}

/// The span of the message called [`name`](nexus:rust.argument) in the
/// protobuf file at [`file`](nexus:rust.argument), relative to
/// [`root`](nexus:rust.argument), from its keyword to its closing brace. A
/// nested message is named after the messages it is nested in,
/// `<outer>.<message>`, without the package.
fn find_message(root: &Path, file: &Path, name: &str) -> Vec<Span> {
    let Ok(text) = std::fs::read_to_string(root.join(file)) else {
        return Vec::new();
    };
    let Ok(descriptor) = protox_parse::parse(&file.to_string_lossy(), &text) else {
        return Vec::new();
    };
    let Some(path) = messages(&descriptor)
        .into_iter()
        .find_map(|(message_name, path)| (message_name == name).then_some(path))
    else {
        return Vec::new();
    };
    let Some(location) = descriptor
        .source_code_info
        .iter()
        .flat_map(|info| &info.location)
        .find(|location| location.path == path)
    else {
        return Vec::new();
    };
    let line_index = LineIndex::new(&text);
    byte_range(&line_index, &location.span)
        .filter(|range| range.start <= range.end && range.end <= text.len())
        .map(|range| line_index.span(file, range))
        .into_iter()
        .collect()
}

/// Every message of a file, nested ones included, with the name a link
/// gives it and the path of its source code info location.
fn messages(file: &FileDescriptorProto) -> Vec<(String, Vec<i32>)> {
    let mut found = Vec::new();
    collect_messages(&file.message_type, "", &[MESSAGE_TYPE], &mut found);
    found
}

fn collect_messages(messages: &[DescriptorProto], prefix: &str, path: &[i32], found: &mut Vec<(String, Vec<i32>)>) {
    for (index, message) in messages.iter().enumerate() {
        let name = format!("{prefix}{}", message.name());
        let mut message_path = path.to_vec();
        message_path.push(i32::try_from(index).unwrap_or(i32::MAX));
        let nested_path: Vec<i32> = message_path.iter().copied().chain([NESTED_TYPE]).collect();
        collect_messages(&message.nested_type, &format!("{name}."), &nested_path, found);
        found.push((name, message_path));
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

    fn message_spans(root: &Path, name: &str) -> Vec<String> {
        find_message(root, Path::new("proto/window.proto"), name)
            .iter()
            .map(ToString::to_string)
            .collect()
    }

    #[test]
    fn finds_top_level_and_nested_messages() {
        let root = std::env::temp_dir().join(format!("nexus-protobuf-{}", std::process::id()));
        let path = root.join("proto/window.proto");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(
            path,
            "syntax = \"proto3\";\npackage example.window;\n\nmessage OpenWindow {\n  string title = 1;\n  message Size {\n    uint32 \
             width = 1;\n  }\n}\n\nmessage Ok { uint64 view_id = 1; }\n",
        )
        .unwrap();
        assert_eq!(message_spans(&root, "OpenWindow"), vec!["proto/window.proto:4:1-9:1"]);
        assert_eq!(message_spans(&root, "OpenWindow.Size"), vec!["proto/window.proto:6:3-8:3"]);
        assert_eq!(message_spans(&root, "Ok"), vec!["proto/window.proto:11:1-11:34"]);
        assert!(message_spans(&root, "Size").is_empty());
        assert!(message_spans(&root, "example.window.Ok").is_empty());
        assert!(message_spans(&root, "Missing").is_empty());
        std::fs::remove_dir_all(root).unwrap();
    }
}
