use super::*;
use crate::engine::Engine;
use rdm_config::EngineConfig;

fn test_engine(directory: &Path) -> Result<Engine, EngineError> {
    Engine::new(
        Arc::new(ConfigManager::from_config(
            EngineConfig::default()
                .with_state_dir(directory.to_path_buf())
                .with_download_dir(directory.to_path_buf())
                .with_max_concurrent_downloads(1),
        )),
        Arc::new(DiskManager::new()),
    )
}

async fn media_task(
    engine: &Engine,
    name: &str,
    size: u64,
    action: PostProcessAction,
) -> Arc<Task> {
    let task = Arc::new(Task::new_with_post_process(
        Uuid::new_v4(),
        "invalid download URL".into(),
        engine.config().config().download_dir.join(name),
        size,
        1,
        None,
        Some(action),
    ));
    task.downloaded_bytes.store(size, Ordering::Relaxed);
    task.set_status(TaskStatus::ConversionFailed("previous failure".into()));
    engine
        .download
        .tasks
        .write()
        .await
        .insert(task.id, Arc::clone(&task));
    task
}

async fn wait_for_conversion(task: &Task) -> Result<TaskStatus, tokio::time::error::Elapsed> {
    tokio::time::timeout(std::time::Duration::from_secs(15), async {
        loop {
            let status = task.status();
            if matches!(
                status,
                TaskStatus::Completed | TaskStatus::ConversionFailed(_)
            ) {
                return status;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
}

#[tokio::test]
async fn conversion_retry_queues_once_and_preserves_remaining_track()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = std::env::temp_dir().join(format!("rdm_conversion_queue_{}", Uuid::new_v4()));
    let engine = test_engine(&directory)?;
    let active = Arc::new(Task::new(
        Uuid::new_v4(),
        String::new(),
        directory.join("download.bin"),
        0,
        1,
        None,
    ));
    active.set_status(TaskStatus::Downloading);
    engine
        .download
        .tasks
        .write()
        .await
        .insert(active.id, Arc::clone(&active));
    let task = media_task(
        &engine,
        "film.mp4",
        8,
        PostProcessAction::MuxVideoAudio {
            video_size: 4,
            audio_url: "invalid companion URL".into(),
            audio_size: 4,
        },
    )
    .await;
    let video = directory.join("film.mp4.video.part");
    tokio::fs::create_dir_all(&directory).await?;
    tokio::fs::write(&video, b"data").await?;

    let (first, second) = tokio::join!(
        engine.retry_conversion(task.id),
        engine.retry_conversion(task.id)
    );
    first?;
    second?;
    assert_eq!(task.status(), TaskStatus::ConversionQueued);
    assert_eq!(
        *engine.download.queue.read().await,
        VecDeque::from([task.id])
    );
    engine.pause_task(active.id).await?;
    assert!(
        matches!(wait_for_conversion(&task).await?, TaskStatus::ConversionFailed(reason) if reason.contains("audio.part"))
    );
    assert_eq!(tokio::fs::read(&video).await?, b"data");
    assert!(!tokio::fs::try_exists(&task.output_path).await?);

    tokio::fs::remove_dir_all(directory).await?;
    Ok(())
}

#[tokio::test]
async fn processing_recovery_distinguishes_staged_and_published_outputs()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = std::env::temp_dir().join(format!("rdm_conversion_restore_{}", Uuid::new_v4()));
    let engine = test_engine(&directory)?;
    let action = PostProcessAction::ConvertMp3 { bitrate_kbps: 128 };
    let interrupted = media_task(&engine, "interrupted.mp3", 4, action.clone()).await;
    let published = media_task(&engine, "published.mp3", 4, action).await;
    tokio::fs::create_dir_all(&directory).await?;
    for task in [&interrupted, &published] {
        task.set_status(TaskStatus::Processing {
            staged_size: Some(5),
        });
        tokio::fs::write(
            directory.join(format!(
                "{}.audio.part",
                task.output_path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
            )),
            b"data",
        )
        .await?;
    }
    tokio::fs::write(
        staging_path(&interrupted.output_path, interrupted.id),
        b"stage",
    )
    .await?;
    tokio::fs::write(&published.output_path, b"final").await?;
    assert!(matches!(
        engine.pause_task(interrupted.id).await,
        Err(EngineError::ConversionInProgress(id)) if id == interrupted.id
    ));
    assert!(matches!(
        engine.delete_task(interrupted.id, true).await,
        Err(EngineError::ConversionInProgress(id)) if id == interrupted.id
    ));
    engine.retry_conversion(interrupted.id).await?;
    assert_eq!(
        interrupted.status(),
        TaskStatus::Processing {
            staged_size: Some(5)
        }
    );
    engine.save_state().await?;

    let restored = test_engine(&directory)?;
    assert!(matches!(
        restored.get_task(interrupted.id).await?.status(),
        TaskStatus::ConversionFailed(_)
    ));
    assert_eq!(
        restored.get_task(published.id).await?.status(),
        TaskStatus::Completed
    );
    assert!(tokio::fs::try_exists(directory.join("interrupted.mp3.audio.part")).await?);
    assert!(!tokio::fs::try_exists(directory.join("published.mp3.audio.part")).await?);
    assert_eq!(tokio::fs::read(&published.output_path).await?, b"final");
    tokio::fs::remove_dir_all(directory).await?;
    Ok(())
}

#[tokio::test]
async fn ffmpeg_launch_failure_preserves_tracks_and_allows_retry()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = std::env::temp_dir().join(format!("rdm_ffmpeg_missing_{}", Uuid::new_v4()));
    let engine = test_engine(&directory)?;
    let binary_name = if cfg!(windows) {
        "ffmpeg.exe"
    } else {
        "ffmpeg"
    };
    tokio::fs::create_dir_all(directory.join("bin").join(binary_name)).await?;
    let task = media_task(
        &engine,
        "audio.mp3",
        4,
        PostProcessAction::ConvertMp3 { bitrate_kbps: 128 },
    )
    .await;
    let audio = directory.join("audio.mp3.audio.part");
    tokio::fs::write(&audio, b"data").await?;

    engine.resume_task(task.id).await?;
    assert!(
        matches!(wait_for_conversion(&task).await?, TaskStatus::ConversionFailed(reason) if reason.contains("FFmpeg error"))
    );
    assert_eq!(tokio::fs::read(&audio).await?, b"data");
    assert!(!tokio::fs::try_exists(&task.output_path).await?);
    assert!(!tokio::fs::try_exists(staging_path(&task.output_path, task.id)).await?);
    assert!(matches!(
        test_engine(&directory)?.get_task(task.id).await?.status(),
        TaskStatus::ConversionFailed(_)
    ));
    tokio::fs::remove_dir_all(directory).await?;
    Ok(())
}

#[tokio::test]
async fn concurrent_tasks_reserve_output_and_track_names() -> Result<(), Box<dyn std::error::Error>>
{
    let directory = std::env::temp_dir().join(format!("rdm_reservation_{}", Uuid::new_v4()));
    let engine = test_engine(&directory)?;
    let probe = ResourceProbe {
        url: "unused URL".into(),
        file_name: "audio.mp3".into(),
        total_size: 4,
        supports_ranges: false,
        content_type: None,
        post_process: Some(PostProcessAction::ConvertMp3 { bitrate_kbps: 128 }),
    };
    let (first, second) = tokio::join!(
        engine
            .download
            .create_task(probe.clone(), None, None, None, None),
        engine
            .download
            .create_task(probe.clone(), None, None, None, None),
    );
    let first = first?;
    let second = second?;
    assert_ne!(first.output_path, second.output_path);
    let existing_track = track_paths(
        &first.output_path,
        first.total_size,
        first.post_process.as_ref(),
    )
    .next()
    .ok_or("missing reserved track")?
    .0;
    let regular = engine
        .download
        .create_task(
            ResourceProbe {
                post_process: None,
                ..probe
            },
            Some(existing_track.clone()),
            None,
            None,
            None,
        )
        .await?;
    assert_ne!(regular.output_path, existing_track);
    tokio::fs::remove_dir_all(directory).await?;
    Ok(())
}

async fn make_fixture(binary: &Path, destination: &Path, args: &[&str]) -> std::io::Result<()> {
    let output = tokio::process::Command::new(binary)
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-nostdin",
            "-f",
            "lavfi",
        ])
        .args(args)
        .arg(destination)
        .output()
        .await?;
    if !output.status.success() {
        return Err(std::io::Error::other(String::from_utf8_lossy(
            &output.stderr,
        )));
    }
    Ok(())
}

