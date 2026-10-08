FROM eclipse-temurin:17-jdk-jammy

ARG ANDROID_COMMAND_LINE_TOOLS=11076708
ARG FREERDP_REF=3.26.0

ENV ANDROID_HOME=/opt/android-sdk
ENV ANDROID_SDK_ROOT=/opt/android-sdk
ENV PATH=/opt/android-sdk/cmdline-tools/latest/bin:/opt/android-sdk/platform-tools:${PATH}

RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    git \
    patch \
    pkg-config \
    unzip \
    wget \
    && rm -rf /var/lib/apt/lists/*
RUN mkdir -p ${ANDROID_HOME}/cmdline-tools && \
    wget -q https://dl.google.com/android/repository/commandlinetools-linux-${ANDROID_COMMAND_LINE_TOOLS}_latest.zip -O /tmp/tools.zip && \
    unzip -q /tmp/tools.zip -d ${ANDROID_HOME}/cmdline-tools && \
    mv ${ANDROID_HOME}/cmdline-tools/cmdline-tools ${ANDROID_HOME}/cmdline-tools/latest && \
    rm /tmp/tools.zip
RUN yes | sdkmanager --licenses && \
    sdkmanager "platform-tools" "platforms;android-36" "build-tools;36.0.0" "cmake;3.22.1" "ndk;29.0.13113456"
RUN git clone --depth 1 --branch ${FREERDP_REF} https://github.com/FreeRDP/FreeRDP.git /opt/freerdp

WORKDIR /opt/freerdp/client/Android/Studio

COPY patches/ /tmp/patches/
COPY uds-app/ /tmp/uds-app/

RUN patch -p1 < /tmp/patches/SessionActivity.patch && \
    patch -p1 < /tmp/patches/session_menu.patch && \
    patch -p1 < /tmp/patches/SessionInput.patch && \
    patch -p1 < /tmp/patches/session_layout.patch && \
    cp /tmp/uds-app/AndroidManifest.xml aFreeRDP/src/main/AndroidManifest.xml && \
    cp -r /tmp/uds-app/java/* aFreeRDP/src/main/java/ && \
    cp -r /tmp/uds-app/res/* aFreeRDP/src/main/res/ && \
    mkdir -p aFreeRDP/src/main/jniLibs && \
    cp -r /tmp/uds-app/jniLibs/* aFreeRDP/src/main/jniLibs/

RUN ./gradlew --no-daemon assembleDebug
