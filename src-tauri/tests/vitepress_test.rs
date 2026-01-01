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
    let content = vitepress_read_document(doc_path.clone()).await.unwrap();
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
