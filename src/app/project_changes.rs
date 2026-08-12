#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PatchLineKind {
    Add,
    Delete,
    Context,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PatchLine {
    pub kind: PatchLineKind,
    pub content: String,
    pub old_line: Option<u32>,
    pub new_line: Option<u32>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PatchHunk {
    pub old_start: u32,
    pub new_start: u32,
    pub heading: String,
    pub lines: Vec<PatchLine>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FilePatch {
    pub path: String,
    pub hunks: Vec<PatchHunk>,
}

/// Extract document paths, Markdown section context and line numbers from a
/// unified Git patch. Binary changes and patches without hunks are omitted.
pub(crate) fn parse_document_patch(patch: &str) -> Vec<FilePatch> {
    let mut files = Vec::<FilePatch>::new();
    let mut file_index = None;
    let mut hunk_index = None;
    let mut old_line = 0;
    let mut new_line = 0;

    for line in patch.lines() {
        if line.starts_with("diff --git ") {
            files.push(FilePatch {
                path: String::new(),
                hunks: Vec::new(),
            });
            file_index = Some(files.len() - 1);
            hunk_index = None;
            continue;
        }
        let Some(current_file) = file_index else {
            continue;
        };
        if let Some(path) = line.strip_prefix("+++ ") {
            let path = path.trim();
            if path != "/dev/null" {
                files[current_file].path = path.strip_prefix("b/").unwrap_or(path).to_string();
            }
            continue;
        }
        if files[current_file].path.is_empty() {
            if let Some(path) = line.strip_prefix("--- ") {
                let path = path.trim();
                if path != "/dev/null" {
                    files[current_file].path = path.strip_prefix("a/").unwrap_or(path).to_string();
                }
                continue;
            }
        }
        if let Some((old_start, new_start, heading)) = parse_hunk_header(line) {
            old_line = old_start;
            new_line = new_start;
            files[current_file].hunks.push(PatchHunk {
                old_start,
                new_start,
                heading,
                lines: Vec::new(),
            });
            hunk_index = Some(files[current_file].hunks.len() - 1);
            continue;
        }
        let Some(current_hunk) = hunk_index else {
            continue;
        };
        if line.starts_with("\\ No newline") || line.is_empty() {
            continue;
        }
        let marker = line.as_bytes()[0];
        let content = line[1..].to_string();
        let hunk = &mut files[current_file].hunks[current_hunk];
        if marker != b'-'
            && hunk
                .lines
                .iter()
                .all(|item| item.kind == PatchLineKind::Context)
        {
            if let Some(heading) = markdown_heading(&content) {
                hunk.heading = heading;
            }
        }
        match marker {
            b'+' => {
                hunk.lines.push(PatchLine {
                    kind: PatchLineKind::Add,
                    content,
                    old_line: None,
                    new_line: Some(new_line),
                });
                new_line += 1;
            }
            b'-' => {
                hunk.lines.push(PatchLine {
                    kind: PatchLineKind::Delete,
                    content,
                    old_line: Some(old_line),
                    new_line: None,
                });
                old_line += 1;
            }
            b' ' => {
                hunk.lines.push(PatchLine {
                    kind: PatchLineKind::Context,
                    content,
                    old_line: Some(old_line),
                    new_line: Some(new_line),
                });
                old_line += 1;
                new_line += 1;
            }
            _ => {}
        }
    }

    files.retain(|file| !file.path.is_empty() && !file.hunks.is_empty());
    files
}

fn parse_hunk_header(line: &str) -> Option<(u32, u32, String)> {
    let rest = line.strip_prefix("@@ -")?;
    let (old_range, rest) = rest.split_once(" +")?;
    let (new_range, heading) = rest.split_once(" @@")?;
    let old_start = old_range.split(',').next()?.parse().ok()?;
    let new_start = new_range.split(',').next()?.parse().ok()?;
    Some((old_start, new_start, heading.trim().to_string()))
}

fn markdown_heading(line: &str) -> Option<String> {
    let trimmed = line.trim_start();
    let level = trimmed.bytes().take_while(|byte| *byte == b'#').count();
    if !(1..=6).contains(&level) || trimmed.as_bytes().get(level) != Some(&b' ') {
        return None;
    }
    let heading = trimmed[level + 1..].trim().trim_end_matches('#').trim();
    (!heading.is_empty()).then(|| heading.to_string())
}

#[cfg(test)]
mod tests {
    use super::{parse_document_patch, PatchLineKind};

    #[test]
    fn parses_document_location_and_changed_lines() {
        let patch = r#"diff --git a/docs/auth.md b/docs/auth.md
index 111..222 100644
--- a/docs/auth.md
+++ b/docs/auth.md
@@ -8,4 +8,5 @@
 ## Token 刷新
 旧说明
-过期后重新登录
+过期前自动刷新
+失败后重新登录
"#;
        let files = parse_document_patch(patch);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, "docs/auth.md");
        let hunk = &files[0].hunks[0];
        assert_eq!(hunk.heading, "Token 刷新");
        assert_eq!(hunk.new_start, 8);
        assert!(hunk.lines.iter().any(|line| {
            line.kind == PatchLineKind::Add
                && line.new_line == Some(10)
                && line.content == "过期前自动刷新"
        }));
        assert!(hunk.lines.iter().any(|line| {
            line.kind == PatchLineKind::Delete
                && line.old_line == Some(10)
                && line.content == "过期后重新登录"
        }));
    }

    #[test]
    fn keeps_deleted_file_path_and_ignores_binary_patch() {
        let patch = r#"diff --git a/docs/old.md b/docs/old.md
deleted file mode 100644
--- a/docs/old.md
+++ /dev/null
@@ -1 +0,0 @@
-old
diff --git a/logo.png b/logo.png
Binary files a/logo.png and b/logo.png differ
"#;
        let files = parse_document_patch(patch);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, "docs/old.md");
    }
}
