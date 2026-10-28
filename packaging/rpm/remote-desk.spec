Name:           remote-desk
Version:        0.1.0
Release:        1%{?dist}
Summary:        Privacy-first, low-latency remote desktop system
License:        MIT or Apache-2.0
URL:            https://github.com/remote-desk/remote-desk
Source0:        %{name}-%{version}.tar.gz

BuildRequires:  cargo
BuildRequires:  rust >= 1.75
BuildRequires:  systemd-rpm-macros

Requires:       pipewire
Requires:       libX11

%description
Remote-Desk provides secure, low-latency remote desktop streaming and remote control
across Linux, Windows, and Android devices using WebRTC and Ed25519 authentication.

%prep
%autosetup

%build
cargo build --release --workspace

%install
install -Dm755 target/release/remote-desk-host %{buildroot}%{_bindir}/remote-desk-host
install -Dm755 target/release/remote-desk-signal %{buildroot}%{_bindir}/remote-desk-signal
install -Dm644 packaging/systemd/remote-desk.service %{buildroot}%{_unitdir}/remote-desk.service
install -Dm644 packaging/desktop/org.remotedesk.RemoteDesk.desktop %{buildroot}%{_datadir}/applications/org.remotedesk.RemoteDesk.desktop

%files
%license LICENSE-MIT LICENSE-APACHE
%{_bindir}/remote-desk-host
%{_bindir}/remote-desk-signal
%{_unitdir}/remote-desk.service
%{_datadir}/applications/org.remotedesk.RemoteDesk.desktop

%changelog
* Wed Oct 28 2026 Remote-Desk Team <maintainers@remotedesk.org> - 0.1.0-1
- Initial production release
