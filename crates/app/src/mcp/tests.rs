use super::*;
use std::collections::HashSet;

#[test]
fn every_tool_has_a_unique_name_and_an_object_schema() {
    let tools = tools();
    let tools = tools.as_array().unwrap();
    let names: HashSet<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
    assert_eq!(names.len(), tools.len());
    for t in tools {
        assert_eq!(t["inputSchema"]["type"], "object", "{}", t["name"]);
        for required in t["inputSchema"]["required"].as_array().into_iter().flatten() {
            let key = required.as_str().unwrap();
            assert!(t["inputSchema"]["properties"].get(key).is_some(), "{} requires {key} undescribed", t["name"]);
        }
    }
}

#[test]
fn a_tool_that_does_not_exist_is_an_error() {
    assert!(call("nope", &json!({})).is_err());
}

#[test]
fn export_choices_are_read_as_the_example_cli_names_them() {
    assert_eq!(profile("MP3-V2").unwrap(), Profile::Mp3Vbr { quality: 2 });
    assert_eq!(profile("mp3-320").unwrap(), Profile::Mp3Cbr { kbps: 320 });
    assert_eq!(profile("flac").unwrap(), Profile::Flac);
    assert!(profile("mp3-v7").is_err(), "not one of the window's choices");
    assert_eq!(normalize("album:-14").unwrap(), Normalize::Album { target: -14.0 });
    assert_eq!(normalize("off").unwrap(), Normalize::Off);
    assert!(normalize("album:-9").is_err());
    assert_eq!(video_quality("small").unwrap(), VideoProfile::Mp4 { crf: 28 });
    assert_eq!(status("in progress").unwrap(), Status::InProgress);
    assert!(status("exported").is_err(), "set by an export, not by hand");
}

#[test]
fn a_path_must_be_an_absolute_recording() {
    assert!(Rec::from(&json!({ "path": "set.mp3" })).is_err());
    assert!(Rec::from(&json!({ "path": "/nowhere/set.mp3" })).is_err());
    let notes = std::env::temp_dir().join(format!("trackcut-mcp-{}.txt", std::process::id()));
    std::fs::write(&notes, "x").unwrap();
    assert!(Rec::from(&json!({ "path": notes })).is_err());
    let _ = std::fs::remove_file(notes);
}

#[test]
fn split_ids_name_the_splits_before_the_call() {
    let mut edit = RecordingEdit::default();
    edit.insert(Split::confirmed(100));
    edit.insert(Split::suggested(200, 2.0));
    assert_eq!(split_ids(&json!({ "remove": [1, 0] }), "remove", &edit).unwrap(), [200, 100]);
    assert!(split_ids(&json!({ "remove": [2] }), "remove", &edit).is_err());
    assert!(split_ids(&json!({}), "remove", &edit).unwrap().is_empty());
}
