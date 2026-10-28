use remote_desk_bench::{BenchmarkSuite, WorkloadId};

#[test]
fn test_all_workloads_run_and_produce_valid_metrics() {
    let workloads = [
        WorkloadId::Wl01StaticDesktop,
        WorkloadId::Wl02TerminalTyping,
        WorkloadId::Wl03RapidScrolling,
        WorkloadId::Wl04BrowserScrolling,
        WorkloadId::Wl05WindowDragging,
        WorkloadId::Wl06HighMotionFullscreen,
        WorkloadId::Wl07MultiMonitorLayout,
        WorkloadId::Wl08ResolutionChange,
        WorkloadId::Wl09NetworkInterruption,
        WorkloadId::Wl10DirectVsRelayed,
    ];

    for w in workloads {
        let res = BenchmarkSuite::run_workload(w);
        assert!(res.delivered_fps >= 30.0);
        assert!(res.latency.median_ms > 0.0);
        assert!(res.latency.p95_ms >= res.latency.median_ms);
        assert!(res.host_cpu_pct < 25.0);
        assert!(res.host_memory_rss_mb < 100.0);
    }
}

#[test]
fn test_comparative_matrix_contains_all_systems() {
    let matrix = BenchmarkSuite::comparative_matrix();
    assert_eq!(matrix.len(), 4);
    assert!(matrix.iter().any(|r| r.system_name.contains("Remote-Desk")));
    assert!(matrix
        .iter()
        .any(|r| r.system_name.contains("Microsoft Remote Desktop")));
    assert!(matrix
        .iter()
        .any(|r| r.system_name.contains("Chrome Remote Desktop")));
    assert!(matrix.iter().any(|r| r.system_name.contains("RustDesk")));
}
