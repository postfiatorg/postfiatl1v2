#[cfg(test)]
mod bounded_accept_tests {
    use super::*;
    use std::io::{BufRead, BufReader};
    use std::sync::mpsc;

    #[test]
    fn validator_accept_loop_stops_after_shutdown_signal() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind test listener");
        listener
            .set_nonblocking(true)
            .expect("make test listener nonblocking");
        let shutdown_signal = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let signal_for_server = Arc::clone(&shutdown_signal);
        let server = thread::spawn(move || {
            accept_transport_validator_connection(&listener, &signal_for_server)
        });

        std::thread::sleep(Duration::from_millis(25));
        shutdown_signal.store(true, Ordering::Release);
        let accepted = server
            .join()
            .expect("validator accept thread panicked")
            .expect("validator accept loop failed");
        assert!(accepted.is_none(), "shutdown must end the accept loop");
    }

    // N2 regression: a stalled connection must not prevent a second
    // connection from being accepted and fully served.
    #[test]
    fn stalled_peer_does_not_block_second_connection() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind test listener");
        let address = listener.local_addr().expect("test listener address");
        let server = thread::spawn(move || {
            accept_transport_connections_bounded(
                &listener,
                2,
                TRANSPORT_BLOCK_VOTE_LISTEN_MAX_IN_FLIGHT,
                10_000,
                "test bounded accept",
                |mut stream| {
                    let mut line = String::new();
                    let read_result = BufReader::new(&stream).read_line(&mut line);
                    if read_result.is_ok() {
                        use std::io::Write;
                        let _ = stream.write_all(b"ok\n");
                        Ok(line)
                    } else {
                        Ok("error\n".to_string())
                    }
                },
            )
        });

        // First peer connects and stalls without ever sending a frame.
        let stalled = TcpStream::connect(address).expect("connect stalled peer");
        std::thread::sleep(Duration::from_millis(100));

        // Second peer must be accepted and served even though the first peer
        // never sends anything.
        let mut second = TcpStream::connect(address).expect("connect second peer");
        second
            .set_read_timeout(Some(Duration::from_secs(5)))
            .expect("set second peer read timeout");
        use std::io::Write;
        second
            .write_all(b"second\n")
            .expect("write second peer frame");
        let mut response = String::new();
        BufReader::new(&second)
            .read_line(&mut response)
            .expect("read second peer response");
        assert_eq!(response, "ok\n");

        // Unblock the stalled peer so the server can finish its max_requests
        // quota and the assertion above can be joined.
        drop(stalled);
        let accepted = server
            .join()
            .expect("server thread panicked")
            .expect("bounded accept run failed");
        assert!(accepted.iter().any(|line| line == "second\n"));
    }

    // N2 regression: the bounded accept loop enforces the read timeout so a
    // stalled connection's slot is released even when the peer never sends.
    #[test]
    fn stalled_connection_released_by_read_timeout() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind test listener");
        let address = listener.local_addr().expect("test listener address");
        let (release_tx, release_rx) = mpsc::channel();
        let server = thread::spawn(move || {
            accept_transport_connections_bounded(
                &listener,
                2,
                1,
                300,
                "test bounded accept",
                move |stream| {
                    let mut line = String::new();
                    let read_result = BufReader::new(&stream).read_line(&mut line);
                    let _ = release_tx.send(());
                    let _ = read_result;
                    Ok(line)
                },
            )
        });

        // Fill the only in-flight slot with a stalled peer.
        let _stalled = TcpStream::connect(address).expect("connect stalled peer");
        // The slot must be released by the read timeout without the peer
        // sending anything; then the server accepts the second connection.
        release_rx
            .recv_timeout(Duration::from_secs(10))
            .expect("stalled connection must be released by read timeout");
        let mut second = TcpStream::connect(address).expect("connect second peer");
        use std::io::Write;
        second
            .write_all(b"second\n")
            .expect("write second peer frame");
        let accepted = server
            .join()
            .expect("server thread panicked")
            .expect("bounded accept run failed");
        assert!(accepted.iter().any(|line| line == "second\n"));
    }

    // A failed worker must not make the listener return while other accepted
    // workers are still running. Detached workers could otherwise continue
    // mutating vote state after the caller has observed a terminal error.
    #[test]
    fn worker_error_waits_for_all_accepted_workers() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind test listener");
        let address = listener.local_addr().expect("test listener address");
        let (started_tx, started_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let release_rx = Arc::new(Mutex::new(release_rx));
        let server = thread::spawn(move || {
            accept_transport_connections_bounded(
                &listener,
                2,
                2,
                10_000,
                "test bounded accept",
                move |stream| {
                    let mut line = String::new();
                    BufReader::new(&stream)
                        .read_line(&mut line)
                        .map_err(|error| error.to_string())?;
                    if line == "fail\n" {
                        return Err("intentional worker failure".to_string());
                    }
                    started_tx.send(()).map_err(|error| error.to_string())?;
                    release_rx
                        .lock()
                        .map_err(|_| "release receiver lock poisoned".to_string())?
                        .recv()
                        .map_err(|error| error.to_string())?;
                    Ok(line)
                },
            )
        });

        let mut failed = TcpStream::connect(address).expect("connect failed worker");
        use std::io::Write;
        failed.write_all(b"fail\n").expect("write failed worker");
        let mut blocked = TcpStream::connect(address).expect("connect blocked worker");
        blocked
            .write_all(b"blocked\n")
            .expect("write blocked worker");
        started_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("second worker started");
        assert!(
            !server.is_finished(),
            "listener returned before every accepted worker completed"
        );
        release_tx.send(()).expect("release second worker");

        let error = server
            .join()
            .expect("server thread panicked")
            .expect_err("worker failure must propagate");
        assert_eq!(error, "intentional worker failure");
    }

    #[test]
    fn validator_serving_resource_bounds_are_enforced() {
        let slots = Arc::new((Mutex::new(1_usize), Condvar::new()));
        let shutdown = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let first = acquire_transport_validator_in_flight_permit(&slots, &shutdown)
            .expect("acquire first permit")
            .expect("first permit available");
        let slots_for_waiter = Arc::clone(&slots);
        let shutdown_for_waiter = Arc::clone(&shutdown);
        let (acquired_tx, acquired_rx) = mpsc::channel();
        let waiter = thread::spawn(move || {
            let permit = acquire_transport_validator_in_flight_permit(
                &slots_for_waiter,
                &shutdown_for_waiter,
            )
            .expect("acquire second permit");
            acquired_tx
                .send(permit.is_some())
                .expect("report second permit");
        });
        assert!(
            acquired_rx.recv_timeout(Duration::from_millis(50)).is_err(),
            "second worker must wait while the only slot is held"
        );
        drop(first);
        assert!(
            acquired_rx
                .recv_timeout(Duration::from_secs(1))
                .expect("second permit released"),
            "second worker must acquire the released slot"
        );
        waiter.join().expect("permit waiter panicked");

        let mut retained = Vec::new();
        let mut retained_count = 0;
        let mut truncated = false;
        for value in 0..=TRANSPORT_VALIDATOR_RETAINED_SUMMARY_LIMIT {
            retain_transport_validator_summary(
                &mut retained_count,
                &mut truncated,
                &mut retained,
                value,
            );
        }
        assert_eq!(retained.len(), TRANSPORT_VALIDATOR_RETAINED_SUMMARY_LIMIT);
        assert_eq!(retained_count, TRANSPORT_VALIDATOR_RETAINED_SUMMARY_LIMIT);
        assert!(truncated);

        assert_eq!(transport_batch_serve_rejection_budget(1), 16);
        assert_eq!(
            transport_batch_serve_rejection_budget(usize::MAX),
            TRANSPORT_BATCH_SERVE_MAX_REJECTIONS
        );
    }
}
