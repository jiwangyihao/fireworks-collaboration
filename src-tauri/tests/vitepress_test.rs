use fireworks_collaboration_lib::app::commands::vitepress::{
    vitepress_create_document, vitepress_create_folder, vitepress_delete, vitepress_detect_project,
    vitepress_get_doc_tree, vitepress_read_document, vitepress_rename, DocTreeNodeType,
};
use std::fs;
use tempfile::tempdir;

#[tokio::test]
async fn test_vitepress_detect_project() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    // Case 1: Not a vitepress project
    let result = vitepress_detect_project(root.to_string_lossy().to_string())
        .await
        .unwrap();
    assert!(!result.is_vitepress);

    // Case 2: Is a vitepress project
    let config_dir = root.join(".vitepress");
    fs::create_dir(&config_dir).unwrap();
    fs::write(config_dir.join("config.mts"), "").unwrap();
    fs::write(root.join("package.json"), r#"{"name": "test-project"}"#).unwrap();

    let result = vitepress_detect_project(root.to_string_lossy().to_string())
        .await
        .unwrap();
    assert!(result.is_vitepress);
    assert_eq!(
        result.config_path,
        Some(".vitepress/config.mts".to_string())
    );
    assert_eq!(result.project_name, Some("test-project".to_string()));
}

#[tokio::test]
async fn test_vitepress_crud() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    let root_str = root.to_string_lossy().to_string();

    // 1. Create Document
    let doc_path = vitepress_create_document(root_str.clone(), "test-doc".to_string(), None)
        .await
        .unwrap();

    assert!(fs::exists(&doc_path).unwrap());

    // 2. Read Document
    let content = vitepress_read_document(doc_path.clone(), None, None)
        .await
        .unwrap();
    assert!(content.content.contains("# test-doc"));

    // 3. Rename
    let new_path = vitepress_rename(doc_path.clone(), "renamed-doc.md".to_string())
        .await
        .unwrap();
    assert!(!fs::exists(&doc_path).unwrap());
    assert!(fs::exists(&new_path).unwrap());

    // 4. Delete
    let deleted = vitepress_delete(new_path.clone()).await.unwrap();
    assert!(deleted);
    assert!(!fs::exists(&new_path).unwrap());
}

#[tokio::test]
async fn test_vitepress_folder_and_tree() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    let root_str = root.to_string_lossy().to_string();

    // Create folder structure:
    // /docs
    //   /guide
    //     index.md
    //   api.md

    let docs_dir = root.join("docs");
    fs::create_dir(&docs_dir).unwrap();

    vitepress_create_folder(docs_dir.to_string_lossy().to_string(), "guide".to_string())
        .await
        .unwrap();

    vitepress_create_document(
        docs_dir.to_string_lossy().to_string(),
        "api".to_string(),
        None,
    )
    .await
    .unwrap();

    // Get Tree
    let tree = vitepress_get_doc_tree(root_str.clone(), Some("docs".to_string()))
        .await
        .unwrap();

    assert_eq!(tree.name, "docs");
    assert_eq!(tree.node_type, DocTreeNodeType::Folder);

    let children = tree.children.unwrap();
    // Sorted: Folder "guide" first, then File "api.md"
    assert_eq!(children.len(), 2);
    assert_eq!(children[0].name, "guide");
    assert_eq!(children[0].node_type, DocTreeNodeType::Folder);
    assert_eq!(children[1].name, "api.md");
    assert_eq!(children[1].node_type, DocTreeNodeType::File);
}

#[tokio::test]
async fn test_vitepress_save_document() {
    let dir = tempdir().unwrap();
    let root_str = dir.path().to_string_lossy().to_string();

    let doc_path = vitepress_create_document(root_str.clone(), "save-test.md".to_string(), None)
        .await
        .unwrap();

    use fireworks_collaboration_lib::app::commands::vitepress::vitepress_save_document;

    let new_content = "---\ntitle: Updated Title\n---\n\n# New Content";
    let save_result = vitepress_save_document(doc_path.clone(), new_content.to_string())
        .await
        .unwrap();

    assert!(save_result.success);

    let read_result = vitepress_read_document(doc_path, None, None).await.unwrap();
    assert_eq!(read_result.content, new_content);
}

#[tokio::test]
async fn test_vitepress_frontmatter_parsing() {
    let dir = tempdir().unwrap();

    let content = r#"---
title: "My Awesome Page"
order: 10
draft: true
---
# Body Content
"#;

    let doc_path = dir.path().join("frontmatter-test.md");
    fs::write(&doc_path, content).unwrap();

    let result = vitepress_read_document(doc_path.to_string_lossy().to_string(), None, None)
        .await
        .unwrap();

    let fm = result.frontmatter.unwrap();
    assert_eq!(
        fm.get("title").unwrap().as_str().unwrap(),
        "My Awesome Page"
    );
    assert_eq!(fm.get("order").unwrap().as_i64().unwrap(), 10);
    assert_eq!(fm.get("draft").unwrap().as_bool().unwrap(), true);
}

