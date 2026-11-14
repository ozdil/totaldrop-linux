// SPDX-FileCopyrightText: 2026 The TotalDrop Contributors
// SPDX-License-Identifier: GPL-3.0-or-later
//
//! End-to-end client tests against a rendezvous server (`TOTALDROP_TEST_SERVER`)
//! and relay (`TOTALDROP_TEST_RELAY`), plus offline protocol boundary checks
//! that run without an external server.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::{Path, PathBuf};
use std::time::Duration;

use totaldrop_core::rendezvous::{self, Mailbox};
use totaldrop_core::{
    Error, IncomingOffer, PathOptions, PreparedSend, ReceiveOptions, SendOptions, no_events,
};
use totaldrop_proto::wire::Mood;

/// External rendezvous server URL when `TOTALDROP_TEST_SERVER` is configured.
fn server() -> Option<String> {
    std::env::var("TOTALDROP_TEST_SERVER")
        .ok()
        .filter(|s| !s.is_empty())
}

const TEST_RATE_BURST: u32 = 4;
const TEST_RATE_INTERVAL: Duration = Duration::from_secs(5);

/// External rate-limited rendezvous server URL when `TOTALDROP_TEST_SERVER_LIMITED` is configured.
fn limited_server() -> Option<String> {
    std::env::var("TOTALDROP_TEST_SERVER_LIMITED")
        .ok()
        .filter(|s| !s.is_empty())
}

/// External relay URL when `TOTALDROP_TEST_RELAY` is configured.
fn relay() -> Option<String> {
    std::env::var("TOTALDROP_TEST_RELAY")
        .ok()
        .filter(|s| !s.is_empty())
}

/// Loopback transfer options without a relay.
fn no_relay() -> PathOptions {
    PathOptions {
        relay: String::new(),
        stun: String::new(),
        relay_only: false,
        gateway: totaldrop_core::portmap::Gateway::Off,
    }
}

