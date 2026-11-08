use super::*;
mod fixture;
use fixture::*;
mod failures;

#[tokio::test]
async fn inline_sftp_writes_four_mib_in_chunks_and_preserves_utf8() {
    let (sftp, state) = fixture().await;
    let content = format!("{}x", "界".repeat((4 * 1024 * 1024) / 3));
    assert_eq!(content.len(), 4 * 1024 * 1024);
    let request = input(SshFileWriteMode::Overwrite, content);
    state
        .lock()
        .unwrap()
        .files
        .insert(request.path.clone(), b"old".to_vec());
    write_inline(sftp, &request).await.unwrap();
    cleaned(&state).await;
    let state = state.lock().unwrap();
    assert_eq!(state.files[&request.path], request.content.as_bytes());
    assert!(state.writes > 1);
    assert_eq!(state.closes, 1);
}

#[tokio::test]
async fn inline_sftp_create_append_and_empty_overwrite() {
    let (sftp, state) = fixture().await;
    write_inline(
        sftp.clone(),
        &input(SshFileWriteMode::Create, "first".into()),
    )
    .await
    .unwrap();
    assert!(write_inline(
        sftp.clone(),
        &input(SshFileWriteMode::Create, "lost".into())
    )
    .await
    .is_err());
    write_inline(
        sftp.clone(),
        &input(SshFileWriteMode::Append, "\n第二行".into()),
    )
    .await
    .unwrap();
    assert_eq!(
        state.lock().unwrap().files["/fixture.txt"],
        "first\n第二行".as_bytes()
    );
    write_inline(sftp, &input(SshFileWriteMode::Overwrite, String::new()))
        .await
        .unwrap();
    cleaned(&state).await;
    assert!(state.lock().unwrap().files["/fixture.txt"].is_empty());
}

#[tokio::test]
async fn inline_sftp_write_and_commit_failures_keep_original() {
    for fail_write in [true, false] {
        let (sftp, state) = fixture().await;
        {
            let mut state = state.lock().unwrap();
            state
                .files
                .insert("/fixture.txt".into(), b"original".to_vec());
            state.fail_write = fail_write;
            state.fail_commit = !fail_write;
        }
        assert!(
            write_inline(sftp, &input(SshFileWriteMode::Overwrite, "new".into()))
                .await
                .is_err()
        );
        cleaned(&state).await;
        assert_eq!(state.lock().unwrap().files["/fixture.txt"], b"original");
    }
}

#[tokio::test]
async fn inline_sftp_create_race_and_symlink_do_not_overwrite() {
    let (sftp, state) = fixture().await;
    state.lock().unwrap().race_create = true;
    assert!(
        write_inline(sftp.clone(), &input(SshFileWriteMode::Create, "new".into()))
            .await
            .is_err()
    );
    cleaned(&state).await;
    assert_eq!(state.lock().unwrap().files["/fixture.txt"], b"concurrent");
    state.lock().unwrap().symlink = true;
    assert!(
        write_inline(sftp, &input(SshFileWriteMode::Overwrite, "new".into()))
            .await
            .is_err()
    );
}

#[tokio::test]
async fn inline_sftp_cancellation_cleans_staging_and_keeps_original() {
    let (sftp, state) = fixture().await;
    let started = Arc::new(tokio::sync::Notify::new());
    let resume = Arc::new(tokio::sync::Notify::new());
    {
        let mut state = state.lock().unwrap();
        state
            .files
            .insert("/fixture.txt".into(), b"original".to_vec());
        state.hold_write = Some((started.clone(), resume.clone()));
    }
    let task = tokio::spawn(async move {
        write_inline(sftp, &input(SshFileWriteMode::Overwrite, "new".into())).await
    });
    tokio::time::timeout(std::time::Duration::from_secs(2), started.notified())
        .await
        .unwrap();
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    resume.notify_one();
    cleaned(&state).await;
    assert_eq!(state.lock().unwrap().files["/fixture.txt"], b"original");
}
