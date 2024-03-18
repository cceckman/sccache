//! Tests for storage check:
//! Many sccache processes may hit the same storage backend, e.g. multiple workers or test runners.
//! This test makes sure they don't conflict.
//!
//! Any copyright is dedicated to the Public Domain.
//! http://creativecommons.org/publicdomain/zero/1.0/

use anyhow::{Context, Result};
use once_cell::sync::Lazy;
use serial_test::serial;
use std::path::PathBuf;
use std::process::{Command, Stdio};

#[macro_use]
extern crate log;

static SCCACHE_BIN: Lazy<PathBuf> = Lazy::new(|| assert_cmd::cargo::cargo_bin("sccache"));

fn sccache_worker(port: u16, barrier: &std::sync::Barrier) -> anyhow::Result<()> {
    // Wait for all threads to be synced up to launch:
    barrier.wait();

    // Launch SCCACHE_SERVER as a child of this thread:
    let handle = Command::new(SCCACHE_BIN.as_os_str())
        .stdout(Stdio::null())
        // .stderr(Stdio::piped())
        .env("SCCACHE_SERVER_PORT", format!("{}", port))
        .env("SCCACHE_START_SERVER", "1")
        .env("SCCACHE_NO_DAEMON", "1")
        .spawn()
        .with_context(|| format!("failed to spawn process for port {}", port));
    barrier.wait();

    // Try to check stats; return an error if doing so fails.
    let stats_error = if handle.is_ok() {
        // Run get-stats just to check liveness / check for "listening"
        Command::new(SCCACHE_BIN.as_os_str())
            .arg("--show-stats")
            //.stdout(Stdio::null())
            //.stderr(Stdio::null())
            .env("SCCACHE_SERVER_PORT", format!("{}", port))
            .status()
            .map(|_| ())
            .with_context(|| format!("failed to check server on port {}", port))
    } else {
        Ok(())
    };

    // TODO: DO NOT SUBMIT / WIP:
    // This can reliably reproduce, on the server's stderr,
    // the GCS errors in #2132.
    // However, we aren't capturing them / checking for them here...
    // and in CI, this doesn't really check anything.
    //
    // Probably need more discussion / thought about the strategy here.

    // We've gotten what we need; kill it.
    handle.and_then(|mut child| {
        child
            .kill()
            .with_context(|| format!("failed to kill server on port {}", port))
    })?;
    stats_error
}

#[test]
#[serial]
fn test_multiple_sccache() -> Result<()> {
    trace!("sccache multiple instances");

    const COUNT: u16 = 100;

    // TODO : Should this use the "internal server launch" protocol?

    // Use a barrier to sync up all "launcher" threads.
    let barrier = std::sync::Barrier::new((COUNT + 1) as usize);
    let results: Vec<_> = std::thread::scope(|scope| {
        // Don't overlap with the default sccache server port.
        let handles: Vec<_> = (1..=COUNT)
            .map(|i| {
                let barrier = &barrier;
                let port = 4226 + i;
                scope.spawn(move || sccache_worker(port, barrier))
            })
            .collect();

        // All threads have launched; allow them to launch their servers:
        barrier.wait();
        // Proceed to show-stats:
        barrier.wait();

        // Join all workers:
        handles.into_iter().map(|x| match x.join() {
            Ok(result) => result,
            Err(_) => Err(anyhow::anyhow!("error joining worker thread")),
        }).collect()
    });

    let errors: anyhow::Result<Vec<()>> = results.into_iter().collect();
    errors.map(|_| ())
}
