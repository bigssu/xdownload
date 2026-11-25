//! Tests of the app state (no window): queueing, messages and the screens' key coverage.
use super::*;
use queue::{Outcome, State, Update};

fn app() -> App {
    // Screenshot mode: demo content, and nothing is read from or written to the user files.
    let mut app = App::fresh(Settings::default(), Some(PathBuf::from("unused.png")), mpsc::channel().1);
    app.queue = Queue::default();
    app.urls.clear();
    app
}

#[test]
fn links_are_trimmed_and_blank_lines_dropped() {
    assert_eq!(parse_urls("  https://a \r\n\n\thttps://b\n   \n"), ["https://a", "https://b"]);
    assert!(parse_urls(" \n\t\n").is_empty());
}

#[test]
fn adding_downloads_queues_each_link_once_with_the_chosen_options() {
    let mut app = app();
    app.add_downloads();
    assert_eq!((app.notice.0.key, app.notice.1, app.queue.items.len()), ("need_url", Tone::Bad, 0));
    (app.s.format, app.s.height, app.s.subs) = (0, 1440, true);
    app.urls = "https://youtu.be/a\nhttps://youtu.be/b\nhttps://youtu.be/a".into();
    app.add_downloads();
    assert_eq!((app.queue.items.len(), app.notice.0.text(), app.urls.as_str()), (2, i18n::fill("added", &[("n", "2")]), ""));
    let Work::Download { options, .. } = &app.queue.items[0].work else { panic!("a download was queued") };
    assert_eq!((options.format, options.height, options.subs), (downloader::Format::Mp4, 1440, true));
    // Options are fixed when queued: later changes only affect new items.
    app.s.format = 1;
    app.urls = "https://youtu.be/a\nhttps://youtu.be/c".into();
    app.add_downloads();
    assert_eq!(app.queue.items.len(), 4, "the same link with other options is different work");
    app.urls = "https://youtu.be/c".into();
    app.add_downloads();
    assert_eq!((app.queue.items.len(), app.notice.0.key), (4, "already"));
}

#[test]
fn dropped_files_go_to_the_matching_converter() {
    let dir = std::env::temp_dir().join(format!("xdownload2_drop_{}", std::process::id()));
    std::fs::create_dir_all(dir.join("sub")).unwrap();
    for name in ["a.mov", "b.png", "c.txt", "sub/d.png"] {
        std::fs::write(dir.join(name), "").unwrap();
    }
    let mut app = app();
    app.s.image_target = 2;
    app.add_dropped(vec![dir.join("a.mov"), dir.join("b.png"), dir.join("c.txt")]);
    let targets: Vec<&str> = app.queue.items.iter().map(|item| match &item.work {
        Work::Convert { target, .. } => target.extension(),
        Work::Download { .. } => "download",
    }).collect();
    assert_eq!((targets, app.tab, app.notice.0.key), (vec!["mp4", "webp"], Tab::Convert, "added"));
    // A folder is searched for both kinds; subfolders only when asked.
    let mut app = self::app();
    app.add_dropped(vec![dir.clone()]);
    assert_eq!(app.queue.items.len(), 2);
    app.s.subfolders = true;
    app.add_dropped(vec![dir.clone()]);
    assert_eq!(app.queue.items.len(), 3, "the items already waiting are not added twice");
    let mut app = self::app();
    app.add_dropped(vec![dir.join("c.txt")]);
    assert_eq!((app.queue.items.len(), app.notice.0.key), (0, "need_convert_files"));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn command_line_inputs_fill_the_link_box_and_the_converter() {
    let mut app = app();
    app.take_inputs(vec!["https://youtu.be/a".into(), "https://youtu.be/b".into(), "no-such-file.mov".into()]);
    assert_eq!(app.urls, "https://youtu.be/a\nhttps://youtu.be/b");
    assert!(app.queue.items.is_empty(), "a path that does not exist adds nothing");
}

#[test]
fn worker_messages_reach_the_queue_and_mark_a_finished_download() {
    let mut app = app();
    app.urls = "https://youtu.be/a".into();
    app.add_downloads();
    let id = app.queue.items[0].id;
    app.queue.start_next(Limits::new(3, false));
    for update in [Update::Progress(0.5), Update::File("C:/out/Song.mp4".into()), Update::Finished(Outcome::Done)] {
        app.tx.send(Msg { id, update }).unwrap();
    }
    app.drain();
    assert_eq!((app.queue.items[0].state, app.queue.items[0].progress, app.downloaded), (State::Done, 1.0, true));
    assert!(!app.queue.active());
}

#[test]
fn settings_choices_map_to_targets() {
    let mut app = app();
    (app.s.format, app.s.video_target, app.s.video_height, app.s.video_quality, app.s.image_target) = (9, 2, 720, 2, 1);
    assert_eq!(app.format(), downloader::Format::M4a, "an out-of-range value is clamped");
    assert_eq!(app.video_target(), converter::Target::Video { target: converter::VideoTarget::Gif, height: 720, quality: 2 });
    assert_eq!(app.image_target(), converter::Target::Image(converter::ImageTarget::Png));
}

#[test]
fn every_screen_draws_without_showing_a_raw_key() {
    // Keys are looked up by name, so draw every screen and check no raw key reached it.
    let ctx = egui::Context::default();
    theme::setup(&ctx);
    let mut app = App::fresh(Settings::default(), Some(PathBuf::from("unused.png")), mpsc::channel().1);
    let keys: Vec<&str> = i18n::TABLE.iter().map(|row| row.0).collect();
    let output = ctx.run(egui::RawInput { screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(900.0, 3000.0))), ..Default::default() }, |ctx| {
        app.title_bar(ctx);
        app.footer(ctx);
        egui::CentralPanel::default().show(ctx, |ui| app.draw_everything(ui));
    });
    let mut texts = 0;
    for shape in &output.shapes {
        if let egui::Shape::Text(text) = &shape.shape {
            let drawn = text.galley.job.text.as_str();
            texts += 1;
            assert!(!keys.contains(&drawn) && !drawn.contains('{'), "untranslated text: {drawn}");
        }
    }
    assert!(texts > 40, "the screens were really drawn ({texts} texts)");
}

