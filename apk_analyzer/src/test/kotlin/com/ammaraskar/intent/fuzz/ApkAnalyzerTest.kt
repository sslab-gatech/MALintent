package com.ammaraskar.intent.fuzz

import org.junit.Ignore
import org.junit.Test
import java.io.File

class ApkAnalyzerTest {
    @Ignore("Ignored until we actually commit actual testing apk files into the repo")
    @Test
    fun testLoadsApk() {
        ApkAnalyzer(File("../../VulnerableApp/app/build/outputs/apk/debug/app-debug.apk"))
    }
}