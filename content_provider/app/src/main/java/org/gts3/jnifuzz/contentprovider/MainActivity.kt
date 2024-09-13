package org.gts3.jnifuzz.contentprovider

import android.content.pm.PackageManager
import android.os.Bundle
import android.view.View
import android.widget.Toast
import androidx.appcompat.app.AppCompatActivity


class MainActivity : AppCompatActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_main)
    }

    fun grantUriPermissions(view: View?) {

        // Bail out if view is null
        if (view == null) {
            Toast.makeText(this, "Failed to grant permissions (view is null)", Toast.LENGTH_LONG).show()
            return
        }

        val context = view.getContext();

        // Go through all packages
        val pm = getPackageManager();
        val packages = pm.getInstalledPackages(PackageManager.GET_META_DATA);

        for (packageInfo in packages) {
            val packageName = packageInfo.packageName;

            // Skip packages that start with "com.android" or "android"
            if (packageName.startsWith("com.android") || packageName.startsWith("android")) {
                continue;
            }

            UriPermissionManager.grantUriPermissionsForPackage(context, packageName)
        }
    }
}

