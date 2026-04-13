Name:       syswall
Version:    0.2.0
Release:    1%{?dist}
Summary:    Application-level firewall for Linux desktop
License:    MIT
URL:        https://github.com/yrbane/SysWall

%description
SysWall is a desktop application-level firewall for Linux providing
real-time connection monitoring, intelligent auto-learning, and
granular rule management via nftables.

SysWall est un pare-feu applicatif de bureau pour Linux offrant
une surveillance des connexions en temps réel, un auto-apprentissage
intelligent et une gestion granulaire des règles via nftables.

%post
getent group syswall > /dev/null 2>&1 || groupadd syswall
mkdir -p /var/lib/syswall /var/log/syswall /var/run/syswall
setcap 'cap_net_admin,cap_net_raw,cap_sys_ptrace,cap_dac_read_search,cap_bpf,cap_perfmon=ep' /usr/bin/syswall-daemon 2>/dev/null || true
systemctl daemon-reload

%preun
systemctl stop syswall 2>/dev/null || true
systemctl disable syswall 2>/dev/null || true

%postun
systemctl daemon-reload

%files
/usr/bin/syswall-daemon
/usr/bin/syswall-ui
/usr/lib/systemd/system/syswall.service
/usr/share/applications/syswall.desktop
%config(noreplace) /etc/syswall/config.toml
