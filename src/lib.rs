#[cfg(test)]
mod test;

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::hash::Hash;
use std::io::Write;
use std::path::{Path, PathBuf};

use clap::{Parser, Subcommand};
use itertools::Itertools;
use log::*;
use tree_sitter::TreeCursor;

/// Group imports in workspace source files.
///
/// This roughly corresponds to the `group_imports` unstable rustfmt option, with the difference
/// that `rustfmt` does not distinguish workspace crates from external ones.
///
/// By default, displays a diff without applying changes. Returns code 0 when no changes are
/// necessary.
/// The --fix flag allows applying the changes.
///
/// See
/// https://rust-lang.github.io/rustfmt/?version=v1.4.38&search=#group_imports
/// https://github.com/rust-lang/rustfmt/blob/master/src/reorder.rs
#[derive(Parser)]
#[clap(verbatim_doc_comment)]
pub struct MainFlags {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    GroupImports(Flags),
}

#[derive(Parser)]
pub struct Flags {
    #[clap(default_value_os_t = std::env::current_dir().unwrap())]
    pub workspace: PathBuf,
    /// Apply changes
    #[clap(long)]
    pub fix: bool,
    #[clap(skip = true)]
    pub rustfmt: bool,
}

#[derive(Default, Debug)]
struct Use {
    start: tree_sitter::Point,
    end: tree_sitter::Point,
    contents: String,
    module: String,
    module_decl: bool,
}
/// This defines the order
#[derive(Ord, PartialOrd, Eq, PartialEq, Hash, Copy, Clone, Debug)]
enum UseType {
    Module,
    Std,
    External,
    Workspace,
    Crate,
}

