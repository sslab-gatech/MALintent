package org.gts3.jnifuzz.contentprovider

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.Intent.EXTRA_PACKAGE_NAME
import android.net.Uri
import android.util.Log

class UriPermissionManager : BroadcastReceiver() {
    private val suffixes = listOf(
        "aac",
        "apk",
        "gif",
        "html",
        "jpg",
        "midi",
        "mp3",
        "mp4",
        "ogg",
        "pdf",
        "png",
        "txt",
        "wav",
        "wma",
        "wmv",
        "xml"
    );

    override fun onReceive(p0: Context?, p1: Intent?) {
        Log.i("UriPermissionManager", "Received intent")

        // Load the package name from the intent (string extra EXTRA_PACKAGE_NAME)
        val packageName = p1?.getStringExtra(EXTRA_PACKAGE_NAME)

        // Log if the context is null
        if (p0 == null) {
            Log.i("UriPermissionManager", "Context is null")
            return
        }

        if (packageName != null) {
            if (packageName.startsWith("com.android") || packageName.startsWith("android")) {
                Log.i("UriPermissionManager", "Package name starts with com.android or android")
                return
            }

            // Grant permissions to the package
            grantUriPermissionsForPackage(p0!!, packageName!!);
        } else {
            Log.i("UriPermissionManager", "No package specified, not granting permissions")
            return
        }
    }

    fun grantUriPermissionsForPackage(context: Context, packageName: String) {
        for (suffix in suffixes) {
            for (i in 0..10) {
                val uri =
                    Uri.parse("content://" + context.packageName + ".provider/external_files/extra_input_" + i + "." + suffix);

                Log.i(
                    "contentprovider",
                    "Granting permission to " + packageName + " for " + uri.toString()
                );

                context.grantUriPermission(
                    packageName,
                    uri,
                    Intent.FLAG_GRANT_WRITE_URI_PERMISSION or Intent.FLAG_GRANT_READ_URI_PERMISSION
                );
            }
        }
    }

    companion object {
        fun grantUriPermissionsForPackage(context: Context, packageName: String) {
            UriPermissionManager().grantUriPermissionsForPackage(context, packageName)
        }
    }
}
