Name: udslauncher
Version: %{version}
Release: %{release}
Summary: UDS Launcher
License: BSD-3-Clause
URL: https://www.udsenterprise.com

AutoReq: yes
AutoProv: yes

%global debug_package %{nil}
%global _builddir %{_topdir}
%global _sourcedir %{_topdir}

%changelog
* Fri Dec 19 2025 Adolfo <info@udsenterprise.com> - %{version}-%{release}
- Initial release


%description
Launcher for UDS Broker.

%prep
# Nothing

%post
/usr/bin/update-mime-database /usr/share/mime || true
/usr/bin/update-desktop-database /usr/share/applications || true
/usr/bin/gtk-update-icon-cache -f -q /usr/share/icons/hicolor 2>/dev/null || true

if [ ! -d /media ]; then
    mkdir -m 755 /media
    echo "/media created for compatibility"
fi

%build
# Nothing

%postun
/usr/bin/update-desktop-database

%install
cp -a %{DESTDIR}/* %{buildroot}/

%files
/usr/share/udslauncher
/usr/share/applications/udslauncher.desktop
/usr/share/mime/packages/udslauncher.xml