fn node_as_utf8(node: tree_sitter::Node<'_>, source: &str) -> anyhow::Result<String> {
    Ok(node.utf8_text(source.as_bytes())?.to_string())
}
/// Process a `use` or `mod` line, extracting the module name, comments, and attributes.
fn process_line(cursor: &mut TreeCursor, source: &str) -> anyhow::Result<Use> {
    let node = cursor.node();
    let mut u = Use {
        start: node.range().start_point,
        end: node.range().end_point,
        module_decl: node.kind() == "mod_item",
        ..Default::default()
    };
    let mut contents = vec![node_as_utf8(node, source)?];
    // Include comments and cfg
    let mut sibling = node;
    while let Some(s) = sibling.prev_sibling() {
        sibling = s;

        if ["line_comment", "attribute_item", "inner_attribute_item"].contains(&sibling.kind()) {
            let content = node_as_utf8(sibling, source)?;
            // Don't take module-level comments along
            if !content.starts_with("//!") {
                u.start = sibling.range().start_point;
                contents.push(content);
            }
        } else {
            break;
        }
    }
    u.contents = contents.into_iter().rev().join("\n");
    // Find module
    cursor.goto_first_child();
    while cursor.goto_next_sibling() {
        // TODO: Handle `use_as_clause`
        if [
            "identifier",
            "scoped_identifier",
            "use_wildcard",
            "scoped_use_list",
        ]
        .contains(&cursor.node().kind())
        {
            u.module = node_as_utf8(cursor.node(), source)?
                .split("::")
                .find(|s| !s.is_empty())
                .unwrap()
                .to_string();
            break;
        }
    }
    cursor.goto_parent();
    Ok(u)
}
/// Returns whether the file has been changed, or would have been changed.
pub fn process_file(
    filename: &Path,
    package_name: &str,
    workspace_packages: &HashSet<String>,
    args: &Flags,
) -> anyhow::Result<bool> {
    // Phase 1: parse with tree-sitter
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(tree_sitter_rust::language())?;
    let source = std::fs::read_to_string(filename)?;
    let tree = parser.parse(&source, None).unwrap();

    // Phase 2: find `use` and `mod` statements
    let mut uses: Vec<Use> = vec![];
    let mut mods_names = HashSet::<String>::default();
    let mut macros_defs = HashSet::<String>::default();
    let mut cursor = tree.walk();
    cursor.goto_first_child();
    loop {
        let node = cursor.node();
        if node.kind() == "macro_definition" {
            cursor.goto_first_child();
            cursor.goto_next_sibling();
            macros_defs.insert(node_as_utf8(cursor.node(), &source)?);
            cursor.goto_parent();
        }
        // Use node
        if node.kind() == "use_declaration" {
            uses.push(process_line(&mut cursor, &source)?);
        } else if node.kind() == "mod_item" {
            let mut decl_list = false;
            cursor.goto_first_child();
            loop {
                if cursor.node().kind() == "identifier" {
                    mods_names.insert(node_as_utf8(cursor.node(), &source)?);
                } else if cursor.node().kind() == "declaration_list" {
                    decl_list = true;
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
            cursor.goto_parent();
            if !decl_list {
                uses.push(process_line(&mut cursor, &source)?);
            }
            // TODO: Look into sub-modules
        } else {
            match uses.last() {
                Some(u) if node.range().start_point.row == u.end.row => {
                    // Simplification for the deletion later.
                    anyhow::bail!(
                        "use or mod expression on line {} contains another expression. This is unsupported.",
                        u.end.row
                    );
                }
                _ => {}
            }
        }
        if !cursor.goto_next_sibling() {
            break;
        }
    }
    debug!("Macros: {:?}", macros_defs);
    // Special case of macros_rules declarations, where the pub use must be after the definition.
    uses.retain(|u| !macros_defs.contains(&u.module));
    debug!("Modules: {:?}", mods_names);

    // Phase 3: Group imports
    let mut grouped = BTreeMap::<UseType, Vec<&Use>>::default();
    for u in &uses {
        let import_type = if u.module == "std" {
            UseType::Std
        } else if u.module == package_name || u.module == "crate" || u.module == "super" {
            UseType::Crate
        } else if mods_names.contains(&u.module) || u.module_decl || u.module == "self" {
            UseType::Module
        } else if workspace_packages.contains(u.module.as_str()) {
            UseType::Workspace
        } else {
            UseType::External
        };
        grouped.entry(import_type).or_default().push(u);
    }
    debug!("Grouped uses {:#?}", grouped);

    // Phase 4: Insert into source file
    let imports = grouped
        .values()
        .map(|uses| {
            uses.iter()
                .map(|u| &u.contents)
                .chain(std::iter::once(&Default::default()))
                .join("\n")
        })
        .join("\n");

    let lines: BTreeSet<usize> = grouped
        .values()
        .flatten()
        .flat_map(|l| (l.start.row..=l.end.row))
        .collect();
    let mut source_modified = source
        .lines()
        .enumerate()
        .filter_map(|(i, l)| {
            if lines.iter().next() == Some(&i) {
                Some(imports.as_str())
            } else if
            // We ensured earlier that these lines do not contain anything else
            lines.contains(&i)
                ||
            // Remove previous spacing
            l.is_empty() && (i > 0 && lines.contains(&(i - 1)))
            {
                None
            } else {
                Some(l)
            }
        })
        // New line at end
        .chain(std::iter::once(""))
        .join("\n");

    // Phase 4: Run rustfmt; this should not be needed in most cases.
    // TODO: Ensure it is not needed. The difference comes from the ordering of
    // super::,crate:: etc. imports. Most of the runtime is due to running rustfmt.
    let modified = source != source_modified;
    if modified && args.rustfmt {
        let mut cmd = std::process::Command::new("rustfmt")
            .current_dir(&args.workspace)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()?;
        let mut stdin = cmd.stdin.take().unwrap();
        stdin.write_all(source_modified.as_bytes())?;
        drop(stdin);
        let out = cmd.wait_with_output()?;
        anyhow::ensure!(out.status.success());
        source_modified = String::from_utf8(out.stdout)?;
    }

    // Phase 5: Write output or diff
    let modified = source != source_modified;
    if modified {
        if !args.fix {
            warn!(
                "Diff in {:?}:\n{}",
                filename,
                prettydiff::diff_lines(&source, &source_modified).format_with_context(
                    Some(prettydiff::text::ContextConfig {
                        context_size: 5,
                        skipping_marker: "..."
                    }),
                    true
                )
            );
        } else {
            std::fs::write(filename, &source_modified)?;
            info!("Wrote {:?}", filename);
        }
    }
    Ok(modified)
}
