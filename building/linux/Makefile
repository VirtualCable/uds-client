#!/usr/bin/make -f
# -*- makefile -*-

.PHONY: all install-udslauncher install-udslauncher-unmanaged install-udslauncher-common clean

VERSION := $(shell [ -f ../../../VERSION ] && cat ../../../VERSION || echo "devel")
RELEASE := 1

# Distro to build for (Debian12, Fedora41, openSUSE15)
DISTRO ?= Debian12
DESTDIR ?= ./package/$(DISTRO)

# Output directory from rustbuilder
OUTPUTDIR := builders/$(DISTRO)/output

# Install targets
APPSDIR := $(DESTDIR)/usr/share/applications
OWNDIR := $(DESTDIR)/usr/share/udslauncher

# all rule is install-udslauncher
all: install-udslauncher

help:
	@echo "Usage:"
	@echo "  make install-udslauncher DISTRO=[Debian12 | Debian12 | Fedora | openSUSE]"
	@echo "  make clean DISTRO=Debian12"


# === Build binaries using rustbuilder.py ===
$(OUTPUTDIR)/launcher: rustbuilder.py
	python3 rustbuilder.py $(DISTRO)
	@test -f $(OUTPUTDIR)/launcher || (echo "Missing udslauncher-client binary" && exit 1)


# === Common installation logic ===
install-udslauncher: $(OUTPUTDIR)/launcher
	mkdir -p $(APPSDIR) $(OWNDIR)

	# Install main binary
	cp $(OUTPUTDIR)/launcher $(OWNDIR)/
	# Install shared libraries if any (all *.so.* files)
	cp -a $(OUTPUTDIR)/*.so* $(OWNDIR)/ 2>/dev/null || true

	# Install icon
	cp ../../assets/img/uds-icon.png $(OWNDIR)/

	# Install desktop entries
	cp desktop/udslauncher.desktop $(APPSDIR)/

	# Permissions
	chmod 755 $(OWNDIR)/launcher
	# If any .so files were copied, set their permissions too
	chmod 644 $(OWNDIR)/*.so* 2>/dev/null || true
	chmod 644 $(OWNDIR)/uds-icon.png
	# Also, desktop file
	chmod 644 $(APPSDIR)/udslauncher.desktop

# TODO: Create appimage, igel and thinpro versions (as on v4.x)

# === Clean build artifacts ===
clean:
	rm -rf $(DESTDIR)
