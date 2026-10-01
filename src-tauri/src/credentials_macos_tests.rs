//! Regression coverage for one-read macOS credential sessions without touching the real keychain.

use super::*;

#[test]
fn replacement_and_removal_survive_restart_without_reusing_legacy_keys() {
    for id in NATIVE_SESSION_CREDENTIAL_IDS {
        let mut persisted = CredentialBundle::default();
        let mut session = CredentialSession::default();
        warm_bundle(&mut session, true, || Ok(()), || Ok(persisted.clone())).unwrap();
        change_bundle(&mut session, id, Some("replacement"), |bundle| {
            persisted = bundle.clone();
            Ok(())
        })
        .unwrap();
        assert_eq!(
            session.secrets.get(id).map(String::as_str),
            Some("replacement")
        );
        let mut restarted = CredentialSession::default();
        warm_bundle(
            &mut restarted,
            true,
            || Ok(()),
            || {
                let mut loaded = persisted.clone();
                migrate_legacy(&mut loaded, |legacy_id| {
                    assert_ne!(legacy_id, id, "replaced legacy identity must be skipped");
                    Ok(None)
                })?;
                Ok(loaded)
            },
        )
        .unwrap();
        assert_eq!(
            restarted.secrets.get(id).map(String::as_str),
            Some("replacement")
        );
        change_bundle(&mut restarted, id, None, |bundle| {
            persisted = bundle.clone();
            Ok(())
        })
        .unwrap();
        assert!(!restarted.secrets.contains_key(id));
        migrate_legacy(&mut persisted, |legacy_id| {
            assert_ne!(legacy_id, id, "removed legacy identity must stay retired");
            Ok(None)
        })
        .unwrap();
        assert!(!persisted.secrets.contains_key(id));
        assert!(persisted.retired_legacy_ids.contains(id));
    }
}

#[test]
fn failed_bundle_write_keeps_the_prior_session_credential() {
    let mut session = CredentialSession::default();
    warm_bundle(&mut session, false, || panic!("empty"), || panic!("empty")).unwrap();
    change_bundle(&mut session, "brave", Some("prior"), |_| Ok(())).unwrap();
    for key in [Some("replacement"), None] {
        assert!(change_bundle(&mut session, "brave", key, |_| Err(bundle_error())).is_err());
        assert_eq!(
            session.secrets.get("brave").map(String::as_str),
            Some("prior")
        );
        assert_eq!(
            session
                .bundle
                .as_ref()
                .unwrap()
                .secrets
                .get("brave")
                .map(String::as_str),
            Some("prior")
        );
    }
}

#[test]
fn reads_one_bundle_after_one_authentication_and_reuses_every_key() {
    let mut session = CredentialSession::default();
    let mut prompts = 0;
    let mut reads = 0;
    warm_bundle(
        &mut session,
        true,
        || {
            prompts += 1;
            Ok(())
        },
        || {
            reads += 1;
            Ok(CredentialBundle {
                secrets: NATIVE_SESSION_CREDENTIAL_IDS
                    .iter()
                    .map(|id| (id.to_string(), format!("key-{id}")))
                    .collect(),
                ..Default::default()
            })
        },
    )
    .expect("warm bundle");
    assert_eq!(prompts, 1);
    assert_eq!(reads, 1);
    assert_eq!(session.secrets.len(), 6);
    warm_bundle(
        &mut session,
        true,
        || panic!("no repeated prompt"),
        || panic!("no repeated read"),
    )
    .expect("reuse session");
}

#[test]
fn empty_vault_never_authenticates_or_reads_secrets() {
    let mut session = CredentialSession::default();
    warm_bundle(
        &mut session,
        false,
        || panic!("empty vault"),
        || panic!("empty vault"),
    )
    .expect("empty vault");
    assert!(!session.authenticated);
    assert!(session.bundle.is_some());
}

#[test]
fn cancelled_authentication_never_reads_and_can_be_retried() {
    let mut session = CredentialSession::default();
    assert!(
        warm_bundle(
            &mut session,
            true,
            || Err(legacy_locked_error()),
            || panic!("cancelled")
        )
        .is_err()
    );
    assert!(!session.authenticated);
    assert!(session.bundle.is_none());
    warm_bundle(
        &mut session,
        true,
        || Ok(()),
        || Ok(CredentialBundle::default()),
    )
    .expect("retry");
}