fn scratch(name: &str) -> PathBuf {
    let mut rnd = [0u8; 6];
    getrandom::fill(&mut rnd).unwrap();
    let dir = std::env::temp_dir().join(format!(
        "totaldrop-e2e-{name}-{}",
        rnd.iter().map(|b| format!("{b:02x}")).collect::<String>()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn pseudo_random(len: usize, seed: u8) -> Vec<u8> {
    let mut x = u32::from(seed).wrapping_mul(2_654_435_761).wrapping_add(1);
    (0..len)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            x as u8
        })
        .collect()
}

fn same_tree(a: &Path, b: &Path) {
    let mut left: Vec<_> = walk(a)
        .into_iter()
        .map(|p| p.strip_prefix(a).unwrap().to_path_buf())
        .collect();
    let mut right: Vec<_> = walk(b)
        .into_iter()
        .map(|p| p.strip_prefix(b).unwrap().to_path_buf())
        .collect();
    left.sort();
    right.sort();
    assert_eq!(left, right, "different file lists");
    for rel in left {
        let (x, y) = (a.join(&rel), b.join(&rel));
        if x.is_file() {
            assert_eq!(
                std::fs::read(&x).unwrap(),
                std::fs::read(&y).unwrap(),
                "{rel:?} differs"
            );
        }
    }
}

fn walk(p: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for e in std::fs::read_dir(p).unwrap() {
        let path = e.unwrap().path();
        out.push(path.clone());
        if path.is_dir() {
            out.extend(walk(&path));
        }
    }
    out
}

async fn transfer(
    server: &str,
    paths: Vec<PathBuf>,
    dest: &Path,
) -> (totaldrop_core::SendReport, totaldrop_core::ReceiveReport) {
    let prepared = PreparedSend::new(
        &SendOptions {
            server: server.to_owned(),
            words: 2,
            paths: no_relay(),
        },
        paths,
    )
    .await
    .unwrap();
    let code = prepared.code().to_string();
    let sender = tokio::spawn(prepared.run(no_events()));
    let offer = IncomingOffer::connect(
        &ReceiveOptions {
            server: server.to_owned(),
            paths: no_relay(),
        },
        &code,
    )
    .await
    .unwrap();
    let received = offer.accept(dest, no_events()).await.unwrap();
    let sent = sender.await.unwrap().unwrap();
    (sent, received)
}

#[tokio::test(flavor = "multi_thread")]
async fn folder_with_large_empty_and_nested_files_arrives_intact() {
    let Some(server) = server() else { return };
    let src = scratch("src");
    let dest = scratch("dest");
    let album = src.join("album");
    std::fs::create_dir_all(album.join("nested/deeper")).unwrap();
    std::fs::write(album.join("big.bin"), pseudo_random(5 * 512 * 1024, 1)).unwrap();
    std::fs::write(album.join("one-chunk.bin"), pseudo_random(1 << 20, 2)).unwrap();
    std::fs::write(album.join("empty.txt"), b"").unwrap();
    std::fs::write(album.join("nested/deeper/sarki u.txt"), "hello world").unwrap();
    std::fs::create_dir_all(album.join("empty-folder")).unwrap();

    let (sent, received) = transfer(&server, vec![album.clone()], &dest).await;
    assert_eq!(sent.files_ok, 4);
    assert_eq!(sent.files_failed, 0);
    assert_eq!(received.files_ok, 4);
    same_tree(&src, &dest);

    let a = std::fs::metadata(album.join("big.bin"))
        .unwrap()
        .modified()
        .unwrap();
    let b = std::fs::metadata(dest.join("album/big.bin"))
        .unwrap()
        .modified()
        .unwrap();
    let diff = a.duration_since(b).unwrap_or_else(|e| e.duration());
    assert!(diff < Duration::from_secs(1));

    assert!(
        walk(&dest)
            .iter()
            .all(|p| !p.to_string_lossy().contains(".totaldrop-"))
    );
    std::fs::remove_dir_all(src).unwrap();
    std::fs::remove_dir_all(dest).unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_text_arrives_as_a_text() {
    let Some(server) = server() else { return };
    let dest = scratch("dest");
    let words = "Link: https://totaldrop.app\n\tsecond line";
    let prepared = PreparedSend::text(
        &SendOptions {
            server: server.clone(),
            words: 2,
            paths: no_relay(),
        },
        words,
        "text.txt",
    )
    .await
    .unwrap();
    assert_eq!(prepared.totals(), (1, words.len() as u64));
    let code = prepared.code().to_string();
    let sender = tokio::spawn(prepared.run(no_events()));
    let offer = IncomingOffer::connect(
        &ReceiveOptions {
            server,
            paths: no_relay(),
        },
        &code,
    )
    .await
    .unwrap();
    assert!(offer.summary().text);
    let received = offer.accept_text().await.unwrap();
    let sent = sender.await.unwrap().unwrap();
    assert_eq!((sent.files_ok, received.files_ok), (1, 1));
    assert_eq!(received.text.as_deref(), Some(words));
    assert!(walk(&dest).is_empty(), "{:?}", walk(&dest));
    std::fs::remove_dir_all(dest).unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn existing_files_are_never_overwritten() {
    let Some(server) = server() else { return };
    let src = scratch("src");
    let dest = scratch("dest");
    std::fs::write(src.join("note.txt"), "new").unwrap();
    std::fs::write(dest.join("note.txt"), "old").unwrap();
    let (_, received) = transfer(&server, vec![src.join("note.txt")], &dest).await;
    assert_eq!(
        std::fs::read_to_string(dest.join("note.txt")).unwrap(),
        "old"
    );
    assert_eq!(
        std::fs::read_to_string(dest.join("note (2).txt")).unwrap(),
        "new"
    );
    assert_eq!(received.placed, vec![dest.join("note (2).txt")]);
    std::fs::remove_dir_all(src).unwrap();
    std::fs::remove_dir_all(dest).unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn wrong_code_fails_on_both_sides_and_burns_the_nameplate() {
    let Some(server) = server() else { return };
    let src = scratch("src");
    std::fs::write(src.join("secret.txt"), "top secret").unwrap();
    let prepared = PreparedSend::new(
        &SendOptions {
            server: server.clone(),
            words: 2,
            paths: no_relay(),
        },
        vec![src.join("secret.txt")],
    )
    .await
    .unwrap();
    let real = prepared.code().clone();
    let sender = tokio::spawn(prepared.run(no_events()));

    let mut guess = real.canonical();
    let last = guess.rsplit('-').next().unwrap().parse::<u16>().unwrap();
    let wrong = (last + 1) % 2048;
    guess = format!("{}-{wrong}", guess.rsplit_once('-').unwrap().0);
    let opts = ReceiveOptions {
        server: server.clone(),
        paths: no_relay(),
    };
    let attacker = IncomingOffer::connect(&opts, &guess).await;
    assert!(matches!(attacker, Err(Error::WrongCode)), "{attacker:?}");
    let sent = sender.await.unwrap();
    assert!(matches!(sent, Err(Error::WrongCode)), "{sent:?}");

    let late = IncomingOffer::connect(&opts, &real.to_string()).await;
    assert!(matches!(late, Err(Error::Rendezvous(_))), "{late:?}");
    std::fs::remove_dir_all(src).unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn declined_offer_is_reported_to_the_sender() {
    let Some(server) = server() else { return };
    let src = scratch("src");
    std::fs::write(src.join("a.txt"), "a").unwrap();
    let prepared = PreparedSend::new(
        &SendOptions {
            server: server.clone(),
            words: 3,
            paths: no_relay(),
        },
        vec![src.join("a.txt")],
    )
    .await
    .unwrap();
    let code = prepared.code().to_string();
    assert_eq!(code.split('-').count(), 4, "three words plus the nameplate");
    let sender = tokio::spawn(prepared.run(no_events()));
    let offer = IncomingOffer::connect(
        &ReceiveOptions {
            server,
            paths: no_relay(),
        },
        &code,
    )
    .await
    .unwrap();
    assert_eq!(offer.summary().files, 1);
    assert_eq!(offer.words().len(), 4);
    offer.decline().await.unwrap();
    let sent = sender.await.unwrap();
    assert!(
        matches!(
            sent,
            Err(Error::Rejected(
                totaldrop_proto::wire::RejectReason::Declined
            ))
        ),
        "{sent:?}"
    );
    std::fs::remove_dir_all(src).unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn server_refuses_a_third_claim() {
    let Some(server) = server() else { return };
    let alloc = rendezvous::allocate(&server).await.unwrap();
    let n = alloc.nameplate;
    let a = Mailbox::claim(&server, n, Some(&alloc)).await.unwrap();
    let b = Mailbox::claim(&server, n, None).await.unwrap();
    assert_eq!(a.id, b.id);
    let c = Mailbox::claim(&server, n, None).await;
    assert!(matches!(c, Err(Error::Rendezvous(r)) if r == "nameplate full"));
}

#[tokio::test(flavor = "multi_thread")]
async fn only_the_allocator_can_claim_first() {
    let Some(server) = server() else { return };
    let alloc = rendezvous::allocate(&server).await.unwrap();
    assert!(matches!(
        Mailbox::claim(&server, alloc.nameplate, None).await,
        Err(Error::Rendezvous(_))
    ));
    let other = rendezvous::allocate(&server).await.unwrap();
    assert_ne!(other.nameplate, alloc.nameplate);
    assert!(matches!(
        Mailbox::claim(&server, alloc.nameplate, Some(&other)).await,
        Err(Error::Rendezvous(_))
    ));
    assert!(matches!(
        Mailbox::claim(&server, 4242, None).await,
        Err(Error::Rendezvous(_))
    ));
    Mailbox::claim(&server, alloc.nameplate, Some(&alloc))
        .await
        .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn scary_close_retires_the_nameplate() {
    let Some(server) = server() else { return };
    let alloc = rendezvous::allocate(&server).await.unwrap();
    let n = alloc.nameplate;
    let mut a = Mailbox::claim(&server, n, Some(&alloc)).await.unwrap();
    let mut b = Mailbox::claim(&server, n, None).await.unwrap();
    a.add("pake", vec![1; 32]).await.unwrap();
    b.add("pake", vec![2; 32]).await.unwrap();
    assert_eq!(a.expect("pake").await.unwrap(), vec![2; 32]);
    assert_eq!(b.expect("pake").await.unwrap(), vec![1; 32]);
    a.close(Mood::Scary).await;
    assert!(matches!(b.expect("confirm").await, Err(Error::WrongCode)));
    b.close(Mood::Lonely).await;
    tokio::time::sleep(Duration::from_millis(100)).await;
    let next = rendezvous::allocate(&server).await.unwrap();
    assert_ne!(next.nameplate, n, "a retired nameplate must not be reused");
}

#[tokio::test(flavor = "multi_thread")]
async fn codes_back_to_back_all_open() {
    let Some(server) = server() else { return };
    for i in 0..24 {
        let alloc = rendezvous::allocate(&server).await.unwrap();
        let n = alloc.nameplate;
        let mailbox = Mailbox::claim(&server, n, Some(&alloc))
            .await
            .unwrap_or_else(|e| panic!("code {i} (nameplate {n}) did not open: {e}"));
        if i % 2 == 0 {
            mailbox.close(Mood::Lonely).await;
        } else {
            drop(mailbox);
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_never_allocated_nameplate_is_not_in_use() {
    let Some(server) = server() else { return };
    let err = Mailbox::claim(&server, 4_000_000_000, None)
        .await
        .unwrap_err();
    assert!(err.is_code_not_in_use(), "{err}");
}

fn too_many<T>(r: Result<T, Error>) -> bool {
    matches!(r, Err(Error::Rendezvous(reason)) if reason == "too many requests")
}

#[track_caller]
fn assert_too_many<T: std::fmt::Debug>(r: Result<T, Error>) {
    let shown = format!("{r:?}");
    assert!(too_many(r), "expected \"too many requests\", got {shown}");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_client_over_its_rate_is_refused_until_it_waits() {
    let Some(server) = limited_server() else {
        return;
    };
    for _ in 0..=TEST_RATE_BURST {
        use tokio_tungstenite::tungstenite::client::IntoClientRequest;
        let mut req = format!("{server}/v0/allocate")
            .into_client_request()
            .unwrap();
        req.headers_mut()
            .insert("Origin", "https://example.com".parse().unwrap());
        assert!(matches!(
            tokio_tungstenite::connect_async(req).await,
            Err(tokio_tungstenite::tungstenite::Error::Http(r)) if r.status() == 403
        ));
    }
    let alloc = rendezvous::allocate(&server).await.unwrap();
    let _sender = Mailbox::claim(&server, alloc.nameplate, Some(&alloc))
        .await
        .unwrap();
    rendezvous::allocate(&server).await.unwrap();
    let unused = Mailbox::claim(&server, 4_000_000_000, None)
        .await
        .unwrap_err();
    assert!(unused.is_code_not_in_use(), "{unused}");
    assert_too_many(rendezvous::allocate(&server).await);
    assert_too_many(Mailbox::claim(&server, alloc.nameplate, None).await);
    assert_too_many(rendezvous::allocate(&server).await);
    tokio::time::sleep(TEST_RATE_INTERVAL + Duration::from_millis(300)).await;
    Mailbox::claim(&server, alloc.nameplate, None)
        .await
        .unwrap();
    assert_too_many(rendezvous::allocate(&server).await);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_websocket_from_a_web_page_is_refused() {
    use tokio_tungstenite::tungstenite::client::IntoClientRequest;

    let (Some(server), Some(relay)) = (server(), relay()) else {
        return;
    };
    let pair = format!("{}-{}", "1".repeat(64), "2".repeat(64));
    let urls = [
        format!("{server}/v0/allocate"),
        format!("{server}/v0/mailbox/7"),
        format!("{}/{pair}", relay.trim_end_matches('/')),
    ];
    for url in &urls {
        let mut req = url.as_str().into_client_request().unwrap();
        req.headers_mut()
            .insert("Origin", "https://example.com".parse().unwrap());
        match tokio_tungstenite::connect_async(req).await {
            Err(tokio_tungstenite::tungstenite::Error::Http(resp)) => {
                assert_eq!(resp.status(), 403, "{url}");
            }
            other => panic!("{url}: a browser's WebSocket was not refused: {other:?}"),
        }
        let req = url.as_str().into_client_request().unwrap();
        assert!(
            tokio_tungstenite::connect_async(req).await.is_ok(),
            "{url}: refused without Origin"
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn plain_websocket_to_another_host_is_refused() {
    let r = rendezvous::allocate("ws://example.com").await;
    assert!(matches!(r, Err(Error::Rendezvous(_))));
}

async fn transfer_with(
    server: &str,
    send_paths: PathOptions,
    recv_paths: PathOptions,
    files: Vec<PathBuf>,
    dest: &Path,
) -> (totaldrop_core::SendReport, totaldrop_core::ReceiveReport) {
    let prepared = PreparedSend::new(
        &SendOptions {
            server: server.to_owned(),
            words: 2,
            paths: send_paths,
        },
        files,
    )
    .await
    .unwrap();
    let code = prepared.code().to_string();
    let sender = tokio::spawn(prepared.run(no_events()));
    let offer = IncomingOffer::connect(
        &ReceiveOptions {
            server: server.to_owned(),
            paths: recv_paths,
        },
        &code,
    )
    .await
    .unwrap();
    let received = offer.accept(dest, no_events()).await.unwrap();
    let sent = sender.await.unwrap().unwrap();
    (sent, received)
}

#[tokio::test(flavor = "multi_thread")]
async fn relay_only_transfer_goes_through_the_relay() {
    let (Some(server), Some(relay_url)) = (server(), relay()) else {
        return;
    };
    let src = scratch("src");
    let dest = scratch("dest");
    std::fs::write(src.join("film.bin"), pseudo_random(3 << 20, 7)).unwrap();
    let only = PathOptions {
        relay: relay_url,
        stun: String::new(),
        relay_only: true,
        gateway: totaldrop_core::portmap::Gateway::Off,
    };
    let (sent, received) = transfer_with(
        &server,
        only.clone(),
        only,
        vec![src.join("film.bin")],
        &dest,
    )
    .await;
    assert_eq!((sent.files_ok, received.files_ok), (1, 1));
    same_tree(&src, &dest);
    std::fs::remove_dir_all(src).unwrap();
    std::fs::remove_dir_all(dest).unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_direct_path_wins_over_the_relay() {
    let (Some(server), Some(relay_url)) = (server(), relay()) else {
        return;
    };
    let src = scratch("src");
    let dest = scratch("dest");
    std::fs::write(src.join("photo.bin"), pseudo_random(2 << 20, 8)).unwrap();
    let both = PathOptions {
        relay: relay_url,
        stun: String::new(),
        relay_only: false,
        gateway: totaldrop_core::portmap::Gateway::Off,
    };
    let (sent, received) = transfer_with(
        &server,
        both.clone(),
        both,
        vec![src.join("photo.bin")],
        &dest,
    )
    .await;
    assert_eq!((sent.files_ok, received.files_ok), (1, 1));
    same_tree(&src, &dest);
    std::fs::remove_dir_all(src).unwrap();
    std::fs::remove_dir_all(dest).unwrap();
}

fn counting(
    moved: std::sync::Arc<std::sync::atomic::AtomicU64>,
    slow: bool,
) -> totaldrop_core::Events {
    std::sync::Arc::new(move |e| {
        if let totaldrop_core::Event::Progress { done, .. } = e {
            moved.store(done, std::sync::atomic::Ordering::SeqCst);
            if slow {
                std::thread::sleep(Duration::from_millis(20));
            }
        }
    })
}

fn stop_when(
    stop: totaldrop_core::Stop,
    moved: std::sync::Arc<std::sync::atomic::AtomicU64>,
    after: u64,
) {
    tokio::spawn(async move {
        while moved.load(std::sync::atomic::Ordering::SeqCst) < after {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        stop.stop();
    });
}

fn left_nothing(dest: &Path) {
    let names: Vec<_> = std::fs::read_dir(dest).unwrap().collect();
    assert!(names.is_empty(), "left behind: {:?}", walk(dest));
}

fn left_only_whole_files(dest: &Path, src: &Path) {
    for path in walk(dest) {
        let rel = path.strip_prefix(dest).unwrap();
        if path.is_dir() {
            assert!(
                std::fs::read_dir(&path).unwrap().next().is_some(),
                "empty folder left behind: {rel:?}"
            );
        } else {
            let original =
                std::fs::read(src.join(rel)).unwrap_or_else(|_| panic!("left behind: {rel:?}"));
            assert!(
                std::fs::read(&path).unwrap() == original,
                "partial file left behind: {rel:?}"
            );
        }
    }
}

fn folder(src: &Path, files: usize, size: usize) -> PathBuf {
    let dir = src.join("album");
    std::fs::create_dir_all(dir.join("raw")).unwrap();
    for i in 0..files {
        let seed = u8::try_from(i % 251).unwrap();
        std::fs::write(
            dir.join("raw").join(format!("{i:03}.bin")),
            pseudo_random(size, seed),
        )
        .unwrap();
    }
    dir
}

async fn prepared(server: &str, what: PathBuf) -> PreparedSend {
    PreparedSend::new(
        &SendOptions {
            server: server.to_owned(),
            words: 2,
            paths: no_relay(),
        },
        vec![what],
    )
    .await
    .unwrap()
}

async fn offer(server: String, code: &str) -> IncomingOffer {
    IncomingOffer::connect(
        &ReceiveOptions {
            server,
            paths: no_relay(),
        },
        code,
    )
    .await
    .unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn receiver_stops_mid_transfer() {
    let Some(server) = server() else { return };
    let (src, dest) = (scratch("src"), scratch("dest"));
    let p = prepared(&server, folder(&src, 2, 32 << 20)).await;
    let code = p.code().to_string();
    let sender = tokio::spawn(p.run(counting(Default::default(), false)));
    let o = offer(server, &code).await;
    let (stop, moved) = (totaldrop_core::Stop::new(), std::sync::Arc::default());
    stop_when(stop.clone(), std::sync::Arc::clone(&moved), 4 << 20);
    let r = o.accept_with(&dest, counting(moved, false), &stop).await;
    assert!(matches!(r, Err(Error::Stopped)), "{r:?}");
    let s = tokio::time::timeout(Duration::from_secs(10), sender)
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(s, Err(Error::Cancelled(_))), "{s:?}");
    left_nothing(&dest);
    std::fs::remove_dir_all(src).unwrap();
    std::fs::remove_dir_all(dest).unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn sender_stops_mid_folder() {
    let Some(server) = server() else { return };
    let (src, dest) = (scratch("src"), scratch("dest"));
    let total: u64 = 24 * (2 << 20);
    let p = prepared(&server, folder(&src, 24, 2 << 20)).await;
    let code = p.code().to_string();
    let (stop, moved) = (totaldrop_core::Stop::new(), std::sync::Arc::default());
    stop_when(stop.clone(), std::sync::Arc::clone(&moved), 4 << 20);
    let sender = tokio::spawn(p.run_with(counting(std::sync::Arc::clone(&moved), false), stop));
    let o = offer(server, &code).await;
    let r = o.accept(&dest, counting(Default::default(), true)).await;
    assert!(matches!(r, Err(Error::Cancelled(_))), "{r:?}");
    let s = sender.await.unwrap();
    assert!(matches!(s, Err(Error::Stopped)), "{s:?}");
    let sent = moved.load(std::sync::atomic::Ordering::SeqCst);
    assert!(
        sent < total / 2,
        "sent {sent} of {total} bytes after the stop"
    );
    left_only_whole_files(&dest, &src);
    std::fs::remove_dir_all(src).unwrap();
    std::fs::remove_dir_all(dest).unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn receiver_stops_at_the_offer() {
    let Some(server) = server() else { return };
    let src = scratch("src");
    std::fs::write(src.join("a.txt"), b"a").unwrap();
    let p = prepared(&server, src.join("a.txt")).await;
    let code = p.code().to_string();
    let sender = tokio::spawn(p.run(no_events()));
    let o = offer(server, &code).await;
    assert!(matches!(o.stop().await, Error::Stopped));
    let s = tokio::time::timeout(Duration::from_secs(10), sender)
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(s, Err(Error::Cancelled(_))), "{s:?}");
    std::fs::remove_dir_all(src).unwrap();
}

#[tokio::test]
async fn stop_while_waiting_and_while_connecting() {
    let Some(server) = server() else { return };
    let src = scratch("src");
    std::fs::write(src.join("a.txt"), b"a").unwrap();
    let p = prepared(&server, src.join("a.txt")).await;
    let code = p.code().to_string();
    let stop = totaldrop_core::Stop::new();
    let sender = tokio::spawn(p.run_with(no_events(), stop.clone()));
    tokio::time::sleep(Duration::from_millis(200)).await;
    stop.stop();
    let s = tokio::time::timeout(Duration::from_secs(1), sender)
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(s, Err(Error::Stopped)), "{s:?}");

    let stop = totaldrop_core::Stop::new();
    let opts = ReceiveOptions {
        server,
        paths: no_relay(),
    };
    let mut connecting = std::pin::pin!(IncomingOffer::connect_with(&opts, &code, &stop));
    let first =
        std::future::poll_fn(|cx| std::task::Poll::Ready(connecting.as_mut().poll(cx))).await;
    assert!(first.is_pending(), "{first:?}");
    stop.stop();
    let r = tokio::time::timeout(Duration::from_secs(1), connecting)
        .await
        .unwrap();
    assert!(matches!(r, Err(Error::Stopped)), "{r:?}");
    std::fs::remove_dir_all(src).unwrap();
}

type RelayWs =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

async fn relay_sign_in(
    relay: &str,
    own: &totaldrop_core::transport::EndpointKey,
    peer: &[u8; 32],
) -> Option<RelayWs> {
    use futures_util::{SinkExt, StreamExt};
    use tokio_tungstenite::tungstenite::Message;
    use totaldrop_proto::relay as wire;
    let url = format!("{relay}/{}", wire::pair_name(&own.public(), peer));
    let (mut ws, _) = tokio_tungstenite::connect_async(url).await.ok()?;
    let challenge = match tokio::time::timeout(Duration::from_secs(5), ws.next()).await {
        Ok(Some(Ok(Message::Binary(b)))) => wire::parse_challenge(&b)?,
        _ => return None,
    };
    let signature = own.sign(&wire::signed_bytes(&challenge)).ok()?;
    ws.send(Message::Binary(
        wire::hello(&own.public(), &signature).into(),
    ))
    .await
    .ok()?;
    match tokio::time::timeout(Duration::from_secs(5), ws.next()).await {
        Ok(Some(Ok(Message::Binary(b)))) if wire::is_ok(&b) => Some(ws),
        _ => None,
    }
}

async fn relay_passes(ws: &mut RelayWs, at: &mut RelayWs, from: [u8; 32], to: [u8; 32]) {
    use futures_util::{SinkExt, StreamExt};
    use tokio_tungstenite::tungstenite::Message;
    use totaldrop_proto::relay as wire;
    ws.send(Message::Binary(wire::frame(&to, b"datagram").into()))
        .await
        .unwrap();
    let deadline = tokio::time::Instant::now()
        .checked_add(Duration::from_secs(5))
        .unwrap();
    loop {
        match tokio::time::timeout_at(deadline, at.next()).await {
            Ok(Some(Ok(Message::Binary(b)))) => {
                let (key, datagrams) = wire::parse_frame(&b).unwrap();
                assert_eq!(key, from);
                assert_eq!(datagrams, vec![&b"datagram"[..]]);
                return;
            }
            Ok(Some(Ok(Message::Ping(_) | Message::Pong(_)))) => continue,
            other => panic!("no datagram: {other:?}"),
        }
    }
}

async fn relay_closes(ws: &mut RelayWs, wait: Duration) -> bool {
    use futures_util::StreamExt;
    use tokio_tungstenite::tungstenite::Message;
    let deadline = tokio::time::Instant::now().checked_add(wait).unwrap();
    loop {
        match tokio::time::timeout_at(deadline, ws.next()).await {
            Err(_) => return false,
            Ok(None | Some(Err(_)) | Some(Ok(Message::Close(_)))) => return true,
            Ok(Some(Ok(_))) => continue,
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_new_sign_in_replaces_the_old_socket_of_its_key() {
    use totaldrop_core::transport::EndpointKey;
    let Some(relay) = relay() else { return };
    let (a, b) = (
        EndpointKey::generate().unwrap(),
        EndpointKey::generate().unwrap(),
    );
    let mut old_a = relay_sign_in(&relay, &a, &b.public()).await.unwrap();
    let mut b_ws = relay_sign_in(&relay, &b, &a.public()).await.unwrap();
    let mut new_a = relay_sign_in(&relay, &a, &b.public())
        .await
        .expect("the key's holder may sign in again");
    assert!(
        relay_closes(&mut old_a, Duration::from_secs(5)).await,
        "the replaced socket is closed"
    );
    let _ = old_a.close(None).await;
    drop(old_a);
    relay_passes(&mut b_ws, &mut new_a, b.public(), a.public()).await;
    relay_passes(&mut new_a, &mut b_ws, a.public(), b.public()).await;
    assert!(!relay_closes(&mut b_ws, Duration::from_secs(1)).await);
    assert!(!relay_closes(&mut new_a, Duration::from_secs(1)).await);
    relay_passes(&mut b_ws, &mut new_a, b.public(), a.public()).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn the_relay_ends_the_pair_when_one_side_leaves() {
    use totaldrop_core::transport::EndpointKey;
    let Some(relay) = relay() else { return };
    let (a, b) = (
        EndpointKey::generate().unwrap(),
        EndpointKey::generate().unwrap(),
    );
    let mut a_ws = relay_sign_in(&relay, &a, &b.public()).await.unwrap();
    let mut b_ws = relay_sign_in(&relay, &b, &a.public()).await.unwrap();
    relay_passes(&mut a_ws, &mut b_ws, a.public(), b.public()).await;
    a_ws.close(None).await.unwrap();
    assert!(relay_closes(&mut b_ws, Duration::from_secs(5)).await);
}
