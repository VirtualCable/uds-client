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

%build
# Nothing

%install
cp -a %{DESTDIR}/* %{buildroot}/

%files
/usr/share/udslauncher
/usr/share/applications/udslauncher.desktop
