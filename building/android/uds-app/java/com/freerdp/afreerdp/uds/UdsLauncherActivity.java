package com.freerdp.afreerdp.uds;

import android.app.Activity;
import android.content.Intent;
import android.net.Uri;
import android.os.Bundle;
import android.os.Handler;
import android.os.Looper;
import android.util.Log;
import android.view.View;
import android.widget.Button;
import android.widget.EditText;
import android.widget.ProgressBar;
import android.widget.TextView;

import com.freerdp.afreerdp.R;
import com.freerdp.freerdpcore.domain.BookmarkBase;
import com.freerdp.freerdpcore.presentation.SessionActivity;

import org.json.JSONArray;
import org.json.JSONObject;

import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;

public class UdsLauncherActivity extends Activity {
    private static final String TAG = "UdsLauncher";
    private static final int MAX_PREPARATION_RETRIES = 30;

    private EditText editUrl;
    private Button btnConnect;
    private ProgressBar progressBar;
    private TextView txtStatus;
    private TextView txtError;

    private final Handler mainHandler = new Handler(Looper.getMainLooper());
    private final ExecutorService executor = Executors.newSingleThreadExecutor();

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        setContentView(R.layout.activity_uds_launcher);

        editUrl = findViewById(R.id.edit_uds_url);
        btnConnect = findViewById(R.id.btn_connect);
        progressBar = findViewById(R.id.progress_bar);
        txtStatus = findViewById(R.id.txt_status);
        txtError = findViewById(R.id.txt_error);

        btnConnect.setOnClickListener(v -> {
            String url = editUrl.getText().toString().trim();
            if (!url.isEmpty()) {
                startUdsLaunch(url);
            }
        });

