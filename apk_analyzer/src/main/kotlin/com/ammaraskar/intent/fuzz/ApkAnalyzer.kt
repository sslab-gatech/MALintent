package com.ammaraskar.intent.fuzz

import jadx.api.JadxArgs
import jadx.api.JadxDecompiler
import jadx.core.dex.visitors.IDexTreeVisitor
import jadx.core.dex.visitors.ReSugarCode
import java.io.File

class ApkAnalyzer(private val apkFile: File) {

    private val _intentTemplates: MutableList<IntentTemplate> = mutableListOf()
    val intentTemplates: List<IntentTemplate>
        get() = _intentTemplates

    init {
        val jadxArgs = JadxArgs()
        jadxArgs.setInputFile(apkFile)

        JadxDecompiler(jadxArgs).use { decompiler ->
            decompiler.load();
            val intentExtraUsageVisitor = IntentExtraUsageTreeVisitor()
            addCustomPassAfter<ReSugarCode>(decompiler.root.passes, intentExtraUsageVisitor)

            val manifestResource =
                decompiler.resources.stream().filter { resource -> resource.originalName.equals("AndroidManifest.xml") }
                    .findFirst();
            if (!manifestResource.isPresent) {
                throw IllegalArgumentException("APK does not contain AndroidManifest.xml")
            }

            val contents = manifestResource.get().loadContent();
            val intentReceivers =
                parseIntentReceiversFromManifest(contents.text.codeStr, decompiler)

            // Invoke the tree visitor by decompiling all the classes in the apk.
            for (cls in decompiler.classes) {
                // Don't bother looking at built-in android, kotlin or java classes.
                if (cls.fullName.startsWith("androidx.") || cls.fullName.startsWith("kotlin.")) {
                    continue;
                }
                cls.decompile()
            }

            for (intentReceiver in intentReceivers) {
                _intentTemplates.add(IntentTemplate(
                    intentReceiver.receiverType,
                    intentReceiver.componentName,
                    intentReceiver.actions,
                    intentReceiver.categories,
                    // For now we assign all extras to every intent receiver. In the future maybe with some more
                    // advanced static analysis we could figure out which extras correspond to which intent receiver.
                    intentExtraUsageVisitor.extras,
                ))
            }
        }
    }

}

// From https://github.com/skylot/jadx/issues/1482
private inline fun <reified T> addCustomPassAfter(passes: MutableList<IDexTreeVisitor>, customPass: IDexTreeVisitor) {
    for ((i, pass) in passes.withIndex()) {
        if (pass is T) {
            passes.add(i + 1, customPass)
            break
        }
    }
}
