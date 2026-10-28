use std::path::Path;
use std::sync::Arc;

use margin_kernel::StoreError;
use margin_terminals::{Error, SessionId, TerminalSize, Terminals};
use margin_test_support::{FakePtyHost, RecordingPtyEvents};

const VAULT: &str = "/vault";

fn size() -> TerminalSize {
    TerminalSize::new(80, 24)
}

fn id() -> SessionId {
    SessionId::new(1)
}

fn setup() -> (FakePtyHost, Terminals, Arc<RecordingPtyEvents>) {
    let host = FakePtyHost::new();
    let terminals = Terminals::new(Arc::new(host.clone()));
    (host, terminals, Arc::new(RecordingPtyEvents::new()))
}

#[test]
fn spawn_opens_a_session_in_the_vault_directory() {
    let (host, terminals, events) = setup();
    terminals
        .spawn(id(), Path::new(VAULT), size(), events)
        .expect("the session starts");
    let session = host.last_session().expect("the host started a session");
    assert_eq!(session.cwd(), Path::new(VAULT));
    assert_eq!(session.size(), size());
}

#[test]
fn spawn_without_a_vault_is_refused() {
    let (host, terminals, events) = setup();
    let result = terminals.spawn(id(), Path::new(""), size(), events);
    assert!(matches!(result, Err(Error::NoVault)));
    assert_eq!(host.session_count(), 0);
}

#[test]
fn spawn_reports_a_host_failure_and_keeps_no_session() {
    let (host, terminals, events) = setup();
    host.fail_next_spawn(StoreError::Unavailable("no pseudo-terminal".to_owned()));
    let result = terminals.spawn(id(), Path::new(VAULT), size(), events);
    assert!(matches!(
        result,
        Err(Error::Host(StoreError::Unavailable(_)))
    ));
    assert!(
        matches!(terminals.write(id(), b"x"), Err(Error::NotRunning(session)) if session == id())
    );
}

#[test]
fn output_and_exit_reach_the_caller() {
    let (host, terminals, events) = setup();
    terminals
        .spawn(
            id(),
            Path::new(VAULT),
            size(),
            Arc::<RecordingPtyEvents>::clone(&events),
        )
        .expect("the session starts");
    let session = host.last_session().expect("the host started a session");
    session.output(b"hi");
    assert_eq!(events.output_bytes(), b"hi");
    session.exit(3);
    assert_eq!(events.exit_codes(), [3]);
}

#[test]
fn write_reaches_the_session_and_is_refused_once_it_exits() {
    let (host, terminals, events) = setup();
    terminals
        .spawn(id(), Path::new(VAULT), size(), events)
        .expect("the session starts");
    let session = host.last_session().expect("the host started a session");
    terminals
        .write(id(), b"ls\n")
        .expect("the session accepts input");
    assert_eq!(session.written(), b"ls\n");
    session.exit(0);
    assert!(
        matches!(terminals.write(id(), b"ls\n"), Err(Error::NotRunning(session)) if session == id())
    );
}

#[test]
fn an_unknown_session_is_not_running() {
    let (_host, terminals, _events) = setup();
    assert!(
        matches!(terminals.write(id(), b"x"), Err(Error::NotRunning(session)) if session == id())
    );
    assert!(
        matches!(terminals.resize(id(), size()), Err(Error::NotRunning(session)) if session == id())
    );
}

#[test]
fn resize_reaches_the_session() {
    let (host, terminals, events) = setup();
    terminals
        .spawn(id(), Path::new(VAULT), size(), events)
        .expect("the session starts");
    terminals
        .resize(id(), TerminalSize::new(120, 40))
        .expect("the session resizes");
    let session = host.last_session().expect("the host started a session");
    assert_eq!(session.resizes(), [TerminalSize::new(120, 40)]);
}

#[test]
fn kill_ends_one_session() {
    let (host, terminals, events) = setup();
    terminals
        .spawn(id(), Path::new(VAULT), size(), events)
        .expect("the session starts");
    terminals.kill(id());
    assert!(
        host.last_session()
            .expect("the host started a session")
            .killed()
    );
    assert!(
        matches!(terminals.write(id(), b"x"), Err(Error::NotRunning(session)) if session == id())
    );
}

#[test]
fn kill_all_ends_every_session() {
    let (host, terminals, events) = setup();
    terminals
        .spawn(
            SessionId::new(1),
            Path::new(VAULT),
            size(),
            Arc::<RecordingPtyEvents>::clone(&events),
        )
        .expect("the session starts");
    terminals
        .spawn(SessionId::new(2), Path::new(VAULT), size(), events)
        .expect("the session starts");
    terminals.kill_all();
    assert!(
        host.session(0)
            .expect("the host started a session")
            .killed()
    );
    assert!(
        host.session(1)
            .expect("the host started a session")
            .killed()
    );
}

#[test]
fn a_replaced_session_does_not_end_its_replacement() {
    let (host, terminals, events) = setup();
    terminals
        .spawn(
            id(),
            Path::new(VAULT),
            size(),
            Arc::<RecordingPtyEvents>::clone(&events),
        )
        .expect("the session starts");
    let replaced = host.session(0).expect("the host started a session");
    terminals
        .spawn(
            id(),
            Path::new(VAULT),
            size(),
            Arc::<RecordingPtyEvents>::clone(&events),
        )
        .expect("the session starts");
    assert!(replaced.killed());
    replaced.exit(0);
    terminals
        .write(id(), b"x")
        .expect("the replacement session still accepts input");
}

#[test]
fn dropping_the_service_ends_every_session() {
    let (host, terminals, events) = setup();
    terminals
        .spawn(id(), Path::new(VAULT), size(), events)
        .expect("the session starts");
    drop(terminals);
    assert!(
        host.last_session()
            .expect("the host started a session")
            .killed()
    );
}
