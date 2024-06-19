# Guide

This document provides a guide on how to build and use the UDSClient application on Windows.

## Environment Setup

### Windows Environment

- **Operating System:** Windows 11 - 64 bit
- **Python Version:** [Python 3.10](https://www.python.org/ftp/python/3.10.0/python-3.10.0-amd64.exe)
- **PyInstaller:** [PyInstaller](https://www.pyinstaller.org/)
- **WiX Toolset:** [WiX Toolset 3.14](https://github.com/wixtoolset/wix3/releases/download/wix3141rtm/wix314.exe)
- **Additional Dependencies:** Packed in `requirements.txt`.

### Initialize Environment with virtualenv

Open Command Prompt (CMD) and execute the following commands:
 
```shell
    cd "Your path to source code"
    cd uds-client\src\
    pip install virtualenv
    virtualenv myenv
    call myenv\Scripts\activate
    pip install -r requirements.txt
```

##  Build Picture to Icon( Optional)

```
    pyrcc5 -o .\UDSResources_rc.py .\UDSResources.qrc
```

## Create PyInstaller Spec File

Generate a .spec file using pyi-makespec to configure the build options:

```
    pyi-makespec --onefile  UDSClient.py  
```

This will create a file named UDSClient.spec with content similar to the following example:

```python
# -*- mode: python ; coding: utf-8 -*-
# pyi-makespec --onefile  UDSClient.py  

a = Analysis(
    ['UDSClient.py'],
    pathex=[],
    binaries=[],
    datas=[],
    hiddenimports=['win32crypt', 'certifi'],
    hookspath=[],
    hooksconfig={},
    runtime_hooks=[],
    excludes=[],
    noarchive=False,
    optimize=0,
    single_file=True, 
)
pyz = PYZ(a.pure)

exe = EXE(
    pyz,
    a.scripts,
    a.binaries,
    a.datas,
    [],
    name='UDSClient',
    debug=False,
    bootloader_ignore_signals=False,
    strip=False,
    upx=True,
    upx_exclude=[],
    runtime_tmpdir=None,
    console=True,
    disable_windowed_traceback=False,
    argv_emulation=False,
    target_arch=None,
    codesign_identity=None,
    entitlements_file=None,
)
```

### Note here:
You must include the `hiddenimports=['win32crypt', 'certifi']` line to avoid the error "ModuleNotFoundError: No module named 'win32crypt'" when processing RDP connections form client.

Finally, build the executable using the UDSClient.spec file:

```
pyinstaller UDSClient.spec 
```
      
The output file will be located at:
```
uds-client\src\dist\UDSclient.exe
```
    

## Initialize WiX Toolset to Build MSI for Windows 10
To create an MSI installer (UDSClient.msi), you need to prepare a .wxs file manually. Below is a basic example (UDSclient.wxs).
Change the path where you store your source code on line 7 to:

```xml
<?xml version="1.0" encoding="UTF-8"?>
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
    <Product Id="*" Name="UDSClient" Language="1033" Version="1.0.0.0" Manufacturer="UDS" UpgradeCode="77f76869-ba64-4853-8758-6b262cdfc0e4">
        <Package InstallerVersion="200" Compressed="yes" InstallScope="perMachine" />
        <MajorUpgrade DowngradeErrorMessage="A newer version of [ProductName] is already installed." />
        <MediaTemplate EmbedCab="yes" />
        <WixVariable Id="UDSCLIENTDIR" Value="C:\Users\admin.WIN10-CY\AppData\Roaming\MobaXterm\home" />
        <Feature Id="ProductFeature" Title="UDSClient" Level="1">
            <ComponentGroupRef Id="ProductComponents" />
        </Feature>
        <Property Id="REINSTALLMODE" Value="amus" />
</Product>
    <Fragment>
        <Directory Id="TARGETDIR" Name="SourceDir">
            <Directory Id="ProgramFilesFolder">
                <Directory Id="INSTALLFOLDER" Name="[ProductName]" />
            </Directory>
        </Directory>
    </Fragment>
    <Fragment>
        <ComponentGroup Id="ProductComponents" Directory="INSTALLFOLDER">
            <Component Id="VDIClientComponent" Guid="{77f76869-ba64-4853-8758-6b262cdfc0e3}">          
                <File Id="VDIClientEXE" Source="!(wix.UDSCLIENTDIR)\uds-client\src\dist\UDSClient.exe" KeyPath="yes" />
                <RegistryValue Root="HKCR" Key="uds\shell\open\command" Type="string"  Value='"[INSTALLFOLDER]\[ProductName]\[ProductName].exe" %1' />
                <RegistryValue Root="HKCR" Key="uds" Type="string"  Value='URL:Uds Protocol SSL' />
                <RegistryKey Root="HKCR" Key="uds">
                    <RegistryValue Type="string" Name="URL Protocol" Value="" />
                </RegistryKey>
                <RegistryValue Root="HKCR" Key="udss\shell\open\command" Type="string" Value='"[INSTALLFOLDER]\[ProductName]\[ProductName]exe" %1' />
                <RegistryValue Root="HKCR" Key="udss" Type="string"  Value='URL:Uds Protocol SSL' />
                <RegistryKey Root="HKCR" Key="udss">
                    <RegistryValue Type="string" Name="URL Protocol" Value="" />
                </RegistryKey>
            </Component>
        </ComponentGroup>
    </Fragment>
</Wix>
```


## Build MSI Installer

To build the MSI installer, use the following commands: 
```shell
"C:\Program Files (x86)\WiX Toolset v3.14\bin\candle.exe" "UDSclient.wxs"
"C:\Program Files (x86)\WiX Toolset v3.14\bin\light.exe" "UDSclient.wixobj"
```

Upon successful execution, you should find UDSClient.msi (approximately 44 MB) in your directory.   