#[tokio::test]
async fn test_vitepress_title_extraction() {
    let dir = tempdir().unwrap();
    let root_str = dir.path().to_string_lossy().to_string();

    // File 1: Title from Frontmatter
    let fm_doc = dir.path().join("fm-title.md");
    fs::write(
        &fm_doc,
        "---\ntitle: \"Frontmatter Title\"\n---\n\n# Ignored Header",
    )
    .unwrap();

    // File 2: Title from H1
    let h1_doc = dir.path().join("h1-title.md");
    fs::write(&h1_doc, "# Header Title\n\nSome text").unwrap();

    // File 3: No Title (fallback to filename)
    let no_title_doc = dir.path().join("no-title.md");
    fs::write(&no_title_doc, "Just text").unwrap();

    let tree = vitepress_get_doc_tree(root_str, None).await.unwrap();
    let children = tree.children.unwrap();

    // Sort order is by name: fm-title, h1-title, no-title
    let fm_node = children.iter().find(|n| n.name == "fm-title.md").unwrap();
    assert_eq!(fm_node.title, Some("Frontmatter Title".to_string()));

    let h1_node = children.iter().find(|n| n.name == "h1-title.md").unwrap();
    assert_eq!(h1_node.title, Some("Header Title".to_string()));

    let no_title_node = children.iter().find(|n| n.name == "no-title.md").unwrap();
    // Current logic: extract_title returns None if no title found.
    // The tree builder keeps it as None, frontend handles fallback?
    // Let's check `vitepress.rs`:
    // extract_title returns Option<String>.
    // DocTreeNode struct has title: Option<String>.
    assert_eq!(no_title_node.title, None);
}

#[tokio::test]
async fn test_vitepress_check_dependencies() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    let root_str = root.to_string_lossy().to_string();

    use fireworks_collaboration_lib::app::commands::vitepress::vitepress_check_dependencies;

    // Case 1: Empty directory
    let status = vitepress_check_dependencies(root_str.clone())
        .await
        .unwrap();
    assert!(!status.installed);
    assert!(!status.node_modules_exists);

    // Case 2: Installed (mock files)
    fs::write(root.join("pnpm-lock.yaml"), "").unwrap();
    let node_modules = root.join("node_modules");
    fs::create_dir(&node_modules).unwrap();
    let pnpm_store = node_modules.join(".pnpm");
    fs::create_dir(&pnpm_store).unwrap();

    let status = vitepress_check_dependencies(root_str).await.unwrap();
    assert!(status.installed);
    assert!(status.pnpm_lock_exists);
    assert!(status.node_modules_exists);
    assert!(status.pnpm_store_exists);
}

#[tokio::test]
async fn test_vitepress_error_handling() {
    // Test 1: read non-existent file
    let result = vitepress_read_document("/non/existent/path.md".to_string(), None, None).await;
    assert!(result.is_err());

    // Test 2: rename non-existent file
    let result = vitepress_rename("/non/existent/path.md".to_string(), "new.md".to_string()).await;
    assert!(result.is_err());

    // Test 3: delete non-existent file
    let result = vitepress_delete("/non/existent/path.md".to_string()).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_vitepress_create_duplicate() {
    let dir = tempdir().unwrap();
    let root_str = dir.path().to_string_lossy().to_string();

    // Create first
    let _ = vitepress_create_document(root_str.clone(), "duplicate.md".to_string(), None)
        .await
        .unwrap();

    // Try to create duplicate
    let result =
        vitepress_create_document(root_str.clone(), "duplicate.md".to_string(), None).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("already exists"));

    // Same for folder
    let _ = vitepress_create_folder(root_str.clone(), "dup-folder".to_string())
        .await
        .unwrap();

    let result = vitepress_create_folder(root_str, "dup-folder".to_string()).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("already exists"));
}

#[tokio::test]
async fn test_vitepress_rename_conflict() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    let root_str = root.to_string_lossy().to_string();

    // Create two files
    let path1 = vitepress_create_document(root_str.clone(), "file1.md".to_string(), None)
        .await
        .unwrap();
    let _ = vitepress_create_document(root_str.clone(), "file2.md".to_string(), None)
        .await
        .unwrap();

    // Try to rename file1 to file2 (conflict)
    let result = vitepress_rename(path1, "file2.md".to_string()).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("already exists"));
}

#[tokio::test]
async fn test_vitepress_tree_filters_hidden() {
    let dir = tempdir().unwrap();
    let root_str = dir.path().to_string_lossy().to_string();

    // Create visible file
    fs::write(dir.path().join("visible.md"), "# Visible").unwrap();

    // Create hidden/excluded items
    fs::write(dir.path().join(".hidden.md"), "# Hidden").unwrap();
    fs::create_dir(dir.path().join(".vitepress")).unwrap();
    fs::create_dir(dir.path().join("node_modules")).unwrap();
    fs::create_dir(dir.path().join("dist")).unwrap();

    let tree = vitepress_get_doc_tree(root_str, None).await.unwrap();
    let children = tree.children.unwrap();

    // Only visible.md should be present
    assert_eq!(children.len(), 1);
    assert_eq!(children[0].name, "visible.md");
}

#[tokio::test]
async fn test_vitepress_config_fallback() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    // Test various config file extensions
    let config_dir = root.join(".vitepress");
    fs::create_dir(&config_dir).unwrap();

    // Create config.ts (should be detected)
    fs::write(config_dir.join("config.ts"), "").unwrap();

    let result = vitepress_detect_project(root.to_string_lossy().to_string())
        .await
        .unwrap();
    assert!(result.is_vitepress);
    assert_eq!(result.config_path, Some(".vitepress/config.ts".to_string()));
}

#[tokio::test]
async fn test_vitepress_content_root_not_found() {
    let dir = tempdir().unwrap();
    let root_str = dir.path().to_string_lossy().to_string();

    // Try to get tree with non-existent content root
    let result = vitepress_get_doc_tree(root_str, Some("nonexistent".to_string())).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("not found"));
}
