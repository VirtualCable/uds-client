#!/usr/bin/make -f
# -*- makefile -*-

.PHONY: all install-udsclient install-udsclient-unmanaged install-udsclient-common clean

VERSION := $(shell [ -f ../../../VERSION ] && cat ../../../VERSION || echo "devel")
RELEASE := 1

# Distro to build for (Debian12, Fedora41, openSUSE15)
DISTRO ?= Debian12
DESTDIR ?= ./package/$(DISTRO)

# Output directory from rustbuilder
OUTPUTDIR := builders/$(DISTRO)/output

# Install targets
BINDIR := $(DESTDIR)/usr/bin
SBINDIR := $(DESTDIR)/usr/sbin
APPSDIR := $(DESTDIR)/usr/share/applications
POLKITDIR := $(DESTDIR)/usr/share/polkit-1/actions
SYSTEMDDIR := $(DESTDIR)/etc/systemd/system
AUTOSTARTDIR := $(DESTDIR)/etc/xdg/autostart
ICONDIR := $(DESTDIR)/usr/share/udsclient

# No all rule, because the build depends on the variant (managed/unmanaged), etc..

help:
	@echo "Usage:"
	@echo "  make install-udsclient DISTRO=Debian12"
	@echo "  make install-udsclient-unmanaged DISTRO=Debian12"
	@echo "  make clean DISTRO=Debian12"


# === Build binaries using rustbuilder.py ===
$(OUTPUTDIR)/udsclient-client: rustbuilder.py
	python3 rustbuilder.py $(DISTRO)
	@test -f $(OUTPUTDIR)/udsclient-client || (echo "Missing udsclient-client binary" && exit 1)


# === Common installation logic ===
install-udsclient-common: $(OUTPUTDIR)/udsclient-client
	mkdir -p $(BINDIR) $(SBINDIR) $(APPSDIR) $(POLKITDIR) $(AUTOSTARTDIR) $(ICONDIR)

	# Install shared Rust binaries
	cp $(OUTPUTDIR)/udsclient-client $(BINDIR)/
	cp $(OUTPUTDIR)/udsclient-service $(SBINDIR)/
	cp $(OUTPUTDIR)/gui-helper $(BINDIR)/

	# Install icon
	cp ../../assets/img/uds-icon.png $(ICONDIR)/

	# Install PolicyKit
	cp policy/org.openuds.pkexec.udsclient_config.policy $(POLKITDIR)/

	# Install desktop entries
	cp desktop/udsclient_config.desktop $(APPSDIR)/
	cp desktop/udsclient_client.desktop $(AUTOSTARTDIR)/


	# Install systemd service for RH-based distros
ifneq (,$(filter $(DISTRO),Fedora41 openSUSE15))
	mkdir -p $(SYSTEMDDIR)
	cp debian/udsclient.service $(SYSTEMDDIR)/
endif

	# Permissions
	chmod 755 $(BINDIR)/udsclient-client $(BINDIR)/gui-helper
	chmod 755 $(SBINDIR)/udsclient-service
	chmod 644 $(POLKITDIR)/org.openuds.pkexec.udsclient_config.policy
	chmod 644 $(ICONDIR)/uds-icon.png

# === Install managed variant ===
install-udsclient: install-udsclient-common
	# Install managed config
	cp $(OUTPUTDIR)/udsclient-config $(SBINDIR)/udsclient-config


# === Install unmanaged variant ===
install-udsclient-unmanaged: install-udsclient-common
	# Install unmanaged config
	cp $(OUTPUTDIR)/udsclient-unmanaged-config $(SBINDIR)/udsclient-config


# === Clean build artifacts ===
clean:
	rm -rf $(DESTDIR)
