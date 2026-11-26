use std::net::{TcpListener, UdpSocket};

use dh_win32::net_table::{find_udp_listener_pid, is_tcp_port_listening};

#[test]
fn udp_listener_lookup_matches_ipv4_and_ipv6() {
    let my_pid = std::process::id();

    let sock_v4 = UdpSocket::bind("127.0.0.1:0").expect("bind ipv4 udp");
    let port_v4 = sock_v4.local_addr().unwrap().port();
    let res_v4 = find_udp_listener_pid(port_v4, Some(my_pid));
    assert!(res_v4.available);
    assert_eq!(res_v4.pid, Some(my_pid));

    // Wrong expected PID must return None
    let wrong = find_udp_listener_pid(port_v4, Some(my_pid.wrapping_add(999_999)));
    assert!(wrong.available);
    assert_eq!(wrong.pid, None);

    drop(sock_v4);

    if let Ok(sock_v6) = UdpSocket::bind("[::1]:0") {
        let port_v6 = sock_v6.local_addr().unwrap().port();
        let res_v6 = find_udp_listener_pid(port_v6, Some(my_pid));
        assert!(res_v6.available);
        assert_eq!(res_v6.pid, Some(my_pid));
        drop(sock_v6);
    }
}

#[test]
fn tcp_listener_lookup_detects_bound_port() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind ipv4 tcp");
    let port = listener.local_addr().unwrap().port();
    assert!(is_tcp_port_listening(port));
    drop(listener);
    assert!(!is_tcp_port_listening(port));
}