/// Manual end-to-end checks against the real network (downloads yt-dlp / ffmpeg on first run):
/// `cargo test --release live -- --ignored --nocapture --test-threads 1`
mod live {
    use super::*;
    use std::time::Instant;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("xdownload2_live_{}_{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Runs the queue to the end the way the app does, without a window.
    fn run_queue(queue: &mut Queue, limits: Limits, use_gpu: bool) -> Vec<PathBuf> {
        let (tx, rx) = mpsc::channel::<Msg>();
        let mut files = Vec::new();
        while queue.active() {
            for (id, work, job) in queue.start_next(limits) {
                let tx = tx.clone();
                std::thread::spawn(move || queue::run(work, &job, use_gpu, &|update| {
                    let _ = tx.send(Msg { id, update });
                }));
            }
            let Ok(msg) = rx.recv_timeout(std::time::Duration::from_secs(600)) else { panic!("no worker message for 10 minutes") };
            if let Update::File(path) = &msg.update {
                files.push(path.clone());
            }
            queue.apply(msg);
        }
        for item in &queue.items {
            println!("  {:?} {} {:?}", item.state, item.title, item.note);
        }
        files
    }

    fn options(format: downloader::Format, dir: &Path) -> downloader::Options {
        downloader::Options { format, height: 480, subs: false, embed_info: false, playlist: false, out_dir: dir.to_string_lossy().into_owned() }
    }

    const LINKS: [&str; 3] = ["https://www.youtube.com/watch?v=jNQXAC9IVRw", "https://www.youtube.com/watch?v=aqz-KE-bpKQ", "https://www.youtube.com/watch?v=YE7VzlLtp-4"];

    /// The stream list ffmpeg prints (on stderr) when given only an input.
    fn streams(file: &Path) -> String {
        let ffmpeg = binaries::ensure_ffmpeg(&Job::default(), &mut |_| {}).unwrap();
        let output = proc::command(&ffmpeg).args(["-hide_banner", "-i"]).arg(file).output().unwrap();
        String::from_utf8_lossy(&output.stderr).into_owned()
    }

    #[test]
    #[ignore]
    fn live_parallel_downloads_beat_sequential_ones() {
        let mut seconds = Vec::new();
        for parallel in [1, 3] {
            let dir = temp(&format!("parallel{parallel}"));
            let mut queue = Queue::default();
            for link in LINKS {
                queue.add(Work::Download { url: link.into(), options: options(downloader::Format::Mp4, &dir) });
            }
            let started = Instant::now();
            let files = run_queue(&mut queue, Limits::new(parallel, false), false);
            seconds.push(started.elapsed().as_secs_f32());
            assert_eq!((queue.count(State::Done), files.len()), (3, 3), "parallel {parallel}");
            assert!(files.iter().all(|file| file.is_file()));
            let _ = std::fs::remove_dir_all(dir);
        }
        println!("3 downloads: one at a time {:.1}s, three at once {:.1}s", seconds[0], seconds[1]);
        assert!(seconds[1] < seconds[0], "three at once must be faster");
    }

    #[test]
    #[ignore]
    fn live_formats_and_embedded_extras() {
        let dir = temp("formats");
        let mut queue = Queue::default();
        let mut rich = options(downloader::Format::Mp4, &dir);
        (rich.subs, rich.embed_info) = (true, true);
        queue.add(Work::Download { url: LINKS[0].into(), options: rich });
        queue.add(Work::Download { url: LINKS[0].into(), options: options(downloader::Format::M4a, &dir) });
        queue.add(Work::Download { url: LINKS[0].into(), options: options(downloader::Format::Mp3, &dir) });
        queue.add(Work::Download { url: "https://www.youtube.com/watch?v=xxxxxxxxxxx".into(), options: options(downloader::Format::Mp3, &dir) });
        let files = run_queue(&mut queue, Limits::new(3, false), false);
        assert_eq!((queue.count(State::Done), queue.count(State::Failed)), (3, 1), "a bad link fails alone");
        let mut extensions: Vec<String> = files.iter().map(|file| file.extension().unwrap().to_string_lossy().into_owned()).collect();
        extensions.sort();
        assert_eq!(extensions, ["m4a", "mp3", "mp4"]);
        let mp4 = files.iter().find(|file| file.extension().unwrap() == "mp4").unwrap();
        let listing = streams(mp4);
        println!("{listing}");
        assert!(listing.contains("Subtitle:") && listing.contains("attached pic"), "subtitles and thumbnail are inside the file");
        let leftovers: Vec<String> = std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|name| name.ends_with(".vtt") || name.ends_with(".webp") || name.ends_with(".part")).collect();
        assert!(leftovers.is_empty(), "no side files are left: {leftovers:?}");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    #[ignore]
    fn live_conversions_on_gpu_and_cpu() {
        use converter::{ImageTarget, Target, VideoTarget};
        let dir = temp("convert");
        // A 1080p source with detail, so the encoders have real work.
        let mut queue = Queue::default();
        let mut source = options(downloader::Format::Mp4, &dir);
        source.height = 1080;
        queue.add(Work::Download { url: LINKS[1].into(), options: source });
        let video = run_queue(&mut queue, Limits::new(1, false), false)[0].clone();
        let ffmpeg = binaries::ensure_ffmpeg(&Job::default(), &mut |_| {}).unwrap();
        let encoder = converter::detect_encoder(&ffmpeg);
        println!("hardware encoder: {encoder:?}");
        let mp4 = Target::Video { target: VideoTarget::Mp4, height: 720, quality: 1 };
        let mut seconds = Vec::new();
        for use_gpu in [false, true] {
            let mut queue = Queue::default();
            queue.add(Work::Convert { source: video.clone(), target: mp4 });
            let started = Instant::now();
            let files = run_queue(&mut queue, Limits::new(1, use_gpu), use_gpu);
            seconds.push(started.elapsed().as_secs_f32());
            assert_eq!(queue.count(State::Done), 1);
            let listing = streams(&files[0]);
            assert!(listing.contains("Video: h264") && listing.contains("Audio: aac") && listing.contains("x720"), "{listing}");
            println!("  {} = {} bytes", files[0].file_name().unwrap().to_string_lossy(), files[0].metadata().unwrap().len());
        }
        println!("720p H.264 conversion: CPU {:.1}s, {encoder:?} {:.1}s", seconds[0], seconds[1]);
        if encoder != converter::Encoder::Cpu {
            assert!(seconds[1] < seconds[0], "the hardware encoder must be faster");
        }
        // The other targets, and many images at once.
        let mut queue = Queue::default();
        for target in [VideoTarget::Mp3, VideoTarget::Gif] {
            queue.add(Work::Convert { source: video.clone(), target: Target::Video { target, height: 0, quality: 1 } });
        }
        let pictures = dir.join("pictures");
        std::fs::create_dir_all(&pictures).unwrap();
        for n in 0..24u8 {
            image::RgbaImage::from_fn(1600, 1200, |x, y| image::Rgba([(x % 256) as u8, (y % 256) as u8, n * 10, 255])).save(pictures.join(format!("p{n:02}.png"))).unwrap();
        }
        for target in [ImageTarget::Jpg, ImageTarget::Webp] {
            for source in converter::collect_media_files(&[pictures.clone()], Target::Image(target), false) {
                queue.add(Work::Convert { source, target: Target::Image(target) });
            }
        }
        let mut image_seconds = Vec::new();
        for image_limit in [1, Limits::new(1, false).image] {
            let mut batch = Queue::default();
            for n in 0..24 {
                batch.add(Work::Convert { source: pictures.join(format!("p{n:02}.png")), target: Target::Image(ImageTarget::Jpg) });
            }
            let started = Instant::now();
            run_queue(&mut batch, Limits { download: 1, video: 1, image: image_limit }, false);
            image_seconds.push(started.elapsed().as_secs_f32());
            assert_eq!(batch.count(State::Done), 24);
        }
        println!("24 images to JPG: one at a time {:.1}s, {} at once {:.1}s", image_seconds[0], Limits::new(1, false).image, image_seconds[1]);
        assert!(image_seconds[1] < image_seconds[0], "images at once must be faster");
        let started = Instant::now();
        let files = run_queue(&mut queue, Limits::new(1, false), false);
        println!("mp3 + gif + 48 image conversions in {:.1}s", started.elapsed().as_secs_f32());
        assert_eq!((queue.count(State::Done), queue.items.len()), (50, 50));
        assert!(files.iter().all(|file| file.metadata().is_ok_and(|meta| meta.len() > 0)));
        let _ = std::fs::remove_dir_all(dir);
    }
}