#[tokio::test]
#[ignore = "requires RDM_TEST_FFMPEG pointing to an FFmpeg binary"]
async fn ffmpeg_publishes_mp4_and_mp3_with_conversion_only_retries()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = std::env::temp_dir().join(format!("rdm_ffmpeg_integration_{}", Uuid::new_v4()));
    let engine = test_engine(&directory)?;
    let binary_name = if cfg!(windows) {
        "ffmpeg.exe"
    } else {
        "ffmpeg"
    };
    let binary = directory.join("bin").join(binary_name);
    tokio::fs::create_dir_all(directory.join("bin")).await?;
    tokio::fs::copy(std::env::var("RDM_TEST_FFMPEG")?, &binary).await?;
    let video = directory.join("film.mp4.video.part");
    let audio = directory.join("film.mp4.audio.part");
    make_fixture(
        &binary,
        &video,
        &[
            "-i",
            "color=c=blue:s=64x64:r=10",
            "-t",
            "0.4",
            "-c:v",
            "mpeg4",
            "-f",
            "mp4",
        ],
    )
    .await?;
    make_fixture(
        &binary,
        &audio,
        &[
            "-i",
            "sine=frequency=440",
            "-t",
            "0.4",
            "-c:a",
            "aac",
            "-f",
            "mp4",
        ],
    )
    .await?;
    let video_bytes = tokio::fs::read(&video).await?;
    let audio_bytes = tokio::fs::read(&audio).await?;
    let video_size = video_bytes.len() as u64;
    let audio_size = audio_bytes.len() as u64;
    let film = media_task(
        &engine,
        "film.mp4",
        video_size + audio_size,
        PostProcessAction::MuxVideoAudio {
            video_size,
            audio_url: "invalid companion URL".into(),
            audio_size,
        },
    )
    .await;

    let staged = staging_path(&film.output_path, film.id);
    assert!(engine.download.multimedia.prepare_conversion(&film).await? > 0);
    assert!(tokio::fs::try_exists(&staged).await?);
    assert!(!tokio::fs::try_exists(&film.output_path).await?);
    assert!(tokio::fs::try_exists(&video).await? && tokio::fs::try_exists(&audio).await?);

    tokio::fs::write(&video, vec![0; video_bytes.len()]).await?;
    engine.retry_conversion(film.id).await?;
    assert!(
        matches!(wait_for_conversion(&film).await?, TaskStatus::ConversionFailed(reason) if reason.contains("FFmpeg mux failed"))
    );
    assert!(tokio::fs::try_exists(&video).await?);
    assert_eq!(tokio::fs::read(&audio).await?, audio_bytes);
    assert!(!tokio::fs::try_exists(&staged).await?);
    assert!(!tokio::fs::try_exists(&film.output_path).await?);

    tokio::fs::write(&video, video_bytes).await?;
    tokio::fs::write(&film.output_path, b"existing output").await?;
    engine.retry_conversion(film.id).await?;
    assert!(
        matches!(wait_for_conversion(&film).await?, TaskStatus::ConversionFailed(reason) if reason.contains("ya existe"))
    );
    assert_eq!(
        tokio::fs::read(&film.output_path).await?,
        b"existing output"
    );
    assert!(!tokio::fs::try_exists(&staged).await?);
    assert!(tokio::fs::try_exists(&video).await? && tokio::fs::try_exists(&audio).await?);

    tokio::fs::remove_file(&film.output_path).await?;
    engine.resume_task(film.id).await?;
    assert_eq!(wait_for_conversion(&film).await?, TaskStatus::Completed);
    assert_eq!(&tokio::fs::read(&film.output_path).await?[4..8], b"ftyp");
    assert!(!tokio::fs::try_exists(&video).await? && !tokio::fs::try_exists(&audio).await?);
    assert!(!tokio::fs::try_exists(staged).await?);

    let mp3 = media_task(
        &engine,
        "audio.mp3",
        audio_size,
        PostProcessAction::ConvertMp3 { bitrate_kbps: 128 },
    )
    .await;
    let mp3_input = directory.join("audio.mp3.audio.part");
    tokio::fs::write(&mp3_input, audio_bytes).await?;
    let segment = Arc::new(DownloadSegment::new(0, 0, audio_size - 1));
    segment
        .downloaded_bytes
        .store(audio_size, Ordering::Relaxed);
    segment.set_status(SegmentStatus::Completed);
    mp3.set_segments(vec![segment]);
    mp3.set_status(TaskStatus::Paused);
    engine.resume_task(mp3.id).await?;
    assert_eq!(wait_for_conversion(&mp3).await?, TaskStatus::Completed);
    assert!(tokio::fs::read(&mp3.output_path).await?.starts_with(b"ID3"));
    assert!(!tokio::fs::try_exists(mp3_input).await?);
    tokio::fs::remove_dir_all(directory).await?;
    Ok(())
}
