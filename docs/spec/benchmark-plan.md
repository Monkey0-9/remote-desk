# Performance & Reliability Benchmark Plan: Remote-Desk Platform

| Document Metadata | Details |
|---|---|
| **Document ID** | RD-SPEC-PERF-008 |
| **Author / Agent** | Performance Engineering & Benchmarking Agent |
| **Measurement Targets** | Latency, FPS, Frame Drops, CPU, Memory, Bandwidth |
| **Reference Systems** | Microsoft Remote Desktop, Chrome Remote Desktop, RustDesk |
| **Status** | Approved Benchmark Plan |

---

## 1. Benchmarking Philosophy & Empirical Discipline

Remote-Desk follows the rule: **No optimization without reproducible measurement, and no claims of superior performance without public empirical data.**

All performance claims must:
1. Use identical hardware, screen resolution (1080p60 and 4K60), display scaling, and network simulation parameters.
2. Publish full test methodology, test scripts, and raw metric distributions.
3. Compare against industry benchmarks under identical test conditions:
   - **Microsoft Remote Desktop (RDP)**
   - **Chrome Remote Desktop (CRD)**
   - **RustDesk**

---

## 2. Standardized Evaluation Workloads

To simulate realistic user behavior, the benchmark harness runs 10 automated, repeatable workloads:

| Workload ID | Name | Description & Automation Script | Target Metric Focus |
|---|---|---|---|
| `WL-01` | **Static Desktop** | Displaying standard wallpaper with static text editor open. Zero user activity for 60s. | Idle CPU (< 0.5%), Idle Bandwidth (< 15 Kbps), Idle RAM. |
| `WL-02` | **Terminal Typing** | Automated script typing continuous shell commands and displaying fast stdout at 120 WPM. | Key-to-screen response latency, text crispness. |
| `WL-03` | **Rapid Document Scrolling** | Continuous vertical page-down scrolling in a 100-page PDF document at 30 pages/min. | Frame rate stability (60 FPS), frame jitter, frame drops. |
| `WL-04` | **Web Browser Scrolling** | Smooth pixel scrolling across a complex DOM page (e.g. news portal with images/video). | Scroll smoothness, video codec blockiness/artifacts. |
| `WL-05` | **Window Dragging & Movement**| Dragging a 1200x800 window in circles across the desktop continuously for 30s. | Motion fluidity, tearing, encoder queue buildup. |
| `WL-06` | **High-Motion Fullscreen** | Playing 1080p60 Big Buck Bunny test clip in fullscreen media player. | Encoder throughput, peak bitrate, dropped frame %. |
| `WL-07` | **Multi-Monitor Layout** | Two 1080p displays attached. Active window moved across monitor boundaries. | Topology switch latency, monitor re-encoding time. |
| `WL-08` | **Dynamic Resolution Change**| Host desktop resolution switches dynamically from 1920x1080 to 2560x1440. | Adaptation time, canvas scaling recovery (< 200ms). |
| `WL-09` | **Network Interruption** | NetEm drops 100% of packets for 3 seconds, then restores connection. | Session recovery time (< 1.0s), key state integrity. |
| `WL-10` | **Direct vs. TURN Relayed** | Comparing identical workload (WL-03) over direct LAN P2P vs. WAN TURN relay. | Latency overhead, TURN encryption penalty. |

---

## 3. Metrics & Measurement Methodology

```
+-------------------------------------------------------------------------+
|                  LATENCY MEASUREMENT PIPELINE (PROBE)                   |
+-------------------------------------------------------------------------+
| [Controller Input Injection]                                            |
|   └── Record Monotonic Timestamp T0                                     |
|         │                                                               |
|         ▼ (WebRTC DataChannel)                                          |
| [Host Input Processing & Frame Render]                                  |
|   └── Inverts screen pixel color at cursor (Black <-> White)            |
|         │                                                               |
|         ▼ (Hardware Video Encoder & SRTP Stream)                        |
| [Controller Video Frame Presentation]                                   |
|   └── High-speed camera (240 FPS) or frame-buffer probe detects color   |
|   └── Record Monotonic Timestamp T1                                     |
|                                                                         |
| GLASS-TO-GLASS LATENCY = T1 - T0                                        |
+-------------------------------------------------------------------------+
```

### 3.1 Key Metrics Table
| Metric | Description | LAN Target | WAN Target (50ms RTT) |
|---|---|---|---|
| **Input-to-Visible-Response** | Time from touch/click to visible screen update. | Median < 35ms, P95 < 60ms | Median < 80ms, P95 < 130ms |
| **Delivered Frame Rate** | Sustained frames rendered per second on controller. | 60.0 FPS ± 0.5 | 45.0 - 60.0 FPS |
| **Dropped Frame Rate** | Percentage of frames dropped due to buffer overflow. | < 0.2% | < 2.0% |
| **Host Idle CPU** | CPU load when screen is static. | < 0.5% (single core) | < 0.5% |
| **Host Active CPU** | CPU load during active 1080p60 streaming. | < 12% (hardware enc) | < 15% |
| **Host Memory (RSS)** | Resident memory footprint of host daemon. | < 60 MB RSS | < 65 MB RSS |
| **Controller Memory (RSS)** | RAM footprint of Android controller app. | < 120 MB RSS | < 130 MB RSS |
| **Connection Setup Time** | Handshake initiation to first visible frame. | < 1.2s | < 2.5s |
| **Session Recovery Time** | Time to resume streaming after 3s network dropout. | < 1.0s | < 2.0s |

---

## 4. Benchmark Test Rig & Environment Standardization

To guarantee reproducible test runs:
1. **Host Hardware Reference**:
   - CPU: Intel Core i7 / AMD Ryzen 7 (8 cores, 16 threads).
   - GPU: Intel Iris Xe / NVIDIA RTX (hardware encoding via VA-API / NVENC).
   - OS: Ubuntu 24.04 LTS (Wayland / GNOME 46) & Windows 11 23H2.
2. **Controller Hardware Reference**:
   - Device: Google Pixel 8 / Samsung Galaxy S23 (Snapdragon 8 Gen 2 / Tensor G3).
   - OS: Android 14.
3. **Network Emulation Rig (`tc netem`)**:
   - **Profile A (Ideal LAN)**: 1000 Mbps, 0ms delay, 0% loss, 0% jitter.
   - **Profile B (Home Wi-Fi)**: 100 Mbps, 8ms delay, 0.2% loss, 2ms jitter.
   - **Profile C (Congested WAN / 4G)**: 25 Mbps, 50ms delay, 2.0% loss, 10ms jitter.
   - **Profile D (Severe Enterprise Firewall)**: 10 Mbps, 80ms delay, 3.0% loss, symmetric NAT (forces TURN relay).

---

## 5. Automated Benchmark Suite Implementation

The repository shall provide an automated benchmarking harness in `benches/`:
- `benches/latency_e2e.rs`: Uses `criterion` to measure core message serialization, key agreement derivation, and input transformation speed.
- `tools/benchmark-runner/`: Shell and Python scripts orchestrating automated desktop actions via `xdotool` / `ydotool` and logging metrics into structured CSV/JSON format.
- Output artifacts include percentile distributions (P50, P90, P95, P99), latency histograms, and CPU/RAM time series graphs.
