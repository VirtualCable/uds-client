package com.freerdp.afreerdp.uds;

import android.util.Log;

public class UdsNative {
    private static final String TAG = "UdsNative";

    static {
        try {
            System.loadLibrary("uds_android");
            Log.i(TAG, "libuds_android.so loaded successfully");
        } catch (UnsatisfiedLinkError e) {
            Log.e(TAG, "Failed to load libuds_android.so", e);
        }
    }

    public static native String getScript(String host, String ticket, String scrambler);

    public static native int startTunnel(String host, int port, String ticket, byte[] sharedSecret);

    public static native void stopTunnel();
}