        handleIntent(getIntent());
    }

    @Override
    protected void onNewIntent(Intent intent) {
        super.onNewIntent(intent);
        setIntent(intent);
        handleIntent(intent);
    }

    private void handleIntent(Intent intent) {
        if (intent == null) return;
        Uri uri = intent.getData();
        if (uri != null && ("udssv2".equalsIgnoreCase(uri.getScheme()) || "udss".equalsIgnoreCase(uri.getScheme()))) {
            String rawUrl = uri.toString();
            editUrl.setText(rawUrl);
            startUdsLaunch(rawUrl);
        }
    }

    private void startUdsLaunch(String rawUrl) {
        txtError.setVisibility(View.GONE);
        progressBar.setVisibility(View.VISIBLE);
        btnConnect.setEnabled(false);
        txtStatus.setText(R.string.uds_resolving_ticket);

        executor.execute(() -> processUrl(rawUrl));
    }

    private void processUrl(String rawUrl) {
        try {
            // Format: udssv2://<host>[:port]/<ticket>/<scrambler>
            String stripped = rawUrl;
            if (stripped.startsWith("udssv2://")) {
                stripped = stripped.substring("udssv2://".length());
            } else if (stripped.startsWith("udss://")) {
                stripped = stripped.substring("udss://".length());
            }

            int firstSlash = stripped.indexOf('/');
            if (firstSlash == -1) {
                showError("Invalid UDS URL format: missing ticket");
                return;
            }
            String host = stripped.substring(0, firstSlash);
            String rest = stripped.substring(firstSlash + 1);

            int secondSlash = rest.indexOf('/');
            if (secondSlash == -1) {
                showError("Invalid UDS URL format: missing scrambler");
                return;
            }
            String ticket = rest.substring(0, secondSlash);
            String scrambler = rest.substring(secondSlash + 1);

            if (scrambler.contains("?")) {
                scrambler = scrambler.substring(0, scrambler.indexOf('?'));
            }
            if (scrambler.contains("/")) {
                scrambler = scrambler.substring(0, scrambler.indexOf('/'));
            }

            fetchAndLaunch(host, ticket, scrambler, 0);
        } catch (Exception e) {
            Log.e(TAG, "Error launching UDS session", e);
            showError("Launch error: " + e.getMessage());
        }
    }

    private void fetchAndLaunch(String host, String ticket, String scrambler, int retryCount) {
        try {
            updateStatus("Connecting to " + host + "...");
            String responseStr = UdsNative.getScript(host, ticket, scrambler);
            if (responseStr == null || responseStr.isEmpty()) {
                showError("Empty response from native broker client");
                return;
            }

            Log.d(TAG, "Native getScript response: " + responseStr);
            JSONObject json = new JSONObject(responseStr);

            if (json.has("error") && !json.isNull("error")) {
                JSONObject errJson = json.getJSONObject("error");
                String errorMsg = errJson.optString("message", "Unknown broker error");
                boolean isRetryable = errJson.optBoolean("is_retryable", false);
                int percent = errJson.optInt("percent", -1);

                if (isRetryable && retryCount < MAX_PREPARATION_RETRIES) {
                    String msg = "Preparing service" + (percent >= 0 ? " (" + percent + "%)" : "") + "...";
                    updateStatus(msg);
                    Thread.sleep(2000);
                    fetchAndLaunch(host, ticket, scrambler, retryCount + 1);
                    return;
                }
                showError("Broker error: " + errorMsg);
                return;
            }

            if (!json.has("result")) {
                showError("Invalid broker response: missing result");
                return;
            }

            JSONObject params = json.getJSONObject("result");
            String server = params.getString("server");
            int port = params.getInt("port");
            String user = params.optString("user", "");
            String password = params.optString("password", "");
            String domain = params.optString("domain", "");

            if (params.has("tunnel") && !params.isNull("tunnel")) {
                updateStatus("Setting up secure UDS 5.0 tunnel...");
                JSONObject tunnelObj = params.getJSONObject("tunnel");
                String tunnelHost = tunnelObj.getString("host");
                int tunnelPort = tunnelObj.getInt("port");
                String tunnelTicket = tunnelObj.getString("ticket");

                byte[] sharedSecret = null;
                if (tunnelObj.has("shared_secret")) {
                    JSONArray ssArray = tunnelObj.getJSONArray("shared_secret");
                    sharedSecret = new byte[ssArray.length()];
                    for (int i = 0; i < ssArray.length(); i++) {
                        sharedSecret[i] = (byte) ssArray.getInt(i);
                    }
                } else if (params.has("shared_secret")) {
                    JSONArray ssArray = params.getJSONArray("shared_secret");
                    sharedSecret = new byte[ssArray.length()];
                    for (int i = 0; i < ssArray.length(); i++) {
                        sharedSecret[i] = (byte) ssArray.getInt(i);
                    }
                }

                if (sharedSecret == null || sharedSecret.length != 32) {
                    showError("Error: Missing or invalid shared secret for tunnel");
                    return;
                }

                UdsNative.stopTunnel();
                int localPort = UdsNative.startTunnel(tunnelHost, tunnelPort, tunnelTicket, sharedSecret);
                if (localPort <= 0) {
                    showError("Failed to start native UDS tunnel (code: " + localPort + ")");
                    return;
                }

                server = "127.0.0.1";
                port = localPort;
            }

            updateStatus("Starting RDP session...");

            BookmarkBase bookmark = new BookmarkBase();
            bookmark.setHostname(server);
            bookmark.setPort(port);
            bookmark.setUsername(user);
            bookmark.setPassword(password);
            bookmark.setDomain(domain);
            bookmark.setLabel("UDS - " + (domain.isEmpty() ? "" : domain + "\\") + user);

            BookmarkBase.ScreenSettings screen = bookmark.getActiveScreenSettings();
            screen.setResolution(BookmarkBase.ScreenSettings.FITSCREEN);
            screen.setColors(32);

            mainHandler.post(() -> {
                progressBar.setVisibility(View.GONE);
                btnConnect.setEnabled(true);
                txtStatus.setText(R.string.uds_connected);

                Intent sessionIntent = new Intent(UdsLauncherActivity.this, SessionActivity.class);
                sessionIntent.putExtra("uds_bookmark", bookmark);
                startActivity(sessionIntent);
            });

        } catch (Exception e) {
            Log.e(TAG, "Error in fetchAndLaunch", e);
            showError("Connection failed: " + e.getMessage());
        }
    }

    private void updateStatus(String status) {
        mainHandler.post(() -> txtStatus.setText(status));
    }

    private void showError(String error) {
        mainHandler.post(() -> {
            progressBar.setVisibility(View.GONE);
            btnConnect.setEnabled(true);
            txtStatus.setText("");
            txtError.setText(error);
            txtError.setVisibility(View.VISIBLE);
        });
    }

    @Override
    protected void onDestroy() {
        super.onDestroy();
        executor.shutdown();
        UdsNative.stopTunnel();
    }
}
