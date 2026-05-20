use bus::DomainEventBus;
use common::tool_channel::{Channel, NonUiPolicy};
use klynt_core::privacy::PrivacyGuard;
use klynt_core::tools::notebook_edit::{run_for_test as nb_run, NotebookEditArgs};
use klynt_execpolicy::Policy;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

const NB: &str = r##"{
  "cells": [
    {"cell_type":"code","source":["print('hi')\n"],"outputs":[],"execution_count":null,"metadata":{}},
    {"cell_type":"markdown","source":["# Title\n"],"metadata":{}}
  ],
  "metadata":{},"nbformat":4,"nbformat_minor":5
}"##;

#[tokio::test]
async fn replaces_cell_source() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("nb.ipynb"), NB).unwrap();
    let pol = Arc::new(Policy::empty());
    let pri = Arc::new(PrivacyGuard::from_globs(&[]).unwrap());
    let bus = Arc::new(DomainEventBus::new(64));
    let (tx, _rx) = mpsc::channel(32);
    nb_run(
        NotebookEditArgs {
            path: "nb.ipynb".into(),
            cell_index: 0,
            new_source: "print('updated')\n".into(),
        },
        dir.path().to_path_buf(),
        pol,
        pri,
        Some(tx),
        bus,
        CancellationToken::new(),
        Channel::Desktop,
        NonUiPolicy::Allow,
        None,
        "".to_string(),
        None,
        false,
        5,
        86400,
        "".to_string(),
        None,
    )
    .await
    .unwrap();
    let saved = std::fs::read_to_string(dir.path().join("nb.ipynb")).unwrap();
    let v: serde_json::Value = serde_json::from_str(&saved).unwrap();
    let cell0_src = &v["cells"][0]["source"];
    assert_eq!(cell0_src, &serde_json::json!("print('updated')\n"));
}

#[tokio::test]
async fn rejects_out_of_range_index() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("nb.ipynb"), NB).unwrap();
    let pol = Arc::new(Policy::empty());
    let pri = Arc::new(PrivacyGuard::from_globs(&[]).unwrap());
    let bus = Arc::new(DomainEventBus::new(64));
    let (tx, _rx) = mpsc::channel(32);
    let r = nb_run(
        NotebookEditArgs {
            path: "nb.ipynb".into(),
            cell_index: 99,
            new_source: "x".into(),
        },
        dir.path().to_path_buf(),
        pol,
        pri,
        Some(tx),
        bus,
        CancellationToken::new(),
        Channel::Desktop,
        NonUiPolicy::Allow,
        None,
        "".to_string(),
        None,
        false,
        5,
        86400,
        "".to_string(),
        None,
    )
    .await;
    assert!(r.is_err());
}
