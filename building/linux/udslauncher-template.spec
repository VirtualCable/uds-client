%define _topdir %(echo $PWD)/rpm
%define name PKGNAME
%define version 0.0.0
%define release 1
%define buildroot %{_topdir}/%{name}-%{version}-%{release}-root
%define _binary_payload w9.xzdio

BuildRoot: %{buildroot}
Name: %{name}
Version: %{version}
Release: %{release}
Summary: Actor for Universal Desktop Services (UDS) Broker
License: BSD3
Group: Admin
Requires: libXScrnSaver xset
Vendor: Virtual Cable S.L.U.
URL: http://www.udsenterprise.com
Provides: udsactor

%define _rpmdir %{_topdir}/../../
%define _rpmfilename %%{NAME}-%%{VERSION}-%%{RELEASE}.%%{ARCH}.rpm

%install
curdir=`pwd`
cd %{_topdir}/..
make DESTDIR=$RPM_BUILD_ROOT DISTRO=rh install-udsactor
cd $curdir

%clean
rm -rf $RPM_BUILD_ROOT
curdir=`pwd`
cd %{_topdir}/..
make DESTDIR=$RPM_BUILD_ROOT DISTRO=rh clean
cd $curdir

%post
systemctl enable udsactor.service > /dev/null 2>&1 || true

%preun
systemctl disable udsactor.service > /dev/null 2>&1 || true
systemctl stop udsactor.service > /dev/null 2>&1 || true

%postun
if [ $1 -eq 0 ]; then
    rm -rf /etc/udsactor
    rm -f /var/log/udsactor.log
fi

%description
This package provides the required components to allow this machine to work in an environment managed by UDS Broker.

%files
%defattr(-,root,root)
/usr/bin/udsactor-client
/usr/bin/gui-helper
/usr/sbin/udsactor-service
/usr/sbin/udsactor-config
/etc/systemd/system/udsactor.service
/etc/xdg/autostart/udsactor_client.desktop
/usr/share/applications/udsactor_config.desktop
/usr/share/udsactor/uds-icon.png
/usr/share/polkit-1/actions/org.openuds.pkexec.udsactor_config.policy
