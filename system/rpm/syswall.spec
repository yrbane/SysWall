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
/bin/sh /usr/share/syswall/postinst.sh

%preun
/bin/sh /usr/share/syswall/prerm.sh

%postun
systemctl daemon-reload 2>/dev/null || true

%files
/usr/bin/syswall-daemon
/usr/bin/syswall-ui
/usr/lib/systemd/system/syswall.service
/usr/share/applications/syswall.desktop
/usr/share/syswall/postinst.sh
/usr/share/syswall/prerm.sh
%config(noreplace) /etc/syswall/config.toml
