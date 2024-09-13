package com.ammaraskar.intent.fuzz

import junit.framework.Assert.assertEquals
import org.junit.Test

internal class IntentReceiverTest {

    @Test
    fun parseIntentTargetsWorksOnARealManifest() {
        val testManifest = """
            <?xml version="1.0" encoding="utf-8"?>
            <manifest xmlns:android="http://schemas.android.com/apk/res/android" android:versionCode="1" android:versionName="1.0" android:compileSdkVersion="30" android:compileSdkVersionCodename="11" package="com.ammaraskar.vulnerableapp" platformBuildVersionCode="30" platformBuildVersionName="11">
                <uses-sdk android:minSdkVersion="23" android:targetSdkVersion="30"/>
                <application android:theme="@style/AppTheme" android:label="@string/app_name" android:icon="@mipmap/ic_launcher" android:debuggable="true" android:allowBackup="true" android:supportsRtl="true" android:extractNativeLibs="false" android:roundIcon="@mipmap/ic_launcher_round" android:appComponentFactory="androidx.core.app.CoreComponentFactory">
                    <activity android:name="com.ammaraskar.vulnerableapp.MainActivity">
                        <intent-filter>
                            <action android:name="android.intent.action.MAIN"/>
                            <category android:name="android.intent.category.LAUNCHER"/>
                        </intent-filter>
                    </activity>
                </application>
            </manifest>
        """.trimIndent()

        val targets = parseIntentReceiversFromManifest(testManifest, decompiler)
        assertEquals(targets.size, 1)
        assertEquals(targets[0].componentName, "com.ammaraskar.vulnerableapp/com.ammaraskar.vulnerableapp.MainActivity")
        assertEquals(targets[0].categories, listOf("android.intent.category.LAUNCHER"))
        assertEquals(targets[0].actions, listOf("android.intent.action.MAIN"))
    }

}