#[test]
fn failed_load_retries_without_another_biometric_prompt() {
    let mut session = CredentialSession::default();
    assert!(warm_bundle(&mut session, true, || Ok(()), || Err(bundle_error())).is_err());
    assert!(session.authenticated);
    assert!(session.bundle.is_none());
    warm_bundle(
        &mut session,
        true,
        || panic!("already authenticated"),
        || Ok(CredentialBundle::default()),
    )
    .expect("retry load");
}

#[test]
fn vault_payload_is_bounded_closed_and_redacted() {
    let bytes = br#"{"secrets":{"openai":"fixture-key"},"retired_legacy_ids":["openai"]}"#;
    assert_eq!(
        decode_bundle(bytes).expect("valid").secrets["openai"],
        "fixture-key"
    );
    for invalid in [
        br#"{"secrets":{"foreign":"fixture-key"},"retired_legacy_ids":[]}"#.as_slice(),
        br#"{"secrets":{"openai":""},"retired_legacy_ids":[]}"#.as_slice(),
        br#"{"secrets":{},"retired_legacy_ids":[],"extra":"fixture-key"}"#.as_slice(),
    ] {
        let error = decode_bundle(invalid).err().expect("reject corrupt bundle");
        assert!(
            !serde_json::to_string(&error)
                .expect("error")
                .contains("fixture-key")
        );
    }
    assert!(decode_bundle(&vec![b'x'; MAX_BUNDLE_BYTES + 1]).is_err());
}

#[test]
fn public_index_contains_only_identities_and_keeps_deleted_legacy_entries_retired() {
    let bundle = CredentialBundle {
        secrets: HashMap::from([("openai".into(), "fixture-secret".into())]),
        retired_legacy_ids: HashSet::from(["openai".into(), "localmail".into()]),
    };
    let index = CredentialIndex {
        configured_ids: bundle.secrets.keys().cloned().collect(),
        retired_legacy_ids: bundle.retired_legacy_ids.clone(),
    };
    let serialized = serde_json::to_string(&index).expect("index");
    assert!(!serialized.contains("fixture-secret"));
    assert!(!index.configured_ids.contains("localmail"));
    assert!(index.retired_legacy_ids.contains("localmail"));
}

#[test]
fn migration_preserves_replacements_deletions_and_inaccessible_old_entries() {
    let mut bundle = CredentialBundle {
        secrets: HashMap::from([("openai".into(), "replacement-key".into())]),
        retired_legacy_ids: HashSet::from(["localmail".into()]),
    };
    assert!(
        migrate_legacy(&mut bundle, |id| match id {
            "openai" | "localmail" => panic!("do not read replaced or deleted entries"),
            "brave" => Ok(Some("accessible-old-key".into())),
            _ => Ok(None), // The native reader disables dialogs and skips unauthorized entries.
        })
        .expect("best-effort migration")
    );
    assert_eq!(bundle.secrets["openai"], "replacement-key");
    assert_eq!(bundle.secrets["brave"], "accessible-old-key");
    assert!(bundle.retired_legacy_ids.contains("brave"));
    assert!(!bundle.secrets.contains_key("localmail"));
    assert!(!bundle.retired_legacy_ids.contains("anthropic"));
}

#[test]
fn rejects_foreign_or_unexpected_public_metadata() {
    for invalid in [
        r#"{"configured_ids":["foreign"],"retired_legacy_ids":[]}"#,
        r#"{"configured_ids":[],"retired_legacy_ids":["foreign"]}"#,
        r#"{"configured_ids":[],"retired_legacy_ids":[],"secret":"fixture"}"#,
    ] {
        assert!(decode_index(invalid).is_err());
    }
}

#[test]
fn simultaneous_session_requests_coalesce_authentication_and_vault_reads() {
    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    };
    let session = Arc::new(Mutex::new(CredentialSession::default()));
    let prompts = Arc::new(AtomicUsize::new(0));
    let reads = Arc::new(AtomicUsize::new(0));
    let threads: Vec<_> = (0..6)
        .map(|_| {
            let session = session.clone();
            let prompts = prompts.clone();
            let reads = reads.clone();
            std::thread::spawn(move || {
                warm_bundle(
                    &mut session.lock().expect("session lock"),
                    true,
                    || {
                        prompts.fetch_add(1, Ordering::SeqCst);
                        Ok(())
                    },
                    || {
                        reads.fetch_add(1, Ordering::SeqCst);
                        Ok(CredentialBundle::default())
                    },
                )
                .expect("coalesced session");
            })
        })
        .collect();
    for thread in threads {
        thread.join().expect("session thread");
    }
    assert_eq!(prompts.load(Ordering::SeqCst), 1);
    assert_eq!(reads.load(Ordering::SeqCst), 1);
}
